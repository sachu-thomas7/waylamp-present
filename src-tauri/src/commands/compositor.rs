use crate::compositor::{self, CompositorStatus, SourceGeometry};
use crate::config::StreamDeckConfig;
use crate::error::AppError;
use crate::pipeline::PipelineHandle;

#[tauri::command]
pub fn compositor_list_scenes(config: tauri::State<'_, StreamDeckConfig>) -> Vec<String> {
    compositor::list_scenes(&config)
}

/// Starts the compositor on scene `scene_id` from the config, or the built-in demo layout.
#[tauri::command]
pub async fn compositor_start(
    scene_id: Option<String>,
    config: tauri::State<'_, StreamDeckConfig>,
    pipeline: tauri::State<'_, PipelineHandle>,
) -> Result<(), AppError> {
    let layout = compositor::resolve_layout(&config, scene_id.as_deref())?;
    pipeline.compositor_start(layout).await
}

#[tauri::command]
pub async fn compositor_stop(pipeline: tauri::State<'_, PipelineHandle>) -> Result<(), AppError> {
    pipeline.compositor_stop().await
}

#[tauri::command]
pub async fn compositor_set_layout(
    sources: Vec<SourceGeometry>,
    pipeline: tauri::State<'_, PipelineHandle>,
) -> Result<(), AppError> {
    pipeline.compositor_set_layout(sources).await
}

#[tauri::command]
pub async fn compositor_get_state(
    pipeline: tauri::State<'_, PipelineHandle>,
) -> Result<CompositorStatus, AppError> {
    pipeline.compositor_status().await
}
