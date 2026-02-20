use crate::models::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Immutable snapshot of an Ollama connection.
/// To change the URL/key/timeout, construct a new OllamaBackend and swap it
/// into the RwLock - don't mutate an existing one.
pub struct OllamaBackend {
    client: Client,
    base_url: String,
}

// -- Ollama API wire types --

#[derive(Deserialize)]
struct TagsResponse {
    models: Option<Vec<TagModel>>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    #[allow(dead_code)]
    model: Option<String>,
    size: Option<u64>,
    details: Option<TagModelDetails>,
}

#[derive(Deserialize)]
struct TagModelDetails {
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

#[derive(Serialize)]
struct GenRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenOptions>,
}

#[derive(Serialize)]
struct GenOptions {
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Deserialize)]
struct GenResponse {
    response: Option<String>,
    model: Option<String>,
    #[allow(dead_code)]
    done: Option<bool>,
}

// -- Construction --

impl OllamaBackend {
    /// Build from individual parameters. Returns None if url is empty.
    pub fn new(url: &str, api_key: Option<&str>, timeout_secs: u64) -> Option<Self> {
        if url.is_empty() {
            return None;
        }

        let mut builder = Client::builder().timeout(Duration::from_secs(timeout_secs));

        if let Some(key) = api_key {
            let mut headers = reqwest::header::HeaderMap::new();
            if let Ok(val) = format!("Bearer {}", key).parse() {
                headers.insert(reqwest::header::AUTHORIZATION, val);
            }
            builder = builder.default_headers(headers);
        }

        let client = builder.build().ok()?;

        Some(Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
        })
    }

    /// Build from the static plugin config (tauri.conf.json).
    pub fn from_config(config: &crate::config::LlmConfig) -> Option<Self> {
        let url = config.ollama_url.as_deref()?;
        Self::new(
            url,
            config.ollama_api_key.as_deref(),
            config.ollama_timeout_secs,
        )
    }

    /// Build from a runtime OllamaSettings payload.
    pub fn from_settings(s: &OllamaSettings) -> Option<Self> {
        let url = s.url.as_deref()?;
        Self::new(url, s.api_key.as_deref(), s.timeout_secs.unwrap_or(120))
    }

    // -- Public API --

    /// Fast connectivity check. 3-second timeout, swallows all errors.
    pub async fn is_reachable(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// GET /api/tags -> Vec<ModelInfo>.
    /// Each model ID is prefixed with "ollama:" so `generate` can route to this backend.
    pub async fn list_models(&self) -> crate::Result<Vec<ModelInfo>> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(crate::Error::OllamaError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let tags: TagsResponse = resp
            .json()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("parse failed: {}", e)))?;

        Ok(tags
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|m| {
                let details = m.details.as_ref();
                ModelInfo {
                    id: format!("ollama:{}", m.name),
                    name: m.name.clone(),
                    available: true,
                    reason: None,
                    provider: "ollama".into(),
                    metadata: Some(serde_json::json!({
                        "family": details.and_then(|d| d.family.clone()),
                        "parameter_size": details.and_then(|d| d.parameter_size.clone()),
                        "quantization_level": details.and_then(|d| d.quantization_level.clone()),
                        "size_bytes": m.size,
                    })),
                }
            })
            .collect())
    }

    /// POST /api/generate with stream:false -> GenerateResponse.
    /// The model ID should still have the "ollama:" prefix; this method strips it.
    pub async fn generate(&self, req: &GenerateRequest) -> crate::Result<GenerateResponse> {
        let model_name = req.model.strip_prefix("ollama:").unwrap_or(&req.model);

        let body = GenRequest {
            model: model_name.to_string(),
            prompt: req.prompt.clone(),
            stream: false,
            options: Some(GenOptions {
                temperature: req.temperature,
                num_predict: req.max_tokens,
            }),
        };

        let resp = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::Error::OllamaError(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        let gen: GenResponse = resp
            .json()
            .await
            .map_err(|e| crate::Error::OllamaError(format!("parse failed: {}", e)))?;

        Ok(GenerateResponse {
            text: gen.response.unwrap_or_default(),
            model: format!(
                "ollama:{}",
                gen.model.unwrap_or_else(|| model_name.to_string())
            ),
            provider: "ollama".into(),
        })
    }
}
