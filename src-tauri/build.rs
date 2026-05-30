fn main() {
    // Inject build timestamp as a compile-time env variable
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono_or_manual_timestamp()
    );
    tauri_build::build()
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
