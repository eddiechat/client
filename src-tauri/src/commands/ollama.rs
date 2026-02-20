use crate::error::EddieError;

#[tauri::command]
pub async fn ollama_complete(
    url: String,
    model: String,
    system_prompt: String,
    user_prompt: String,
    temperature: f64,
) -> Result<String, EddieError> {
    crate::adapters::ollama::chat_complete(&url, &model, &system_prompt, &user_prompt, temperature)
        .await
}
