//! Message Classification
//!
//! Classifies messages as chat-worthy or non-chat (newsletters, automated, transactional).
//! Non-chat messages can be hidden from the chat UI but remain in cache.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::sync::db::{CachedMessage, MessageClassification, SyncDatabase};
use crate::types::error::HimalayaError;

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

/// Message classifier
pub struct MessageClassifier {
    db: Arc<SyncDatabase>,
    newsletter_domains: HashSet<String>,
    automated_senders: HashSet<String>,
    transactional_patterns: Vec<String>,
}

impl MessageClassifier {
    /// Create a new message classifier
    pub fn new(db: Arc<SyncDatabase>) -> Self {
        Self {
            db,
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
        ].iter().map(|s| s.to_string()).collect()
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
        ].iter().map(|s| s.to_string()).collect()
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
        ].iter().map(|s| s.to_string()).collect()
    }

    /// Classify a message
    pub fn classify(&self, message: &CachedMessage) -> ClassificationResult {
        let mut reasons: Vec<String> = Vec::new();
        let mut scores: Vec<(Classification, f32)> = Vec::new();

        let from_lower = message.from_address.to_lowercase();
        let subject_lower = message.subject.as_ref()
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Check for noreply/no-reply sender
        if from_lower.contains("noreply") || from_lower.contains("no-reply") ||
           from_lower.contains("donotreply") || from_lower.contains("do-not-reply") {
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
            if text_lower.contains("unsubscribe") || text_lower.contains("opt-out") ||
               text_lower.contains("email preferences") {
                reasons.push("Contains unsubscribe link".to_string());
                scores.push((Classification::Newsletter, 0.6));
            }
        }

        // Check for transactional patterns in subject
        for pattern in &self.transactional_patterns {
            if subject_lower.contains(pattern) {
                reasons.push(format!("Subject contains transactional pattern: {}", pattern));
                scores.push((Classification::Transactional, 0.7));
                break;
            }
        }

        // Check for common newsletter subject patterns
        if subject_lower.contains("newsletter") || subject_lower.contains("digest") ||
           subject_lower.contains("weekly update") || subject_lower.contains("daily update") ||
           subject_lower.starts_with("[") && subject_lower.contains("]") {
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
        if subject_lower.contains("invitation:") || subject_lower.contains("calendar") ||
           subject_lower.contains("accepted:") || subject_lower.contains("declined:") ||
           subject_lower.contains("reminder:") {
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

    /// Classify and store result in database
    pub fn classify_and_store(&self, message: &CachedMessage) -> Result<ClassificationResult, HimalayaError> {
        let result = self.classify(message);

        let classification = MessageClassification {
            message_id: message.id,
            classification: result.classification.as_str().to_string(),
            confidence: result.confidence,
            is_hidden_from_chat: result.classification.is_hidden_by_default(),
            classified_at: Utc::now(),
        };

        self.db.set_message_classification(&classification)?;

        Ok(result)
    }

    /// Batch classify messages
    pub fn classify_batch(&self, messages: &[CachedMessage]) -> Result<Vec<ClassificationResult>, HimalayaError> {
        let mut results = Vec::with_capacity(messages.len());

        for message in messages {
            let result = self.classify_and_store(message)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get classification for a message (from cache or compute)
    pub fn get_or_classify(&self, message: &CachedMessage) -> Result<ClassificationResult, HimalayaError> {
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
    pub fn is_chat_message(&self, message: &CachedMessage) -> Result<bool, HimalayaError> {
        let result = self.get_or_classify(message)?;
        Ok(result.classification == Classification::Chat ||
           result.classification == Classification::Unknown)
    }

    /// Update classification settings (mark a message as chat/non-chat)
    pub fn set_hidden(&self, message_id: i64, hidden: bool) -> Result<(), HimalayaError> {
        if let Some(mut classification) = self.db.get_message_classification(message_id)? {
            classification.is_hidden_from_chat = hidden;
            self.db.set_message_classification(&classification)?;
        }
        Ok(())
    }

    /// Add a domain to the newsletter list
    pub fn add_newsletter_domain(&mut self, domain: &str) {
        self.newsletter_domains.insert(domain.to_lowercase());
    }

    /// Add a sender to the automated list
    pub fn add_automated_sender(&mut self, email: &str) {
        self.automated_senders.insert(email.to_lowercase());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_message(from: &str, subject: &str, body: Option<&str>) -> CachedMessage {
        CachedMessage {
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
}
