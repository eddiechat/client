use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
pub struct SyncStatus {
    pub phase: String,
    pub message: String,
}

pub fn emit_status(app: &tauri::AppHandle, phase: &str, message: &str) {
    let _ = app.emit("sync:status", SyncStatus {
        phase: phase.to_string(),
        message: message.to_string(),
    });
}

#[derive(Clone, serde::Serialize)]
pub struct ConversationsUpdated {
    pub account_id: String,
    pub count: usize,
}

pub fn emit_conversations_updated(app: &tauri::AppHandle, account_id: &str, count: usize) {
    let _ = app.emit("sync:conversations-updated", ConversationsUpdated {
        account_id: account_id.to_string(),
        count,
    });
}

#[derive(Clone, serde::Serialize)]
pub struct OnboardingComplete {
    pub account_id: String,
}

pub fn emit_onboarding_complete(app: &tauri::AppHandle, account_id: &str) {
    let _ = app.emit("onboarding:complete", OnboardingComplete {
        account_id: account_id.to_string(),
    });
}
