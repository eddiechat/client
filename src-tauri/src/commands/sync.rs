
use crate::adapters::sqlite;
use crate::error::EddieError;
use crate::services::logger;

#[tauri::command]
pub async fn sync_now(
    wake_tx: tauri::State<'_, tokio::sync::mpsc::Sender<()>>,
) -> Result<String, EddieError> {
    logger::info("Manual sync triggered");
    let _ = wake_tx.send(()).await;
    Ok("Sync triggered".to_string())
}

#[derive(serde::Serialize)]
pub struct TaskStatus {
    pub name: String,
    pub status: String,
}

#[derive(serde::Serialize)]
pub struct TrustContact {
    pub name: String,
    pub email: String,
    pub message_count: i32,
}

#[derive(serde::Serialize)]
pub struct OnboardingStatus {
    pub tasks: Vec<TaskStatus>,
    pub message_count: usize,
    pub trust_contacts: Vec<TrustContact>,
    pub trust_contact_count: usize,
    pub is_complete: bool,
}

#[tauri::command]
pub async fn get_onboarding_status(
    pool: tauri::State<'_, sqlite::DbPool>,
    account_id: String,
) -> Result<OnboardingStatus, EddieError> {
    let tasks = sqlite::onboarding_tasks::get_tasks(&pool, &account_id)?;
    let message_count = sqlite::messages::count_messages(&pool, &account_id)?;
    let trust_contacts_raw = sqlite::conversations::get_trust_contacts(&pool, &account_id)?;
    let trust_contact_count = sqlite::conversations::count_trust_contacts(&pool, &account_id)?;

    let all_done = !tasks.is_empty() && tasks.iter().all(|t| t.status == "done");
    let is_complete = all_done || message_count >= 600;

    let task_statuses: Vec<TaskStatus> = tasks
        .iter()
        .map(|t| TaskStatus {
            name: t.name.clone(),
            status: t.status.clone(),
        })
        .collect();

    let trust_contacts: Vec<TrustContact> = trust_contacts_raw
        .into_iter()
        .map(|c| TrustContact {
            name: c.name,
            email: c.email,
            message_count: c.message_count,
        })
        .collect();

    Ok(OnboardingStatus {
        tasks: task_statuses,
        message_count,
        trust_contacts,
        trust_contact_count,
        is_complete,
    })
}