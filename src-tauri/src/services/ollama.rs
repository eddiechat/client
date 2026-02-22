use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::services::logger;

use crate::adapters::sqlite::DbPool;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const PREFERRED_MODEL: &str = "ministral-3:3b";
pub const DEFAULT_KEY: &str = "__DEFAULT__";

#[derive(Clone, serde::Serialize)]
pub struct OllamaEntry {
    pub models: Vec<String>,
    pub selected_model: Option<String>,
}

pub type OllamaState = Arc<RwLock<HashMap<String, OllamaEntry>>>;

fn make_entry(models: Vec<String>, persisted_model: Option<&str>) -> OllamaEntry {
    let selected_model = if models.contains(&PREFERRED_MODEL.to_string()) {
        Some(PREFERRED_MODEL.to_string())
    } else if let Some(p) = persisted_model {
        if !p.is_empty() && models.contains(&p.to_string()) {
            Some(p.to_string())
        } else {
            None
        }
    } else {
        None
    };

    OllamaEntry {
        models,
        selected_model,
    }
}

/// Populate the OllamaState by querying all unique Ollama URLs.
pub async fn populate(pool: &DbPool, state: &OllamaState) {
    // 1. Read global settings
    let default_url = crate::adapters::sqlite::settings::get_setting(pool, "ollama_url")
        .ok()
        .flatten()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());

    let persisted_model = crate::adapters::sqlite::settings::get_setting(pool, "ollama_model")
        .ok()
        .flatten();

    // 2. Fetch models for the default URL
    let default_models = crate::adapters::ollama::fetch_models(&default_url).await;
    logger::debug(&format!("Found {} models at default URL {}", default_models.len(), default_url));

    let mut map = HashMap::new();
    map.insert(
        DEFAULT_KEY.to_string(),
        make_entry(default_models.clone(), persisted_model.as_deref()),
    );

    // 3. Read all skills to apply per-skill model selection
    let skills = get_all_skill_settings(pool);

    for (skill_id, skill_model) in &skills {
        map.insert(
            skill_id.clone(),
            make_entry(default_models.clone(), Some(skill_model.as_str())),
        );
    }

    // 4. Write to shared state
    let mut guard = state.write().await;
    *guard = map;
    logger::debug(&format!("Ollama state populated with {} entries", guard.len()));
}

/// Update a single skill's entry in the OllamaState after save.
/// Uses the default models list and sets the selected model from skill settings.
pub async fn update_skill_entry(state: &OllamaState, skill_id: &str, settings_json: &str) {
    let parsed: serde_json::Value = match serde_json::from_str(settings_json) {
        Ok(v) => v,
        Err(_) => return,
    };

    let skill_model = parsed.get("ollamaModel")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut guard = state.write().await;

    let models = guard.get(DEFAULT_KEY)
        .map(|e| e.models.clone())
        .unwrap_or_default();

    guard.insert(
        skill_id.to_string(),
        make_entry(models, Some(skill_model)),
    );
}

/// Query all skills and extract (id, ollama_model) from their settings JSON.
fn get_all_skill_settings(pool: &DbPool) -> Vec<(String, String)> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut stmt = match conn.prepare("SELECT id, settings FROM skills") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
        ))
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(|r| {
        let (id, settings_json) = r.ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&settings_json).ok()?;
        let model = parsed.get("ollamaModel")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Some((id, model))
    })
    .collect()
}
