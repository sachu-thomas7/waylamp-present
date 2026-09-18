use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;
use tauri::WebviewWindowBuilder;
use tauri::WebviewUrl;

use crate::bus::EventBus;
use crate::events::AppEvent;
use crate::config_loader::ConfigLoader;
use crate::slide::SlideDeck;
use crate::slide_state_machine;

pub struct TimerAbortHandle(pub tokio::task::AbortHandle);

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
            crate::commands::scene::scene_push_image,
            crate::commands::scene::scene_push_video,
            crate::commands::scene::scene_set_source,
            crate::commands::scene::scene_get_active_source,
        ])
        .setup(move |app| {
            WebviewWindowBuilder::new(
                app,
                "presentation",
                WebviewUrl::App("presentation/index.html".into()),
            )
            .title("StreamDeck — Presentation")
            .inner_size(1920.0, 1080.0)
            .visible(false)
            .decorations(false)
            .always_on_top(true)
            .fullscreen(false)
            .resizable(false)
            .build()?;

            let data_dir = app.path().app_data_dir()
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
                                crate::slide::SlidePayload::from_entry(
                                    m.current_slide_entry(),
                                    m.deck_len() as u32,
                                )
                            };
                            bus_for_restore.publish(crate::events::AppEvent::SlideUpdate(first_payload));
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

            let config_path = app
                .path()
                .app_config_dir()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .join("streamdeck.toml");

            let config = ConfigLoader::load_or_default(&config_path).map_err(|e| {
                eprintln!("Config load failed: {e}");
                Box::new(std::io::Error::other(e.to_string()))
                    as Box<dyn std::error::Error>
            })?;

            let watcher = ConfigLoader::spawn_watcher(config_path, Arc::clone(&bus_arc))
                .map_err(|e| {
                    Box::new(std::io::Error::other(e.to_string()))
                        as Box<dyn std::error::Error>
                })?;

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
                    window.app_handle().exit(0);
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