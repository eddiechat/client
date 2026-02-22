use crate::adapters::sqlite::{self, DbPool};
use crate::error::EddieError;
use crate::services::logger;

pub fn distill_messages(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let messages = sqlite::messages::get_unextracted_messages(pool, account_id)?;
    if messages.is_empty() {
        return Ok(0);
    }
    logger::debug(&format!("Distilling message previews: account_id={}, pending={}", account_id, messages.len()));
    let mut count = 0;

    for msg in &messages {
        let result = distill(&msg.body_text, 200);
        sqlite::messages::update_extracted(
            pool,
            &msg.id,
            &result,
        )?;
        count += 1;
    }

    Ok(count)
}

/// Extract a chat-style preview from an email body.
/// Strips quotes, signatures, and forwarded headers,
/// then truncates to `max_len` characters.
pub fn distill(body: &str, max_len: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut clean_lines: Vec<&str> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        // Stop at signature
        if line.trim() == "--" || line.trim() == "-- " {
            break;
        }

        // Skip quoted lines
        if line.starts_with('>') {
            continue;
        }

        // Skip attribution line: any line ending with ':'
        // that is immediately followed by a quoted line
        let trimmed = line.trim();
        if trimmed.ends_with(':') || trimmed.ends_with("wrote:") {
            let next_non_empty = lines[i + 1..]
                .iter()
                .find(|l| !l.trim().is_empty());
            if let Some(next) = next_non_empty {
                if next.starts_with('>') {
                    continue;
                }
            }
        }


        // Stop at forwarded message
        if trimmed.starts_with("---------- Forwarded message")
            || trimmed.starts_with("Begin forwarded message")
            || trimmed.starts_with("---------- Videresendt mail")
        {
            clean_lines.push("[Forwarded]");
            break;
        }

        clean_lines.push(trimmed);
    }

    // Collapse into a single string, removing excessive blank lines
    let mut result = String::new();
    let mut prev_blank = false;

    for line in &clean_lines {
        if line.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push(' ');
            }
            prev_blank = true;
        } else {
            if !result.is_empty() && !prev_blank {
                result.push(' ');
            }
            result.push_str(line);
            prev_blank = false;
        }
    }

    let result = result.trim().to_string();

    if result.len() <= max_len {
        result
    } else {
        let truncated = &result[..floor_char_boundary(&result, max_len)];
        format!("{}…", truncated.trim_end())
    }
}

fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        s.len()
    } else {
        let mut i = index;
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_message() {
        let body = "Hey, are you free for lunch tomorrow?";
        assert_eq!(distill(body, 200), "Hey, are you free for lunch tomorrow?");
    }

    #[test]
    fn test_strips_quotes() {
        let body = "Sounds good!\n\n> On Feb 8, 2026, Alice wrote:\n> Let's meet at noon";
        assert_eq!(distill(body, 200), "Sounds good!");
    }

    #[test]
    fn test_strips_signature() {
        let body = "See you there!\n\n--\nBrian\nCEO, Acme Corp";
        assert_eq!(distill(body, 200), "See you there!");
    }

    #[test]
    fn test_strips_forwarded() {
        let body = "FYI see below\n\n---------- Forwarded message ----------\nFrom: Alice\nSubject: Hi\n\nOriginal content";
        assert_eq!(distill(body, 200), "FYI see below");
    }

    #[test]
    fn test_truncation() {
        let body = "a".repeat(300);
        let result = distill(&body, 200);
        assert!(result.len() <= 204); // 200 + "…"
    }

    #[test]
    fn test_danish_quote_attribution() {
        let body = "Lyder godt!\n\nDen 8. feb. 2026 kl. 12:00 skrev Martin:\n> Vi ses i morgen";
        assert_eq!(distill(body, 200), "Lyder godt!");
    }

    #[test]
    fn test_empty_body() {
        assert_eq!(distill("", 200), "");
    }
}
