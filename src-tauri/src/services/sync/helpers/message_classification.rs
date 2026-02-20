use crate::adapters::sqlite::{self, DbPool};

use crate::error::EddieError;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::debug;

/// Classify all unprocessed messages for an account.
///
/// Phase 1: no trust data (sender_trust = None).
/// Phase 2+ will pass trust lookups for improved accuracy.
pub fn classify_messages(pool: &DbPool, account_id: &str) -> Result<usize, EddieError> {
    let messages = sqlite::messages::get_unprocessed_messages(pool, account_id)?;
    if messages.is_empty() {
        return Ok(0);
    }
    debug!(account_id = %account_id, pending = messages.len(), "Classifying messages");
    let classifier = MessageClassifier::new();
    let mut count = 0;

    for msg in &messages {
        let references = serde_json::from_str::<Vec<String>>(&msg.references_ids)
            .ok()
            .filter(|refs| !refs.is_empty())
            .map(|refs| refs.join(" "));

        let input = ClassificationInput {
            from_address: msg.from_address.clone(),
            subject: msg.subject.clone(),
            in_reply_to: msg.in_reply_to.clone(),
            references,
            sender_trust: None,
            body_text: msg.body_text.clone(),
        };

        let result = classifier.classify(&input);
        sqlite::messages::update_classification(
            pool,
            &msg.id,
            result.classification.as_str(),
            false,
        )?;
        count += 1;
    }

    Ok(count)
}

// -------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Message classification categories.
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
}

/// Trust level of the sender, derived from the trust network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TrustLevel {
    /// The user themselves.
    User,
    /// A known alias of the user.
    Alias,
    /// Someone the user has emailed (from sent folder scan).
    Connection,
    /// An imported contact (e.g., from CardDAV).
    Contact,
}


/// Input for classification — a pure data struct with no database dependencies.
pub struct ClassificationInput {
    pub from_address: String,
    pub subject: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub sender_trust: Option<TrustLevel>,
    pub body_text: Option<String>,
}

/// Classification result.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub classification: Classification,
    #[allow(dead_code)]
    pub confidence: f32,
    #[allow(dead_code)]
    pub reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Email headers helper
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
}

// ---------------------------------------------------------------------------
// Signal aggregation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// MessageClassifier
// ---------------------------------------------------------------------------

/// Multi-signal message classifier.
///
/// Analyses messages across multiple tiers (RFC headers, sender, trust network,
/// subject, conversation threading) and aggregates weighted signals to produce
/// a classification with confidence score.
pub struct MessageClassifier {
    automated_senders: HashSet<String>,

    // ESP domains
    marketing_esp_domains: HashSet<String>,
    transactional_esp_domains: HashSet<String>,
    mixed_esp_domains: HashSet<String>,

    // Patterns
    noreply_local_parts: Vec<String>,
    transactional_subject_kw: Vec<String>,
    automated_subject_kw: Vec<String>,
    newsletter_subject_kw: Vec<String>,
}

impl MessageClassifier {
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

    /// Classify using only the fields available in [`ClassificationInput`].
    pub fn classify(&self, input: &ClassificationInput) -> ClassificationResult {
        self.classify_with_headers(input, None)
    }

    /// Classify with optional raw email headers for improved accuracy.
    pub fn classify_with_headers(
        &self,
        input: &ClassificationInput,
        headers: Option<&EmailHeaders>,
    ) -> ClassificationResult {
        let mut signals: Vec<Signal> = Vec::new();

        let from_lower = input.from_address.to_lowercase();
        let subject_lower = input
            .subject
            .as_ref()
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Tier 1: RFC-standard headers (highest confidence)
        if let Some(hdrs) = headers {
            self.analyze_rfc_headers(hdrs, &mut signals);
        }

        // Tier 2: Sender analysis
        self.analyze_sender(&from_lower, &mut signals);

        // Tier 3: Trust network
        Self::analyze_trust(input.sender_trust, &mut signals);

        // Tier 4: Subject analysis
        if !subject_lower.is_empty() {
            self.analyze_subject(&subject_lower, &mut signals);
        }

        // Tier 5: Content analysis
        if let Some(ref text) = input.body_text {
            self.analyze_content(text, &mut signals);
        }

        // Tier 6: Conversation threading
        self.analyze_threading(
            input.in_reply_to.as_deref(),
            input.references.as_deref(),
            &mut signals,
        );

        Self::aggregate(signals)
    }

    // -- Tier 1: RFC headers -------------------------------------------------

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

    // -- Tier 2: Sender analysis ---------------------------------------------

    fn analyze_sender(&self, from_lower: &str, out: &mut Vec<Signal>) {
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

    // -- Tier 3: Trust network -----------------------------------------------

    fn analyze_trust(sender_trust: Option<TrustLevel>, out: &mut Vec<Signal>) {
        match sender_trust {
            Some(TrustLevel::Connection) => {
                out.push(Signal::new(
                    Classification::Chat,
                    1.5,
                    "Sender is a known connection (sent folder scan)",
                ));
            }
            Some(TrustLevel::Contact) => {
                out.push(Signal::new(
                    Classification::Chat,
                    1.2,
                    "Sender is a known contact",
                ));
            }
            // User/Alias = self-sent; don't bias classification.
            Some(TrustLevel::User | TrustLevel::Alias) | None => {}
        }
    }

    // -- Tier 4: Subject analysis --------------------------------------------

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
        if let Some(bracket_pos) = subject_lower.find(']') {
            if subject_lower.starts_with("[") && bracket_pos < 60 && bracket_pos > 1 {
                let after_bracket = &subject_lower[bracket_pos + 1..];
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
    }

    // -- Tier 5: Content analysis

    fn analyze_content(&self, text: &str, out: &mut Vec<Signal>) {
        let text_lower = text.to_lowercase();

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

        if text_lower.contains("view in browser")
            || text_lower.contains("view this email in")
            || text_lower.contains("view as a web")
        {
            out.push(Signal::new(
                Classification::Newsletter,
                0.5,
                "Body contains \"view in browser\" text",
            ));
        }

        let transactional_phrases = [
            "order number", "order #", "tracking number",
            "track your", "your receipt", "amount charged",
            "has been processed", "shipping details",
        ];
        for phrase in &transactional_phrases {
            if text_lower.contains(phrase) {
                out.push(Signal::new(
                    Classification::Transactional,
                    0.5,
                    format!("Body contains transactional phrase: \"{}\"", phrase),
                ));
                break;
            }
        }
    }

    // -- Tier 6: Conversation threading --------------------------------------

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

    // -- Signal aggregation --------------------------------------------------

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

    // -- Data tables ---------------------------------------------------------

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
