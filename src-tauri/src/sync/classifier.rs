//! Message Classification
//!
//! Classifies messages as chat-worthy or non-chat (newsletters, automated, transactional).
//! Non-chat messages can be hidden from the chat UI but remain in cache.

#![allow(dead_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::sync::db::{CachedChatMessage, MessageClassification, SyncDatabase};
use crate::types::error::EddieError;

/// Message classification categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Classification {
    /// Human-to-human chat message
    Chat,
    /// Newsletter or mailing list
    Newsletter,
    /// Automated notification (GitHub, CI/CD, alerts)
    Automated,
    /// Transactional email (receipts, shipping, password reset)
    Transactional,
    /// Unknown/unclassified
    Unknown,
}

impl Classification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Newsletter => "newsletter",
            Self::Automated => "automated",
            Self::Transactional => "transactional",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "chat" => Self::Chat,
            "newsletter" => Self::Newsletter,
            "automated" => Self::Automated,
            "transactional" => Self::Transactional,
            _ => Self::Unknown,
        }
    }

    /// Should this classification be hidden from chat UI by default?
    pub fn is_hidden_by_default(&self) -> bool {
        match self {
            Self::Chat => false,
            Self::Newsletter => true,
            Self::Automated => true,
            Self::Transactional => true,
            Self::Unknown => false, // Show unknown messages
        }
    }
}

/// Classification result
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub classification: Classification,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

/// Legacy message classifier (simple rule-based)
pub struct LegacyMessageClassifier {
    newsletter_domains: HashSet<String>,
    automated_senders: HashSet<String>,
    transactional_patterns: Vec<String>,
}

impl LegacyMessageClassifier {
    /// Create a new legacy classifier
    pub fn new() -> Self {
        Self {
            newsletter_domains: Self::default_newsletter_domains(),
            automated_senders: Self::default_automated_senders(),
            transactional_patterns: Self::default_transactional_patterns(),
        }
    }

    fn default_newsletter_domains() -> HashSet<String> {
        [
            // Marketing platforms
            "mailchimp.com",
            "sendgrid.net",
            "sendgrid.com",
            "constantcontact.com",
            "campaign-archive.com",
            "mail.beehiiv.com",
            "substack.com",
            "buttondown.email",
            "convertkit.com",
            "mailerlite.com",
            "hubspot.com",
            "drip.com",
            "klaviyo.com",
            // News and newsletters
            "theatlantic.com",
            "nytimes.com",
            "washingtonpost.com",
            "medium.com",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn default_automated_senders() -> HashSet<String> {
        [
            // Code hosting
            "noreply@github.com",
            "notifications@github.com",
            "gitlab@mg.gitlab.com",
            "bitbucket@mg.bitbucket.org",
            // CI/CD
            "builds@circleci.com",
            "builds@travis-ci.com",
            "no-reply@vercel.com",
            "notify@netlify.com",
            // Cloud services
            "no-reply@sns.amazonaws.com",
            "noreply@google.com",
            "azure-noreply@microsoft.com",
            // Monitoring
            "alerts@sentry.io",
            "noreply@pagerduty.com",
            "notifications@datadoghq.com",
            // Project management
            "noreply@slack.com",
            "notification@asana.com",
            "noreply@trello.com",
            "notifications@linear.app",
            "no-reply@notion.so",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn default_transactional_patterns() -> Vec<String> {
        [
            "receipt",
            "invoice",
            "order confirmation",
            "shipping confirmation",
            "delivery notification",
            "password reset",
            "verify your email",
            "confirm your email",
            "account verification",
            "two-factor",
            "2fa",
            "security alert",
            "sign-in attempt",
            "subscription",
            "renewal",
            "payment",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    /// Classify a message
    pub fn classify(&self, message: &CachedChatMessage) -> ClassificationResult {
        let mut reasons: Vec<String> = Vec::new();
        let mut scores: Vec<(Classification, f32)> = Vec::new();

        let from_lower = message.from_address.to_lowercase();
        let subject_lower = message
            .subject
            .as_ref()
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Check for noreply/no-reply sender
        if from_lower.contains("noreply")
            || from_lower.contains("no-reply")
            || from_lower.contains("donotreply")
            || from_lower.contains("do-not-reply")
        {
            reasons.push("Sender is no-reply address".to_string());
            scores.push((Classification::Automated, 0.7));
        }

        // Check for automated senders
        if self.automated_senders.contains(&from_lower) {
            reasons.push(format!("Known automated sender: {}", from_lower));
            scores.push((Classification::Automated, 0.9));
        }

        // Check domain for newsletters
        if let Some(domain) = from_lower.split('@').last() {
            if self.newsletter_domains.contains(domain) {
                reasons.push(format!("Known newsletter domain: {}", domain));
                scores.push((Classification::Newsletter, 0.8));
            }
        }

        // Check for List-Unsubscribe header patterns
        if let Some(text) = &message.text_body {
            let text_lower = text.to_lowercase();
            if text_lower.contains("unsubscribe")
                || text_lower.contains("opt-out")
                || text_lower.contains("email preferences")
            {
                reasons.push("Contains unsubscribe link".to_string());
                scores.push((Classification::Newsletter, 0.6));
            }
        }

        // Check for transactional patterns in subject
        for pattern in &self.transactional_patterns {
            if subject_lower.contains(pattern) {
                reasons.push(format!(
                    "Subject contains transactional pattern: {}",
                    pattern
                ));
                scores.push((Classification::Transactional, 0.7));
                break;
            }
        }

        // Check for common newsletter subject patterns
        if subject_lower.contains("newsletter")
            || subject_lower.contains("digest")
            || subject_lower.contains("weekly update")
            || subject_lower.contains("daily update")
            || subject_lower.starts_with("[") && subject_lower.contains("]")
        {
            reasons.push("Subject suggests newsletter".to_string());
            scores.push((Classification::Newsletter, 0.6));
        }

        // Check for mailing list indicators
        if subject_lower.starts_with("re: [") || subject_lower.starts_with("[") {
            // Mailing list format like [list-name] or Re: [list-name]
            reasons.push("Mailing list format detected".to_string());
            scores.push((Classification::Newsletter, 0.5));
        }

        // Check for calendar/event notifications
        if subject_lower.contains("invitation:")
            || subject_lower.contains("calendar")
            || subject_lower.contains("accepted:")
            || subject_lower.contains("declined:")
            || subject_lower.contains("reminder:")
        {
            reasons.push("Calendar/event notification".to_string());
            scores.push((Classification::Automated, 0.8));
        }

        // If no signals found, likely a chat message
        if scores.is_empty() {
            return ClassificationResult {
                classification: Classification::Chat,
                confidence: 0.6,
                reasons: vec!["No automated/newsletter signals detected".to_string()],
            };
        }

        // Aggregate scores by classification
        let mut class_scores: std::collections::HashMap<Classification, f32> =
            std::collections::HashMap::new();

        for (class, score) in &scores {
            *class_scores.entry(*class).or_insert(0.0) += score;
        }

        // Find the highest scoring classification
        let (best_class, best_score) = class_scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or((Classification::Unknown, 0.0));

        // Normalize confidence to 0-1 range
        let confidence = (best_score / scores.len() as f32).min(1.0);

        ClassificationResult {
            classification: best_class,
            confidence,
            reasons,
        }
    }

}

// ---------------------------------------------------------------------------
// Enhanced Message Classification
// ---------------------------------------------------------------------------

/// Optional raw email headers for enhanced classification accuracy.
///
/// When available, RFC-standard headers like `List-Id` (RFC 2919),
/// `Auto-Submitted` (RFC 3834), and `List-Unsubscribe` (RFC 2369)
/// provide near-definitive classification signals.
#[derive(Debug, Clone, Default)]
pub struct EmailHeaders {
    /// Lowercase header name → list of values (headers can appear multiple times).
    headers: std::collections::HashMap<String, Vec<String>>,
}

impl EmailHeaders {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a header (name is lowercased automatically).
    pub fn insert(&mut self, name: &str, value: &str) {
        self.headers
            .entry(name.to_lowercase())
            .or_default()
            .push(value.to_string());
    }

    /// Get the first value for a header (case-insensitive lookup).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_lowercase())
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }

    /// Check whether a header is present.
    pub fn has(&self, name: &str) -> bool {
        self.headers.contains_key(&name.to_lowercase())
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
}

/// Internal weighted signal produced by a single analysis rule.
#[derive(Debug, Clone)]
struct Signal {
    classification: Classification,
    weight: f32,
    reason: String,
}

impl Signal {
    fn new(class: Classification, weight: f32, reason: impl Into<String>) -> Self {
        Self {
            classification: class,
            weight,
            reason: reason.into(),
        }
    }
}

/// Enhanced multi-signal message classifier.
pub struct EnhancedMessageClassifier {
    // -- Known senders (exact from-address matches) --
    automated_senders: HashSet<String>,

    // -- ESP domains (matched against from-address domain and suffixes) --
    marketing_esp_domains: HashSet<String>,
    transactional_esp_domains: HashSet<String>,
    mixed_esp_domains: HashSet<String>,

    // -- Patterns --
    noreply_local_parts: Vec<String>,
    transactional_subject_kw: Vec<String>,
    automated_subject_kw: Vec<String>,
    newsletter_subject_kw: Vec<String>,
}

impl EnhancedMessageClassifier {
    pub fn new() -> Self {
        Self {
            automated_senders: Self::build_automated_senders(),
            marketing_esp_domains: Self::build_marketing_esp_domains(),
            transactional_esp_domains: Self::build_transactional_esp_domains(),
            mixed_esp_domains: Self::build_mixed_esp_domains(),
            noreply_local_parts: Self::build_noreply_patterns(),
            transactional_subject_kw: Self::build_transactional_subject_keywords(),
            automated_subject_kw: Self::build_automated_subject_keywords(),
            newsletter_subject_kw: Self::build_newsletter_subject_keywords(),
        }
    }

    /// Classify using only the fields available in `CachedChatMessage`.
    pub fn classify(&self, message: &CachedChatMessage) -> ClassificationResult {
        self.classify_with_headers(message, None)
    }

    /// Classify with optional raw email headers for improved accuracy.
    pub fn classify_with_headers(
        &self,
        message: &CachedChatMessage,
        headers: Option<&EmailHeaders>,
    ) -> ClassificationResult {
        let mut signals: Vec<Signal> = Vec::new();

        let from_lower = message.from_address.to_lowercase();
        let subject_lower = message
            .subject
            .as_ref()
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Tier 1: RFC-standard headers (highest confidence)
        if let Some(hdrs) = headers {
            self.analyze_rfc_headers(hdrs, &mut signals);
        }

        // Tier 2: Sender analysis
        self.analyze_sender(&from_lower, &message.from_name, &mut signals);

        // Tier 3: Subject analysis
        if !subject_lower.is_empty() {
            self.analyze_subject(&subject_lower, &mut signals);
        }

        // Tier 4: Content analysis
        self.analyze_content(
            message.text_body.as_deref(),
            message.html_body.as_deref(),
            &mut signals,
        );

        // Tier 5: Conversation threading (positive Chat signal)
        self.analyze_threading(
            message.in_reply_to.as_deref(),
            message.references.as_deref(),
            &mut signals,
        );

        // Aggregate signals into a result
        Self::aggregate(signals)
    }

    fn analyze_rfc_headers(&self, hdrs: &EmailHeaders, out: &mut Vec<Signal>) {
        // Auto-Submitted (RFC 3834): definitive automated indicator.
        if let Some(val) = hdrs.get("auto-submitted") {
            let v = val.to_lowercase();
            if v != "no" {
                out.push(Signal::new(
                    Classification::Automated,
                    1.5,
                    format!("Auto-Submitted: {}", val),
                ));
                return;
            }
        }

        // List-Id (RFC 2919): definitive mailing list indicator.
        if hdrs.has("list-id") {
            out.push(Signal::new(
                Classification::Newsletter,
                1.5,
                "List-Id header present (RFC 2919)",
            ));
        }

        // List-Unsubscribe (RFC 2369): strong newsletter/bulk indicator.
        if hdrs.has("list-unsubscribe") {
            out.push(Signal::new(
                Classification::Newsletter,
                1.2,
                "List-Unsubscribe header present (RFC 2369)",
            ));
        }

        // Feedback-ID (Google bulk sender spec)
        if hdrs.has("feedback-id") {
            out.push(Signal::new(
                Classification::Newsletter,
                1.0,
                "Feedback-ID header present (bulk sender)",
            ));
        }

        // Precedence header
        if let Some(val) = hdrs.get("precedence") {
            match val.to_lowercase().trim() {
                "bulk" | "list" => {
                    out.push(Signal::new(
                        Classification::Newsletter,
                        1.0,
                        format!("Precedence: {}", val),
                    ));
                }
                "junk" => {
                    out.push(Signal::new(
                        Classification::Automated,
                        0.8,
                        "Precedence: junk",
                    ));
                }
                _ => {}
            }
        }

        // X-Mailer indicating a marketing platform
        if let Some(mailer) = hdrs.get("x-mailer") {
            let m = mailer.to_lowercase();
            let marketing_mailers = [
                "mailchimp",
                "phpmailer",
                "campaign",
                "sendinblue",
                "brevo",
                "hubspot",
                "klaviyo",
            ];
            for name in &marketing_mailers {
                if m.contains(name) {
                    out.push(Signal::new(
                        Classification::Newsletter,
                        0.8,
                        format!("X-Mailer indicates marketing platform: {}", mailer),
                    ));
                    break;
                }
            }
        }

        // Return-Path containing bounce/noreply
        if let Some(rp) = hdrs.get("return-path") {
            let rp_lower = rp.to_lowercase();
            if rp_lower.contains("bounce") || rp_lower.contains("noreply") {
                out.push(Signal::new(
                    Classification::Automated,
                    0.5,
                    "Return-Path suggests automated sender",
                ));
            }
        }
    }

    fn analyze_sender(
        &self,
        from_lower: &str,
        _from_name: &Option<String>,
        out: &mut Vec<Signal>,
    ) {
        // Exact known automated sender
        if self.automated_senders.contains(from_lower) {
            out.push(Signal::new(
                Classification::Automated,
                1.3,
                format!("Known automated sender: {}", from_lower),
            ));
            return;
        }

        // Noreply / donotreply in local part
        if let Some(local) = from_lower.split('@').next() {
            for pattern in &self.noreply_local_parts {
                if local.contains(pattern.as_str()) {
                    out.push(Signal::new(
                        Classification::Automated,
                        0.7,
                        format!("Sender local part matches noreply pattern: {}", pattern),
                    ));
                    break;
                }
            }
        }

        // ESP domain detection
        if let Some(domain) = from_lower.split('@').last() {
            if self.domain_matches_set(domain, &self.marketing_esp_domains) {
                out.push(Signal::new(
                    Classification::Newsletter,
                    1.0,
                    format!("From domain matches marketing ESP: {}", domain),
                ));
            } else if self.domain_matches_set(domain, &self.transactional_esp_domains) {
                out.push(Signal::new(
                    Classification::Transactional,
                    0.7,
                    format!("From domain matches transactional ESP: {}", domain),
                ));
            } else if self.domain_matches_set(domain, &self.mixed_esp_domains) {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.3,
                    format!("From domain matches mixed-use ESP: {}", domain),
                ));
            }
        }
    }

    fn domain_matches_set(&self, domain: &str, set: &HashSet<String>) -> bool {
        if set.contains(domain) {
            return true;
        }
        for known in set {
            if domain.ends_with(&format!(".{}", known)) {
                return true;
            }
        }
        false
    }

    fn analyze_subject(&self, subject_lower: &str, out: &mut Vec<Signal>) {
        // Transactional keywords
        for kw in &self.transactional_subject_kw {
            if subject_lower.contains(kw.as_str()) {
                out.push(Signal::new(
                    Classification::Transactional,
                    0.7,
                    format!("Subject contains transactional keyword: \"{}\"", kw),
                ));
                break;
            }
        }

        // Automated / notification keywords
        for kw in &self.automated_subject_kw {
            if subject_lower.contains(kw.as_str()) {
                out.push(Signal::new(
                    Classification::Automated,
                    0.6,
                    format!("Subject contains automated keyword: \"{}\"", kw),
                ));
                break;
            }
        }

        // Newsletter keywords
        for kw in &self.newsletter_subject_kw {
            if subject_lower.contains(kw.as_str()) {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.5,
                    format!("Subject contains newsletter keyword: \"{}\"", kw),
                ));
                break;
            }
        }

        // Calendar / event notifications
        let calendar_prefixes = [
            "invitation:",
            "accepted:",
            "declined:",
            "tentative:",
            "canceled:",
            "cancelled:",
            "updated invitation:",
        ];
        for prefix in &calendar_prefixes {
            if subject_lower.starts_with(prefix) {
                out.push(Signal::new(
                    Classification::Automated,
                    0.9,
                    format!("Calendar event format: starts with \"{}\"", prefix),
                ));
                break;
            }
        }

        // GitHub-style bracketed prefix
        if subject_lower.starts_with("[")
            && subject_lower
                .find(']')
                .map_or(false, |pos| pos < 60 && pos > 1)
        {
            let after_bracket = &subject_lower[subject_lower.find(']').unwrap() + 1..];
            let notification_verbs = [
                "new issue",
                "pull request",
                "merged",
                "closed",
                "opened",
                "commented",
                "assigned",
                "mentioned",
                "review requested",
                "build",
                "failed",
                "passed",
                "deployed",
            ];
            for verb in &notification_verbs {
                if after_bracket.contains(verb) {
                    out.push(Signal::new(
                        Classification::Automated,
                        0.7,
                        format!("Bracketed prefix with notification verb: \"{}\"", verb),
                    ));
                    break;
                }
            }
        }
    }

    fn analyze_content(
        &self,
        text_body: Option<&str>,
        html_body: Option<&str>,
        out: &mut Vec<Signal>,
    ) {
        if let Some(text) = text_body {
            let text_lower = text.to_lowercase();

            // Unsubscribe / opt-out
            if text_lower.contains("unsubscribe")
                || text_lower.contains("opt-out")
                || text_lower.contains("opt out")
                || text_lower.contains("email preferences")
                || text_lower.contains("manage your subscription")
            {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.6,
                    "Body contains unsubscribe/opt-out language",
                ));
            }

            // "View in browser"
            if text_lower.contains("view in browser")
                || text_lower.contains("view this email in")
                || text_lower.contains("view as a web")
                || text_lower.contains("view in your browser")
            {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.5,
                    "Body contains \"view in browser\" text",
                ));
            }

            // Transactional body patterns
            let transactional_body = [
                "order number",
                "order #",
                "tracking number",
                "track your",
                "shipping details",
                "delivery status",
                "your receipt",
                "amount charged",
                "payment of $",
                "payment of €",
                "has been processed",
            ];
            for pattern in &transactional_body {
                if text_lower.contains(pattern) {
                    out.push(Signal::new(
                        Classification::Transactional,
                        0.5,
                        format!("Body contains transactional phrase: \"{}\"", pattern),
                    ));
                    break;
                }
            }
        }

        if let Some(html) = html_body {
            let html_lower = html.to_lowercase();

            // Link density
            let link_count = html_lower.matches("href=").count();
            if link_count > 15 {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.6,
                    format!("High link density in HTML body ({} links)", link_count),
                ));
            } else if link_count > 8 {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.3,
                    format!("Moderate link density in HTML body ({} links)", link_count),
                ));
            }

            // Tracking pixel detection
            if (html_lower.contains("width=\"1\"") && html_lower.contains("height=\"1\""))
                || (html_lower.contains("width='1'") && html_lower.contains("height='1'"))
                || (html_lower.contains("width:1px") && html_lower.contains("height:1px"))
                || html_lower.contains("width=1 height=1")
            {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.5,
                    "HTML contains tracking pixel (1×1 image)",
                ));
            }

            // HTML-only with no text/plain alternative
            if text_body.is_none() && html.len() > 2000 {
                out.push(Signal::new(
                    Classification::Newsletter,
                    0.3,
                    "HTML-only email with no text/plain alternative",
                ));
            }
        }
    }

    fn analyze_threading(
        &self,
        in_reply_to: Option<&str>,
        references: Option<&str>,
        out: &mut Vec<Signal>,
    ) {
        if in_reply_to.is_some() {
            out.push(Signal::new(
                Classification::Chat,
                0.8,
                "Message is a reply (In-Reply-To header present)",
            ));
        }

        if let Some(refs) = references {
            let ref_count = refs.matches('<').count();
            if ref_count >= 3 {
                out.push(Signal::new(
                    Classification::Chat,
                    1.0,
                    format!("Deep conversation thread ({} references)", ref_count),
                ));
            } else if ref_count >= 1 {
                out.push(Signal::new(
                    Classification::Chat,
                    0.5,
                    "Part of a conversation thread",
                ));
            }
        }
    }

    fn aggregate(signals: Vec<Signal>) -> ClassificationResult {
        if signals.is_empty() {
            return ClassificationResult {
                classification: Classification::Chat,
                confidence: 0.5,
                reasons: vec!["No classification signals detected; defaulting to Chat".into()],
            };
        }

        let mut totals: std::collections::HashMap<Classification, f32> =
            std::collections::HashMap::new();
        let mut reasons: Vec<String> = Vec::new();

        for sig in &signals {
            *totals.entry(sig.classification).or_insert(0.0) += sig.weight;
            reasons.push(sig.reason.clone());
        }

        let mut ranked: Vec<(Classification, f32)> = totals.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (winner, winner_score) = ranked[0];
        let runner_up_score = ranked.get(1).map(|r| r.1).unwrap_or(0.0);

        let margin = winner_score - runner_up_score;
        let confidence = if margin > 2.0 {
            0.95
        } else if margin > 1.2 {
            0.88
        } else if margin > 0.7 {
            0.78
        } else if margin > 0.3 {
            0.65
        } else {
            0.52
        };

        ClassificationResult {
            classification: winner,
            confidence,
            reasons,
        }
    }

    fn build_automated_senders() -> HashSet<String> {
        [
            "noreply@github.com",
            "notifications@github.com",
            "gitlab@mg.gitlab.com",
            "bitbucket@mg.bitbucket.org",
            "builds@circleci.com",
            "builds@travis-ci.com",
            "no-reply@vercel.com",
            "notify@netlify.com",
            "noreply-dmarc-support@google.com",
            "no-reply@sns.amazonaws.com",
            "noreply@google.com",
            "azure-noreply@microsoft.com",
            "alerts@sentry.io",
            "noreply@pagerduty.com",
            "notifications@datadoghq.com",
            "alertmanager@prometheus.io",
            "noreply@slack.com",
            "notification@asana.com",
            "noreply@trello.com",
            "notifications@linear.app",
            "no-reply@notion.so",
            "noreply@atlassian.com",
            "jira@atlassian.com",
            "noreply@accounts.google.com",
            "account-security-noreply@accountprotection.microsoft.com",
            "no-reply@access.watch",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_marketing_esp_domains() -> HashSet<String> {
        [
            "mailchimp.com",
            "mail.mailchimp.com",
            "campaign-archive.com",
            "constantcontact.com",
            "mail.beehiiv.com",
            "substack.com",
            "buttondown.email",
            "convertkit.com",
            "mailerlite.com",
            "hubspot.com",
            "drip.com",
            "klaviyo.com",
            "getresponse.com",
            "aweber.com",
            "activecampaign.com",
            "campaignmonitor.com",
            "createsend.com",
            "sendinblue.com",
            "brevo.com",
            "mailjet.com",
            "moosend.com",
            "benchmarkemail.com",
            "keap-mail.com",
            "infusionmail.com",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_transactional_esp_domains() -> HashSet<String> {
        [
            "postmarkapp.com",
            "mandrillapp.com",
            "sparkpostmail.com",
            "ses.amazonaws.com",
            "amazonses.com",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_mixed_esp_domains() -> HashSet<String> {
        [
            "sendgrid.net",
            "sendgrid.com",
            "mailgun.org",
            "mailgun.com",
            "smtp.com",
            "socketlabs.com",
            "pepipost.com",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_noreply_patterns() -> Vec<String> {
        [
            "noreply",
            "no-reply",
            "no_reply",
            "donotreply",
            "do-not-reply",
            "do_not_reply",
            "notifications",
            "notification",
            "mailer-daemon",
            "postmaster",
            "auto-confirm",
            "auto-reply",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_transactional_subject_keywords() -> Vec<String> {
        [
            "receipt",
            "invoice",
            "order confirmation",
            "shipping confirmation",
            "delivery notification",
            "delivery update",
            "password reset",
            "reset your password",
            "verify your email",
            "confirm your email",
            "email verification",
            "account verification",
            "two-factor",
            "2fa code",
            "verification code",
            "security code",
            "security alert",
            "sign-in attempt",
            "login attempt",
            "new sign-in",
            "subscription confirmed",
            "payment received",
            "payment confirmation",
            "payment failed",
            "refund",
            "billing statement",
            "your order",
            "shipment",
            "out for delivery",
            "has been delivered",
            "has shipped",
            "renewal notice",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_automated_subject_keywords() -> Vec<String> {
        [
            "build failed",
            "build succeeded",
            "build passed",
            "pipeline failed",
            "pipeline succeeded",
            "deployment",
            "deployed to",
            "deploy failed",
            "incident",
            "alert:",
            "warning:",
            "error:",
            "monitoring alert",
            "uptime alert",
            "downtime",
            "disk space",
            "cpu usage",
            "new comment on",
            "mentioned you",
            "assigned to you",
            "review requested",
            "merge request",
            "pull request",
            "new issue:",
            "issue closed",
            "commit pushed",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn build_newsletter_subject_keywords() -> Vec<String> {
        [
            "newsletter",
            "digest",
            "weekly update",
            "daily update",
            "monthly roundup",
            "weekly roundup",
            "this week in",
            "top stories",
            "what's new in",
            "issue #",
            "edition #",
            "curated",
            "weekly picks",
            "daily brief",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }
}

/// Message classifier wrapper that runs legacy classifier first,
/// then enhanced classifier on messages classified as "chat".
/// Optionally delegates to an Ollama-based LLM classifier when configured.
pub struct MessageClassifier {
    db: Arc<SyncDatabase>,
    legacy: LegacyMessageClassifier,
    enhanced: EnhancedMessageClassifier,
    ollama: tokio::sync::RwLock<Option<crate::sync::ollama_classifier::OllamaClassifier>>,
}

impl MessageClassifier {
    /// Create a new message classifier
    pub fn new(db: Arc<SyncDatabase>) -> Self {
        Self {
            db,
            legacy: LegacyMessageClassifier::new(),
            enhanced: EnhancedMessageClassifier::new(),
            ollama: tokio::sync::RwLock::new(None),
        }
    }

    /// Set or clear the Ollama classifier
    pub async fn set_ollama(
        &self,
        classifier: Option<crate::sync::ollama_classifier::OllamaClassifier>,
    ) {
        let mut ollama = self.ollama.write().await;
        *ollama = classifier;
    }

    /// Get the current Ollama config hash (if configured)
    pub async fn ollama_config_hash(&self) -> Option<String> {
        let ollama = self.ollama.read().await;
        ollama.as_ref().map(|o| o.config_hash().to_string())
    }

    /// Classify using Ollama if available, falling back to rule-based.
    /// Stores the result in the database with the appropriate `classified_by` value.
    pub async fn classify_and_store_async(
        &self,
        message: &CachedChatMessage,
    ) -> Result<ClassificationResult, EddieError> {
        let ollama = self.ollama.read().await;
        let (result, classified_by) = if let Some(ollama_classifier) = ollama.as_ref() {
            match ollama_classifier.classify(message).await {
                Ok(result) => (result, Some(ollama_classifier.config_hash().to_string())),
                Err(e) => {
                    tracing::warn!(
                        "Ollama classification failed, falling back to rule-based: {}",
                        e
                    );
                    (self.classify(message), None)
                }
            }
        } else {
            (self.classify(message), None)
        };

        let classification = MessageClassification {
            message_id: message.id,
            classification: result.classification.as_str().to_string(),
            confidence: result.confidence,
            is_hidden_from_chat: result.classification.is_hidden_by_default(),
            classified_at: Utc::now(),
            classified_by,
        };

        self.db.set_message_classification(&classification)?;
        Ok(result)
    }

    /// Classify a message using the sequential approach:
    /// 1. Run legacy classifier first
    /// 2. If result is "Chat", run enhanced classifier
    /// 3. Return the final result
    pub fn classify(&self, message: &CachedChatMessage) -> ClassificationResult {
        // Step 1: Run legacy classifier
        let legacy_result = self.legacy.classify(message);

        // Step 2: If legacy classified as Chat, run enhanced classifier
        if legacy_result.classification == Classification::Chat {
            self.enhanced.classify(message)
        } else {
            // Use legacy result for non-chat messages
            legacy_result
        }
    }

    /// Classify and store result in database
    pub fn classify_and_store(
        &self,
        message: &CachedChatMessage,
    ) -> Result<ClassificationResult, EddieError> {
        let result = self.classify(message);

        let classification = MessageClassification {
            message_id: message.id,
            classification: result.classification.as_str().to_string(),
            confidence: result.confidence,
            is_hidden_from_chat: result.classification.is_hidden_by_default(),
            classified_at: Utc::now(),
            classified_by: None,
        };

        self.db.set_message_classification(&classification)?;

        Ok(result)
    }

    /// Batch classify messages
    pub fn classify_batch(
        &self,
        messages: &[CachedChatMessage],
    ) -> Result<Vec<ClassificationResult>, EddieError> {
        let mut results = Vec::with_capacity(messages.len());

        for message in messages {
            let result = self.classify_and_store(message)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get classification for a message (from cache or compute)
    pub fn get_or_classify(
        &self,
        message: &CachedChatMessage,
    ) -> Result<ClassificationResult, EddieError> {
        // Try to get from cache
        if let Some(cached) = self.db.get_message_classification(message.id)? {
            return Ok(ClassificationResult {
                classification: Classification::from_str(&cached.classification),
                confidence: cached.confidence,
                reasons: vec!["Cached classification".to_string()],
            });
        }

        // Classify and store
        self.classify_and_store(message)
    }

    /// Check if a message should be shown in chat UI
    pub fn is_chat_message(&self, message: &CachedChatMessage) -> Result<bool, EddieError> {
        let result = self.get_or_classify(message)?;
        Ok(result.classification == Classification::Chat
            || result.classification == Classification::Unknown)
    }

    /// Update classification settings (mark a message as chat/non-chat)
    pub fn set_hidden(&self, message_id: i64, hidden: bool) -> Result<(), EddieError> {
        if let Some(mut classification) = self.db.get_message_classification(message_id)? {
            classification.is_hidden_from_chat = hidden;
            self.db.set_message_classification(&classification)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_message(from: &str, subject: &str, body: Option<&str>) -> CachedChatMessage {
        CachedChatMessage {
            id: 1,
            account_id: "test@example.com".to_string(),
            folder_name: "INBOX".to_string(),
            uid: 1,
            message_id: None,
            in_reply_to: None,
            references: None,
            from_address: from.to_string(),
            from_name: None,
            to_addresses: "[]".to_string(),
            cc_addresses: None,
            subject: Some(subject.to_string()),
            date: Some(Utc::now()),
            flags: "[]".to_string(),
            has_attachment: false,
            body_cached: body.is_some(),
            text_body: body.map(|s| s.to_string()),
            html_body: None,
            raw_size: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_classify_chat() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        let msg = make_test_message("friend@example.com", "Hey, how are you?", None);
        let result = classifier.classify(&msg);

        assert_eq!(result.classification, Classification::Chat);
    }

    #[test]
    fn test_classify_automated() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        let msg = make_test_message("noreply@github.com", "[repo] New issue: Bug report", None);
        let result = classifier.classify(&msg);

        assert_eq!(result.classification, Classification::Automated);
    }

    #[test]
    fn test_classify_newsletter() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        let msg = make_test_message(
            "newsletter@substack.com",
            "Weekly Newsletter: Top Stories",
            Some("Click here to unsubscribe from this newsletter."),
        );
        let result = classifier.classify(&msg);

        assert_eq!(result.classification, Classification::Newsletter);
    }

    #[test]
    fn test_classify_transactional() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        let msg = make_test_message("orders@store.com", "Your order confirmation #12345", None);
        let result = classifier.classify(&msg);

        assert_eq!(result.classification, Classification::Transactional);
    }

    #[test]
    fn test_sequential_classification() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        // Test 1: Non-chat message (automated) - should use legacy result, not run enhanced
        let automated_msg = make_test_message(
            "noreply@github.com",
            "[repo] New issue opened",
            None,
        );
        let result = classifier.classify(&automated_msg);
        assert_eq!(result.classification, Classification::Automated);

        // Test 2: Chat message (no signals) - should run both classifiers
        // Legacy will classify as Chat, then enhanced will refine
        let chat_msg = make_test_message(
            "friend@example.com",
            "Hey, want to grab lunch?",
            Some("Let me know if you're free!"),
        );
        let result = classifier.classify(&chat_msg);
        assert_eq!(result.classification, Classification::Chat);

        // Test 3: Newsletter classified by legacy - should use legacy result
        let newsletter_msg = make_test_message(
            "newsletter@substack.com",
            "Weekly Newsletter: Top Stories",
            Some("Click here to unsubscribe"),
        );
        let result = classifier.classify(&newsletter_msg);
        assert_eq!(result.classification, Classification::Newsletter);
    }

    #[test]
    fn test_enhanced_classifier_reclassifies_chat() {
        let db = Arc::new(SyncDatabase::in_memory().unwrap());
        let classifier = MessageClassifier::new(db);

        // Message with threading that legacy might classify as chat,
        // but enhanced should confirm as chat with higher confidence
        let mut threaded_msg = make_test_message(
            "colleague@work.com",
            "Re: Project update",
            Some("Sounds good, let's meet tomorrow."),
        );
        threaded_msg.in_reply_to = Some("<previous@example.com>".to_string());
        threaded_msg.references = Some("<a@x.com> <b@x.com> <c@x.com>".to_string());

        let result = classifier.classify(&threaded_msg);
        assert_eq!(result.classification, Classification::Chat);
        // Enhanced classifier should give higher confidence due to threading signals
        assert!(result.confidence > 0.6);
    }
}
