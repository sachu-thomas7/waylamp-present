use tauri::{WebviewUrl, WebviewWindowBuilder};

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![commands::get_build_info])
        .setup(|app| {
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}