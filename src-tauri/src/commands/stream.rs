use std::sync::Arc;

use crate::error::AppError;
use crate::stream::{Destination, StreamSettings, StreamStatus};
use crate::stream_service::{StreamConfigView, StreamController};

type Ctl<'a> = tauri::State<'a, Arc<StreamController>>;

/// Engine calls block (state changes, keychain), so they never run on the async threads.
async fn blocking<T: Send + 'static>(
    ctl: &Ctl<'_>,
    f: impl FnOnce(&StreamController) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let c = Arc::clone(ctl.inner());
    tokio::task::spawn_blocking(move || f(&c))
        .await
        .map_err(|e| AppError::Internal(format!("stream task failed: {e}")))?
}

#[tauri::command]
pub async fn stream_get_config(ctl: Ctl<'_>) -> Result<StreamConfigView, AppError> {
    blocking(&ctl, |c| c.config_view()).await
}

#[tauri::command]
pub async fn stream_get_status(ctl: Ctl<'_>) -> Result<StreamStatus, AppError> {
    blocking(&ctl, |c| c.status()).await
}

/// `key` is stored in the OS keychain; `None` keeps the existing key.
#[tauri::command]
pub async fn stream_save_destination(
    destination: Destination,
    key: Option<String>,
    ctl: Ctl<'_>,
) -> Result<StreamConfigView, AppError> {
    blocking(&ctl, move |c| c.save_destination(destination, key)).await
}

#[tauri::command]
pub async fn stream_remove_destination(
    id: String,
    ctl: Ctl<'_>,
) -> Result<StreamConfigView, AppError> {
    blocking(&ctl, move |c| c.remove_destination(&id)).await
}

#[tauri::command]
pub async fn stream_set_destination_enabled(
    id: String,
    enabled: bool,
    ctl: Ctl<'_>,
) -> Result<StreamConfigView, AppError> {
    blocking(&ctl, move |c| c.set_destination_enabled(&id, enabled)).await
}

#[tauri::command]
pub async fn stream_set_settings(
    settings: StreamSettings,
    ctl: Ctl<'_>,
) -> Result<StreamConfigView, AppError> {
    blocking(&ctl, move |c| c.set_settings(settings)).await
}

#[tauri::command]
pub async fn stream_start(ctl: Ctl<'_>) -> Result<StreamStatus, AppError> {
    blocking(&ctl, |c| c.start()).await
}

#[tauri::command]
pub async fn stream_stop(ctl: Ctl<'_>) -> Result<StreamStatus, AppError> {
    blocking(&ctl, |c| c.stop()).await
}
