use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::events::AppEvent;
use crate::slide::SlideDeck;
use crate::slide_state_machine;

pub struct TimerAbortHandle(pub tokio::task::AbortHandle);
pub struct DisplayWatchAbortHandle(pub tauri::async_runtime::JoinHandle<()>);

pub fn run() {
    let (bus, rx) = EventBus::new();
    let bus_arc = Arc::new(bus);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::clone(&bus_arc))
        .invoke_handler(tauri::generate_handler![
            crate::commands::info::get_build_info,
            crate::commands::system::ping,
            crate::commands::config::get_config,
            crate::commands::presentation::presentation_show,
            crate::commands::presentation::presentation_hide,
            crate::commands::slide::slide_advance,
            crate::commands::slide::slide_rewind,
            crate::commands::slide::slide_jump_to,
            crate::commands::slide::slide_get_current,
            crate::commands::slide::slide_timer_reset,
            crate::commands::deck::deck_import,
            crate::commands::deck::deck_get_all_slides,
            crate::commands::deck::deck_restore,
            crate::commands::deck::deck_clear,
            crate::commands::scene::scene_push_image,
            crate::commands::scene::scene_push_video,
            crate::commands::scene::scene_set_source,
            crate::commands::scene::scene_get_active_source,
            crate::commands::display::display_list,
            crate::commands::display::display_assign_presentation,
        ])
        .setup(move |app| {
            let presentation_url = if cfg!(debug_assertions) {
                WebviewUrl::External("http://localhost:5174".parse().expect("valid URL"))
            } else {
                WebviewUrl::App("presentation/index.html".into())
            };

            WebviewWindowBuilder::new(app, "presentation", presentation_url)
                .title("StreamDeck — Presentation")
                .inner_size(1920.0, 1080.0)
                .visible(false)
                .decorations(false)
                .always_on_top(true)
                .fullscreen(false)
                .resizable(false)
                .build()?;

            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            let deck = SlideDeck::stub();
            let machine = slide_state_machine::new_handle(deck, data_dir.clone());
            let machine_for_restore = Arc::clone(&machine);
            let bus_for_restore = Arc::clone(&bus_arc);
            let app_handle_for_restore = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                slide_state_machine::restore_state(&machine_for_restore, &data_dir).await;
                let payload = machine_for_restore.read().await.current_payload();
                bus_for_restore.publish(crate::events::AppEvent::SlideUpdate(payload));

                let deck_data_dir = data_dir.join("decks");
                let active_id_path = deck_data_dir.join("active_deck_id.txt");
                if let Ok(raw) = std::fs::read_to_string(&active_id_path) {
                    let deck_id = raw.trim().to_string();
                    let deck_json_path = deck_data_dir.join(&deck_id).join("deck.json");
                    if deck_json_path.exists() {
                        if let Ok(deck) = crate::slide::SlideDeck::load_from_json(&deck_json_path) {
                            let first_payload = {
                                let mut m = machine_for_restore.write().await;
                                m.load_deck(deck);
                                m.current_payload()
                            };
                            bus_for_restore
                                .publish(crate::events::AppEvent::SlideUpdate(first_payload));
                        }
                    }
                }

                let abort = slide_state_machine::spawn_timer_task(
                    Arc::clone(&machine_for_restore),
                    Arc::clone(&bus_for_restore),
                );
                app_handle_for_restore.manage(TimerAbortHandle(abort));
            });

            app.manage(machine);

            let scene_handle = crate::scene::new_scene_handle();
            app.manage(scene_handle);

            tauri::async_runtime::spawn(run_event_bridge(app.handle().clone(), rx));

            let config_path = ConfigLoader::resolve_path(app.handle()).map_err(|e| {
                Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
            })?;

            let config = ConfigLoader::load_or_default(&config_path).map_err(|e| {
                eprintln!("Config load failed: {e}");
                Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
            })?;

            let watcher =
                ConfigLoader::spawn_watcher(config_path, Arc::clone(&bus_arc)).map_err(|e| {
                    Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
                })?;

            let display_handle =
                crate::display::new_display_handle(config.presentation.display_id.clone());
            if let Some(id) = &config.presentation.display_id {
                if let Ok(displays) = crate::display::list_displays(app.handle()) {
                    if let Some(target) = displays.iter().find(|d| &d.id == id) {
                        if let Some(window) = app.get_webview_window("presentation") {
                            let _ = crate::display::apply_position(&window, target);
                        }
                    }
                }
            }
            let display_watch_abort = crate::display::spawn_watch_task(
                app.handle().clone(),
                Arc::clone(&bus_arc),
                Arc::clone(&display_handle),
            );
            app.manage(display_handle);
            app.manage(DisplayWatchAbortHandle(display_watch_abort));

            app.manage(config);
            app.manage(watcher);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "controller" {
                if let tauri::WindowEvent::Destroyed = event {
                    if let Some(abort) = window.app_handle().try_state::<TimerAbortHandle>() {
                        abort.0.abort();
                    }
                    if let Some(abort) = window.app_handle().try_state::<DisplayWatchAbortHandle>()
                    {
                        abort.0.abort();
                    }
                    if let Some(presentation) =
                        window.app_handle().get_webview_window("presentation")
                    {
                        let _ = presentation.close();
                    }
                    window.app_handle().exit(0);
                }
            } else if window.label() == "presentation" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Treat the OS close button the same as "Stop Presenting": hide, don't
                    // destroy, this single long-lived window, and sync state back to the
                    // controller (which otherwise never learns presenting stopped).
                    api.prevent_close();
                    let app_handle = window.app_handle().clone();
                    if let Some(bus) = app_handle.try_state::<Arc<EventBus>>() {
                        crate::commands::presentation::hide_presentation(&app_handle, &bus);
                    }
                    let _ = app_handle
                        .global_shortcut()
                        .unregister(crate::commands::presentation::escape_shortcut());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn run_event_bridge(
    handle: tauri::AppHandle,
    mut rx: tokio::sync::broadcast::Receiver<AppEvent>,
) {
    loop {
        match rx.recv().await {
            Ok(event) => {
                let _ = handle.emit("app-event", &event);
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                let _ = handle.emit(
                    "app-event",
                    &AppEvent::BusLag {
                        receiver_id: "frontend-bridge".into(),
                        dropped: n,
                    },
                );
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}
