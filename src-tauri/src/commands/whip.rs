use std::sync::Arc;

use crate::error::AppError;
use crate::whip::{QrMatrix, WhipController, WhipInfo};

#[tauri::command]
pub async fn whip_get_info(
    controller: tauri::State<'_, Arc<WhipController>>,
) -> Result<WhipInfo, AppError> {
    controller.info().await
}

#[tauri::command]
pub async fn whip_set_enabled(
    enabled: bool,
    controller: tauri::State<'_, Arc<WhipController>>,
) -> Result<(), AppError> {
    controller.set_enabled(enabled).await
}

#[tauri::command]
pub async fn whip_set_token(
    token: Option<String>,
    controller: tauri::State<'_, Arc<WhipController>>,
) -> Result<(), AppError> {
    controller.set_token(token)
}

#[tauri::command]
pub async fn whip_get_qr(
    slot: String,
    controller: tauri::State<'_, Arc<WhipController>>,
) -> Result<QrMatrix, AppError> {
    controller.qr(&slot).await
}
