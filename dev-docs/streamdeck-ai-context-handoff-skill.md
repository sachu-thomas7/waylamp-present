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
            NOTE: share EventBus via Arc<EventBus>; do not reconstruct Arc from app.state()
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
| `toml` crate | 0.8 | Parse/serialise `streamdeck.toml` |
| `serde` | 1.x (already present) | Derive Deserialize/Serialize on config structs |
| `notify` | 6.x | File-system watcher for hot-reload |
| `keyring` | 3.x | OS keychain for WHIP tokens |
| `tempfile` | 3.x | Dev-dependency only for isolated test directories |
| `tauri::Manager::path()` | Tauri v2.11.x | Resolve app config dir — no plugin needed |

### Acceptance Criteria

| # | Criterion | Testability Class | Verification |
|---|---|---|---|
| AC1 | Invalid TOML on startup shows a user-facing error dialog (not a Rust panic) | integration | Manual verification in a Tauri runtime |
| AC2 | Valid config change on disk is reflected in the running app within 500 ms | integration | Integration test or manual timing verification with real file write + bus receive |
| AC3 | Config version field enables migration: loading a v0 config in a v1 app applies default values for new fields without error | unit | Rust unit test |
| AC4 | Sensitive fields (WHIP bearer tokens) are stored in OS keychain via `keyring` crate, not in TOML plaintext | unit/structural | Rust tests + verify.sh guard |

### Definition of Done

All four AC above passing CI where automatable, plus AC1 manually verified before F0.4 is marked complete and Phase 1 work begins.

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

[dev-dependencies]
tempfile           = "3"

[features]
default         = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

### Ownership & Shared State Contract

```rust
// CRITICAL OWNERSHIP RULE FOR F0.4
// EventBus must be wrapped in Arc<EventBus> BEFORE .manage().
// Never build Arc from app.state::<EventBus>().inner() inside .setup(),
// because that produces Arc<&EventBus>, not Arc<EventBus>.
//
// Correct pattern:
//   let (bus, rx) = EventBus::new();
//   let bus_arc = Arc::new(bus);
//   Builder::default()
//       .manage(Arc::clone(&bus_arc))
//       .setup(move |app| {
//           let watcher = ConfigLoader::spawn_watcher(path, Arc::clone(&bus_arc))?;
//           app.manage(watcher);
//           Ok(())
//       })
//
// The notify watcher MUST be stored in managed state. Dropping it stops watching silently.
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
    Ping { message: String },
    ConfigReloaded,
}
```

### New Types to Define (F0.4)

```rust
// src-tauri/src/config.rs  ← NEW FILE
// verify.sh run_f0_4() checks this file with raw grep patterns.
// Natural code should satisfy them; do not add artificial comments just to pass checks.
// Required natural matches:
// - "StreamDeckConfig"
// - "serde::Deserialize" (satisfied by use/import style if written directly)
// - "version"
// Forbidden:
// - bearer_token.*String

use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamDeckConfig {
    pub version: u32,
    pub app: AppConfig,
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
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { log_level: "info".into() }
    }
}
```

```rust
// src-tauri/src/config_loader.rs  ← NEW FILE
// verify.sh checks config_loader.rs for the raw grep pattern "notify".
// The normal notify imports satisfy this naturally.

use crate::AppError;
use crate::config::StreamDeckConfig;
use std::path::PathBuf;
use std::sync::Arc;

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
        bus: Arc<crate::bus::EventBus>,
    ) -> Result<notify::RecommendedWatcher, AppError> { todo!() }
}
```

### verify.sh — F0.4 Checks Decoded

| verify.sh label | File | grep pattern | Natural code that satisfies it |
|---|---|---|---|
| StreamDeckConfig struct | `src-tauri/src/config.rs` | `StreamDeckConfig` | struct name |
| serde Deserialize | `src-tauri/src/config.rs` | `serde::Deserialize` | explicit import path or derive-related direct usage |
| notify hot-reload | `src-tauri/src/config_loader.rs` | `notify` | `use notify::{...}` |
| version field | `src-tauri/src/config.rs` | `version` | `pub version: u32` |
| No plaintext tokens | `src-tauri/src/config.rs` | `bearer_token.*String` | must remain absent |

### app.rs — VERIFIED CURRENT CONTENT (MODIFY only as specified)

```rust
// src-tauri/src/app.rs — VERIFIED STATE after F0.3
// Only add what is listed under REQUIRED MODIFICATIONS — do not touch anything else.
// Additional implementation notes learned from F0.4:
// - use std::sync::Arc;
// - use std::io::Error::other(...) instead of Error::new(ErrorKind::Other, ...)
// - collapse nested ifs to satisfy clippy::collapsible_if
// - .setup() should be move when it captures rx and bus_arc
// - if a dialog plugin is not available, document the exact fallback behaviour before coding
```

### lib.rs — add two module declarations

```rust
// src-tauri/src/lib.rs — ADD two lines only

mod error;
pub mod commands;
pub mod events;
pub mod bus;
pub mod config;
pub mod config_loader;

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

CRITICAL CONSTRAINTS (learned from F0.2 + F0.3 + F0.4):
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] paths — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Do NOT drop the notify watcher handle — dropping it silently stops file watching
  ✗  WHIP tokens must never be written to TOML — keyring only
  ✗  Use handle.emit() not handle.emit_all() — emit_all was removed in Tauri v2
  ✗  Do NOT construct Arc from app.state().inner() when Arc<T> is required
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
|---|---|---|
| `src-tauri/Cargo.toml` | **MODIFY** | Add `toml = "0.8"`, `notify = "6"`, `keyring = "3"`, and `[dev-dependencies] tempfile = "3"` |
| `src-tauri/src/error.rs` | **MODIFY** | Add `Config(String)` and `Keychain(String)` variants only |
| `src-tauri/src/events.rs` | **MODIFY** | Add `ConfigReloaded` variant only |
| `src-tauri/src/config.rs` | **CREATE** | `StreamDeckConfig`, `AppConfig`, `Default` impls |
| `src-tauri/src/config_loader.rs` | **CREATE** | `load_or_default()`, `spawn_watcher()` |
| `src-tauri/src/app.rs` | **MODIFY** | Config load + watcher in `.setup()`, `.manage(config)`, Arc ownership pattern |
| `src-tauri/src/lib.rs` | **MODIFY** | Add `pub mod config;` and `pub mod config_loader;` |
| `src-tauri/src/commands/config.rs` | **CREATE** | `get_config()` command returning `StreamDeckConfig` |
| `src-tauri/src/commands/mod.rs` | **MODIFY** | Add `pub mod config;` |

### Tests Required

| File | Tests Required |
|---|---|
| `src-tauri/src/config_loader.rs` | Load valid TOML → correct struct; invalid TOML → `AppError::Config`; missing file → default written + returned; v0 config loaded by v1 → defaults applied for new fields |
| `src-tauri/src/config.rs` | `StreamDeckConfig::default()` produces `version = 1`; round-trip serialise → deserialise → equal |

### Verification Contract

```bash
cargo test                    # config load, migration, round-trip tests
cargo clippy -- -D warnings   # zero warnings
npm run typecheck             # TS still passes — no frontend changes expected
bash verify.sh f0.4           # pattern + AC gate
```

If verify.sh is referenced in a future handoff, document each raw grep-based check in the handoff itself. Prefer real acceptance tests over pattern checks wherever possible.

### Output Format

```
- One fenced code block per file, file path as comment on line 1
- Complete file content for every file — no partial diffs
- #[cfg(test)] unit tests inside the same Rust file
- No prose explanation unless asked
- Avoid unnecessary comments; add comments only when they capture a non-obvious invariant, ownership rule, verify-script contract, or lint constraint
```

---

## ═══════════════════════════════════════════
## LESSONS LEARNED (F0.2 + F0.3 + F0.4 — do not repeat)
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
L8  Arc<EventBus> must be created before .manage() — never reconstruct from app.state()
L9  Use std::io::Error::other(...) instead of Error::new(ErrorKind::Other, ...)
L10 verify.sh uses raw grep checks; document those checks when they matter to implementation
L11 Prefer real acceptance tests over hardcoded grep patterns in verify.sh for future features
L12 Empty TOML and v0 TOML are not equivalent for migration tests; use explicit `version = 0`
```
