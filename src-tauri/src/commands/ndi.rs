use std::sync::Arc;

use crate::bus::EventBus;
use crate::error::AppError;
use crate::ndi::{self, NdiDiscoveryHandle, NdiSnapshot, NdiStateHandle};
use crate::pipeline::PipelineHandle;

#[tauri::command]
pub async fn ndi_list_sources(
    provider: tauri::State<'_, NdiDiscoveryHandle>,
    state: tauri::State<'_, NdiStateHandle>,
) -> Result<NdiSnapshot, AppError> {
    Ok(ndi::snapshot(&provider, &state).await)
}

#[tauri::command]
pub async fn ndi_add_source(
    name: String,
    provider: tauri::State<'_, NdiDiscoveryHandle>,
    pipeline: tauri::State<'_, PipelineHandle>,
    bus: tauri::State<'_, Arc<EventBus>>,
    state: tauri::State<'_, NdiStateHandle>,
) -> Result<(), AppError> {
    ndi::add_source(&name, &provider, &pipeline, &bus, &state).await
}

#[tauri::command]
pub async fn ndi_remove_source(
    name: String,
    pipeline: tauri::State<'_, PipelineHandle>,
    bus: tauri::State<'_, Arc<EventBus>>,
    state: tauri::State<'_, NdiStateHandle>,
) -> Result<(), AppError> {
    ndi::remove_source(&name, &pipeline, &bus, &state).await
}
