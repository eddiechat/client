use crate::adapters::sqlite::{self, DbPool};
use crate::error::EddieError;
use tokio::sync::mpsc;

#[tauri::command]
pub async fn queue_action(
    pool: tauri::State<'_, DbPool>,
    wake_tx: tauri::State<'_, mpsc::Sender<()>>,
    account_id: String,
    action_type: String,
    payload: String,
) -> Result<String, EddieError> {
    let action_id = sqlite::action_queue::enqueue(&pool, &account_id, &action_type, &payload)?;
    let _ = wake_tx.send(()).await;
    Ok(action_id)
}
