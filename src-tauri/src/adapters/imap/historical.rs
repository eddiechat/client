use std::collections::HashMap;

use async_imap::types::Fetch;
use futures::StreamExt;
use imap_proto::types::{BodyStructure, ContentEncoding, SectionPath};
use crate::services::logger;

use super::connection::ImapConnection;
use super::envelopes::{parse_envelope, Envelope, parse_references_value};
use crate::error::EddieError;

/// Collects a FETCH stream tolerantly — logs and skips individual responses
/// that fail to parse (e.g., IMAP literal strings in BODYSTRUCTURE).
/// Returns all successfully parsed Fetch items.
pub async fn collect_tolerant<E: std::fmt::Display>(
    stream: impl futures::Stream<Item = Result<Fetch, E>>,
    context: &str,
) -> Vec<Fetch> {
    futures::pin_mut!(stream);
    let mut items = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(fetch) => items.push(fetch),
            Err(e) => {
                logger::warn(&format!(
                    "Skipping unparseable IMAP response ({}): {}", context, e
                ));
            }
        }
    }
    items
}


// ---------------------------------------------------------------------------
// Historical fetch
// ---------------------------------------------------------------------------

/// Fetch envelopes and text bodies for all messages since a given date,
/// processing in batches from newest to oldest.
///
/// For each batch, calls `on_batch` with the parsed envelopes and a list
/// of (UID, text_body) pairs. The caller can run classification, ingestion,
/// and conversation rebuilding per batch.
///
/// `since` must be in IMAP date format: "08-Feb-2025"
pub async fn fetch_historical<F>(
    conn: &mut ImapConnection,
    folder: &str,
    since: &str,
    batch_size: u32,
    max_batches: Option<u32>,
    below_uid: Option<u32>,  // Only process UIDs below this
    mut on_batch: F,
) -> Result<usize, EddieError>
where
    F: FnMut(Vec<Envelope>, Vec<(u32, String, bool)>) -> Result<(), String>,
{
    // Step 1: SELECT folder
    conn.select_folder(folder).await?;

    // Step 2: SEARCH for UIDs since date
    let uid_set = conn
        .session
        .uid_search(format!("SINCE {}", since))
        .await
        .map_err(|e| EddieError::Backend(format!("SEARCH failed: {}", e)))?;

    let mut uids: Vec<u32> = uid_set.into_iter().collect();
    uids.sort_unstable();
    uids.reverse();

    // Filter out already-processed UIDs
    if let Some(below) = below_uid {
        uids.retain(|&uid| uid < below);
    }

    let total = uids.len();
    if total == 0 {
        logger::debug(&format!("No UIDs to fetch in {}", folder));
        return Ok(0);
    }

    logger::info(&format!("Starting historical fetch: folder={}, total={}, since={}", folder, total, since));

    // Step 3: Process in batches
    let mut batch_count = 0;
    for chunk in uids.chunks(batch_size as usize) {
        let uid_list: String = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let fetch_query = if conn.has_gmail_ext {
            "(UID FLAGS ENVELOPE BODYSTRUCTURE X-GM-LABELS)"
        } else {
            "(UID FLAGS ENVELOPE BODYSTRUCTURE)"
        };

        // Round trip 1: Envelopes + BODYSTRUCTURE
        let fetches = collect_tolerant(
            conn.session
                .uid_fetch(&uid_list, fetch_query)
                .await
                .map_err(|e| EddieError::Backend(format!("FETCH envelopes failed: {}", e)))?,
            &format!("envelopes in {}", folder),
        ).await;

        let mut envelopes: Vec<Envelope> = Vec::new();
        let mut text_parts: Vec<(u32, Vec<u32>, bool, String)> = Vec::new();

        for fetch in &fetches {
            if let Some(env) = parse_envelope(fetch) {
                envelopes.push(env);
            }
            if let (Some(uid), Some(bs)) = (fetch.uid, fetch.bodystructure()) {
                if let Some((part, encoding)) = find_mime_part(bs, &[], "plain") {
                    text_parts.push((uid, part, false, encoding_to_string(encoding)));
                } else if let Some((part, encoding)) = find_mime_part(bs, &[], "html") {
                    text_parts.push((uid, part, true, encoding_to_string(encoding)));
                }
            }
        }

        // Round trip 2: References headers (once per batch, full uid_list)
        let refs_fetches = collect_tolerant(
            conn.session
                .uid_fetch(&uid_list, "(UID BODY.PEEK[HEADER.FIELDS (References)])")
                .await
                .map_err(|e| EddieError::Backend(format!("FETCH refs failed: {}", e)))?,
            &format!("references in {}", folder),
        ).await;

        for fetch in &refs_fetches {
            if let Some(uid) = fetch.uid {
                let refs = parse_references_value(
                    &String::from_utf8_lossy(fetch.header().unwrap_or(&[]))
                );
                if let Some(env) = envelopes.iter_mut().find(|e| e.uid == uid) {
                    env.references = refs;
                }
            }
        }

        // Round trip 3: Fetch text bodies, grouped by part number
        let mut bodies: Vec<(u32, String, bool)> = Vec::new();
        let mut uid_is_html: HashMap<u32, bool> = HashMap::new();
        let mut uid_encoding: HashMap<u32, String> = HashMap::new();

        if !text_parts.is_empty() {
            let mut by_part: HashMap<Vec<u32>, Vec<u32>> = HashMap::new();
            for (uid, part, is_html, encoding) in &text_parts {
                by_part.entry(part.clone()).or_default().push(*uid);
                uid_is_html.insert(*uid, *is_html);
                uid_encoding.insert(*uid, encoding.clone());
            }

            for (part, part_uids) in &by_part {
                let part_uid_list: String = part_uids
                    .iter()
                    .map(|u| u.to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                let fetch_query = format!("(UID BODY.PEEK[{}])", part_to_string(part));

                let body_fetches = collect_tolerant(
                    conn.session
                        .uid_fetch(&part_uid_list, &fetch_query)
                        .await
                        .map_err(|e| EddieError::Backend(format!("FETCH body failed: {}", e)))?,
                    &format!("bodies in {}", folder),
                ).await;

                let path = part_to_section_path(part);

                for fetch in &body_fetches {
                    if let Some(uid) = fetch.uid {
                        if let Some(section_data) = fetch.section(&path) {
                            let encoding = uid_encoding.get(&uid).cloned().unwrap_or_default();
                            let decoded = decode_body(section_data, &encoding)?;

                            let is_html = uid_is_html.get(&uid).copied().unwrap_or(false);
                            bodies.push((uid, decoded, is_html));
                        }
                    }
                }
            }
        }

        logger::debug(&format!("Processing batch: folder={}, batch={}, envelopes={}, bodies={}", folder, batch_count + 1, envelopes.len(), bodies.len()));
        on_batch(envelopes, bodies).map_err(|e| EddieError::Backend(e))?;

        batch_count += 1;
        if let Some(max) = max_batches {
            if batch_count >= max {
                return Ok(total);
            }
        }
    }

    Ok(total)
}

// ---------------------------------------------------------------------------
// MIME part helpers
// ---------------------------------------------------------------------------

pub fn find_mime_part<'a>(
    body: &'a BodyStructure<'a>,
    prefix: &[u32],
    subtype: &str,
) -> Option<(Vec<u32>, &'a ContentEncoding<'a>)> {
    match body {
        BodyStructure::Text { common, other, .. } => {
            if common.ty.subtype.to_lowercase() == subtype {
                let path = if prefix.is_empty() { vec![1] } else { prefix.to_vec() };
                Some((path, &other.transfer_encoding))
            } else {
                None
            }
        }
        BodyStructure::Basic { common, other, .. } => {
            let mime = format!(
                "{}/{}",
                common.ty.ty.to_lowercase(),
                common.ty.subtype.to_lowercase()
            );
            if mime == format!("text/{}", subtype) {
                let path = if prefix.is_empty() { vec![1] } else { prefix.to_vec() };
                Some((path, &other.transfer_encoding))
            } else {
                None
            }
        }
        BodyStructure::Multipart { bodies, .. } => {
            for (i, part) in bodies.iter().enumerate() {
                let mut part_path = prefix.to_vec();
                part_path.push((i + 1) as u32);
                if let Some(found) = find_mime_part(part, &part_path, subtype) {
                    return Some(found);
                }
            }
            None
        }
        BodyStructure::Message { body, .. } => {
            let inner = if prefix.is_empty() { vec![1] } else { prefix.to_vec() };
            find_mime_part(body, &inner, subtype)
        }
    }
}

pub fn encoding_to_string(enc: &ContentEncoding) -> String {
    match enc {
        ContentEncoding::SevenBit => "7bit".to_string(),
        ContentEncoding::EightBit => "8bit".to_string(),
        ContentEncoding::Binary => "binary".to_string(),
        ContentEncoding::Base64 => "base64".to_string(),
        ContentEncoding::QuotedPrintable => "quoted-printable".to_string(),
        ContentEncoding::Other(s) => s.to_lowercase(),
    }
}

pub fn part_to_string(part: &[u32]) -> String {
    part.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

pub fn part_to_section_path(part: &[u32]) -> SectionPath {
    SectionPath::Part(part.to_vec(), None)
}

pub fn decode_body(raw: &[u8], encoding: &str) -> Result<String, EddieError> {
    let bytes = match encoding {
        "quoted-printable" => {
            quoted_printable::decode(raw, quoted_printable::ParseMode::Robust)
                .unwrap_or_else(|_| raw.to_vec())
        }
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
        _ => raw.to_vec(),
    };
    Ok(String::from_utf8_lossy(&bytes).to_string())
}
