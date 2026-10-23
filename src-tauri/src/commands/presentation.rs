use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// Tracks whether the presentation window is currently visible.
// Prevents double-registration of the Escape shortcut.
static PRESENTING: AtomicBool = AtomicBool::new(false);

pub fn is_presenting() -> bool {
    PRESENTING.load(Ordering::SeqCst)
}

pub(crate) fn escape_shortcut() -> Shortcut {
    Shortcut::new(None::<Modifiers>, Code::Escape)
}

pub(crate) fn hide_presentation<R: tauri::Runtime>(app: &tauri::AppHandle<R>, bus: &Arc<EventBus>) {
    if let Some(win) = app.get_webview_window("presentation") {
        let _ = win.set_fullscreen(false);
        let _ = win.hide();
    }
    PRESENTING.store(false, Ordering::SeqCst);
    bus.publish(AppEvent::PresentationVisibility { visible: false });
}

#[tauri::command]
pub async fn presentation_show<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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

    // Always exclusive fullscreen, single display or not — matches PowerPoint's slideshow
    // behavior. The window is already positioned on the assigned/fallback display by
    // display_assign_presentation or the startup/hotplug logic in app.rs/display.rs, so no
    // positioning is needed here. Safety net for getting out of fullscreen is now: the
    // Escape shortcut below (registered unconditionally), and closing the window via
    // Alt+F4/OS close, which app.rs's CloseRequested handler routes through
    // hide_presentation instead of destroying the window.
    //
    // Apply geometry after show(), not before: WebView2 does not reliably recompute its
    // viewport (100vw/100vh) for a resize applied while the window is still hidden, which
    // left the presentation content laid out against a stale, tiny size that no later
    // ResizeObserver firing ever corrected (confirmed via manual launch test).
    window
        .show()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    window
        .set_decorations(false)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    window
        .set_fullscreen(true)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Only pin always-on-top when there's a genuine second display: Windows doesn't expose
    // a mirrored output (e.g. a ClickShare dongle in mirror mode) as a second display, so a
    // single-display report also covers that case. Always-on-top there would keep the
    // presentation painted over everything even after Alt+Tab, since alt-tab only moves
    // keyboard focus, not stacking order — blocking the operator from ever bringing the
    // controller back to the front on a one-screen (or mirrored) rig.
    let multi_display = crate::display::list_displays(&app)
        .map(|d| d.len() >= 2)
        .unwrap_or(false);
    window
        .set_always_on_top(multi_display)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    window
        .set_focus()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Publish before the shortcut registration below, so a failure there can't skip this.
    let sm = machine.read().await;
    bus.publish(AppEvent::SlideUpdate(sm.current_payload()));
    drop(sm);

    PRESENTING.store(true, Ordering::SeqCst);
    bus.publish(AppEvent::PresentationVisibility { visible: true });

    // Needed to escape fullscreen; clear stale registrations first, never fail the command on it.
    let _ = app.global_shortcut().unregister(escape_shortcut());

    let app_handle = app.clone();
    let bus_weak: Weak<EventBus> = Arc::downgrade(&bus);
    if let Err(e) = app.global_shortcut().on_shortcut(
        escape_shortcut(),
        move |_shortcut_app, _shortcut, event| {
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
        },
    ) {
        eprintln!("Escape shortcut registration failed (non-fatal): {e}");
    }

    Ok(())
}

#[tauri::command]
pub async fn presentation_hide<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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
