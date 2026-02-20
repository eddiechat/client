use crate::error::EddieError;
use tracing::{debug, warn};

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(serde::Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(serde::Deserialize)]
struct ModelItem {
    id: String,
}

/// Send a chat completion request to an Ollama-compatible `/v1/chat/completions` endpoint.
pub async fn chat_complete(
    url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f64,
) -> Result<String, EddieError> {
    let endpoint = format!("{}/v1/chat/completions", url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": temperature,
        "stream": false
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| EddieError::Backend(format!("HTTP client error: {}", e)))?;

    let resp = client
        .post(&endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| EddieError::Backend(format!("Ollama request failed: {}", e)))?;

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| EddieError::Backend(format!("Failed to parse Ollama response: {}", e)))?;

    let text = parsed
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(text)
}

/// Fetch available model IDs from an Ollama-compatible `/v1/models` endpoint.
/// Returns empty vec on any error (connection refused, timeout, parse error).
pub async fn fetch_models(url: &str) -> Vec<String> {
    let endpoint = format!("{}/v1/models", url.trim_end_matches('/'));
    debug!("Fetching Ollama models from {}", endpoint);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to build HTTP client: {}", e);
            return Vec::new();
        }
    };

    match client.get(&endpoint).send().await {
        Ok(resp) => match resp.json::<ModelsResponse>().await {
            Ok(body) => body.data.into_iter().map(|m| m.id).collect(),
            Err(e) => {
                warn!("Failed to parse Ollama models response: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            warn!("Failed to reach Ollama at {}: {}", endpoint, e);
            Vec::new()
        }
    }
}
