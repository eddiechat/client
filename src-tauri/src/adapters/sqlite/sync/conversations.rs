use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, BTreeSet};

use super::DbPool;
use crate::error::EddieError;

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
                "SELECT m.conversation_id, m.participant_key, m.date, m.subject,
                        m.from_address, m.from_name, m.classification, m.is_important,
                        m.imap_flags,
                        CASE WHEN e.id IS NOT NULL THEN 1 ELSE 0 END AS is_trusted
                 FROM messages m
                 LEFT JOIN entities e ON e.email = m.from_address AND e.account_id = m.account_id
                                       AND e.trust_level NOT IN ('user', 'alias')
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

#[derive(serde::Serialize)]
pub struct Cluster {
    pub id: String,
    pub name: String,
    pub from_name: Option<String>,
    pub message_count: i32,
    pub unread_count: i32,
    pub keywords: String,
    pub last_activity: i64,
    pub account_id: String,
    pub is_join: bool,
    pub domains: String, // JSON array of sender emails
}
pub fn fetch_clusters(
    pool: &DbPool,
    account_id: &str,
) -> Result<Vec<Cluster>, EddieError> {
    let conn = pool.get()?;
    let sender_names = name_lookup(pool, account_id)?;

    // 1. Raw per-sender clusters
    let mut stmt = conn
        .prepare(
            "SELECT
                    from_address,
                    COUNT(*) AS message_count,
                    SUM(CASE WHEN NOT EXISTS (
                        SELECT 1 FROM json_each(imap_flags) WHERE value = 'Seen'
                    ) THEN 1 ELSE 0 END) AS unread_count,
                    MAX(date) AS last_activity,
                    account_id
                FROM messages
                WHERE account_id = ?1
                GROUP BY from_address
                ORDER BY last_activity DESC;",
        )?;

    struct RawCluster {
        sender: String,
        message_count: i32,
        unread_count: i32,
        last_activity: i64,
        account_id: String,
    }

    let raw: Vec<RawCluster> = stmt
        .query_map(params![account_id], |row| {
            Ok(RawCluster {
                sender: row.get(0)?,
                message_count: row.get(1)?,
                unread_count: row.get(2)?,
                last_activity: row.get(3)?,
                account_id: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // 2. Get sender → group_id mapping
    let sender_to_group = super::line_groups::get_domain_to_group(pool, account_id)?;

    // 3. Merge joined senders, keep singles as-is
    let mut join_groups: HashMap<String, Vec<&RawCluster>> = HashMap::new();
    let mut singles: Vec<&RawCluster> = Vec::new();

    for rc in &raw {
        if let Some(group_id) = sender_to_group.get(&rc.sender) {
            join_groups.entry(group_id.clone()).or_default().push(rc);
        } else {
            singles.push(rc);
        }
    }

    let mut clusters: Vec<Cluster> = Vec::new();

    // Joined groups — group_id is the user-chosen name
    for (group_id, members) in &join_groups {
        let mut senders: Vec<&str> = members.iter().map(|m| m.sender.as_str()).collect();
        senders.sort();
        let senders_json = serde_json::to_string(&senders).unwrap_or_else(|_| "[]".to_string());

        clusters.push(Cluster {
            id: group_id.clone(),
            name: group_id.clone(),
            from_name: None,
            message_count: members.iter().map(|m| m.message_count).sum(),
            unread_count: members.iter().map(|m| m.unread_count).sum(),
            keywords: "[]".to_string(),
            last_activity: members.iter().map(|m| m.last_activity).max().unwrap_or(0),
            account_id: members[0].account_id.clone(),
            is_join: true,
            domains: senders_json,
        });
    }

    // Singles
    for rc in &singles {
        let display_name = sender_names.get(&rc.sender.to_lowercase()).cloned();
        let name = display_name.clone().unwrap_or_else(|| rc.sender.clone());
        let senders_json = serde_json::to_string(&[&rc.sender]).unwrap_or_else(|_| "[]".to_string());
        clusters.push(Cluster {
            id: rc.sender.clone(),
            name,
            from_name: display_name,
            message_count: rc.message_count,
            unread_count: rc.unread_count,
            keywords: "[]".to_string(),
            last_activity: rc.last_activity,
            account_id: rc.account_id.clone(),
            is_join: false,
            domains: senders_json,
        });
    }

    // Sort by last_activity descending
    clusters.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    Ok(clusters)
}

#[derive(serde::Serialize)]
pub struct Thread {
    pub thread_id: String,
    pub subject: Option<String>,
    pub message_count: i32,
    pub unread_count: i32,
    pub last_activity: i64,
    pub from_name: Option<String>,
    pub from_address: String,
    pub preview: Option<String>,
}

pub fn fetch_cluster_threads(
    pool: &DbPool,
    account_id: &str,
    cluster_id: &str,
) -> Result<Vec<Thread>, EddieError> {
    // Resolve senders for this cluster
    let join_senders = super::line_groups::get_domains_for_group(pool, account_id, cluster_id)?;
    let senders: Vec<String> = if join_senders.is_empty() {
        vec![cluster_id.to_string()]
    } else {
        join_senders
    };

    let conn = pool.get()?;

    let placeholders: Vec<String> = senders.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
    let in_clause = placeholders.join(", ");

    // Query all messages for the cluster senders, ordered for processing
    let query = format!(
        "SELECT thread_id, subject, from_name, from_address, date, imap_flags, distilled_text
         FROM messages
         WHERE account_id = ?1
           AND thread_id IS NOT NULL
           AND from_address IN ({})
         ORDER BY thread_id, date ASC",
        in_clause
    );

    let mut stmt = conn.prepare(&query)?;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    param_values.push(Box::new(account_id.to_string()));
    for s in &senders {
        param_values.push(Box::new(s.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    struct MsgRow {
        thread_id: String,
        subject: Option<String>,
        from_name: Option<String>,
        from_address: String,
        date: i64,
        imap_flags: String,
        distilled_text: Option<String>,
    }

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(MsgRow {
            thread_id: row.get(0)?,
            subject: row.get(1)?,
            from_name: row.get(2)?,
            from_address: row.get(3)?,
            date: row.get(4)?,
            imap_flags: row.get(5)?,
            distilled_text: row.get(6)?,
        })
    })?;

    // Aggregate into threads
    let mut thread_map: HashMap<String, Thread> = HashMap::new();
    let mut thread_order: Vec<String> = Vec::new();

    for row in rows {
        let r = row?;
        let is_unread = {
            let flags: Vec<String> = serde_json::from_str(&r.imap_flags).unwrap_or_default();
            !flags.iter().any(|f| f == "Seen")
        };

        let thread = thread_map.entry(r.thread_id.clone()).or_insert_with(|| {
            thread_order.push(r.thread_id.clone());
            Thread {
                thread_id: r.thread_id.clone(),
                subject: r.subject.clone(),      // first message's subject
                message_count: 0,
                unread_count: 0,
                last_activity: r.date,
                from_name: r.from_name.clone(),   // first message's sender
                from_address: r.from_address.clone(),
                preview: None,
            }
        });

        thread.message_count += 1;
        if is_unread {
            thread.unread_count += 1;
        }
        // Update last_activity and preview from latest message
        if r.date >= thread.last_activity {
            thread.last_activity = r.date;
            if r.distilled_text.is_some() {
                thread.preview = r.distilled_text;
            }
        }
    }

    // Collect and sort by last_activity descending
    let mut threads: Vec<Thread> = thread_order
        .into_iter()
        .filter_map(|id| thread_map.remove(&id))
        .collect();
    threads.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    Ok(threads)
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
