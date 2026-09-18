use std::sync::Arc;
use tauri::Manager;

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::display::{self, DisplayInfo, DisplayStateHandle};
use crate::error::AppError;
use crate::events::AppEvent;

#[tauri::command]
pub async fn display_list(
    app: tauri::AppHandle,
    display_state: tauri::State<'_, DisplayStateHandle>,
) -> Result<Vec<DisplayInfo>, AppError> {
    let displays = display::list_displays(&app)?;
    display_state.write().await.displays = displays.clone();
    Ok(displays)
}

#[tauri::command]
pub async fn display_assign_presentation(
    id: String,
    app: tauri::AppHandle,
    bus: tauri::State<'_, Arc<EventBus>>,
    display_state: tauri::State<'_, DisplayStateHandle>,
) -> Result<(), AppError> {
    let displays = display::list_displays(&app)?;
    let target = displays
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| AppError::NotFound(format!("display not found: {id}")))?
        .clone();

    if let Some(window) = app.get_webview_window("presentation") {
        display::apply_position(&window, &target)?;
    }

    {
        let mut state = display_state.write().await;
        state.displays = displays;
        state.assigned_display_id = Some(id.clone());
    }

    let config_path = ConfigLoader::resolve_path(&app)?;
    let mut config = ConfigLoader::load_or_default(&config_path)?;
    config.presentation.display_id = Some(id.clone());
    ConfigLoader::save(&config_path, &config)?;

    bus.publish(AppEvent::DisplayAssigned { display_id: id });
    Ok(())
}
