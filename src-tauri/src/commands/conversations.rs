use crate::adapters::sqlite;
use crate::adapters::imap::historical;
use crate::adapters::sqlite::conversations::{Conversation, Cluster, Thread};
use crate::adapters::sqlite::messages::Message;
use crate::error::EddieError;
use crate::services::logger;
use crate::services::sync::worker;

#[tauri::command]
pub async fn fetch_conversations(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<Vec<Conversation>, EddieError> {
    sqlite::conversations::fetch_conversations(&pool, &account_id)
}

#[tauri::command]
pub async fn fetch_conversation_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    conversation_id: String,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_conversation_messages(&pool, &account_id, &conversation_id)
}

#[tauri::command]
pub async fn fetch_cluster_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    cluster_id: String,
) -> Result<Vec<Message>, EddieError> {
    if let Some(skill_id) = cluster_id.strip_prefix("skill:") {
        sqlite::messages::fetch_skill_match_messages(&pool, &account_id, skill_id)
    } else {
        sqlite::messages::fetch_cluster_messages(&pool, &account_id, &cluster_id)
    }
}

#[tauri::command]
pub async fn fetch_clusters(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<Vec<Cluster>, EddieError> {
    sqlite::conversations::fetch_clusters(&pool, &account_id)
}

#[tauri::command]
pub async fn fetch_cluster_threads(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    cluster_id: String,
) -> Result<Vec<Thread>, EddieError> {
    sqlite::conversations::fetch_cluster_threads(&pool, &account_id, &cluster_id)
}

#[tauri::command]
pub async fn fetch_thread_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    thread_id: String,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_thread_messages(&pool, &account_id, &thread_id)
}

#[tauri::command]
pub async fn group_domains(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    name: String,
    domains: Vec<String>,
) -> Result<String, EddieError> {
    sqlite::line_groups::group_domains(&pool, &account_id, &name, &domains)
}

#[tauri::command]
pub async fn ungroup_domains(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    group_id: String,
) -> Result<(), EddieError> {
    sqlite::line_groups::ungroup_domains(&pool, &account_id, &group_id)
}

#[tauri::command]
pub async fn move_to_lines(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    emails: Vec<String>,
) -> Result<(), EddieError> {
    for email in &emails {
        sqlite::entities::delete_entity(&pool, &account_id, email)?;
    }
    logger::info(&format!("Deleted entities, rebuilding conversations: account_id={}", account_id));
    sqlite::conversations::rebuild_conversations(&pool, &account_id)?;
    Ok(())
}

#[tauri::command]
pub async fn fetch_recent_messages(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
    limit: u32,
) -> Result<Vec<Message>, EddieError> {
    sqlite::messages::fetch_recent_messages(&pool, &account_id, limit)
}

#[tauri::command]
pub async fn fetch_message_html(
    pool: tauri::State<'_, sqlite::DbPool>,
    message_id: String,
) -> Result<Option<String>, EddieError> {
    let info = sqlite::messages::get_message_imap_info(&pool, &message_id)?;

    // Return cached HTML if available (and fully resolved — no remaining cid: refs)
    if let Some(ref html) = info.body_html {
        if !html.contains("cid:") {
            return Ok(Some(html.clone()));
        }
    }

    // Connect to IMAP and fetch the HTML part
    let (_creds, _self_emails, mut conn) = worker::connect_account(&pool, &info.account_id).await?;
    conn.select_folder(&info.imap_folder).await?;

    // Round trip 1: Get BODYSTRUCTURE to find the HTML MIME part and inline images
    let uid_str = info.imap_uid.to_string();
    let fetches = historical::collect_tolerant(
        conn.session
            .uid_fetch(&uid_str, "(UID BODYSTRUCTURE)")
            .await
            .map_err(|e| EddieError::Backend(format!("FETCH BODYSTRUCTURE failed: {}", e)))?,
        "message html bodystructure",
    ).await;

    let fetch = match fetches.first() {
        Some(f) => f,
        None => return Ok(None),
    };

    let bs = match fetch.bodystructure() {
        Some(bs) => bs,
        None => return Ok(None),
    };

    let (part, encoding) = match historical::find_mime_part(bs, &[], "html") {
        Some(found) => (found.0, historical::encoding_to_string(found.1)),
        None => return Ok(None),
    };

    // Also find inline image parts
    let inline_images = historical::find_inline_images(bs, &[]);
    logger::info(&format!("fetch_message_html: found {} inline images", inline_images.len()));
    for (img_part, cid, mime, enc) in &inline_images {
        logger::info(&format!("  image: part={} cid={} mime={} enc={}", historical::part_to_string(img_part), cid, mime, enc));
    }

    // Round trip 2: Fetch the HTML body part + all inline images in one request
    let mut parts_to_fetch: Vec<String> = vec![
        format!("BODY.PEEK[{}]", historical::part_to_string(&part)),
    ];
    for (img_part, _, _, _) in &inline_images {
        parts_to_fetch.push(format!("BODY.PEEK[{}]", historical::part_to_string(img_part)));
    }
    let fetch_query = format!("(UID {})", parts_to_fetch.join(" "));

    let body_fetches = historical::collect_tolerant(
        conn.session
            .uid_fetch(&uid_str, &fetch_query)
            .await
            .map_err(|e| EddieError::Backend(format!("FETCH body failed: {}", e)))?,
        "message html body + images",
    ).await;

    let body_fetch = match body_fetches.first() {
        Some(f) => f,
        None => return Ok(None),
    };

    // Extract HTML body
    let path = historical::part_to_section_path(&part);
    let section_data = match body_fetch.section(&path) {
        Some(data) => data,
        None => return Ok(None),
    };
    let mut html = historical::decode_body(section_data, &encoding)?;

    // Log cid: references found in the HTML
    let cid_refs: Vec<&str> = html_cid_refs(&html);
    logger::info(&format!("fetch_message_html: cid: refs in HTML: {:?}", cid_refs));

    // Extract inline images and replace cid: references with data: URIs
    for (img_part, cid, mime_type, img_encoding) in &inline_images {
        let img_path = historical::part_to_section_path(img_part);
        let img_data_opt = body_fetch.section(&img_path);
        logger::info(&format!("  image cid={}: section({:?}) data={}", cid, img_part, img_data_opt.map_or("NONE".into(), |d| format!("{} bytes", d.len()))));
        if let Some(img_data) = img_data_opt {
            // Decode transfer encoding, then re-encode as base64 for data: URI
            let raw_bytes = decode_transfer_encoding(img_data, img_encoding);
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);
            let data_uri = format!("data:{};base64,{}", mime_type, b64);
            let search = format!("cid:{}", cid);
            let count = html.matches(&search).count();
            logger::info(&format!("  replacing '{}' ({} occurrences) with data: URI ({} chars)", search, count, data_uri.len()));
            html = html.replace(&search, &data_uri);
        }
    }

    // Cache for next time
    let _ = sqlite::messages::update_body_html_by_id(&pool, &message_id, &html);

    Ok(Some(html))
}

fn html_cid_refs(html: &str) -> Vec<&str> {
    let mut refs = Vec::new();
    let mut search = html;
    while let Some(pos) = search.find("cid:") {
        let start = pos + 4;
        let end = search[start..].find(|c: char| c == '"' || c == '\'' || c == ')' || c == ' ' || c == '>' || c == ';')
            .map(|e| start + e)
            .unwrap_or(std::cmp::min(start + 60, search.len()));
        refs.push(&search[pos..end]);
        search = &search[end..];
    }
    refs
}

fn decode_transfer_encoding(raw: &[u8], encoding: &str) -> Vec<u8> {
    match encoding {
        "base64" => {
            let cleaned: Vec<u8> = raw.iter()
                .filter(|b| !b.is_ascii_whitespace())
                .copied()
                .collect();
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(&cleaned)
                .unwrap_or_else(|_| raw.to_vec())
        }
        "quoted-printable" => {
            quoted_printable::decode(raw, quoted_printable::ParseMode::Robust)
                .unwrap_or_else(|_| raw.to_vec())
        }
        _ => raw.to_vec(),
    }
}