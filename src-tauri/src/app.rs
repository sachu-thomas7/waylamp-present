use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::bus::EventBus;
use crate::config_loader::ConfigLoader;
use crate::events::AppEvent;

use crate::state::TimerAbortHandle;
pub struct DisplayWatchAbortHandle(pub tauri::async_runtime::JoinHandle<()>);
pub struct NdiWatchAbortHandle(pub tauri::async_runtime::JoinHandle<()>);

pub fn run() {
    let (bus, rx) = EventBus::new();
    let bus_arc = Arc::new(bus);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(crate::state::invoke_handler())
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

            let config_path = ConfigLoader::resolve_path(app.handle()).map_err(|e| {
                Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
            })?;

            let config = ConfigLoader::load_or_default(&config_path).map_err(|e| {
                eprintln!("Config load failed: {e}");
                Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
            })?;

            let machine = crate::state::manage_core(
                app,
                Arc::clone(&bus_arc),
                data_dir.clone(),
                config_path.clone(),
                config.clone(),
            );
            let machine_for_restore = Arc::clone(&machine);
            let bus_for_restore = Arc::clone(&bus_arc);
            let app_handle_for_restore = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                crate::state::restore_on_startup(&machine_for_restore, &bus_for_restore, &data_dir)
                    .await;
                crate::state::spawn_timer(
                    &app_handle_for_restore,
                    machine_for_restore,
                    bus_for_restore,
                );
            });

            tauri::async_runtime::spawn(run_event_bridge(app.handle().clone(), rx));

            let watcher =
                ConfigLoader::spawn_watcher(config_path, Arc::clone(&bus_arc)).map_err(|e| {
                    Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
                })?;

            let display_provider: crate::display::DisplayProviderHandle =
                Arc::new(crate::display::TauriDisplayProvider(app.handle().clone()));
            let display_handle =
                crate::state::manage_display(app, &config, Arc::clone(&display_provider));
            if let Some(id) = &config.presentation.display_id {
                if let Ok(displays) = display_provider.list() {
                    if let Some(target) = displays.iter().find(|d| &d.id == id) {
                        if let Some(window) = app.get_webview_window("presentation") {
                            let _ = crate::display::apply_position(&window, target);
                        }
                    }
                }
            }
            let display_watch_abort = crate::display::spawn_watch_task(
                app.handle().clone(),
                display_provider,
                Arc::clone(&bus_arc),
                Arc::clone(&display_handle),
            );
            app.manage(DisplayWatchAbortHandle(display_watch_abort));

            app.manage(watcher);

            let pipeline = crate::state::manage_pipeline(
                app,
                crate::state::production_pipeline_backend(Arc::clone(&bus_arc)),
                Arc::clone(&bus_arc),
            );

            let ndi_provider = crate::ndi::production_discovery();
            let ndi_state = crate::state::manage_ndi(app, Arc::clone(&ndi_provider));
            let ndi_watch_abort = crate::ndi::spawn_watch_task(
                ndi_provider,
                pipeline,
                Arc::clone(&bus_arc),
                ndi_state,
            );
            app.manage(NdiWatchAbortHandle(ndi_watch_abort));

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
                    if let Some(abort) = window.app_handle().try_state::<NdiWatchAbortHandle>() {
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(pipeline) = app.try_state::<crate::pipeline::PipelineHandle>() {
                    pipeline.shutdown_blocking(std::time::Duration::from_secs(3));
                }
            }
        });
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
