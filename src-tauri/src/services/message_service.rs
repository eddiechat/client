//! Message composition service
//!
//! Business logic for building and processing email messages.

use base64::Engine;
use std::fs;

use crate::types::error::{EddieError, Result};
use crate::types::ComposeAttachment;

/// Parameters for building a message with attachments
pub struct ComposeParams {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<ComposeAttachment>,
}

/// Build a raw email message with optional attachments
///
/// Creates a properly formatted MIME message that can be sent via SMTP.
pub fn build_message(params: ComposeParams) -> Result<Vec<u8>> {
    // Generate a unique boundary for the MIME message
    let boundary = format!(
        "----=_Part_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );

    let headers = build_headers(&params, &boundary);

    let raw_message = if params.attachments.is_empty() {
        // Simple text message
        format!("{}\r\n\r\n{}", headers, params.body)
    } else {
        // Multipart message with attachments
        let parts = build_multipart_body(&params.body, &params.attachments, &boundary)?;
        format!("{}\r\n\r\n{}", headers, parts)
    };

    Ok(raw_message.into_bytes())
}

/// Build email headers
fn build_headers(params: &ComposeParams, boundary: &str) -> String {
    let mut headers = vec![
        format!("From: {}", params.from),
        format!("To: {}", params.to.join(", ")),
        format!("Subject: {}", params.subject),
        format!(
            "Date: {}",
            chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000")
        ),
        "MIME-Version: 1.0".to_string(),
    ];

    // Add Cc if present
    if let Some(cc_addrs) = &params.cc {
        if !cc_addrs.is_empty() {
            headers.push(format!("Cc: {}", cc_addrs.join(", ")));
        }
    }

    // Set content type based on whether we have attachments
    if params.attachments.is_empty() {
        headers.push("Content-Type: text/plain; charset=utf-8".to_string());
        headers.push("Content-Transfer-Encoding: 8bit".to_string());
    } else {
        headers.push(format!(
            "Content-Type: multipart/mixed; boundary=\"{}\"",
            boundary
        ));
    }

    headers.join("\r\n")
}

/// Build multipart message body with attachments
fn build_multipart_body(
    body: &str,
    attachments: &[ComposeAttachment],
    boundary: &str,
) -> Result<String> {
    let mut parts = Vec::new();

    // Text body part
    parts.push(format!(
        "--{}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}",
        boundary, body
    ));

    // Attachment parts
    for attachment in attachments {
        let attachment_part = build_attachment_part(attachment, boundary)?;
        parts.push(attachment_part);
    }

    // Close the multipart
    parts.push(format!("--{}--", boundary));

    Ok(parts.join("\r\n"))
}

/// Build a single attachment part
fn build_attachment_part(attachment: &ComposeAttachment, boundary: &str) -> Result<String> {
    // Read file contents
    let file_contents = fs::read(&attachment.path).map_err(|e| {
        EddieError::Io(format!(
            "Failed to read attachment '{}': {}",
            attachment.path, e
        ))
    })?;

    // Base64 encode
    let encoded = base64::engine::general_purpose::STANDARD.encode(&file_contents);

    // Split into 76-character lines for email compatibility
    let encoded_lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(76)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();
    let encoded_formatted = encoded_lines.join("\r\n");

    Ok(format!(
        "--{}\r\nContent-Type: {}; name=\"{}\"\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"{}\"\r\n\r\n{}",
        boundary, attachment.mime_type, attachment.name, attachment.name, encoded_formatted
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_message() {
        let params = ComposeParams {
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            cc: None,
            subject: "Test Subject".to_string(),
            body: "Test body content".to_string(),
            attachments: vec![],
        };

        let result = build_message(params);
        assert!(result.is_ok());

        let message = String::from_utf8(result.unwrap()).unwrap();
        assert!(message.contains("From: sender@example.com"));
        assert!(message.contains("To: recipient@example.com"));
        assert!(message.contains("Subject: Test Subject"));
        assert!(message.contains("Test body content"));
    }
}
