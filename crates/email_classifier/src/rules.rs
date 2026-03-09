//! Deterministic email classification rules.
//!
//! This module is the single source of truth for the rule-based classifier.
//! It is consumed by:
//!   - `lib.rs`        → PyO3 Python extension (used in the labeling notebook)
//!   - `bin/prelabel`  → standalone SQLite CLI (used in production pre-filtering)
//!
//! Rules are ordered from most to least confident and are based on:
//!   - Kang, Shang & Langlois (AAAI 2022) — Yahoo Mail human vs. machine classifier
//!   - Grbovic et al. (Yahoo Research 2014) — email type distribution analysis
//!   - RFC 2369 (List-Unsubscribe) / RFC 5321 (Return-Path)
//!   - Gmail & Yahoo bulk sender requirements (2024)

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

// ── Compiled static regexes ──────────────────────────────────────────────────

/// Mailing-list subject tags: [Newsletter], [GitHub], [JIRA-123], …
static MAILING_LIST_TAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[[\w\s\-]+\]").unwrap());

// ── Static lookup tables ─────────────────────────────────────────────────────

/// Known Email Service Provider sending domains.
/// Emails originating from these are definitively automated.
pub const ESP_DOMAINS: &[&str] = &[
    "mailchimp.com",
    "sendgrid.net",
    "amazonses.com",
    "mandrillapp.com",
    "mailgun.org",
    "sparkpostmail.com",
    "exacttarget.com",
    "mcsv.net",               // Mailchimp CDN
    "sendinblue.com",
    "brevo.com",              // Sendinblue rebranded
    "klaviyomail.com",
    "constantcontact.com",
    "campaignmonitor.com",
    "freshdesk.com",
    "zendesk.com",
    "salesforce.com",
    "marketo.net",
    "eloqua.com",
    "pardot.com",
    "hubspot.com",
    "mailerlite.com",
    "postmarkapp.com",
    "mailjet.com",
];

/// Local-part prefixes that identify automated senders.
/// Matched as: exact equality OR starts_with(prefix + "+" | prefix + ".").
pub const AUTOMATED_LOCAL_PREFIXES: &[&str] = &[
    "noreply",
    "no-reply",
    "donotreply",
    "do-not-reply",
    "mailer",
    "mailer-daemon",
    "newsletter",
    "notifications",
    "notification",
    "updates",
    "update",
    "alerts",
    "alert",
    "bounce",
    "bounces",
    "unsubscribe",
    "marketing",
    "promotions",
    "promotion",
    "deals",
    "offers",
    "news",
    "digest",
    "postmaster",
    "autoresponder",
    "automated",
    "system",
    "robot",
];

/// Gmail category labels that unambiguously indicate non-personal mail.
pub const GMAIL_NOT_CHAT_LABELS: &[&str] = &[
    "CATEGORY_PROMOTIONS",
    "CATEGORY_UPDATES",
    "CATEGORY_FORUMS",
    "CATEGORY_SOCIAL",
];

/// IMAP folder name substrings that imply automated mail buckets.
pub const AUTOMATED_FOLDER_PATTERNS: &[&str] = &[
    "newsletter",
    "promotions",
    "promotion",
    "updates",
    "bulk",
    "spam",
    "junk",
    "marketing",
    "notifications",
    "automated",
    "trash",
];

// ── Public types ─────────────────────────────────────────────────────────────

/// The outcome of the deterministic classifier for a single email.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Definitively a human-written personal email.
    Chat { reason: String },
    /// Definitively automated / bulk / non-personal email.
    NotChat { reason: String },
    /// No rule fired — forward to the ML model.
    Ambiguous,
}

impl Verdict {
    /// Convenience: label string for JSONL output.
    pub fn label(&self) -> Option<&str> {
        match self {
            Verdict::Chat { .. }    => Some("chat"),
            Verdict::NotChat { .. } => Some("not_chat"),
            Verdict::Ambiguous      => None,
        }
    }

    /// Convenience: human-readable reason for the decision.
    pub fn reason(&self) -> &str {
        match self {
            Verdict::Chat    { reason } => reason,
            Verdict::NotChat { reason } => reason,
            Verdict::Ambiguous          => "ambiguous",
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Verdict::Ambiguous)
    }
}

// ── Input struct ─────────────────────────────────────────────────────────────

/// All fields needed to run the deterministic classifier.
/// Strings are cheap to pass as `&str`; JSON array columns are pre-parsed.
pub struct EmailFields<'a> {
    pub from_address:  &'a str,
    pub subject:       &'a str,
    pub body_text:     &'a str,
    pub body_html:     &'a str,
    pub in_reply_to:   Option<&'a str>,
    /// Parsed from the `references_ids` JSON column.
    pub reference_count: usize,
    /// Parsed from `to_addresses` JSON column.
    pub to_count:      usize,
    /// Parsed from `cc_addresses` JSON column.
    pub cc_count:      usize,
    pub imap_folder:   &'a str,
    /// Parsed from `gmail_labels` JSON column.
    pub gmail_labels:  Vec<String>,
}

// ── Core classifier ──────────────────────────────────────────────────────────

/// Run all deterministic rules against `fields` and return a `Verdict`.
///
/// Rules are checked in strict priority order — the first match wins.
/// Each rule is documented with its evidence source.
pub fn classify(fields: &EmailFields<'_>) -> Verdict {
    let from_lower   = fields.from_address.to_lowercase();
    let subject_lower = fields.subject.to_lowercase();
    let body_lower   = fields.body_text.to_lowercase();
    let folder_lower = fields.imap_folder.to_lowercase();
    let combined_body = format!("{} {}", body_lower,
                                fields.body_html.to_lowercase());

    let total_recipients = fields.to_count + fields.cc_count;

    // ── NOT_CHAT rules ────────────────────────────────────────────────────

    // Rule 1: Gmail category labels (authoritative — set by Google's own classifier)
    for label in &fields.gmail_labels {
        if GMAIL_NOT_CHAT_LABELS.contains(&label.as_str()) {
            return Verdict::NotChat {
                reason: format!("gmail_label:{label}"),
            };
        }
    }

    // Rule 2: IMAP folder name implies automated bucket
    // (e.g. user or mail client pre-sorted into Promotions/Spam)
    for pattern in AUTOMATED_FOLDER_PATTERNS {
        if folder_lower.contains(pattern) {
            return Verdict::NotChat {
                reason: format!("imap_folder:{}", fields.imap_folder),
            };
        }
    }

    // Rule 3: Sending infrastructure — known ESP domain in From address
    // (RFC 5321: the sending MTA domain is the strongest infrastructure signal)
    if let Some(domain) = from_domain(&from_lower) {
        for esp in ESP_DOMAINS {
            if domain == *esp || domain.ends_with(&format!(".{esp}")) {
                return Verdict::NotChat {
                    reason: format!("esp_domain:{domain}"),
                };
            }
        }
    }

    // Rule 4: Automated local-part prefix in From address
    // e.g. noreply@, newsletter+abc@, alerts.uk@
    if let Some(local) = from_local(&from_lower) {
        for prefix in AUTOMATED_LOCAL_PREFIXES {
            if local == *prefix
                || local.starts_with(&format!("{prefix}+"))
                || local.starts_with(&format!("{prefix}."))
            {
                return Verdict::NotChat {
                    reason: format!("sender_local:{local}"),
                };
            }
        }
    }

    // Rule 5: Unsubscribe text in body
    // RFC 2369 requires List-Unsubscribe headers; most bulk senders also include
    // plaintext "unsubscribe" links — either is a definitive automated signal.
    if combined_body.contains("unsubscribe") || combined_body.contains("list-unsubscribe") {
        return Verdict::NotChat {
            reason: "body:unsubscribe_text".to_string(),
        };
    }

    // Rule 6: Mailing-list subject tag AND not a reply
    // Pattern: [SomeList] Subject text  →  bulk/list mail
    // Guard: if it has In-Reply-To, the user may be replying to a list thread
    if fields.in_reply_to.is_none() && MAILING_LIST_TAG.is_match(&subject_lower) {
        return Verdict::NotChat {
            reason: "subject:mailing_list_tag".to_string(),
        };
    }

    // Rule 7: Mass recipient count — more than 5 unique To+CC = broadcast
    // Personal emails virtually never have >5 named recipients
    if total_recipients > 5 {
        return Verdict::NotChat {
            reason: format!("recipients:{total_recipients}"),
        };
    }

    // ── CHAT rules ────────────────────────────────────────────────────────

    // Rule 8: Full thread context — In-Reply-To + References both present
    // This is the strongest possible chat signal: a real reply in an ongoing thread
    if fields.in_reply_to.is_some() && fields.reference_count > 0 {
        return Verdict::Chat {
            reason: "thread:in_reply_to+references".to_string(),
        };
    }

    // Rule 9: Direct reply — In-Reply-To alone
    // Still very strong: nearly all replies to personal emails carry this header
    if fields.in_reply_to.is_some() {
        return Verdict::Chat {
            reason: "thread:in_reply_to".to_string(),
        };
    }

    // No rule fired → forward to the ML model
    Verdict::Ambiguous
}

// ── JSON column parsers ───────────────────────────────────────────────────────

/// Parse a JSON array column and return its element count.
/// Returns 0 on any parse error (missing/null columns are treated as empty).
pub fn parse_json_array_len(json_str: &str) -> usize {
    match serde_json::from_str::<Value>(json_str) {
        Ok(Value::Array(v)) => v.len(),
        _ => 0,
    }
}

/// Parse a JSON string array column into a `Vec<String>`.
pub fn parse_json_string_array(json_str: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(json_str) {
        Ok(Value::Array(v)) => v
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => vec![],
    }
}

// ── Address helpers ───────────────────────────────────────────────────────────

/// Extract the domain part of an email address (already lowercased).
fn from_domain(addr: &str) -> Option<&str> {
    addr.split('@').nth(1)
}

/// Extract the local part of an email address (already lowercased).
fn from_local(addr: &str) -> Option<&str> {
    addr.split('@').next()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(from: &str) -> EmailFields<'_> {
        EmailFields {
            from_address:    from,
            subject:         "",
            body_text:       "",
            body_html:       "",
            in_reply_to:     None,
            reference_count: 0,
            to_count:        1,
            cc_count:        0,
            imap_folder:     "INBOX",
            gmail_labels:    vec![],
        }
    }

    #[test]
    fn esp_domain_fires() {
        let f = fields("info@em.mailchimp.com");
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn noreply_fires() {
        let f = fields("noreply@github.com");
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn noreply_plus_fires() {
        let f = fields("noreply+abc@example.com");
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn gmail_promo_fires() {
        let mut f = fields("someone@example.com");
        f.gmail_labels = vec!["CATEGORY_PROMOTIONS".to_string()];
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn unsubscribe_body_fires() {
        let mut f = fields("updates@company.com");
        f.body_text = "Click here to unsubscribe from this list.";
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn reply_with_references_is_chat() {
        let mut f = fields("alice@gmail.com");
        f.in_reply_to     = Some("<msg123@mail.gmail.com>");
        f.reference_count = 2;
        assert!(matches!(classify(&f), Verdict::Chat { .. }));
    }

    #[test]
    fn reply_only_is_chat() {
        let mut f = fields("bob@example.com");
        f.in_reply_to = Some("<msg456@example.com>");
        assert!(matches!(classify(&f), Verdict::Chat { .. }));
    }

    #[test]
    fn ambiguous_goes_to_llm() {
        let f = fields("bob@company.com");
        assert_eq!(classify(&f), Verdict::Ambiguous);
    }

    #[test]
    fn mass_recipients_fires() {
        let mut f = fields("team@company.com");
        f.to_count = 8;
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn mailing_list_subject_fires() {
        let mut f = fields("list@groups.example.com");
        f.subject = "[engineering-team] weekly update";
        assert!(matches!(classify(&f), Verdict::NotChat { .. }));
    }

    #[test]
    fn mailing_list_subject_is_ambiguous_when_reply() {
        let mut f = fields("alice@example.com");
        f.subject     = "[engineering-team] re: deploy schedule";
        f.in_reply_to = Some("<list-msg@groups.example.com>");
        // The reply rule fires before the subject rule fires
        assert!(matches!(classify(&f), Verdict::Chat { .. }));
    }
}