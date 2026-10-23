use tauri::State;

use crate::config::StreamDeckConfig;

#[tauri::command]
pub fn get_config(config: State<StreamDeckConfig>) -> StreamDeckConfig {
    config.inner().clone()
}

#[tauri::command]
pub fn get_startup_notices(notices: State<crate::state::StartupNotices>) -> Vec<String> {
    notices.0.lock().map(|n| n.clone()).unwrap_or_default()
}
