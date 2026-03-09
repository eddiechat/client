use std::collections::HashMap;

use async_imap::types::Fetch;
use imap_proto::BodyStructure;

#[derive(Debug, serde::Serialize)]
pub struct Envelope {
    pub uid: u32,
    pub message_id: String,
    pub date: String,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub imap_flags: Vec<String>,
    pub size_bytes: Option<u32>,
    pub has_attachments: bool,
    pub gmail_labels: Vec<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    /// RFC headers relevant to classification (lowercase key → value).
    pub classification_headers: HashMap<String, String>,
}

pub fn parse_envelope(fetch: &Fetch) -> Option<Envelope> {
    let uid = fetch.uid?;
    let envelope = fetch.envelope()?;
    let flags: Vec<String> = fetch.flags().map(|f| format!("{:?}", f)).collect();
    let size_bytes = fetch.size;
    
    let has_attachments = fetch
        .bodystructure()
        .map(|bs| has_attachments(bs))
        .unwrap_or(false);

    let gmail_labels: Vec<String> = fetch
        .gmail_labels()
        .map(|labels| labels.iter().map(|l| l.trim_start_matches('\\').to_string()).collect())
        .unwrap_or_default();

    let message_id = envelope
        .message_id
        .as_ref()
        .map(|id| {
            let s = String::from_utf8_lossy(id).to_string();
            s.trim_matches(|c| c == '<' || c == '>').to_string()
        })
        .unwrap_or_default();

    let date = envelope
        .date
        .as_ref()
        .map(|d| String::from_utf8_lossy(d).to_string())
        .unwrap_or_default();

    let subject = envelope
        .subject
        .as_ref()
        .map(|s| decode_rfc2047(&String::from_utf8_lossy(s)))
        .unwrap_or_default();

    // Extract the first From address
    let (from_address, from_name) = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            let mailbox = addr.mailbox
                .as_ref()
                .map(|m| String::from_utf8_lossy(m).to_string())
                .unwrap_or_default();
            let host = addr.host
                .as_ref()
                .map(|h| String::from_utf8_lossy(h).to_string())
                .unwrap_or_default();
            let email = format!("{}@{}", mailbox, host);
            let name = addr.name
                .as_ref()
                .map(|n| decode_rfc2047(&String::from_utf8_lossy(n)));
            (email, name)
        })
        .unwrap_or_default();

    // Extract To addresses
    let to_addresses = extract_addresses(&envelope.to);
    let cc_addresses = extract_addresses(&envelope.cc);

    let in_reply_to = envelope
        .in_reply_to
        .as_ref()
        .map(|id| {
            let s = String::from_utf8_lossy(id).to_string();
            s.trim_matches(|c| c == '<' || c == '>').to_string()
        })
        .filter(|s| !s.is_empty());

    Some(Envelope {
        uid,
        message_id,
        date,
        subject,
        from_address,
        from_name,
        to_addresses,
        cc_addresses,
        imap_flags: flags,
        size_bytes,
        has_attachments,
        gmail_labels,
        in_reply_to,
        references: vec![],
        classification_headers: HashMap::new(),
    })
}

fn decode_rfc2047(input: &str) -> String {
    let fake_header = format!("X: {}", input);
    match mailparse::parse_header(fake_header.as_bytes()) {
        Ok((header, _)) => header.get_value(),
        Err(_) => input.to_string(),
    }
}

fn extract_addresses(addrs: &Option<Vec<async_imap::imap_proto::Address<'_>>>) -> Vec<String> {
    addrs
        .as_ref()
        .map(|list| {
            list.iter()
                .map(|addr| {
                    let mailbox = addr.mailbox
                        .as_ref()
                        .map(|m| String::from_utf8_lossy(m).to_string())
                        .unwrap_or_default();
                    let host = addr.host
                        .as_ref()
                        .map(|h| String::from_utf8_lossy(h).to_string())
                        .unwrap_or_default();
                    format!("{}@{}", mailbox, host)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has_attachments(body: &BodyStructure) -> bool {
    match body {
        BodyStructure::Basic { common, .. } => {
            if let Some(ref disposition) = common.disposition {
                let disp_type = disposition.ty.to_lowercase();
                if disp_type == "attachment" {
                    return true;
                }
            }
            let ty = common.ty.ty.to_lowercase();
            ty != "text"
        }
        BodyStructure::Text { .. } => false,
        BodyStructure::Message { body, .. } => has_attachments(body),
        BodyStructure::Multipart { bodies, .. } => {
            bodies.iter().any(|b| has_attachments(b))
        }
    }
}

/// Parse raw RFC 2822 header bytes into a map of lowercase header names to values.
/// Handles line continuation (folding) per RFC 2822 §2.2.3.
/// Only keeps headers in `CLASSIFICATION_HEADERS`; References is handled separately.
const CLASSIFICATION_HEADERS: &[&str] = &[
    "list-id",
    "auto-submitted",
    "list-unsubscribe",
    "precedence",
    "feedback-id",
    "x-mailer",
    "return-path",
];

pub fn parse_classification_headers(raw: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(raw);
    let mut result = HashMap::new();

    // Unfold continuation lines then split into individual header lines
    let mut current_name: Option<String> = None;
    let mut current_value = String::new();

    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of previous header
            current_value.push(' ');
            current_value.push_str(line.trim());
        } else if let Some(colon_pos) = line.find(':') {
            // Flush previous header
            if let Some(ref name) = current_name {
                if CLASSIFICATION_HEADERS.contains(&name.as_str()) {
                    result.insert(name.clone(), current_value.trim().to_string());
                }
            }
            // Start new header
            let name = line[..colon_pos].trim().to_lowercase();
            current_value = line[colon_pos + 1..].to_string();
            current_name = Some(name);
        }
    }

    // Flush last header
    if let Some(ref name) = current_name {
        if CLASSIFICATION_HEADERS.contains(&name.as_str()) {
            result.insert(name.clone(), current_value.trim().to_string());
        }
    }

    result
}

pub fn parse_references_value(header_text: &str) -> Vec<String> {
    // Unfold: join continuation lines (lines starting with whitespace)
    let unfolded = header_text
        .lines()
        .fold(String::new(), |mut acc, line| {
            if line.starts_with(' ') || line.starts_with('\t') {
                acc.push(' ');
                acc.push_str(line.trim());
            } else if !acc.is_empty() {
                acc.push(' ');
                acc.push_str(line);
            } else {
                acc.push_str(line);
            }
            acc
        });

    // Find the References: value
    if let Some(pos) = unfolded.to_lowercase().find("references:") {
        let value = &unfolded[pos + "references:".len()..];
        value
            .split_whitespace()
            .filter(|s| s.starts_with('<') && s.ends_with('>'))
            .map(|s| s[1..s.len() - 1].to_string())
            .collect()
    } else {
        vec![]
    }
}