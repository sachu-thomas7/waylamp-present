use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;

// Tracks whether the presentation window is currently visible.
// Prevents double-registration of the Escape shortcut.
static PRESENTING: AtomicBool = AtomicBool::new(false);

fn escape_shortcut() -> Shortcut {
    Shortcut::new(None::<Modifiers>, Code::Escape)
}

fn hide_presentation(app: &tauri::AppHandle, bus: &Arc<EventBus>) {
    if let Some(win) = app.get_webview_window("presentation") {
        let _ = win.set_fullscreen(false);
        let _ = win.hide();
    }
    PRESENTING.store(false, Ordering::SeqCst);
    bus.publish(AppEvent::PresentationVisibility { visible: false });
}

#[tauri::command]
pub async fn presentation_show(
    app: tauri::AppHandle,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<(), AppError> {
    // Guard: if already presenting, do nothing
    if PRESENTING.load(Ordering::SeqCst) {
        return Ok(());
    }

    let window = app
        .get_webview_window("presentation")
        .ok_or_else(|| AppError::NotFound("presentation window not found".into()))?;

    window.show().map_err(|e| AppError::Internal(e.to_string()))?;
    window.set_fullscreen(true).map_err(|e| AppError::Internal(e.to_string()))?;
    window.set_focus().map_err(|e| AppError::Internal(e.to_string()))?;

    PRESENTING.store(true, Ordering::SeqCst);
    bus.publish(AppEvent::PresentationVisibility { visible: true });

    // Register Escape — unregister is done via spawn OUTSIDE the callback to avoid deadlock
    let app_handle = app.clone();
    let bus_weak: Weak<EventBus> = Arc::downgrade(&bus);
    app.global_shortcut()
        .on_shortcut(escape_shortcut(), move |_shortcut_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let handle = app_handle.clone();
            let bus_weak = bus_weak.clone();
            // Spawn onto async runtime so the unregister happens outside this callback's lock
            tauri::async_runtime::spawn(async move {
                if let Some(bus_arc) = bus_weak.upgrade() {
                    hide_presentation(&handle, &bus_arc);
                }
                let _ = handle.global_shortcut().unregister(escape_shortcut());
            });
        })
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let sm = machine.read().await;
    bus.publish(AppEvent::SlideUpdate(sm.current_payload()));
    Ok(())
}

#[tauri::command]
pub async fn presentation_hide(
    app: tauri::AppHandle,
    bus: tauri::State<'_, Arc<EventBus>>,
) -> Result<(), AppError> {
    if !PRESENTING.load(Ordering::SeqCst) {
        return Ok(());
    }

    hide_presentation(&app, &bus);

    // Safe to unregister here — we are NOT inside the shortcut callback
    let _ = app.global_shortcut().unregister(escape_shortcut());

    Ok(())
}