# StreamDeck — AI Handoff Document
## Feature F0.4 · TOML Configuration Loader

**Document Type:** AI Session Handoff  
**Phase:** Phase 0 — Project Skeleton & Toolchain  
**Feature:** F0.4 — TOML Configuration Loader  
**Prerequisite:** F0.1, F0.2, and F0.3 must be complete and CI green before starting  
**Date:** June 2026  

---

## ═══════════════════════════════════════════
## BLOCK 1 — PROJECT IDENTITY
## ═══════════════════════════════════════════

```
Project : StreamDeck — cross-platform live streaming + presentation desktop app
Mission : Unify local presentation control, multi-camera production, and live-stream egress
          into a single Tauri app. Zero dependency on Mevo Studio / ProPresenter by Phase 5.

Stack   : Tauri v2.11.x  ·  Rust 1.78 / Tokio 1.38  ·  React 18.x / TypeScript 5.x  ·  Vite 5.x
Phase   : Phase 0 — Project Skeleton & Toolchain  (no media pipeline yet)
Task    : F0.4 — TOML Configuration Loader
```

---

## ═══════════════════════════════════════════
## BLOCK 2 — ARCHITECTURE INVARIANTS (never violate)
## ═══════════════════════════════════════════

```
1. PROCESS SEPARATION
   GStreamer pipeline (Phase 2) runs in a dedicated Tokio task.
   Tauri UI events NEVER block the media pipeline thread pool.
   All event patterns established in Phase 0 must remain compatible.

2. EVENT BUS ONLY
   ALL state transitions flow through tokio::sync::broadcast AppEvent enum.
   No direct function calls across subsystem boundaries.
   Config hot-reload MUST publish AppEvent::ConfigReloaded — not call handlers directly.

3. CONFIG AS CODE  ← THIS IS THE INVARIANT F0.4 IMPLEMENTS
   Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
   No hard-coded behaviour anywhere.
   WHIP bearer tokens go in OS keychain (keyring crate), NEVER in TOML plaintext.

4. COEXISTENCE FIRST
   New subsystems are additive. Mevo Studio and ProPresenter continue operating.

5. OBSERVABILITY FROM DAY ONE
   All errors surface as typed AppEvent variants within 100 ms.
   Command handlers return Result<T, AppError> — never unwrap() or panic in handlers.
```

---

## ═══════════════════════════════════════════
## BLOCK 3 — COMPLETED & STABLE STATE
## ═══════════════════════════════════════════

> Treat everything below as ground truth. Do NOT regenerate, refactor, or modify these.

```
F0.1 ✓  Tauri v2 workspace bootstrapped
        - Rust backend at src-tauri/
        - Two frontend entry points: controller/ and presentation/
        - Vite 5.x + React 18.x + TypeScript 5.x wired for both frontends
        - tauri.conf.json: controller (1280×800), presentation (1920×1080)
        - cargo tauri dev launches both windows ✓
        - cargo tauri build produces signed installer on macOS + Windows ✓
        - CI: cargo test, cargo clippy -- -D warnings, npm run typecheck on every PR ✓

F0.2 ✓  IPC Command Layer complete
        - AppError at src-tauri/src/error.rs — VERIFIED CONTENT:
            #[derive(Debug, thiserror::Error, serde::Serialize)]
            #[serde(tag = "type", content = "message")]
            #[non_exhaustive]
            pub enum AppError {
                #[error("Internal error: {0}")]   Internal(String),
                #[error("Not found: {0}")]         NotFound(String),
                #[error("Invalid input: {0}")]     InvalidInput(String),
            }
        - Commands: src-tauri/src/commands/mod.rs, info.rs, system.rs
        - ipc.ts in controller/src/lib/ and presentation/src/lib/
        - No manual From<AppError> impl — Tauri v2 blanket covers it
        - ping dispatch latency < 2ms verified ✓

F0.3 ✓  Event Bus Foundation complete
        - src-tauri/src/events.rs — AppEvent enum (#[non_exhaustive], serde::Serialize)
            Variants: BusLag { receiver_id: String, dropped: u64 }, Ping { message: String }
        - src-tauri/src/bus.rs — EventBus struct, broadcast::channel(256)
            pub fn new() -> (Self, Receiver<AppEvent>)
            pub fn publish(&self, event: AppEvent)
            pub fn subscribe(&self) -> Receiver<AppEvent>
        - src-tauri/src/app.rs — .manage(bus) + run_event_bridge task
            Uses tauri::Emitter trait + handle.emit() (NOT emit_all — removed in Tauri v2)
        - controller/src/lib/events.ts + presentation/src/lib/events.ts
            onAppEvent() wrapper — components never call listen() directly
        - All 4 AC passing CI ✓
```

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F0.4 — TOML Configuration Loader

**Description:**  
Load `streamdeck.toml` from the Tauri app config directory on startup. If the file does not
exist, write a default config and continue. Validate on load — invalid TOML shows a
user-facing error dialog, not a panic. Watch for file changes with the `notify` v6 crate
and hot-reload within 500 ms, publishing `AppEvent::ConfigReloaded` on the bus.
Support config versioning so a v0 file loaded by a v1 app silently applies defaults for
new fields. Store WHIP bearer tokens in the OS keychain via `keyring` — never in TOML.

### Technologies

| Technology | Version | Role |
|---|---|---|
| `toml` crate | 0.8 (add to Cargo.toml) | Parse/serialise `streamdeck.toml` |
| `serde` | 1.x (already present) | Derive Deserialize/Serialize on config structs |
| `notify` | 6.x (add to Cargo.toml) | File-system watcher for hot-reload |
| `keyring` | 3.x (add to Cargo.toml) | OS keychain for WHIP tokens |
| `tauri::Manager::path()` | Tauri v2.11.x | Resolve app config dir — no plugin needed |

### Acceptance Criteria (verbatim from roadmap)

- [ ] Invalid TOML on startup shows a user-facing error dialog (not a Rust panic)
- [ ] Valid config change on disk is reflected in the running app within 500 ms
- [ ] Config version field enables migration: loading a v0 config in a v1 app applies
      default values for new fields without error
- [ ] Sensitive fields (WHIP bearer tokens) are stored in OS keychain via `keyring` crate,
      not in TOML plaintext

### Definition of Done

All four AC above passing CI before F0.4 is marked complete and Phase 1 work begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Cargo.toml — VERIFIED CURRENT STATE + required additions

```toml
# src-tauri/Cargo.toml — CURRENT VERIFIED CONTENT
[package]
name = "streamdeck"
version = "0.1.0"
edition = "2021"
rust-version = "1.78"

[lib]
name = "streamdeck_lib"
path = "src/lib.rs"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "streamdeck"
path = "src/main.rs"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri              = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"
tokio              = { version = "1", features = ["full"] }
sysinfo            = "0.30"
thiserror          = "1"
ts-rs              = { version = "10", features = ["serde-compat"] }
anyhow             = "1"
# ── ADD these three for F0.4 ─────────────────────────────────────────────────
toml               = "0.8"
notify             = "6"
keyring            = "3"

[features]
default         = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

### AppError — VERIFIED CURRENT CONTENT + required additions

```rust
// src-tauri/src/error.rs — ADD two variants only, preserve everything else exactly

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "type", content = "message")]
#[non_exhaustive]
pub enum AppError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // ── ADD for F0.4 ─────────────────────────────────────────────────────────
    #[error("Config error: {0}")]
    Config(String),

    #[error("Keychain error: {0}")]
    Keychain(String),
}
```

### AppEvent — ADD one variant to events.rs

```rust
// src-tauri/src/events.rs — ADD one variant only, preserve BusLag and Ping exactly

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag { receiver_id: String, dropped: u64 },
    Ping { message: String },  // temporary — remove at Phase 1

    // ── ADD for F0.4 ─────────────────────────────────────────────────────────
    ConfigReloaded,            // emitted by watcher task after successful hot-reload
}
```

### New Types to Define (F0.4)

```rust
// src-tauri/src/config.rs  ← NEW FILE

use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u32 = 1;

// #[serde(default)] is required on every struct — it is what enables v0→v1 migration.
// Fields added in future versions must have Default impls or serde will error on old files.
// Do NOT use #[serde(deny_unknown_fields)] — forward-compat requires lenient parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamDeckConfig {
    pub version: u32,
    pub app: AppConfig,
    // Phase 2+ will add: pub ndi: NdiConfig, pub whip: WhipConfig
}

impl Default for StreamDeckConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            app: AppConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub log_level: String,   // "info" | "debug" | "warn" | "error"
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { log_level: "info".into() }
    }
}
```

```rust
// src-tauri/src/config_loader.rs  ← NEW FILE

use crate::AppError;
use crate::config::StreamDeckConfig;
use std::path::PathBuf;

pub struct ConfigLoader;

impl ConfigLoader {
    /// Load config from path. If file missing, write default and return it.
    /// Returns AppError::Config on invalid TOML.
    pub fn load_or_default(path: &PathBuf) -> Result<StreamDeckConfig, AppError> { todo!() }

    /// Spawn a notify::RecommendedWatcher that publishes AppEvent::ConfigReloaded
    /// on the EventBus within 500ms of a valid file change.
    /// Caller MUST store the returned watcher in managed state — dropping it stops watching.
    pub fn spawn_watcher(
        path: PathBuf,
        bus: std::sync::Arc<crate::bus::EventBus>,
    ) -> Result<notify::RecommendedWatcher, AppError> { todo!() }
}
```

### app.rs — VERIFIED CURRENT CONTENT (MODIFY only as specified)

```rust
// src-tauri/src/app.rs — VERIFIED STATE after F0.3
// Only add what is listed under REQUIRED MODIFICATIONS — do not touch anything else.

use crate::bus::EventBus;
use crate::events::AppEvent;
use tauri::Emitter;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

pub fn run() {
    let (bus, rx) = EventBus::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(bus)
        .invoke_handler(tauri::generate_handler![
            crate::commands::info::get_build_info,
            crate::commands::system::ping,
        ])
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

            tauri::async_runtime::spawn(run_event_bridge(app.handle().clone(), rx));

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
            Ok(event) => { let _ = handle.emit("app-event", &event); }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                let _ = handle.emit("app-event", &AppEvent::BusLag {
                    receiver_id: "frontend-bridge".into(),
                    dropped: n,
                });
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── REQUIRED MODIFICATIONS FOR F0.4 ──────────────────────────────────────────
// 1. Add imports:
//      use crate::config_loader::ConfigLoader;
//      use tauri::Manager;
// 2. Add get_config to generate_handler![]
// 3. INSIDE .setup(), AFTER the bridge spawn:
//    a. Resolve path: app.path().app_config_dir()? / "streamdeck.toml"
//    b. Call ConfigLoader::load_or_default(&path)
//       On Err → show dialog via tauri::WebviewWindowBuilder or AppHandle dialog API, return Err
//    c. Call ConfigLoader::spawn_watcher(path, ...) → store watcher via .manage()
//    d. .manage(loaded_config)
// ─────────────────────────────────────────────────────────────────────────────
```

### lib.rs — add two module declarations

```rust
// src-tauri/src/lib.rs — ADD two lines only

mod error;
pub mod commands;
pub mod events;
pub mod bus;
pub mod config;          // ← ADD
pub mod config_loader;   // ← ADD

#[cfg(not(test))]
mod app;
```

---

## ═══════════════════════════════════════════
## BLOCK 6 — SCOPE BOUNDARY
## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  Phase 1+ AppEvent variants (SlideUpdate, SlideAdvance, etc.)
  ✗  GStreamer, NDI, or pipeline config fields in StreamDeckConfig — Phase 2+
  ✗  WHIP destination config fields — Phase 4 (keyring plumbing is fine, WHIP fields are not)
  ✗  Frontend UI for config editing — not part of F0.4
  ✗  tauri-plugin-fs — not needed, use tauri::Manager::path() directly

DO NOT modify:
  ✗  src-tauri/src/bus.rs
  ✗  src-tauri/src/commands/system.rs or info.rs
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  controller/src/lib/events.ts, ipc.ts
  ✗  presentation/src/lib/events.ts, ipc.ts

CRITICAL CONSTRAINTS (learned from F0.2 + F0.3):
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] paths — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Do NOT drop the notify watcher handle — dropping it silently stops file watching
  ✗  WHIP tokens must never be written to TOML — keyring only
  ✗  Use handle.emit() not handle.emit_all() — emit_all was removed in Tauri v2
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
|---|---|---|
| `src-tauri/Cargo.toml` | **MODIFY** | Add `toml = "0.8"`, `notify = "6"`, `keyring = "3"` |
| `src-tauri/src/error.rs` | **MODIFY** | Add `Config(String)` and `Keychain(String)` variants only |
| `src-tauri/src/events.rs` | **MODIFY** | Add `ConfigReloaded` variant only |
| `src-tauri/src/config.rs` | **CREATE** | `StreamDeckConfig`, `AppConfig`, `Default` impls |
| `src-tauri/src/config_loader.rs` | **CREATE** | `load_or_default()`, `spawn_watcher()` |
| `src-tauri/src/app.rs` | **MODIFY** | Config load + watcher in `.setup()`, `.manage(config)` |
| `src-tauri/src/lib.rs` | **MODIFY** | Add `pub mod config;` and `pub mod config_loader;` |
| `src-tauri/src/commands/config.rs` | **CREATE** | `get_config()` command returning `StreamDeckConfig` |
| `src-tauri/src/commands/mod.rs` | **MODIFY** | Add `pub mod config;` |

### Files to Generate: Tests

| File | Tests Required |
|---|---|
| `src-tauri/src/config_loader.rs` | Load valid TOML → correct struct; invalid TOML → `AppError::Config`; missing file → default written + returned; v0 config loaded by v1 → defaults applied for new fields |
| `src-tauri/src/config.rs` | `StreamDeckConfig::default()` produces `version = 1`; round-trip serialise → deserialise → equal |

### Output Format

```
- One fenced code block per file, file path as comment on line 1
- Complete file content for every file — no partial diffs
- #[cfg(test)] unit tests inside the same Rust file
- No prose explanation unless asked
```

### Verification Commands

```bash
cargo test                    # config load, migration, round-trip, watcher tests
cargo clippy -- -D warnings   # zero warnings
npm run typecheck             # TS still passes — no frontend changes expected
bash verify.sh f0.4           # full AC gate
```

---

## ═══════════════════════════════════════════
## LESSONS LEARNED (F0.2 + F0.3 — do not repeat)
## ═══════════════════════════════════════════

```
L1  generate_handler![] requires full paths to defining modules — no pub use re-exports
L2  No manual From<AppError> for InvokeError — Tauri v2 blanket impl covers it
L3  app.rs must stay in its own gated module — #[cfg(not(test))] mod app; in lib.rs
L4  cargo build/clippy require frontend dist/ — cargo test never does
L5  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
L6  notify watcher must be kept alive in managed state — dropping it stops watching silently
L7  #[serde(default)] on every config struct is required for forward-compat migration —
    fields added in later config versions must have Default impls or serde will error
```
