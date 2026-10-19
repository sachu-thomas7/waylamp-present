use crate::error::AppError;
use crate::preview::{self, PreviewHub, PreviewSnapshot};
use crate::state::ConfigPath;

#[tauri::command]
pub fn preview_get_snapshot(hub: tauri::State<'_, PreviewHub>) -> PreviewSnapshot {
    hub.snapshot()
}

#[tauri::command]
pub fn preview_get_layout(config_path: tauri::State<'_, ConfigPath>) -> Result<String, AppError> {
    preview::get_layout(&config_path.0)
}

#[tauri::command]
pub fn preview_set_layout(
    layout: String,
    config_path: tauri::State<'_, ConfigPath>,
) -> Result<(), AppError> {
    preview::set_layout(&config_path.0, &layout)
}
