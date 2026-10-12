use tauri::State;

use crate::config::StreamDeckConfig;

#[tauri::command]
pub fn get_config(config: State<StreamDeckConfig>) -> StreamDeckConfig {
    config.inner().clone()
}
