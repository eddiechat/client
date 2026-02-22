use std::collections::HashMap;
use futures::TryStreamExt;
use crate::services::logger;

use super::connection::ImapConnection;
use crate::error::EddieError;

/// Scans one batch of sent messages (by UID) and returns recipient counts + the max UID processed.
///
/// - `above_uid`: only process UIDs strictly greater than this (pass None for the first batch)
/// - Returns `(recipient_counts, Some(max_uid), remaining_uids)` if messages were found,
///   or `(empty, None, 0)` if done.
pub async fn fetch_sent_recipients_batch(
    conn: &mut ImapConnection,
    batch_size: usize,
    above_uid: Option<u32>,
) -> Result<(HashMap<String, usize>, Option<u32>, usize), EddieError> {
    // UID SEARCH to get all UIDs, then filter client-side
    let search_query = match above_uid {
        Some(uid) => format!("UID {}:*", uid + 1),
        None => "ALL".to_string(),
    };

    let uid_set = conn.session
        .uid_search(&search_query)
        .await
        .map_err(|e| EddieError::Backend(format!("UID SEARCH failed: {}", e)))?;

    // Filter to UIDs strictly above the cursor (IMAP UID search can return the cursor UID itself)
    let mut uids: Vec<u32> = match above_uid {
        Some(cursor) => uid_set.into_iter().filter(|&uid| uid > cursor).collect(),
        None => uid_set.into_iter().collect(),
    };
    uids.sort();

    if uids.is_empty() {
        return Ok((HashMap::new(), None, 0));
    }

    let total_remaining = uids.len();

    // Take only batch_size UIDs
    let batch_uids: Vec<u32> = uids.into_iter().take(batch_size).collect();
    let max_uid = *batch_uids.last().unwrap();

    let uid_list: String = batch_uids.iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");

    logger::debug(&format!(
        "Sent scan batch: fetching {} UIDs (above {:?})",
        batch_uids.len(), above_uid
    ));

    let messages: Vec<_> = conn.session
        .uid_fetch(&uid_list, "BODY.PEEK[HEADER.FIELDS (To Cc Bcc)]")
        .await
        .map_err(|e| EddieError::Backend(format!("UID FETCH failed: {}", e)))?
        .try_collect()
        .await
        .map_err(|e| EddieError::Backend(format!("Failed to collect: {}", e)))?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    for msg in &messages {
        if let Some(header_bytes) = msg.header() {
            let header_text = String::from_utf8_lossy(header_bytes);
            let addresses = parse_recipient_headers(&header_text);
            for addr in addresses {
                *counts.entry(addr).or_insert(0) += 1;
            }
        }
    }

    logger::debug(&format!(
        "Sent scan batch: {} unique recipients from {} messages (max_uid={})",
        counts.len(), batch_uids.len(), max_uid
    ));

    let remaining_after_batch = total_remaining - batch_uids.len();
    Ok((counts, Some(max_uid), remaining_after_batch))
}

fn parse_recipient_headers(header_text: &str) -> Vec<String> {
    let mut addresses = Vec::new();

    for line in reassemble_folded_headers(header_text) {
        let lower = line.to_lowercase();
        if lower.starts_with("to:") || lower.starts_with("cc:") || lower.starts_with("bcc:") {
            // Strip the header name
            let value = line.splitn(2, ':').nth(1).unwrap_or("").trim();
            if let Ok(addrs) = mailparse::addrparse(value) {
                for addr in addrs.iter() {
                    match addr {
                        mailparse::MailAddr::Single(info) => {
                            addresses.push(info.addr.to_lowercase());
                        }
                        mailparse::MailAddr::Group(group) => {
                            for member in &group.addrs {
                                addresses.push(member.addr.to_lowercase());
                            }
                        }
                    }
                }
            }
        }
    }

    addresses
}

/// RFC 2822 headers can be folded across multiple lines.
/// A continuation line starts with whitespace.
fn reassemble_folded_headers(text: &str) -> Vec<String> {
    let mut headers: Vec<String> = Vec::new();

    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of previous header
            if let Some(last) = headers.last_mut() {
                last.push(' ');
                last.push_str(line.trim());
            }
        } else if !line.is_empty() {
            headers.push(line.to_string());
        }
    }

    headers
}
