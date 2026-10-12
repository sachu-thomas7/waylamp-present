fn main() {
    // Inject build timestamp as a compile-time env variable
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono_or_manual_timestamp()
    );
    embed_test_manifest();
    tauri_build::build()
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
