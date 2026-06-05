# StreamDeck — AI Handoff Document
## Feature F0.3 · Event Bus Foundation

**Document Type:** AI Session Handoff  
**Phase:** Phase 0 — Project Skeleton & Toolchain  
**Feature:** F0.3 — Event Bus Foundation  
**Prerequisite:** F0.1 and F0.2 must be completed and passing CI before starting this session  
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
Task    : F0.3 — Event Bus Foundation
```

---

## ═══════════════════════════════════════════
## BLOCK 2 — ARCHITECTURE INVARIANTS (never violate)
## ═══════════════════════════════════════════

```
1. PROCESS SEPARATION
   GStreamer pipeline (added in Phase 2) runs in a dedicated Tokio task.
   Tauri UI events NEVER block the media pipeline thread pool.
   No GStreamer calls from command handlers — use PipelineService actor channel.
   [Not relevant in Phase 0, but ALL event patterns established here must remain
    compatible with this constraint in all future phases.]

2. EVENT BUS ONLY
   ALL state transitions (slide change, scene cut, stream start/stop) flow through
   a tokio::sync::broadcast AppEvent enum.
   No direct function calls across subsystem boundaries.
   [F0.3 IS the implementation of this invariant. Every future phase depends on it.]

3. CONFIG AS CODE
   Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
   No hard-coded behaviour.
   WHIP bearer tokens stored in OS keychain (keyring crate), never in TOML plaintext.

4. COEXISTENCE FIRST
   New subsystems are additive. Mevo Studio and ProPresenter continue operating until
   the phase that explicitly retires them. No breaking changes to external tool integration.

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
        - tauri.conf.json defines two windows:
            controller   → default,  1280×800
            presentation → secondary, 1920×1080
        - `cargo tauri dev` launches both windows simultaneously ✓
        - Both windows render their React apps and show build timestamp ✓
        - `cargo tauri build` produces signed installer on macOS + Windows ✓
        - core:webview:allow-create-webview-window declared in
          src-tauri/capabilities/default.json ✓
        - CI (GitHub Actions) runs cargo test, cargo clippy -- -D warnings,
          npm run typecheck on every PR ✓

F0.2 ✓  IPC Command Layer complete
        - AppError enum at src-tauri/src/error.rs
            #[non_exhaustive], serde tag+content discriminated union, thiserror
            No manual From<AppError> impl — covered by Tauri's blanket impl<T: Serialize>
        - Commands module at src-tauri/src/commands/
            mod.rs       → pub mod info; pub mod system;
            info.rs      → get_build_info() command (migrated from F0.1 commands.rs)
            system.rs    → ping() command + PingResponse struct
        - Tauri wiring at src-tauri/src/app.rs (separated from lib.rs)
            app.rs is gated: #[cfg(not(test))] mod app; in lib.rs
            This prevents generate_context!() from firing during cargo test
        - lib.rs declares: mod error; pub mod commands; #[cfg(not(test))] mod app;
        - ipc.ts at controller/src/lib/ipc.ts and presentation/src/lib/ipc.ts
            AppError TypeScript type, PingResponse interface, ping() wrapper
            No component ever calls invoke() directly
        - All 4 acceptance criteria passing CI ✓
        - ping dispatch latency < 2ms verified in unit test ✓
```

### Key F0.2 Lessons Learned (inform F0.3 implementation)

> These are hard-won fixes from the F0.2 session. Do not repeat these mistakes.

```
LESSON 1 — Module resolution for #[tauri::command]
  tauri::generate_handler![] CANNOT use re-exported symbols (pub use).
  The macro requires full paths into the DEFINING module:
    ✓ generate_handler![commands::info::get_build_info, commands::system::ping]
    ✗ generate_handler![commands::get_build_info]  ← fails even with pub use
  All modules in the path must be pub mod, not just mod.

LESSON 2 — No manual From<T> for InvokeError
  Tauri v2 has blanket impl<T: Serialize> From<T> for InvokeError.
  Adding a manual impl From<AppError> causes E0119 conflicting implementations.
  The AppError derives Serialize — that is sufficient. No From impl needed.

LESSON 3 — generate_context!() is a parse-time proc macro
  #[cfg(not(test))] on a FUNCTION does NOT prevent the macro from expanding.
  The macro fires during file parsing, before cfg attributes are evaluated.
  Solution: move all Tauri wiring (including generate_context!()) into app.rs,
  then gate the entire module: #[cfg(not(test))] mod app;
  This excludes the file from compilation entirely during cargo test.

LESSON 4 — cargo build and cargo clippy require frontend dist/
  Both tools compile app.rs which contains generate_context!().
  generate_context!() validates frontendDist path at compile time.
  Therefore: controller/dist/ and presentation/dist/ MUST exist before
  running cargo build or cargo clippy.
  cargo test is always independent — never needs frontend dist.

LESSON 5 — AppError From<AppError> impl removal is permanent
  The handoff template incorrectly included a manual From impl.
  This is confirmed wrong for Tauri v2.11.x. Do not re-add it.
```

**External tools at this stage:** Mevo Studio handles all streaming. ProPresenter handles all
presentation. StreamDeck has no media functionality yet.

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F0.3 — Event Bus Foundation

**Description:**  
Instantiate the central `tokio::sync::broadcast` channel with a typed `AppEvent` enum.
Wire Tauri's event emitter so all broadcast events are forwarded to both frontend windows.
This is the implementation of Architecture Invariant #2 — the event bus that every future
phase (Phase 1 slides, Phase 2 NDI, Phase 3 compositor, Phase 4 streaming) will publish to
and subscribe from. The patterns established here must scale to 20+ event variants.

### Technologies

| Technology | Version | Role |
|---|---|---|
| `tokio::sync::broadcast` | Tokio 1.38 (built-in) | Single-producer multi-consumer channel, capacity 256 |
| `AppEvent` enum | custom | Typed event variants, `#[non_exhaustive]`, `serde::Serialize` |
| `tauri::AppHandle::emit_all()` | Tauri v2.11.x | Forward Rust broadcast events to both WebView windows |
| `tauri::Listener::listen_global()` | Tauri v2.11.x | Optional: wire frontend events back to Rust if needed |
| `ts-rs` or manual TS types | latest | Keep TypeScript AppEvent type in sync with Rust enum |

### Acceptance Criteria (verbatim from roadmap)

- [ ] `AppEvent` variants compile with `#[non_exhaustive]` to enforce forward compatibility
- [ ] Any event emitted in Rust appears in both windows' `listen()` handlers within 5 ms
- [ ] Bus backpressure: if a receiver lags > 128 events behind, it is force-dropped and
      re-subscribed with a `BusLag` warning event emitted
- [ ] Unit test: 1000 events dispatched under load, zero deadlocks, zero panics

### Definition of Done

All four acceptance criteria above must be passing in CI before this feature is marked
complete and before work on F0.4 (TOML Configuration Loader) begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Rust Types to Define (new in this feature)

```rust
// src-tauri/src/events.rs
// AppEvent is the single event type for ALL subsystem communication across all phases.
// Use #[non_exhaustive] — future phases add variants without breaking existing match arms.
// Use serde::Serialize so Tauri can forward events to the frontend as JSON.

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    // ── Phase 0 diagnostic events ──────────────────────────────────────────
    /// Emitted by the bus bridge when a receiver lags > 128 messages behind.
    /// Forces a re-subscription. Always present regardless of phase.
    BusLag { receiver_id: String, dropped: u64 },

    /// Test/diagnostic event — used by unit tests and the ping command.
    /// Remove or gate behind #[cfg(test)] once Phase 1 events are in place.
    Ping { message: String },

    // ── Phase 1 events (NOT implemented yet — listed for forward reference only)
    // SlideUpdate(SlidePayload)
    // SlideAdvance
    // SlideRewind
    // DeckEnd
    // DeckStart
    //
    // ── Phase 2+ events — DO NOT add yet. Listed here for context only.
    // PipelineError { element: String, message: String }
    // SourceLost { source_id: String }
    // StreamHealth { platform: String, bitrate_kbps: u32, dropped_frames: u64, rtt_ms: u32 }
}
```

```rust
// src-tauri/src/bus.rs
// EventBus holds the broadcast sender and provides a typed API for publishing events.
// Stored in Tauri's managed state so command handlers can access it.

pub struct EventBus {
    sender: tokio::sync::broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> (Self, tokio::sync::broadcast::Receiver<AppEvent>) {
        let (sender, receiver) = tokio::sync::broadcast::channel(256);
        (Self { sender }, receiver)
    }

    pub fn publish(&self, event: AppEvent) {
        // Use send() — if there are no receivers, that is acceptable (not an error).
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }
}
```

```rust
// src-tauri/src/app.rs  — CURRENT VERIFIED CONTENT (MODIFY only as specified below)
// This is the real file confirmed by the developer. Do not invent or assume structure.

use tauri::WebviewWindowBuilder;
use tauri::WebviewUrl;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── REQUIRED MODIFICATIONS FOR F0.3 ─────────────────────────────────────────
// 1. Add `use crate::bus::EventBus;` and `use crate::events::AppEvent;` imports
// 2. Create EventBus BEFORE the builder: let (bus, rx) = EventBus::new();
// 3. Add .manage(bus) to the builder chain (after .plugin(), before .invoke_handler())
// 4. INSIDE the existing .setup() closure, alongside the presentation window spawn,
//    add a second tokio::spawn for the bridge task that owns `rx`.
//    Do NOT replace the presentation window spawn — ADD alongside it.
// 5. The bridge task signature:
//    async fn run_event_bridge(handle: tauri::AppHandle,
//                              mut rx: tokio::sync::broadcast::Receiver<AppEvent>)
// ─────────────────────────────────────────────────────────────────────────────
```

### TypeScript Interface Contract

```typescript
// controller/src/lib/events.ts  (and mirrored in presentation/src/lib/events.ts)
// Typed listener wrapper for AppEvent. Components never call listen() directly.
// @sync-with: src-tauri/src/events.rs AppEvent

import { listen, UnlistenFn } from '@tauri-apps/api/event';

// @sync-with: src-tauri/src/events.rs AppEvent
export type AppEvent =
  | { type: 'BusLag';  payload: { receiver_id: string; dropped: number } }
  | { type: 'Ping';    payload: { message: string } };
  // Phase 1 will extend: | { type: 'SlideUpdate'; payload: SlidePayload }
  // Phase 2 will extend: | { type: 'PipelineError'; payload: { element: string; message: string } }

export type AppEventHandler = (event: AppEvent) => void;

/**
 * Subscribe to all AppEvents from Rust.
 * Returns an unlisten function — call it in useEffect cleanup to prevent leaks.
 * Never call listen() directly in components — use this wrapper.
 */
export async function onAppEvent(handler: AppEventHandler): Promise<UnlistenFn> {
  return listen<AppEvent>('app-event', (e) => handler(e.payload));
}
```

### File Structure This Feature Produces

```
src-tauri/
├── src/
│   ├── events.rs          ← AppEvent enum (NEW)
│   ├── bus.rs             ← EventBus struct with broadcast sender (NEW)
│   ├── app.rs             ← MODIFY: add .manage(bus) + bridge task in .setup()
│   └── lib.rs             ← MODIFY: add mod events; mod bus;
controller/
└── src/
    └── lib/
        └── events.ts      ← AppEvent TS type + onAppEvent() wrapper (NEW)
presentation/
└── src/
    └── lib/
        └── events.ts      ← same as controller events.ts (NEW)
```

### Existing Files to Read (for context only — do not modify unless specified)

```
src-tauri/src/lib.rs           — add mod events; mod bus; ONLY
src-tauri/src/app.rs           — add .manage() and bridge task to existing Builder chain
src-tauri/src/error.rs         — READ ONLY — do not modify AppError
src-tauri/src/commands/        — READ ONLY — do not modify existing commands
tauri.conf.json                — READ ONLY
capabilities/default.json      — READ ONLY
```

---

## ═══════════════════════════════════════════
## BLOCK 6 — SCOPE BOUNDARY
## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  F0.4 TOML config loader (notify crate, StreamDeckConfig) — separate feature
  ✗  Phase 1 AppEvent variants (SlideUpdate, SlideAdvance, DeckEnd, etc.) — too early
  ✗  Phase 2+ variants (PipelineError, SourceLost, StreamHealth) — too early
  ✗  Any GStreamer imports, pipeline code, or NDI references
  ✗  Any slide, scene, or streaming commands — those belong to Phase 1–4
  ✗  Frontend UI components for event visualisation — not part of F0.3
  ✗  listen() calls in App.tsx — only wire events.ts wrapper, not component usage

DO NOT modify:
  ✗  src-tauri/src/error.rs         — AppError is stable; do not add new variants
  ✗  src-tauri/src/commands/        — existing commands are stable
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  Existing CI workflow files (.github/workflows/)
  ✗  controller/src/App.tsx or presentation/src/App.tsx

CRITICAL CONSTRAINTS (learned from F0.2):
  ✗  Do NOT add a manual impl From<AppEvent> for tauri::ipc::InvokeError
  ✗  Do NOT register AppEvent or EventBus as a Tauri command — they are managed state
  ✗  Do NOT use pub use re-exports for anything passed to generate_handler![]
  ✗  The app.rs module must remain gated: #[cfg(not(test))] mod app; in lib.rs

AppEvent enum must use #[non_exhaustive].
Do NOT add phase-specific event variants beyond BusLag and Ping.
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/events.rs` | **CREATE** | `AppEvent` enum — `#[non_exhaustive]`, `serde::Serialize`, `BusLag` + `Ping` variants |
| `src-tauri/src/bus.rs` | **CREATE** | `EventBus` struct wrapping `broadcast::Sender<AppEvent>` |
| `src-tauri/src/lib.rs` | **MODIFY** | Add `mod events;` and `mod bus;` — nothing else |
| `src-tauri/src/app.rs` | **MODIFY** | Add `.manage(event_bus)` + bridge task in `.setup()` closure |
| `controller/src/lib/events.ts` | **CREATE** | `AppEvent` TS type + `onAppEvent()` listener wrapper |
| `presentation/src/lib/events.ts` | **CREATE** | Identical to controller events.ts |

### Files to Generate: Tests

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/bus.rs` | Include `#[cfg(test)]` block | Test: 1000 events, zero deadlocks, zero panics |
| `src-tauri/src/bus.rs` | Include backpressure test | Test: lagged receiver triggers BusLag event |
| `src-tauri/src/bus.rs` | Include latency test | Measure single event publish→receive < 5 ms |

### Output Format

```
- One fenced code block per file
- Use the file path as the language identifier comment at the top of each block:
    ```rust
    // src-tauri/src/events.rs
    ...
    ```
- Include #[cfg(test)] unit tests inside the same Rust files (not separate test files)
- Show COMPLETE file content for every file — no partial diffs
- For MODIFY actions on app.rs: show the COMPLETE file, not just the changed sections
- Do NOT provide prose explanation unless asked
```

### Verification Commands (run after generation)

```bash
# All must pass before F0.3 is considered complete:
cargo test                          # includes 1000-event + backpressure + latency tests
cargo clippy -- -D warnings         # zero warnings
npm run typecheck                   # TypeScript AppEvent type in sync with Rust
bash verify.sh f0.3                 # project verification framework (covers all AC)
```

---

## ═══════════════════════════════════════════
## CONTEXT NOTES FOR THE AI
## ═══════════════════════════════════════════

> Read this section before generating any code.

### 1. EventBus goes into Tauri managed state — not a global static

The `EventBus` must be registered with `.manage(event_bus)` in the Tauri Builder chain
inside `app.rs`. Command handlers that need to publish events retrieve it via:

```rust
#[tauri::command]
pub async fn some_future_command(
    bus: tauri::State<'_, EventBus>,
) -> Result<(), AppError> {
    bus.publish(AppEvent::Ping { message: "hello".into() });
    Ok(())
}
```

Do NOT use a `lazy_static` or `once_cell` global for the sender. Tauri managed state
is the correct pattern — it integrates with Tauri's lifetime management and is
testable without a running Tauri instance.

### 2. The bridge task owns a broadcast Receiver — do not clone the Sender into it

The bridge task (Rust→Frontend delivery) receives from the bus, it does not send.
Create the `EventBus` before `.setup()`, pass its initial `Receiver` to the bridge
task, then `.manage()` the `EventBus` struct (which holds the `Sender`).
The pattern:

```rust
let (bus, rx) = EventBus::new();          // rx is the bridge's receiver
app::Builder::default()
    .manage(bus)                           // Sender goes into managed state
    .setup(|app| {
        // rx moves into the bridge task
        tokio::spawn(bridge_task(app.handle().clone(), rx));
        Ok(())
    })
```

### 3. Backpressure: Lagged error means the receiver missed events — re-subscribe is automatic

`tokio::sync::broadcast::Receiver::recv()` returns `Err(RecvError::Lagged(n))` when
the receiver is too slow and the ring buffer has overwritten `n` messages.
After `Lagged`, the receiver is automatically repositioned to the oldest available
message — you do NOT need to call `subscribe()` again.
The correct response to `Lagged` is:
  1. Emit `AppEvent::BusLag { receiver_id, dropped: n }` to the frontend
  2. Continue the loop — the receiver is still valid

### 4. Both frontend windows must receive events — use emit_all(), not emit()

`app_handle.emit_all("app-event", &event)` broadcasts to ALL WebView windows.
`app_handle.emit("app-event", &event)` targets a specific window by label.
F0.3 uses `emit_all()` — the controller and presentation windows both need events.
Future features that need window-specific events will use `emit()` with the window label.

### 5. events.ts uses the same ipc.ts pattern enforced in F0.2

Just as `ipc.ts` wraps all `invoke()` calls, `events.ts` wraps all `listen()` calls.
No component in either frontend ever calls `listen()` directly.
This will be enforced by ESLint rule in Phase 1.
The wrapper returns the `UnlistenFn` — components must call it in cleanup
(React `useEffect` return function) to prevent event listener leaks.

### 6. The Ping event is a temporary diagnostic — not permanent infrastructure

`AppEvent::Ping` exists only to give the frontend a testable event in Phase 0.
It should be removed or gated behind `#[cfg(debug_assertions)]` once Phase 1
events (`SlideUpdate` etc.) are in place. This is intentional scope — note it
in a comment in events.rs.

### 7. app.rs modification — exact MODIFY instructions (real file confirmed)

The CURRENT `app.rs` is shown verbatim in Block 5 above. It has:
  - `.plugin(tauri_plugin_shell::init())`
  - `.invoke_handler()` with `get_build_info` and `ping`
  - `.setup()` with ONE `tauri::async_runtime::spawn` that creates the presentation window
  - `.run(tauri::generate_context!())`

When modifying `app.rs`, show the COMPLETE file and preserve all existing content. Only ADD:
  - `let (bus, rx) = EventBus::new();` before the builder (not inside setup)
  - `.manage(bus)` to the builder chain (between `.plugin()` and `.invoke_handler()`)
  - A SECOND `tauri::async_runtime::spawn` inside the existing `.setup()` closure for
    the bridge task — the presentation window spawn must remain unchanged
  - Required `use` imports at the top

The `.expect("Failed to create presentation window")` call must stay exactly as-is.
Do NOT convert it to a `?` propagation — it is intentional and matches the current code.

---

## ═══════════════════════════════════════════
## APPENDIX — CURRENT STABLE FILE STRUCTURE
## ═══════════════════════════════════════════

For reference — the state of the repository after F0.2 was completed.

```
src-tauri/
├── Cargo.toml                         ← thiserror, ts-rs, serde, tokio, tauri deps
├── src/
│   ├── lib.rs                         ← mod error; pub mod commands; #[cfg(not(test))] mod app;
│   ├── app.rs                         ← Tauri Builder (gated: not compiled in test)
│   ├── error.rs                       ← AppError enum
│   └── commands/
│       ├── mod.rs                     ← pub mod info; pub mod system;
│       ├── info.rs                    ← get_build_info() command
│       └── system.rs                  ← ping() command + PingResponse + unit tests
controller/
└── src/
    └── lib/
        └── ipc.ts                     ← AppError type + PingResponse + ping() wrapper
presentation/
└── src/
    └── lib/
        └── ipc.ts                     ← same as controller ipc.ts
```

After F0.3 the structure will extend to:

```
src-tauri/
└── src/
    ├── lib.rs                         ← + mod events; mod bus;
    ├── app.rs                         ← + .manage(bus) + bridge task
    ├── events.rs                      ← AppEvent enum (NEW)
    └── bus.rs                         ← EventBus struct (NEW)
controller/
└── src/
    └── lib/
        └── events.ts                  ← AppEvent TS type + onAppEvent() (NEW)
presentation/
└── src/
    └── lib/
        └── events.ts                  ← same as controller events.ts (NEW)
```
