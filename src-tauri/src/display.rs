use std::sync::Arc;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime};
use tokio::sync::RwLock;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
}

pub struct DisplayState {
    pub displays: Vec<DisplayInfo>,
    pub assigned_display_id: Option<String>,
}

pub type DisplayStateHandle = Arc<RwLock<DisplayState>>;

pub fn new_display_handle(assigned_display_id: Option<String>) -> DisplayStateHandle {
    Arc::new(RwLock::new(DisplayState {
        displays: Vec::new(),
        assigned_display_id,
    }))
}

// Tauri's Monitor exposes no stable hardware id — name + position is the closest stable
// proxy available and holds across sleep/wake, but changes if the OS renumbers displays.
fn monitor_id(name: Option<&String>, x: i32, y: i32) -> String {
    format!(
        "{}@{},{}",
        name.map(String::as_str).unwrap_or("unknown"),
        x,
        y
    )
}

/// Enumerate connected displays via the Tauri window manager. Thin wrapper around
/// platform monitor APIs — not unit tested directly; the pure logic below (diffing,
/// fallback selection, disconnect detection) is what's tested.
pub fn list_displays<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<DisplayInfo>, AppError> {
    let monitors = app
        .available_monitors()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let primary_id = app
        .primary_monitor()
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(|m| monitor_id(m.name(), m.position().x, m.position().y));

    Ok(monitors
        .iter()
        .map(|m| {
            let id = monitor_id(m.name(), m.position().x, m.position().y);
            let is_primary = primary_id.as_deref() == Some(id.as_str());
            DisplayInfo {
                id,
                name: m
                    .name()
                    .cloned()
                    .unwrap_or_else(|| "Unknown Display".into()),
                width: m.size().width,
                height: m.size().height,
                x: m.position().x,
                y: m.position().y,
                is_primary,
            }
        })
        .collect())
}

/// Move (but do not show/fullscreen) the presentation window onto the given display.
pub fn apply_position<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    target: &DisplayInfo,
) -> Result<(), AppError> {
    window
        .set_position(PhysicalPosition::new(target.x, target.y))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    window
        .set_size(PhysicalSize::new(target.width, target.height))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}

/// Poll for display hotplug changes every 2s. Publishes DisplaysChanged when the id set
/// changes, and DisplayLost + repositions the presentation window to the fallback display
/// when the currently assigned display disappears.
pub fn spawn_watch_task<R: Runtime>(
    app: AppHandle<R>,
    bus: Arc<crate::bus::EventBus>,
    display_state: DisplayStateHandle,
) -> tauri::async_runtime::JoinHandle<()> {
    // Called from .setup(), outside any Tokio runtime — tokio::spawn would panic here.
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            interval.tick().await;
            let Ok(current) = list_displays(&app) else {
                continue;
            };

            let (previous, assigned) = {
                let state = display_state.read().await;
                (state.displays.clone(), state.assigned_display_id.clone())
            };

            if displays_changed(&previous, &current) {
                display_state.write().await.displays = current.clone();
                bus.publish(crate::events::AppEvent::DisplaysChanged(current.clone()));
            }

            if is_assigned_display_lost(&assigned, &current) {
                display_state.write().await.assigned_display_id = None;
                bus.publish(crate::events::AppEvent::DisplayLost);
                if let (Some(window), Some(fallback)) = (
                    app.get_webview_window("presentation"),
                    pick_fallback(&current),
                ) {
                    let _ = apply_position(&window, fallback);
                }
            }
        }
    })
}

/// True when the display list changed in a way the controller needs to know about
/// (a different set of ids — order/metadata churn without an id change is ignored).
pub fn displays_changed(previous: &[DisplayInfo], current: &[DisplayInfo]) -> bool {
    let mut prev_ids: Vec<&str> = previous.iter().map(|d| d.id.as_str()).collect();
    let mut curr_ids: Vec<&str> = current.iter().map(|d| d.id.as_str()).collect();
    prev_ids.sort_unstable();
    curr_ids.sort_unstable();
    prev_ids != curr_ids
}

/// True when the currently assigned display id is no longer present in the current list.
pub fn is_assigned_display_lost(assigned: &Option<String>, current: &[DisplayInfo]) -> bool {
    match assigned {
        None => false,
        Some(id) => !current.iter().any(|d| &d.id == id),
    }
}

/// Fallback target when the assigned display disappears: primary display, else the first
/// available, else None (no displays at all).
pub fn pick_fallback(current: &[DisplayInfo]) -> Option<&DisplayInfo> {
    current
        .iter()
        .find(|d| d.is_primary)
        .or_else(|| current.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: &str, is_primary: bool) -> DisplayInfo {
        DisplayInfo {
            id: id.into(),
            name: id.into(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            is_primary,
        }
    }

    #[test]
    fn monitor_id_uses_name_and_position() {
        assert_eq!(
            monitor_id(Some(&"DELL".to_string()), 1920, 0),
            "DELL@1920,0"
        );
    }

    #[test]
    fn monitor_id_falls_back_when_unnamed() {
        assert_eq!(monitor_id(None, 0, 0), "unknown@0,0");
    }

    #[test]
    fn displays_changed_false_for_identical_sets_in_different_order() {
        let a = vec![display("a", true), display("b", false)];
        let b = vec![display("b", false), display("a", true)];
        assert!(!displays_changed(&a, &b));
    }

    #[test]
    fn displays_changed_true_when_a_display_disappears() {
        let a = vec![display("a", true), display("b", false)];
        let b = vec![display("a", true)];
        assert!(displays_changed(&a, &b));
    }

    #[test]
    fn assigned_display_lost_true_when_missing() {
        let current = vec![display("a", true)];
        assert!(is_assigned_display_lost(&Some("b".into()), &current));
    }

    #[test]
    fn assigned_display_lost_false_when_present() {
        let current = vec![display("a", true)];
        assert!(!is_assigned_display_lost(&Some("a".into()), &current));
    }

    #[test]
    fn assigned_display_lost_false_when_nothing_assigned() {
        let current = vec![display("a", true)];
        assert!(!is_assigned_display_lost(&None, &current));
    }

    #[test]
    fn pick_fallback_prefers_primary() {
        let current = vec![display("a", false), display("b", true)];
        assert_eq!(pick_fallback(&current).unwrap().id, "b");
    }

    #[test]
    fn pick_fallback_uses_first_when_no_primary_flagged() {
        let current = vec![display("a", false), display("b", false)];
        assert_eq!(pick_fallback(&current).unwrap().id, "a");
    }

    #[test]
    fn pick_fallback_none_when_no_displays() {
        assert!(pick_fallback(&[]).is_none());
    }
}
