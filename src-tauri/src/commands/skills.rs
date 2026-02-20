use crate::adapters::sqlite;
use crate::adapters::sqlite::sync::skills::Skill;
use crate::error::EddieError;
use crate::services::ollama::{self, OllamaState, DEFAULT_KEY};

fn has_model(ollama: &std::collections::HashMap<String, crate::services::ollama::OllamaEntry>, skill_id: &str) -> bool {
    let entry = ollama.get(skill_id).or_else(|| ollama.get(DEFAULT_KEY));
    entry.map_or(false, |e| e.selected_model.is_some())
}

#[tauri::command]
pub async fn list_skills(
    pool: tauri::State<'_, sqlite::DbPool>,
    ollama: tauri::State<'_, OllamaState>,
    account_id: String,
) -> Result<Vec<Skill>, EddieError> {
    let mut skills = sqlite::skills::list_skills(&pool, &account_id)?;
    let guard = ollama.read().await;
    for skill in &mut skills {
        let model_ok = has_model(&guard, &skill.id);
        skill.has_model = model_ok;
        if !model_ok {
            skill.enabled = false;
        }
    }
    Ok(skills)
}

#[tauri::command]
pub async fn get_skill(
    pool: tauri::State<'_, sqlite::DbPool>,
    skill_id: String,
) -> Result<Skill, EddieError> {
    sqlite::skills::get_skill(&pool, &skill_id)
}

#[tauri::command]
pub async fn create_skill(
    pool: tauri::State<'_, sqlite::DbPool>,
    ollama: tauri::State<'_, OllamaState>,
    account_id: String,
    name: String,
    icon: String,
    icon_bg: String,
    prompt: String,
    modifiers: String,
    settings: String,
) -> Result<String, EddieError> {
    let id = sqlite::skills::create_skill(&pool, &account_id, &name, &icon, &icon_bg, &prompt, &modifiers, &settings)?;
    ollama::update_skill_entry(&ollama, &id, &settings).await;
    Ok(id)
}

#[tauri::command]
pub async fn update_skill(
    pool: tauri::State<'_, sqlite::DbPool>,
    ollama: tauri::State<'_, OllamaState>,
    id: String,
    name: String,
    icon: String,
    icon_bg: String,
    prompt: String,
    modifiers: String,
    settings: String,
) -> Result<(), EddieError> {
    sqlite::skills::update_skill(&pool, &id, &name, &icon, &icon_bg, &prompt, &modifiers, &settings)?;
    ollama::update_skill_entry(&ollama, &id, &settings).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_skill(
    pool: tauri::State<'_, sqlite::DbPool>,
    ollama: tauri::State<'_, OllamaState>,
    skill_id: String,
    enabled: bool,
) -> Result<(), EddieError> {
    if enabled {
        let guard = ollama.read().await;
        if !has_model(&guard, &skill_id) {
            return Err(EddieError::InvalidInput(
                "No model selected. Configure a model in skill settings first.".to_string(),
            ));
        }
    }
    sqlite::skills::toggle_skill(&pool, &skill_id, enabled)
}

#[tauri::command]
pub async fn delete_skill(
    pool: tauri::State<'_, sqlite::DbPool>,
    skill_id: String,
) -> Result<(), EddieError> {
    sqlite::skills::delete_skill(&pool, &skill_id)
}
