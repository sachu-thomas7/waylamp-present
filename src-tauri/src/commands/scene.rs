use std::sync::Arc;
use tauri::Manager;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::scene::{SceneMediaPayload, SceneMediaType, SceneSource, SceneStateHandle};

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_VIDEO_BYTES: u64 = 500 * 1024 * 1024;

#[tauri::command]
pub async fn scene_push_image(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    scene: tauri::State<'_, SceneStateHandle>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    let src = std::path::Path::new(&path);

    let ext = src.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return Err(AppError::InvalidInput("image must be PNG, JPEG, or WebP".into()));
    }

    let size = std::fs::metadata(src)
        .map_err(|e| AppError::Internal(e.to_string()))?.len();
    if size > MAX_IMAGE_BYTES {
        return Err(AppError::InvalidInput("image exceeds 10 MB limit".into()));
    }

    let data_dir = app.path().app_data_dir()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let scene_dir = data_dir.join("scene_media");
    std::fs::create_dir_all(&scene_dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let filename = src.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let dest = scene_dir.join(filename);
    std::fs::copy(src, &dest)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let payload = SceneMediaPayload {
        media_type: SceneMediaType::Image,
        path: dest.to_str().unwrap_or_default().to_string(),
        display_name: filename.to_string(),
    };

    {
        let mut s = scene.write().await;
        s.current_media = Some(payload.clone());
    }

    bus.publish(AppEvent::SceneMediaUpdate(payload));
    Ok(())
}

#[tauri::command]
pub async fn scene_push_video(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    scene: tauri::State<'_, SceneStateHandle>,
) -> Result<(), AppError> {
    let src = std::path::Path::new(&path);

    let ext = src.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mp4" | "webm") {
        return Err(AppError::InvalidInput("video must be MP4 or WebM".into()));
    }

    let size = std::fs::metadata(src)
        .map_err(|e| AppError::Internal(e.to_string()))?.len();
    if size > MAX_VIDEO_BYTES {
        return Err(AppError::InvalidInput("video exceeds 500 MB limit".into()));
    }

    let filename = src.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video");

    let payload = SceneMediaPayload {
        media_type: SceneMediaType::Video,
        path: path.clone(),
        display_name: filename.to_string(),
    };

    {
        let mut s = scene.write().await;
        s.current_media = Some(payload.clone());
    }

    bus.publish(AppEvent::SceneMediaUpdate(payload));
    Ok(())
}

#[tauri::command]
pub async fn scene_set_source(
    source: SceneSource,
    bus: tauri::State<'_, Arc<EventBus>>,
    scene: tauri::State<'_, SceneStateHandle>,
) -> Result<(), AppError> {
    {
        let mut s = scene.write().await;
        s.active_source = source.clone();
    }
    bus.publish(AppEvent::SceneSourceChanged { source });
    Ok(())
}

#[tauri::command]
pub async fn scene_get_active_source(
    scene: tauri::State<'_, SceneStateHandle>,
) -> Result<SceneSource, AppError> {
    let s = scene.read().await;
    Ok(s.active_source.clone())
}