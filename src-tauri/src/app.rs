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

const DEV_PRESENTATION_CAPABILITY: &str = r#"{"identifier":"presentation-dev-remote","windows":["presentation"],"remote":{"urls":["http://localhost:5174/*"]},"permissions":["core:event:allow-listen","core:event:allow-unlisten"]}"#;

pub fn run() {
    let (bus, rx) = EventBus::new();
    let bus_arc = Arc::new(bus);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            crate::media_protocol::SCHEME,
            |ctx, request, responder| {
                let app = ctx.app_handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let path = request.uri().path().to_string();
                    let range = request
                        .headers()
                        .get("range")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let staged = app
                        .try_state::<crate::scene::SceneStateHandle>()
                        .and_then(|s| s.blocking_read().current_media.clone())
                        .filter(|m| matches!(m.media_type, crate::scene::SceneMediaType::Video))
                        .map(|m| std::path::PathBuf::from(m.path));
                    let r = match app.try_state::<crate::state::AppDataDir>() {
                        Some(dir) => crate::media_protocol::respond(
                            &dir.0,
                            staged.as_deref(),
                            &path,
                            range.as_deref(),
                        ),
                        None => crate::media_protocol::MediaResponse {
                            status: 503,
                            headers: vec![],
                            body: vec![],
                        },
                    };
                    let mut b = tauri::http::Response::builder().status(r.status);
                    for (k, v) in r.headers {
                        b = b.header(k, v);
                    }
                    if let Ok(resp) = b.body(r.body) {
                        responder.respond(resp);
                    }
                });
            },
        )
        .invoke_handler(crate::state::invoke_handler())
        .setup(move |app| {
            if cfg!(debug_assertions) {
                // Dev-server content is remote and gets zero IPC unless its origin is allowlisted; never shipped.
                app.add_capability(DEV_PRESENTATION_CAPABILITY)?;
            }
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

            let (config, config_recovery) = ConfigLoader::load_or_recover(&config_path);

            let machine = crate::state::manage_core(
                app,
                Arc::clone(&bus_arc),
                data_dir.clone(),
                config_path.clone(),
                config.clone(),
            );
            let notices = app.state::<crate::state::StartupNotices>();
            if let Some(n) = config_recovery.notice("Settings (streamdeck.toml)") {
                notices.push(n);
            }
            for n in tauri::async_runtime::block_on(crate::state::restore_on_startup(
                &machine, &bus_arc, &data_dir,
            )) {
                notices.push(n);
            }
            let machine_for_timer = Arc::clone(&machine);
            let bus_for_timer = Arc::clone(&bus_arc);
            let app_handle_for_timer = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                crate::state::spawn_timer(&app_handle_for_timer, machine_for_timer, bus_for_timer);
            });

            tauri::async_runtime::spawn(run_event_bridge(app.handle().clone(), rx));

            let watcher = ConfigLoader::spawn_watcher(config_path.clone(), Arc::clone(&bus_arc))
                .map_err(|e| {
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

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "controller" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if crate::commands::presentation::is_presenting() {
                        api.prevent_close();
                        confirm_close_while_presenting(window.clone());
                    }
                }
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {});
}

fn confirm_close_while_presenting(window: tauri::Window) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let target = window.clone();
    window
        .dialog()
        .message("The presentation is live on the projector. Closing the controller will end it.")
        .title("Quit while presenting?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit".into(),
            "Keep presenting".into(),
        ))
        .show(move |quit| {
            if quit {
                let _ = target.destroy();
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
                for event in crate::state::resync_events(&handle).await {
                    let _ = handle.emit("app-event", &event);
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}
