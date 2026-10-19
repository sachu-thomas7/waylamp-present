use crate::error::AppError;
use crate::pipeline::{PipelineHandle, PipelineState, TEST_PIPELINE};

#[tauri::command]
pub async fn pipeline_start_test(
    pipeline: tauri::State<'_, PipelineHandle>,
) -> Result<(), AppError> {
    pipeline.start(TEST_PIPELINE).await
}

#[tauri::command]
pub async fn pipeline_pause(pipeline: tauri::State<'_, PipelineHandle>) -> Result<(), AppError> {
    pipeline.pause().await
}

#[tauri::command]
pub async fn pipeline_resume(pipeline: tauri::State<'_, PipelineHandle>) -> Result<(), AppError> {
    pipeline.resume().await
}

#[tauri::command]
pub async fn pipeline_stop(pipeline: tauri::State<'_, PipelineHandle>) -> Result<(), AppError> {
    pipeline.stop().await
}

#[tauri::command]
pub async fn pipeline_get_state(
    pipeline: tauri::State<'_, PipelineHandle>,
) -> Result<PipelineState, AppError> {
    pipeline.state().await
}
