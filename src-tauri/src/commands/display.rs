use std::sync::Arc;
use tauri::Manager;

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::display::{self, DisplayInfo, DisplayProviderHandle, DisplayStateHandle};
use crate::error::AppError;
use crate::events::AppEvent;
use crate::state::ConfigPath;

#[tauri::command]
pub async fn display_list(
    provider: tauri::State<'_, DisplayProviderHandle>,
    display_state: tauri::State<'_, DisplayStateHandle>,
) -> Result<Vec<DisplayInfo>, AppError> {
    let displays = provider.list()?;
    display_state.write().await.displays = displays.clone();
    Ok(displays)
}

#[tauri::command]
pub async fn display_assign_presentation<R: tauri::Runtime>(
    id: String,
    app: tauri::AppHandle<R>,
    bus: tauri::State<'_, Arc<EventBus>>,
    display_state: tauri::State<'_, DisplayStateHandle>,
    provider: tauri::State<'_, DisplayProviderHandle>,
    config_path: tauri::State<'_, ConfigPath>,
) -> Result<(), AppError> {
    let displays = provider.list()?;
    let target = displays
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| AppError::NotFound(format!("display not found: {id}")))?
        .clone();

    let mut config = ConfigLoader::load_or_default(&config_path.0)?;
    config.presentation.display_id = Some(id.clone());
    ConfigLoader::save(&config_path.0, &config)?;

    if let Some(window) = app.get_webview_window("presentation") {
        display::apply_position(&window, &target)?;
    }

    {
        let mut state = display_state.write().await;
        state.displays = displays;
        state.assigned_display_id = Some(id.clone());
    }

    bus.publish(AppEvent::DisplayAssigned { display_id: id });
    Ok(())
}
