use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, BTreeSet};

use super::DbPool;
use super::{entities, messages};
use crate::error::EddieError;
use crate::services::sync::helpers::email_normalization::normalize_email;

pub fn compute_conversation_id(participant_key: &str) -> String {
    let hash = Sha256::digest(participant_key.as_bytes());
    format!("{:x}", hash)[..16].to_string()
}

fn compute_thread_id(root_message_id: &str) -> String {
    let hash = Sha256::digest(root_message_id.as_bytes());
    format!("{:x}", hash)[..16].to_string()
}

/// Rebuild all conversations for an account from the messages table.
///
/// Groups messages by conversation_id, aggregates participant names,
/// picks the latest message as preview, and upserts into conversations.
/// Preserves user preferences (is_muted, is_pinned) on existing conversations.
pub fn rebuild_conversations(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let self_emails = super::entities::get_self_emails(pool, account_id)?;
    let sender_names = name_lookup(pool, account_id)?;

    // ========================================
    // PHASE 1: Thread detection + assignments
    // ========================================

    struct ThreadMsg {
        db_id: String,
        message_id: String,
        in_reply_to: Option<String>,
        references: Vec<String>,
        date: i64,
        from_address: String,
        to_addresses: Vec<String>,
        cc_addresses: Vec<String>,
    }

    let thread_msgs: Vec<ThreadMsg> = {
        let mut stmt = conn.prepare(
            "SELECT id, message_id, in_reply_to, references_ids,
                    date, from_address, to_addresses, cc_addresses
            FROM messages WHERE account_id = ?1
            ORDER BY date ASC"
        )?;

        let rows = stmt.query_map(params![account_id], |row| {
            let refs_json: String = row.get(3)?;
            let to_json: String = row.get(6)?;
            let cc_json: String = row.get(7)?;
            Ok(ThreadMsg {
                db_id: row.get(0)?,
                message_id: row.get(1)?,
                in_reply_to: row.get(2)?,
                references: serde_json::from_str(&refs_json).unwrap_or_default(),
                date: row.get(4)?,
                from_address: row.get(5)?,
                to_addresses: serde_json::from_str(&to_json).unwrap_or_default(),
                cc_addresses: serde_json::from_str(&cc_json).unwrap_or_default(),
            })
        })?;

        rows.filter_map(|r| r.ok()).collect()
    };

    // Build threads via union-find
    let mut uf = UnionFind::new();
    for msg in &thread_msgs {
        if let Some(ref reply_to) = msg.in_reply_to {
            uf.union(&msg.message_id, reply_to);
        }
        for ref_id in &msg.references {
            uf.union(&msg.message_id, ref_id);
        }
    }

    // Group messages by thread root
    // Handle empty message_id: use db_id to avoid merging all no-ID messages
    let mut threads: HashMap<String, Vec<&ThreadMsg>> = HashMap::new();
    for msg in &thread_msgs {
        let root = if msg.message_id.is_empty() {
            msg.db_id.clone()
        } else {
            uf.find(&msg.message_id)
        };
        threads.entry(root).or_default().push(msg);
    }

    // Per-thread: compute participant union + participant changes
    // Maps db_id → (participant_key, conversation_id, participant_changes, thread_id)
    let mut msg_assignments: HashMap<String, (String, String, Option<String>, String)> = HashMap::new();

    for (root, msgs) in &threads {
        // Compute thread_id from thread root
        let thread_id = compute_thread_id(root);

        // Union of all participants across the thread
        let mut all_participants: BTreeSet<String> = BTreeSet::new();
        for msg in msgs {
            let p = collect_participants(
                &msg.from_address, &msg.to_addresses, &msg.cc_addresses, &self_emails,
            );
            all_participants.extend(p);
        }

        let participant_key = all_participants.into_iter().collect::<Vec<_>>().join("\n");
        let conversation_id = compute_conversation_id(&participant_key);

        // Compute participant changes per message (sorted by date)
        let mut sorted: Vec<&&ThreadMsg> = msgs.iter().collect();
        sorted.sort_by_key(|m| m.date);

        let mut prev_participants: BTreeSet<String> = BTreeSet::new();

        for (i, msg) in sorted.iter().enumerate() {
            let current_participants = collect_participants(
                &msg.from_address, &msg.to_addresses, &msg.cc_addresses, &self_emails,
            );

            let changes = if i > 0 {
                let added: Vec<&String> = current_participants.difference(&prev_participants).collect();
                let removed: Vec<&String> = prev_participants.difference(&current_participants).collect();

                if !added.is_empty() || !removed.is_empty() {
                    Some(serde_json::json!({
                        "added": added,
                        "removed": removed,
                    }).to_string())
                } else {
                    None
                }
            } else {
                None
            };

            msg_assignments.insert(
                msg.db_id.clone(),
                (participant_key.clone(), conversation_id.clone(), changes, thread_id.clone()),
            );

            prev_participants = current_participants;
        }
    }

    // Write thread assignments back to messages
    {
        let tx = conn.unchecked_transaction()?;
        for (db_id, (pkey, conv_id, changes, thread_id)) in &msg_assignments {
            tx.execute(
                "UPDATE messages SET participant_key = ?1, conversation_id = ?2, participant_changes = ?3, thread_id = ?4
                 WHERE id = ?5",
                params![pkey, conv_id, changes, thread_id, db_id],
            )?;
        }
        tx.commit()?;
    }

    // ========================================
    // PHASE 2: Aggregate conversations
    // ========================================

    let conv_map = {
        let mut stmt = conn
            .prepare(
                "SELECT m.conversation_id, m.participant_key, m.date,
                        m.distilled_text,
                        m.from_address, m.from_name, m.classification, m.is_important,
                        m.imap_flags,
                        CASE WHEN e.id IS NOT NULL THEN 1 ELSE 0 END AS is_trusted
                 FROM messages m
                 LEFT JOIN entities e ON e.email = m.from_address AND e.account_id = m.account_id
                                       AND e.trust_level NOT IN ('user', 'alias', 'blocked')
                 WHERE m.account_id = ?1
                 ORDER BY m.conversation_id, m.date DESC",
            )?;

        let rows = stmt
            .query_map(params![account_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,         // conversation_id
                    row.get::<_, String>(1)?,         // participant_key
                    row.get::<_, i64>(2)?,            // date
                    row.get::<_, Option<String>>(3)?, // distilled_text
                    row.get::<_, String>(4)?,         // from_address
                    row.get::<_, Option<String>>(5)?, // from_name
                    row.get::<_, Option<String>>(6)?, // classification
                    row.get::<_, i32>(7)?,            // is_important
                    row.get::<_, String>(8)?,         // imap_flags
                    row.get::<_, i32>(9)?,            // is_trusted
                ))
            })?;

        let mut map: HashMap<String, ConversationBuilder> = HashMap::new();
        for row in rows {
            let (conv_id, participant_key, date, distilled_text,
                 from_address, _from_name, classification, is_important, imap_flags, is_trusted) =
                row?;

            let preview = distilled_text
                .as_deref()
                .and_then(|t| t.lines().map(|l| l.trim()).find(|l| !l.is_empty()))
                .map(|s| s.to_string());

            let builder = map.entry(conv_id.clone()).or_insert_with(|| {
                ConversationBuilder {
                    id: conv_id,
                    participant_key,
                    last_message_date: date,
                    last_message_preview: preview,
                    has_chat: false,
                    has_trusted: false,
                    has_important: false,
                    unread_count: 0,
                    total_count: 0,
                    names: BTreeMap::new(),
                    initial_sender_email: None,
                }
            });

            // Track initial sender: query is ordered date DESC, so last
            // non-self from_address we see is from the earliest message.
            if !is_self(&from_address, &self_emails) {
                builder.initial_sender_email = Some(from_address.to_lowercase());
            }

            if classification.as_deref() == Some("chat") {
                builder.has_chat = true;
            }
            if is_trusted != 0 {
                builder.has_trusted = true;
            }
            if is_important != 0 {
                builder.has_important = true;
            }
            let flags: Vec<String> = serde_json::from_str(&imap_flags).unwrap_or_default();
            if !flags.iter().any(|f| f == "Seen") {
                builder.unread_count += 1;
            }
            builder.total_count += 1;
        }
        map
    };

    // Populate participant names from lookup
    let mut conv_map = conv_map; // make mutable
    for builder in conv_map.values_mut() {
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        for addr in builder.participant_key.split('\n') {
            if !addr.is_empty() && !is_self(addr, &self_emails) {
                let name = sender_names.get(addr).cloned()
                    .unwrap_or_else(|| addr.to_string());
                names.insert(addr.to_string(), name);
            }
        }
        builder.names = names;
    }

    // ========================================
    // PHASE 3: Upsert conversations
    // ========================================

    let blocked_emails = entities::get_blocked_emails(pool, account_id)?;
    let trusted_emails = entities::get_trusted_emails(pool, account_id)?;

    let tx = conn.unchecked_transaction()?;

    // Clear old conversations and rebuild fresh
    tx.execute(
        "DELETE FROM conversations WHERE account_id = ?1",
        params![account_id],
    )?;

    let now = chrono::Utc::now().timestamp_millis();
    let mut count = 0;

    for builder in conv_map.values() {
        // Skip blocked conversations:
        // - 1:1: skip if the single participant is blocked
        // - Group: skip if the initial sender is blocked
        if !blocked_emails.is_empty() {
            let participants: Vec<&str> = builder.participant_key.split('\n')
                .filter(|s| !s.is_empty())
                .collect();
            let is_group = participants.len() > 1;
            if is_group {
                if let Some(ref sender) = builder.initial_sender_email {
                    if blocked_emails.contains(sender.as_str()) {
                        continue;
                    }
                }
            } else if !participants.is_empty() && participants.iter().all(|p| blocked_emails.contains(*p)) {
                continue;
            }
        }

        // A conversation is trusted if any participant (sender, To, or Cc) is a connection
        let has_trusted = builder.has_trusted || builder.participant_key.split('\n')
            .any(|p| !p.is_empty() && trusted_emails.contains(p));

        let names_json = serde_json::to_string(&builder.names).ok();

        let classification = if builder.has_chat && has_trusted {
            Some("connections")
        } else if builder.has_chat {
            Some("others")
        } else {
            Some("automated")
        };

        tx.execute(
            "INSERT INTO conversations (
                id, account_id, participant_key, participant_names,
                classification, last_message_date, last_message_preview,
                unread_count, total_count, is_muted, is_pinned, is_important, updated_at,
                initial_sender_email
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?11, ?12)",
            params![
                builder.id,
                account_id,
                builder.participant_key,
                names_json,
                classification,
                builder.last_message_date,
                builder.last_message_preview,
                builder.unread_count,
                builder.total_count,
                builder.has_important,
                now,
                builder.initial_sender_email,
            ],
        )?;

        count += 1;
    }

    tx.commit()?;
    Ok(count)
}

struct ConversationBuilder {
    id: String,
    participant_key: String,
    last_message_date: i64,
    last_message_preview: Option<String>,
    has_chat: bool,
    has_trusted: bool,
    has_important: bool,
    unread_count: i32,
    total_count: i32,
    names: BTreeMap<String, String>,
    initial_sender_email: Option<String>,
}

#[derive(serde::Serialize)]
pub struct Conversation {
    pub id: String,
    pub account_id: String,
    pub participant_key: String,
    pub participant_names: Option<String>,
    pub classification: String,
    pub last_message_date: i64,
    pub last_message_preview: Option<String>,
    pub last_message_is_sent: bool,
    pub last_message_from_name: Option<String>,
    pub unread_count: i32,
    pub total_count: i32,
    pub is_muted: bool,
    pub is_pinned: bool,
    pub is_important: bool,
    pub updated_at: i64,
    pub initial_sender_email: Option<String>,
}

pub fn fetch_conversations(
    pool: &DbPool,
    account_id: &str,
) -> Result<Vec<Conversation>, EddieError> {
    let self_emails = entities::get_self_emails(pool, account_id)?;
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.account_id, c.participant_key, c.participant_names,
                    c.classification, c.last_message_date,
                    lm.distilled_text,
                    c.unread_count, c.is_muted, c.is_pinned, c.is_important, c.updated_at,
                    c.total_count,
                    lm.from_name, lm.from_address, lm.gmail_labels, lm.imap_folder,
                    c.initial_sender_email
             FROM conversations c
             LEFT JOIN (
                 SELECT conversation_id, distilled_text, from_name, from_address,
                        gmail_labels, imap_folder,
                        ROW_NUMBER() OVER (PARTITION BY conversation_id ORDER BY date DESC) AS rn
                 FROM messages
                 WHERE account_id = ?1
             ) lm ON lm.conversation_id = c.id AND lm.rn = 1
             WHERE c.account_id = ?1
             ORDER BY c.last_message_date DESC",
        )?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            let from_name: Option<String> = row.get(13)?;
            let from_address: Option<String> = row.get(14)?;
            let gmail_labels: Option<String> = row.get(15)?;
            let imap_folder: Option<String> = row.get(16)?;
            let is_sent = match (&from_address, &gmail_labels, &imap_folder) {
                (Some(addr), Some(labels), Some(folder)) =>
                    messages::is_sent(labels, folder, addr, &self_emails),
                _ => false,
            };
            Ok(Conversation {
                id: row.get(0)?,
                account_id: row.get(1)?,
                participant_key: row.get(2)?,
                participant_names: row.get(3)?,
                classification: row.get(4)?,
                last_message_date: row.get(5)?,
                last_message_preview: row.get(6)?,
                last_message_is_sent: is_sent,
                last_message_from_name: from_name,
                unread_count: row.get(7)?,
                is_muted: row.get::<_, i32>(8)? != 0,
                is_pinned: row.get::<_, i32>(9)? != 0,
                is_important: row.get::<_, i32>(10)? != 0,
                updated_at: row.get(11)?,
                total_count: row.get(12)?,
                initial_sender_email: row.get(17)?,
            })
        })?;

    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row?);
    }
    Ok(conversations)
}

// ----- Union-Find -----

struct UnionFind {
    parent: HashMap<String, String>,
}

impl UnionFind {
    fn new() -> Self {
        Self { parent: HashMap::new() }
    }

    fn find(&mut self, x: &str) -> String {
        if !self.parent.contains_key(x) {
            self.parent.insert(x.to_string(), x.to_string());
            return x.to_string();
        }
        let p = self.parent[x].clone();
        if p == x {
            return x.to_string();
        }
        let root = self.find(&p);
        self.parent.insert(x.to_string(), root.clone());
        root
    }

    fn union(&mut self, a: &str, b: &str) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

// ----- Helpers -----

fn is_self(addr: &str, self_emails: &[String]) -> bool {
    let normalized = normalize_email(addr);
    self_emails.iter().any(|s| normalize_email(s) == normalized)
}

fn collect_participants(
    from: &str,
    to: &[String],
    cc: &[String],
    self_emails: &[String],
) -> BTreeSet<String> {
    let mut participants = BTreeSet::new();
    let addr = normalize_email(from);
    if !is_self(&addr, self_emails) {
        participants.insert(addr);
    }
    for a in to {
        let addr = normalize_email(a);
        if !is_self(&addr, self_emails) {
            participants.insert(addr);
        }
    }
    for a in cc {
        let addr = normalize_email(a);
        if !is_self(&addr, self_emails) {
            participants.insert(addr);
        }
    }
    participants
}

// Build a name lookup from all messages in the account
fn name_lookup(
    pool: &DbPool,
    account_id: &str,
) -> Result<HashMap<String, String>, EddieError> {
    let conn = pool.get()?;

    let names: HashMap<String, String> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT from_address, from_name FROM messages
            WHERE account_id = ?1 AND from_name IS NOT NULL"
        )?;

        let rows = stmt.query_map(params![account_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        rows.filter_map(|r| r.ok())
            .map(|(addr, name)| (addr.to_lowercase(), name))
            .collect()
    };
    Ok(names)
}

pub fn get_connection_emails(pool: &DbPool, account_id: &str) -> Result<Vec<String>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT participant_key FROM conversations
         WHERE account_id = ?1 AND classification = 'connections'"
    )?;

    let keys: Vec<String> = stmt.query_map(params![account_id], |row| {
        row.get(0)
    })?
    .filter_map(|r| r.ok())
    .collect();

    let emails: Vec<String> = keys.iter()
        .flat_map(|k| k.split('\n'))
        .map(|s| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(emails)
}

#[derive(serde::Serialize)]
pub struct TrustContact {
    pub name: String,
    pub email: String,
    pub message_count: i32,
}

pub fn get_trust_contacts(pool: &DbPool, account_id: &str) -> Result<Vec<TrustContact>, EddieError> {
    let conn = pool.get()?;

    // Query entities with sent_count, joining messages for display names
    let mut stmt = conn.prepare(
        "SELECT e.email, e.display_name, e.sent_count,
                (SELECT m.from_name FROM messages m
                 WHERE m.from_address = e.email AND m.account_id = e.account_id
                   AND m.from_name IS NOT NULL AND m.from_name != ''
                 LIMIT 1) AS msg_name
         FROM entities e
         WHERE e.account_id = ?1 AND e.trust_level = 'connection' AND e.sent_count > 0
         ORDER BY e.sent_count DESC
         LIMIT 20"
    )?;

    let rows = stmt.query_map(params![account_id], |row| {
        let email: String = row.get(0)?;
        let display_name: Option<String> = row.get(1)?;
        let sent_count: i32 = row.get(2)?;
        let msg_name: Option<String> = row.get(3)?;
        Ok((email, display_name, sent_count, msg_name))
    })?;

    let mut contacts = Vec::new();
    for row in rows {
        let (email, display_name, sent_count, msg_name) = row?;
        let name = display_name
            .or(msg_name)
            .unwrap_or_else(|| email.clone());
        contacts.push(TrustContact { name, email, message_count: sent_count });
    }
    Ok(contacts)
}

pub fn count_trust_contacts(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let conn = pool.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE account_id = ?1 AND trust_level = 'connection' AND sent_count > 0",
        params![account_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}
