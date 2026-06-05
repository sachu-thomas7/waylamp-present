use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;
use tauri::WebviewWindowBuilder;
use tauri::WebviewUrl;

use crate::bus::EventBus;
use crate::events::AppEvent;
use crate::config_loader::ConfigLoader;

pub fn run() {
    let (bus, rx) = EventBus::new();
    let bus_arc = Arc::new(bus);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::clone(&bus_arc))
        .invoke_handler(tauri::generate_handler![
            crate::commands::info::get_build_info,
            crate::commands::system::ping,
            crate::commands::config::get_config,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                WebviewWindowBuilder::new(
                    &handle,
                    "presentation",
                    WebviewUrl::App("presentation/index.html".into()),
                )
                .title("StreamDeck — Presentation")
                .inner_size(1920.0, 1080.0)
                .resizable(true)
                .build()
                .expect("Failed to create presentation window");
            });

            tauri::async_runtime::spawn(run_event_bridge(app.handle().clone(), rx));

            let config_path = app
                .path()
                .app_config_dir()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .join("streamdeck.toml");

            let config = ConfigLoader::load_or_default(&config_path).map_err(|e| {
                eprintln!("Config load failed: {e}");
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
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
                // Receiver auto-repositions after Lagged — no re-subscribe needed.
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