/// Get the application version from git tags (embedded at build time)
#[tauri::command]
pub fn get_app_version() -> String {
    env!("GIT_VERSION").to_string()
}
