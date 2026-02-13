use futures::TryStreamExt;
use tracing::{debug, info};

use super::connection::ImapConnection;
use crate::types::error::EddieError;

pub async fn fetch_sent_recipients(
    conn: &mut ImapConnection,
    folder: &str,
    batch_size: u32,
) -> Result<Vec<String>, EddieError> {
    let mailbox = conn.select_folder(folder).await?;

    let exists = mailbox.exists;
    if exists == 0 {
        debug!(folder = %folder, "Sent folder is empty");
        return Ok(vec![]);
    }

    info!(folder = %folder, messages = exists, "Scanning sent folder for recipients");
    let mut all_addresses: Vec<String> = Vec::new();
    let mut start: u32 = 1;

    while start <= exists {
        let end = std::cmp::min(start + batch_size - 1, exists);
        let range = format!("{}:{}", start, end);

        let messages: Vec<_> = conn.session
            .fetch(&range, "BODY.PEEK[HEADER.FIELDS (To Cc Bcc)]")
            .await
            .map_err(|e| EddieError::Backend(format!("FETCH failed: {}", e)))?
            .try_collect()
            .await
            .map_err(|e| EddieError::Backend(format!("Failed to collect: {}", e)))?;

        for msg in &messages {
            if let Some(header_bytes) = msg.header() {
                let header_text = String::from_utf8_lossy(header_bytes);
                let addresses = parse_recipient_headers(&header_text);
                all_addresses.extend(addresses);
            }
        }

        start = end + 1;
    }

    // Deduplicate
    all_addresses.sort();
    all_addresses.dedup();

    info!(unique_recipients = all_addresses.len(), "Sent scan complete");
    Ok(all_addresses)
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