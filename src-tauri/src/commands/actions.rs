use crate::adapters::sqlite::{self, DbPool};
use crate::error::EddieError;
use tokio::sync::mpsc;

#[tauri::command]
pub async fn queue_action(
    app: tauri::AppHandle,
    pool: tauri::State<'_, DbPool>,
    wake_tx: tauri::State<'_, mpsc::Sender<()>>,
    account_id: String,
    action_type: String,
    payload: String,
) -> Result<String, EddieError> {
    let write_mode = sqlite::settings::get_setting(&pool, "write_mode")?
        .map(|v| v == "true")
        .unwrap_or(false);
    if !write_mode {
        return Err(EddieError::InvalidInput("Read-only mode: action not permitted".into()));
    }

    let action_id = sqlite::action_queue::enqueue(&pool, &account_id, &action_type, &payload, None)?;

    // Optimistic DB update for mark_read: persist \\Seen flag locally
    if action_type == "mark_read" {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload) {
            if let Some(ids) = parsed["ids"].as_array() {
                let id_strings: Vec<String> = ids.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if !id_strings.is_empty() {
                    let _ = sqlite::messages::mark_messages_seen(&pool, &id_strings);
                }
            }
        }

        // Rebuild conversations so unread_count is updated immediately
        let conv_count = sqlite::conversations::rebuild_conversations(&pool, &account_id)?;
        crate::services::sync::helpers::status_emit::emit_conversations_updated(&app, &account_id, conv_count);
    }

    let _ = wake_tx.send(()).await;
    Ok(action_id)
}
