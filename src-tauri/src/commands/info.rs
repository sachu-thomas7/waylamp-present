/// Returns the build timestamp injected at compile time.
/// Satisfies F0.1 acceptance criterion: both windows show the build timestamp.
#[tauri::command]
pub fn get_build_info() -> serde_json::Value {
    serde_json::json!({
        "build_timestamp": env!("BUILD_TIMESTAMP"),
        "version": env!("CARGO_PKG_VERSION"),
    })
}