//! Ollama-based message classification using a local LLM
//!
//! Classifies messages by sending them to a local Ollama instance running
//! a small model (e.g. Mistral 3B). Falls back to rule-based classification
//! if Ollama is unavailable.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::{debug, warn};

use crate::sync::classifier::{Classification, ClassificationResult};
use crate::sync::db::CachedChatMessage;
use crate::types::error::EddieError;

/// Classification prompt template sent to the LLM.
const CLASSIFICATION_PROMPT: &str = r#"Classify this email into exactly one category.

Categories:
- chat: Personal human-to-human conversation
- newsletter: Marketing emails, newsletters, mailing lists
- automated: Automated notifications (GitHub, CI/CD, monitoring alerts)
- transactional: Receipts, shipping, password resets, account verification

Email:
From: {from}
Subject: {subject}
Body (first 500 chars): {body}

Respond with ONLY the category name (chat, newsletter, automated, or transactional). Nothing else."#;

/// Configuration for the Ollama classifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub url: String,
    pub model: String,
    pub enabled: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:11434".to_string(),
            model: "mistral:latest".to_string(),
            enabled: false,
        }
    }
}

/// Ollama API generate response (non-streaming)
#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

/// Ollama-based message classifier
pub struct OllamaClassifier {
    client: Client,
    config: OllamaConfig,
    config_hash: String,
}

impl OllamaClassifier {
    pub fn new(config: OllamaConfig) -> Self {
        let config_hash = Self::compute_hash(&config.model, CLASSIFICATION_PROMPT);
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            config,
            config_hash,
        }
    }

    /// Compute SHA-256 hash of model name + prompt template.
    /// Used as the `classified_by` value to detect when re-classification is needed.
    pub fn compute_hash(model: &str, prompt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(model.as_bytes());
        hasher.update(prompt.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Get the config hash (used for the `classified_by` database column)
    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    /// Test connection to the Ollama server
    pub async fn test_connection(&self) -> Result<bool, EddieError> {
        let url = format!("{}/api/tags", self.config.url.trim_end_matches('/'));
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(e) => Err(EddieError::Network(format!(
                "Ollama connection failed: {}",
                e
            ))),
        }
    }

    /// Classify a single message using Ollama
    pub async fn classify(
        &self,
        message: &CachedChatMessage,
    ) -> Result<ClassificationResult, EddieError> {
        let prompt = self.build_prompt(message);
        let response = self.call_ollama(&prompt).await?;
        Ok(Self::parse_response(&response))
    }

    fn build_prompt(&self, message: &CachedChatMessage) -> String {
        let body_preview = message
            .text_body
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(500)
            .collect::<String>();

        CLASSIFICATION_PROMPT
            .replace("{from}", &message.from_address)
            .replace(
                "{subject}",
                message.subject.as_deref().unwrap_or("(no subject)"),
            )
            .replace("{body}", &body_preview)
    }

    async fn call_ollama(&self, prompt: &str) -> Result<String, EddieError> {
        let url = format!(
            "{}/api/generate",
            self.config.url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.1,
                "num_predict": 20
            }
        });

        debug!("Calling Ollama at {} with model {}", url, self.config.model);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| EddieError::Network(format!("Ollama request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(EddieError::Network(format!(
                "Ollama returned status: {}",
                resp.status()
            )));
        }

        let parsed: OllamaGenerateResponse = resp
            .json()
            .await
            .map_err(|e| EddieError::Parse(format!("Failed to parse Ollama response: {}", e)))?;

        Ok(parsed.response.trim().to_lowercase())
    }

    fn parse_response(response: &str) -> ClassificationResult {
        let cleaned = response.trim().to_lowercase();

        // Extract the classification word — the model may include extra text
        let classification = if cleaned.contains("newsletter") {
            Classification::Newsletter
        } else if cleaned.contains("automated") {
            Classification::Automated
        } else if cleaned.contains("transactional") {
            Classification::Transactional
        } else if cleaned.contains("chat") {
            Classification::Chat
        } else {
            warn!("Ollama returned unrecognized classification: {}", cleaned);
            Classification::from_str(&cleaned)
        };

        let confidence = if classification == Classification::Unknown {
            0.3
        } else {
            0.85
        };

        ClassificationResult {
            classification,
            confidence,
            reasons: vec![format!("Ollama classification: {}", cleaned)],
        }
    }
}
