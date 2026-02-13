use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, BTreeSet};

use super::DbPool;
use crate::types::error::EddieError;

pub fn compute_conversation_id(participant_key: &str) -> String {
    let hash = Sha256::digest(participant_key.as_bytes());
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
    let mut threads: HashMap<String, Vec<&ThreadMsg>> = HashMap::new();
    for msg in &thread_msgs {
        let root = uf.find(&msg.message_id);
        threads.entry(root).or_default().push(msg);
    }

    // Per-thread: compute participant union + participant changes
    // Maps db_id → (participant_key, conversation_id, participant_changes)
    let mut msg_assignments: HashMap<String, (String, String, Option<String>)> = HashMap::new();

    for (_root, msgs) in &threads {
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
                (participant_key.clone(), conversation_id.clone(), changes),
            );

            prev_participants = current_participants;
        }
    }

    // Write thread assignments back to messages
    {
        let tx = conn.unchecked_transaction()?;
        for (db_id, (pkey, conv_id, changes)) in &msg_assignments {
            tx.execute(
                "UPDATE messages SET participant_key = ?1, conversation_id = ?2, participant_changes = ?3
                 WHERE id = ?4",
                params![pkey, conv_id, changes, db_id],
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
                "SELECT m.conversation_id, m.participant_key, m.date, m.subject,
                        m.from_address, m.from_name, m.classification, m.is_important,
                        m.imap_flags,
                        CASE WHEN e.id IS NOT NULL THEN 1 ELSE 0 END AS is_trusted
                 FROM messages m
                 LEFT JOIN entities e ON e.email = m.from_address AND e.account_id = m.account_id
                 WHERE m.account_id = ?1
                 ORDER BY m.conversation_id, m.date DESC",
            )?;

        let rows = stmt
            .query_map(params![account_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,    // conversation_id
                    row.get::<_, String>(1)?,    // participant_key
                    row.get::<_, i64>(2)?,       // date
                    row.get::<_, Option<String>>(3)?, // subject
                    row.get::<_, String>(4)?,    // from_address
                    row.get::<_, Option<String>>(5)?, // from_name
                    row.get::<_, Option<String>>(6)?, // classification
                    row.get::<_, i32>(7)?,       // is_important
                    row.get::<_, String>(8)?,    // imap_flags
                    row.get::<_, i32>(9)?,       // is_trusted
                ))
            })?;

        let mut map: HashMap<String, ConversationBuilder> = HashMap::new();
        for row in rows {
            let (conv_id, participant_key, date, subject, _from_address, _from_name,
                 classification, is_important, imap_flags, is_trusted) =
                row?;

            let builder = map.entry(conv_id.clone()).or_insert_with(|| {
                ConversationBuilder {
                    id: conv_id,
                    participant_key,
                    last_message_date: date,
                    last_message_preview: subject,
                    has_chat: false,
                    has_trusted: false,
                    has_important: false,
                    unread_count: 0,
                    total_count: 0,
                    names: BTreeMap::new(),
                }
            });

            if classification.as_deref() == Some("chat") {
                builder.has_chat = true;
            }
            if is_trusted != 0 {
                builder.has_trusted = true;
            }
            if is_important != 0 {
                builder.has_important = true;
            }
            if !imap_flags.contains("Seen") {
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

    let tx = conn.unchecked_transaction()?;

    // Clear old conversations and rebuild fresh
    tx.execute(
        "DELETE FROM conversations WHERE account_id = ?1",
        params![account_id],
    )?;

    let now = chrono::Utc::now().timestamp_millis();
    let mut count = 0;

    for builder in conv_map.values() {
        let names_json = serde_json::to_string(&builder.names).ok();

        let classification = if builder.has_chat && builder.has_trusted {
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
                unread_count, total_count, is_muted, is_pinned, is_important, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?11)",
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
    pub unread_count: i32,
    pub total_count: i32,
    pub is_muted: bool,
    pub is_pinned: bool,
    pub is_important: bool,
    pub updated_at: i64,
}

pub fn fetch_conversations(
    pool: &DbPool,
    account_id: &str,
) -> Result<Vec<Conversation>, EddieError> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, participant_key, participant_names,
                    classification, last_message_date, last_message_preview,
                    unread_count, is_muted, is_pinned, is_important, updated_at,
                    total_count
             FROM conversations
             WHERE account_id = ?1
             ORDER BY last_message_date DESC",
        )?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                account_id: row.get(1)?,
                participant_key: row.get(2)?,
                participant_names: row.get(3)?,
                classification: row.get(4)?,
                last_message_date: row.get(5)?,
                last_message_preview: row.get(6)?,
                unread_count: row.get(7)?,
                is_muted: row.get::<_, i32>(8)? != 0,
                is_pinned: row.get::<_, i32>(9)? != 0,
                is_important: row.get::<_, i32>(10)? != 0,
                updated_at: row.get(11)?,
                total_count: row.get(12)?,
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
    self_emails.iter().any(|s| s.eq_ignore_ascii_case(addr))
}

fn collect_participants(
    from: &str,
    to: &[String],
    cc: &[String],
    self_emails: &[String],
) -> BTreeSet<String> {
    let mut participants = BTreeSet::new();
    let addr = from.to_lowercase();
    if !is_self(&addr, self_emails) {
        participants.insert(addr);
    }
    for a in to {
        let addr = a.to_lowercase();
        if !is_self(&addr, self_emails) {
            participants.insert(addr);
        }
    }
    for a in cc {
        let addr = a.to_lowercase();
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
