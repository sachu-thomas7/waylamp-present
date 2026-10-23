fn main() {
    // Inject build timestamp as a compile-time env variable
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono_or_manual_timestamp()
    );
    embed_test_manifest();
    // With an app manifest, app commands are denied to every window until a capability grants them
    // (the controller gets the `controller-commands` set; the projector gets none).
    println!("cargo:rerun-if-changed=src/state.rs");
    println!("cargo:rerun-if-changed=permissions");
    let commands: &'static [&'static str] = Box::leak(app_commands().into_boxed_slice());
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build")
}

/// Command names from the `generate_handler!` list in state.rs, the single source of truth.
fn app_commands() -> Vec<&'static str> {
    let src = std::fs::read_to_string("src/state.rs").expect("read src/state.rs");
    let list = src
        .split("tauri::generate_handler![")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("generate_handler! list in state.rs");
    list.lines()
        .filter_map(|l| l.trim().strip_prefix("crate::commands::"))
        .filter_map(|l| l.trim_end_matches(',').rsplit("::").next())
        .map(|name| &*Box::leak(name.to_string().into_boxed_str()))
        .collect()
}

// Test binaries don't get tauri-build's app manifest; without Common Controls v6 they fail to start.
fn embed_test_manifest() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}

fn chrono_or_manual_timestamp() -> String {
    // Use SOURCE_DATE_EPOCH for reproducible builds if set
    if let Ok(epoch) = std::env::var("SOURCE_DATE_EPOCH") {
        return format!("epoch:{}", epoch);
    }
    // Fallback: embed the build date from environment or a placeholder
    // In CI, SOURCE_DATE_EPOCH is set. Locally it falls back gracefully.
    std::env::var("BUILD_DATE").unwrap_or_else(|_| "dev-build".to_string())
}
