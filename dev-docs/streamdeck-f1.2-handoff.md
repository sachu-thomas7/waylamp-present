# StreamDeck — AI Handoff Document
## Feature F1.2 · Slide State Machine

**Document Type:** AI Session Handoff
**Phase:** Phase 1 — Presentation Engine
**Feature:** F1.2 — Slide State Machine
**Prerequisite:** F0.1, F0.2, F0.3, F0.4, F1.1 must be complete and CI green before starting
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
Phase   : Phase 1 — Presentation Engine
Task    : F1.2 — Slide State Machine

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
Slide navigation (advance, rewind, jump) MUST flow through the bus —
never via direct state mutation from a command handler.
3. CONFIG AS CODE
Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
No hard-coded behaviour anywhere.
WHIP bearer tokens go in OS keychain (keyring crate), NEVER in TOML plaintext.
4. COEXISTENCE FIRST
New subsystems are additive. Mevo Studio and ProPresenter continue operating.
SlideStateMachine replaces SlidePayload::stub() in F1.1 — it does not remove any
existing commands or window lifecycle behaviour.
5. OBSERVABILITY FROM DAY ONE
All errors surface as typed AppEvent variants within 100 ms.
Command handlers return Result<T, AppError> — never unwrap() or panic in handlers.
6. PRESENTATION WINDOW CONTAINS ZERO BUSINESS LOGIC
The Presentation Window remains a pure renderer. It holds no slide state.
It only reacts to AppEvent::SlideUpdate events. All navigation logic lives
exclusively in SlideStateMachine on the Rust side.
7. STATE MACHINE IS THE SINGLE SOURCE OF TRUTH
Both the Controller and Presentation windows read slide state only from
AppEvent::SlideUpdate published by the state machine.
No frontend component stores a slide index independently.
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
\#[derive(Debug, thiserror::Error, serde::Serialize)]
\#[serde(tag = "type", content = "message")]
\#[non_exhaustive]
pub enum AppError {
\#[error("Internal error: {0}")]   Internal(String),
\#[error("Not found: {0}")]         NotFound(String),
\#[error("Invalid input: {0}")]     InvalidInput(String),
\#[error("Config error: {0}")]      Config(String),
\#[error("Keychain error: {0}")]    Keychain(String),
}
- Commands: src-tauri/src/commands/mod.rs, info.rs, system.rs, config.rs
- ipc.ts in controller/src/lib/ and presentation/src/lib/
- No manual From<AppError> impl — Tauri v2 blanket covers it
- ping dispatch latency < 2ms verified ✓

F0.3 ✓  Event Bus Foundation complete
- src-tauri/src/bus.rs — EventBus struct, broadcast::channel(256)
pub fn new() -> (Self, Receiver<AppEvent>)
pub fn publish(\&self, event: AppEvent)
pub fn subscribe(\&self) -> Receiver<AppEvent>
NOTE: EventBus is Arc-wrapped before .manage() — do NOT reconstruct Arc from app.state()
- src-tauri/src/app.rs — Arc<EventBus> managed, run_event_bridge task running
Uses handle.emit() — NOT emit_all() (removed in Tauri v2)
- All AC passing CI ✓

F0.4 ✓  TOML Configuration Loader complete
- src-tauri/src/config.rs — StreamDeckConfig { version: u32, app: AppConfig }
- src-tauri/src/config_loader.rs — load_or_default(), spawn_watcher()
- notify v6 hot-reload, keyring for WHIP tokens
- AppEvent::ConfigReloaded published on file change ✓

F1.1 ✓  Presentation Window \& Slide Renderer complete
- Presentation window created hidden at startup (visible: false) in app.rs
- presentation_show / presentation_hide commands registered in generate_handler![]
- Global Escape shortcut registered on show, unregistered on hide
Unregister spawned via tauri::async_runtime::spawn to avoid re-entrant deadlock
Weak<EventBus> used in shortcut callback — prevents Arc keeping runtime alive on exit
- PRESENTING: AtomicBool in commands/presentation.rs guards double-registration
- app.rs on_window_event(Destroyed on "controller") calls app_handle.exit(0) for clean shutdown
- AppEvent::PresentationVisibility { visible: bool } drives controller button label
- verify.sh f1.1 passing CI ✓

        src-tauri/src/events.rs — VERIFIED CONTENT:
            use crate::slide::SlidePayload;
    
            #[derive(Debug, Clone, serde::Serialize)]
            #[serde(tag = "type", content = "payload")]
            #[non_exhaustive]
            pub enum AppEvent {
                BusLag { receiver_id: String, dropped: u64 },
                Ping { message: String },
                ConfigReloaded,
                SlideUpdate(SlidePayload),
                PresentationVisibility { visible: bool },
            }
    
        src-tauri/src/lib.rs — VERIFIED CONTENT:
            mod error;
            pub mod commands;
            pub mod events;
            pub mod bus;
            pub mod config;
            pub mod config_loader;
            pub mod slide;
    
            #[cfg(not(test))]
            mod app;
    
            #[cfg(not(test))]
            pub fn run() {
                app::run();
            }
    
        src-tauri/src/commands/mod.rs — VERIFIED CONTENT:
            pub mod config;
            pub mod info;
            pub mod system;
            pub mod presentation;
    
        src-tauri/src/commands/presentation.rs — VERIFIED CONTENT:
            - presentation_show: shows window, sets fullscreen, registers Escape via
              on_shortcut with async spawn for unregister, publishes PresentationVisibility
              and SlideUpdate(SlidePayload::stub()) on bus
            - presentation_hide: hides window, unregisters Escape, publishes
              PresentationVisibility { visible: false }
            - PRESENTING: AtomicBool guards both commands against double execution
            - Weak<EventBus> used in shortcut closure — not Arc — to avoid exit hang
            - hide_presentation() helper shared by both paths
    
        src-tauri/src/app.rs — VERIFIED CONTENT:
            - Plugins: tauri_plugin_shell, tauri_plugin_global_shortcut
            - generate_handler![]: get_build_info, ping, get_config,
              presentation_show, presentation_hide
            - WebviewWindowBuilder for "presentation": visible(false), decorations(false),
              always_on_top(true), fullscreen(false), resizable(false), inner_size(1920,1080)
            - on_window_event: controller Destroyed → unregister Escape → app_handle.exit(0)
            - run_event_bridge: loop on broadcast::Receiver<AppEvent>, handle.emit("app-event")
              Handles Lagged (emits BusLag) and Closed (breaks loop)
    
        src-tauri/Cargo.toml [dependencies] — VERIFIED CONTENT:
            tauri              = { version = "2", features = ["devtools"] }
            tauri-plugin-shell = "2"
            tauri-plugin-global-shortcut = "2"
            serde              = { version = "1", features = ["derive"] }
            serde_json         = "1"
            tokio              = { version = "1", features = ["full"] }
            sysinfo            = "0.30"
            thiserror          = "1"
            ts-rs              = { version = "10", features = ["serde-compat"] }
            anyhow             = "1"
            toml               = "0.8"
            notify             = "6"
            keyring            = "3"
    
        controller/src/lib/ipc.ts — VERIFIED CONTENT:
            ping(), getBuildInfo(), presentationShow(), presentationHide()
            AppError union: Internal | NotFound | InvalidInput | Config | Keychain
            BuildInfo: { build_timestamp: string; version: string }
    
        controller/src/lib/events.ts — VERIFIED CONTENT:
            AppEvent union: BusLag | Ping | ConfigReloaded | SlideUpdate | PresentationVisibility
            onAppEvent(handler): Promise<UnlistenFn> — wraps listen('app-event')
            // @sync-with: src-tauri/src/events.rs AppEvent
    
        controller/src/components/PresentButton.tsx — VERIFIED CONTENT:
            - useEffect subscribes to onAppEvent, sets presenting from PresentationVisibility
            - handleClick calls presentationShow() or presentationHide()
            - No local state guessing — Rust PresentationVisibility is single source of truth
    ```

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F1.2 — Slide State Machine

**Description:**
Implement the Rust-side `SlideStateMachine` as the single source of truth for all slide
navigation. The state machine holds the loaded `SlideDeck`, tracks the current slide index,
and publishes `AppEvent::SlideUpdate` on every navigation action. It replaces the
`SlidePayload::stub()` call currently used in `presentation_show`.

For F1.2, a hardcoded stub deck (5 slides, defined in Rust as a constant) is loaded at
startup so the full navigation flow can be exercised end-to-end without requiring file
import (F1.4). A minimal Controller UI panel shows current/next slide info and
Prev / Next / Jump buttons to exercise every state machine path.

**Navigation rules:**
- Advance past last slide → emit `AppEvent::DeckEnd`, do NOT wrap around
- Rewind before slide 0 → emit `AppEvent::DeckStart`, remain at slide 0
- Jump to invalid index → return `AppError::InvalidInput`, no state change
- State is serialised to disk on every change → crash-restart restores last position
- All state transitions MUST go through the event bus — no direct mutation from commands

**Stub Deck (hardcoded for F1.2 testing):**
```

Slide 0: title="Welcome",          body="StreamDeck F1.2 State Machine"
Slide 1: title="Architecture",     body="Event bus · Rust state · React renderer"
Slide 2: title="Navigation",       body="Advance · Rewind · Jump · Boundary events"
Slide 3: title="Observability",    body="Every transition is a typed AppEvent"
Slide 4: title="Next: F1.3",       body="Controller slide panel with thumbnails"

```
All slides have `notes: ""` and `background_path: None` for F1.2.

### Technologies

| Technology | Version | Role |
|---|---|---|
| `tokio::sync::RwLock` | 1.38 | Wraps SlideStateMachine for concurrent read access |
| `serde_json` | 1.x (already present) | Serialise deck state to disk on every change |
| `tauri::Manager::path()` | v2.11.x | Resolve app data dir for state persistence file |
| React + Zustand | 18.x / 5.x | Controller SlidePanel component, reads from existing slideStore |
| `@tauri-apps/api` | 2.x | invoke() wrappers for slide navigation commands |

### Acceptance Criteria

| # | Criterion | Testability Class | Verification |
|---|---|---|---|
| AC1 | Advancing past the last slide emits AppEvent::DeckEnd and does not wrap around | unit | cargo test: advance from last slide, assert DeckEnd published, index unchanged |
| AC2 | Rewinding before slide 0 emits AppEvent::DeckStart and index stays at 0 | unit | cargo test: rewind from slide 0, assert DeckStart published, index = 0 |
| AC3 | Jump to valid index publishes SlideUpdate with correct payload | unit | cargo test: jump_to(3), assert SlideUpdate(index=3) on bus |
| AC4 | Jump to out-of-range index returns AppError::InvalidInput, no state change | unit | cargo test: jump_to(99), assert Err(InvalidInput), index unchanged |
| AC5 | Concurrent reads from Controller and Presentation windows never block > 1 ms | unit | criterion benchmark: 1000 concurrent reads under RwLock |
| AC6 | State is restored from disk after simulated crash-restart | unit | cargo test: advance to slide 3, write state, reload, assert index = 3 |
| AC7 | Controller UI shows current slide title/body, next slide title, and slide counter (n/total) | integration | Manual: launch app, click Next, verify controller panel updates |
| AC8 | Presentation window updates within one frame (< 16 ms) of a navigation command | integration | Manual: click Next, observe presentation window updates immediately |

### Definition of Done

All unit AC (AC1–AC6) passing `cargo test`, AC7–AC8 manually verified on macOS and Windows
before F1.3 begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Cargo.toml — NO new dependencies for F1.2

All required crates (`tokio`, `serde_json`, `serde`) are already present.
No new entries in Cargo.toml.

### AppEvent — ADD three variants to events.rs

```rust
// src-tauri/src/events.rs — ADD three variants only, preserve all existing variants exactly

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag { receiver_id: String, dropped: u64 },
    Ping { message: String },
    ConfigReloaded,
    SlideUpdate(SlidePayload),
    PresentationVisibility { visible: bool },

    // ADD for F1.2
    SlideAdvance,
    SlideRewind,
    DeckEnd,
    DeckStart,
}
```

NOTE: `SlideAdvance` and `SlideRewind` are notification events only — they carry no payload.
The authoritative new state is always in the accompanying `SlideUpdate` event that the
state machine publishes immediately after the transition.

### SlideDeck — NEW type in src-tauri/src/slide.rs

```rust
// src-tauri/src/slide.rs — ADD SlideDeck below existing SlidePayload.
// Do NOT modify SlidePayload — it is stable and used by F1.1.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideEntry {
    pub index: u32,
    pub title: String,
    pub body: String,
    pub notes: String,
    pub background_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideDeck {
    pub slides: Vec<SlideEntry>,
}

impl SlideDeck {
    /// Hardcoded 5-slide stub deck for F1.2 testing.
    /// Replaced by real deck loading in F1.4.
    pub fn stub() -> Self { todo!() }

    pub fn get(&self, index: usize) -> Option<&SlideEntry> {
        self.slides.get(index)
    }

    pub fn len(&self) -> usize {
        self.slides.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slides.is_empty()
    }
}

impl SlidePayload {
    /// Build a SlidePayload from a SlideEntry for event bus publishing.
    pub fn from_entry(entry: &SlideEntry, total: u32) -> Self {
        Self {
            index: entry.index,
            total,
            title: entry.title.clone(),
            body: entry.body.clone(),
            notes: entry.notes.clone(),
            background_path: entry.background_path.clone(),
        }
    }
}
```


### SlideStateMachine — NEW file src-tauri/src/slide_state_machine.rs

```rust
// src-tauri/src/slide_state_machine.rs  ← NEW FILE

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::events::AppEvent;
use crate::slide::{SlideDeck, SlidePayload};

pub struct SlideStateMachine {
    deck: SlideDeck,
    current_index: usize,
}

impl SlideStateMachine {
    pub fn new(deck: SlideDeck) -> Self {
        Self { deck, current_index: 0 }
    }

    pub fn current_payload(&self) -> SlidePayload {
        let entry = self.deck.get(self.current_index)
            .expect("current_index always within bounds");
        SlidePayload::from_entry(entry, self.deck.len() as u32)
    }

    pub fn current_index(&self) -> usize { self.current_index }
    pub fn total(&self) -> usize { self.deck.len() }
}

// StateMachineHandle is the Arc<RwLock<>> wrapper stored in Tauri managed state.
// Commands hold a tauri::State<'_, StateMachineHandle>.
pub type StateMachineHandle = Arc<RwLock<SlideStateMachine>>;

pub fn new_handle(deck: SlideDeck) -> StateMachineHandle {
    Arc::new(RwLock::new(SlideStateMachine::new(deck)))
}

/// Advance one slide. Publishes SlideAdvance + SlideUpdate, or DeckEnd if at last slide.
pub async fn advance(
    handle: &StateMachineHandle,
    bus: &Arc<EventBus>,
) -> Result<(), AppError> { todo!() }

/// Rewind one slide. Publishes SlideRewind + SlideUpdate, or DeckStart if at slide 0.
pub async fn rewind(
    handle: &StateMachineHandle,
    bus: &Arc<EventBus>,
) -> Result<(), AppError> { todo!() }

/// Jump to a specific index. Publishes SlideUpdate.
/// Returns AppError::InvalidInput if index is out of range.
pub async fn jump_to(
    index: usize,
    handle: &StateMachineHandle,
    bus: &Arc<EventBus>,
) -> Result<(), AppError> { todo!() }

/// Serialise current index to disk for crash recovery.
/// Path: app_data_dir/slide_state.json — format: { "current_index": N }
pub async fn persist_state(
    handle: &StateMachineHandle,
    data_dir: &std::path::Path,
) -> Result<(), AppError> { todo!() }

/// Load persisted index from disk and apply to the state machine.
/// If file missing or invalid → silently start at index 0 (not an error).
pub async fn restore_state(
    handle: &StateMachineHandle,
    data_dir: &std::path::Path,
) { todo!() }
```


### New Tauri Commands (F1.2)

```rust
// src-tauri/src/commands/slide.rs  ← NEW FILE

// slide_advance:
//   Calls slide_state_machine::advance(handle, bus)
//   Returns Ok(()) — the SlideUpdate/DeckEnd event carries the result to the frontend

// slide_rewind:
//   Calls slide_state_machine::rewind(handle, bus)
//   Returns Ok(()) — the SlideUpdate/DeckStart event carries the result

// slide_jump_to:
//   Accepts { index: usize }
//   Calls slide_state_machine::jump_to(index, handle, bus)
//   Returns AppError::InvalidInput if out of range

// slide_get_current:
//   Returns SlidePayload for the current slide without changing state
//   Used by the controller on initial mount to populate the slide panel

#[tauri::command]
pub async fn slide_advance(
    bus: tauri::State<'_, Arc<crate::bus::EventBus>>,
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<(), AppError> { todo!() }

#[tauri::command]
pub async fn slide_rewind(
    bus: tauri::State<'_, Arc<crate::bus::EventBus>>,
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<(), AppError> { todo!() }

#[tauri::command]
pub async fn slide_jump_to(
    index: usize,
    bus: tauri::State<'_, Arc<crate::bus::EventBus>>,
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<(), AppError> { todo!() }

#[tauri::command]
pub async fn slide_get_current(
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<SlidePayload, AppError> { todo!() }
```


### app.rs — REQUIRED MODIFICATIONS

```rust
// src-tauri/src/app.rs — REQUIRED MODIFICATIONS FOR F1.2:
//
// 1. ADD to generate_handler![]:
//    crate::commands::slide::slide_advance,
//    crate::commands::slide::slide_rewind,
//    crate::commands::slide::slide_jump_to,
//    crate::commands::slide::slide_get_current,
//
// 2. In .setup(), BEFORE spawning run_event_bridge:
//    a. Create stub deck:   let deck = SlideDeck::stub();
//    b. Create handle:      let machine = slide_state_machine::new_handle(deck);
//    c. Restore state:      slide_state_machine::restore_state(&machine, &data_dir).await;
//       NOTE: data_dir = app.path().app_data_dir()?
//    d. Manage the handle:  app.manage(machine);
//
// 3. In presentation_show (commands/presentation.rs):
//    REPLACE the SlidePayload::stub() call with slide_get_current equivalent:
//    Read current payload from StateMachineHandle and publish it.
//    The bus.publish(AppEvent::SlideUpdate(...)) call must use real state, not stub.
//
// 4. DO NOT change Arc<EventBus> ownership, on_window_event, or run_event_bridge.
//
// IMPORTANT: restore_state uses async — call it via tauri::async_runtime::block_on()
// inside .setup() since .setup() is synchronous, OR restructure to spawn it and
// publish the initial SlideUpdate after restoration completes.
// Preferred pattern: spawn a task that restores state then publishes initial SlideUpdate.
```


### lib.rs — ADD module declarations

```rust
// src-tauri/src/lib.rs — ADD two lines only, preserve all existing lines exactly

mod error;
pub mod commands;
pub mod events;
pub mod bus;
pub mod config;
pub mod config_loader;
pub mod slide;
pub mod slide_state_machine;   // ADD

#[cfg(not(test))]
mod app;

#[cfg(not(test))]
pub fn run() {
    app::run();
}
```


### commands/mod.rs — ADD slide module

```rust
// src-tauri/src/commands/mod.rs — ADD one line only

pub mod config;
pub mod info;
pub mod system;
pub mod presentation;
pub mod slide;   // ADD
```


### commands/presentation.rs — REQUIRED MODIFICATION

```rust
// src-tauri/src/commands/presentation.rs — MODIFY presentation_show only
//
// REMOVE:   bus.publish(AppEvent::SlideUpdate(SlidePayload::stub()));
//
// REPLACE WITH:
//   {
//       let machine_read = machine.read().await;
//       bus.publish(AppEvent::SlideUpdate(machine_read.current_payload()));
//   }
//
// This requires adding StateMachineHandle as a parameter to presentation_show:
//   machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
//
// All other logic in presentation_show and presentation_hide is UNCHANGED.
```


### Frontend: Controller SlidePanel

```typescript
// controller/src/lib/ipc.ts — ADD four functions only, do not touch existing functions

export async function slideAdvance(): Promise<void> {
  return invoke<void>('slide_advance');
}

export async function slideRewind(): Promise<void> {
  return invoke<void>('slide_rewind');
}

export async function slideJumpTo(index: number): Promise<void> {
  return invoke<void>('slide_jump_to', { index });
}

export async function slideGetCurrent(): Promise<SlidePayload> {
  return invoke<SlidePayload>('slide_get_current');
}
```

```typescript
// controller/src/lib/events.ts — ADD four variants to AppEvent union only
// Preserve all existing variants and onAppEvent() wrapper exactly

// ADD to AppEvent union:
| { type: 'SlideAdvance'; payload: null }
| { type: 'SlideRewind'; payload: null }
| { type: 'DeckEnd'; payload: null }
| { type: 'DeckStart'; payload: null }
```

```typescript
// controller/src/store/slideStore.ts  ← NEW FILE
// Zustand store for the controller — mirrors the Rust state machine's current slide.
// Extended by F1.3 for thumbnails, speaker notes, timer.

import { create } from 'zustand';
import type { SlidePayload } from '../lib/events';

interface ControllerSlideStore {
  current: SlidePayload | null;
  atDeckStart: boolean;
  atDeckEnd: boolean;
  setSlide: (payload: SlidePayload) => void;
  setAtDeckEnd: (val: boolean) => void;
  setAtDeckStart: (val: boolean) => void;
}

export const useControllerSlideStore = create<ControllerSlideStore>((set) => ({
  current: null,
  atDeckStart: false,
  atDeckEnd: false,
  setSlide: (payload) => set({ current: payload, atDeckEnd: false, atDeckStart: false }),
  setAtDeckEnd: (val) => set({ atDeckEnd: val }),
  setAtDeckStart: (val) => set({ atDeckStart: val }),
}));
```

```typescript
// controller/src/components/SlidePanel.tsx  ← NEW FILE
// Minimal navigation panel for F1.2 — shows current slide info and Prev/Next/Jump buttons.
// Thumbnail grid, speaker notes, and timer are F1.3 scope — do NOT add them here.

// Layout:
//   - Current slide: title (large), body (smaller), slide counter "N / Total"
//   - Next slide preview: title only, labelled "Next:"
//   - Buttons: [← Prev]  [Next →]  and a number input + [Jump] button
//   - DeckEnd banner: shown when atDeckEnd is true — "End of deck"
//   - DeckStart banner: shown when atDeckStart is true — "Beginning of deck"

// State source:
//   - useControllerSlideStore for current/atDeckEnd/atDeckStart
//   - onAppEvent in useEffect: SlideUpdate → setSlide, DeckEnd → setAtDeckEnd(true),
//     DeckStart → setAtDeckStart(true)
//   - On mount: invoke slideGetCurrent() to populate store before first event arrives

// Invoke calls: slideAdvance(), slideRewind(), slideJumpTo(n)
// Error handling: console.error only — full error UI is F1.3 scope
```

```typescript
// controller/src/App.tsx — MODIFY
// ADD import and render <SlidePanel /> below <PresentButton />
// Do NOT remove or modify any existing content
```


### SlidePayload type — ADD to controller/src/lib/ipc.ts

```typescript
// controller/src/lib/ipc.ts — ADD SlidePayload interface
// @sync-with: src-tauri/src/slide.rs SlidePayload

export interface SlidePayload {
  index: number;
  total: number;
  title: string;
  body: string;
  notes: string;
  background_path: string | null;
}
```


### verify.sh — run_f1_2() block to ADD

```bash
run_f1_2() {
  section "F1.2 — Slide State Machine"
  if [ ! -f "src-tauri/src/slide_state_machine.rs" ]; then
    skip "slide_state_machine.rs not created — F1.2 not started yet"
    return
  fi

  # Rust checks
  file_contains "SlideStateMachine struct" "src-tauri/src/slide_state_machine.rs" "SlideStateMachine"
  file_contains "RwLock" "src-tauri/src/slide_state_machine.rs" "RwLock"
  file_contains "SlideDeck stub" "src-tauri/src/slide.rs" "stub"
  file_contains "DeckEnd variant" "src-tauri/src/events.rs" "DeckEnd"
  file_contains "DeckStart variant" "src-tauri/src/events.rs" "DeckStart"
  file_contains "slide_advance command" "src-tauri/src/commands/slide.rs" "slide_advance"
  file_contains "slide_jump_to command" "src-tauri/src/commands/slide.rs" "slide_jump_to"
  file_contains "persist_state" "src-tauri/src/slide_state_machine.rs" "persist_state"
  file_contains "restore_state" "src-tauri/src/slide_state_machine.rs" "restore_state"
  file_not_contains "stub() still in presentation_show" "src-tauri/src/commands/presentation.rs" "SlidePayload::stub"

  # Frontend checks
  file_contains "SlidePanel component" "controller/src/components/SlidePanel.tsx" "SlidePanel"
  file_contains "useControllerSlideStore" "controller/src/store/slideStore.ts" "useControllerSlideStore"
  file_contains "DeckEnd in events.ts" "controller/src/lib/events.ts" "DeckEnd"
  file_contains "slideAdvance in ipc.ts" "controller/src/lib/ipc.ts" "slideAdvance"

  check "F1.2 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide 2>&1 | tee /tmp/sd-f12-test.log \
    && ! grep -q "FAILED" /tmp/sd-f12-test.log'
}

# In the dispatch case block, ADD:
# f1.2)
#   run_invariants; run_f1_1; run_f1_2
#   ;;
# f1|phase1)
#   run_invariants; run_f1_1; run_f1_2; run_core
#   ;;
```


---

## ═══════════════════════════════════════════

## BLOCK 6 — SCOPE BOUNDARY

## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  Thumbnail grid, speaker notes panel, elapsed timer — that is F1.3
  ✗  Keyboard shortcuts for slide navigation in controller — F1.3 scope
  ✗  Slide deck import (.pptx / .pdf / .key) — that is F1.4
  ✗  Multi-display assignment UI — that is F1.5
  ✗  GStreamer, NDI, or media pipeline — Phase 2+
  ✗  background_path image loading from disk — stub null; real loading is F1.4
  ✗  Section names on slides — F1.4 introduces them via imported deck schema
  ✗  AppEvent::SlideAdvance payload data beyond notification — intentionally empty

DO NOT modify:
  ✗  src-tauri/src/bus.rs
  ✗  src-tauri/src/config.rs or config_loader.rs
  ✗  src-tauri/src/commands/system.rs, info.rs, config.rs
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  presentation/src/lib/events.ts (presentation window has no navigation commands)
  ✗  presentation/src/App.tsx or SlideCanvas.tsx
  ✗  presentation/src/store/slideStore.ts
  ✗  controller/src/components/PresentButton.tsx

CRITICAL CONSTRAINTS:
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
  ✗  Do NOT construct Arc from app.state().inner() — yields Arc<&T> not Arc<T>
  ✗  Presentation Window must never call invoke() — pure event listener
  ✗  background_path must remain Option<String> not Option<PathBuf> — PathBuf breaks IPC serde
  ✗  Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy on 1.78
  ✗  Do NOT remove PRESENTING AtomicBool from commands/presentation.rs
  ✗  Do NOT remove Weak<EventBus> pattern in shortcut callback — required to prevent exit hang
  ✗  advance/rewind/jump MUST publish SlideUpdate after every successful state change —
     the Presentation Window has no other way to know the new slide content
  ✗  persist_state MUST be called after every successful state change — not just on app exit
```


---

## ═══════════════════════════════════════════

## BLOCK 7 — OUTPUT CONTRACT

## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
| :-- | :-- | :-- |
| `src-tauri/src/slide.rs` | **MODIFY** | Add `SlideEntry`, `SlideDeck` (with `stub()`), `SlidePayload::from_entry()` |
| `src-tauri/src/slide_state_machine.rs` | **CREATE** | `SlideStateMachine`, `StateMachineHandle`, `advance`, `rewind`, `jump_to`, `persist_state`, `restore_state` |
| `src-tauri/src/events.rs` | **MODIFY** | Add `SlideAdvance`, `SlideRewind`, `DeckEnd`, `DeckStart` variants |
| `src-tauri/src/commands/slide.rs` | **CREATE** | `slide_advance`, `slide_rewind`, `slide_jump_to`, `slide_get_current` |
| `src-tauri/src/commands/mod.rs` | **MODIFY** | Add `pub mod slide;` |
| `src-tauri/src/commands/presentation.rs` | **MODIFY** | Replace `SlidePayload::stub()` with real state read from `StateMachineHandle` |
| `src-tauri/src/app.rs` | **MODIFY** | Add `StateMachineHandle` to managed state, add slide commands to handler, restore state on startup |
| `src-tauri/src/lib.rs` | **MODIFY** | Add `pub mod slide_state_machine;` |
| `controller/src/lib/ipc.ts` | **MODIFY** | Add `SlidePayload` interface, `slideAdvance`, `slideRewind`, `slideJumpTo`, `slideGetCurrent` |
| `controller/src/lib/events.ts` | **MODIFY** | Add `SlideAdvance`, `SlideRewind`, `DeckEnd`, `DeckStart` to `AppEvent` union |
| `controller/src/store/slideStore.ts` | **CREATE** | `useControllerSlideStore` with `current`, `atDeckEnd`, `atDeckStart` |
| `controller/src/components/SlidePanel.tsx` | **CREATE** | Current/next slide display, Prev/Next/Jump buttons, deck boundary banners |
| `controller/src/App.tsx` | **MODIFY** | Add `<SlidePanel />` below `<PresentButton />` |
| `verify.sh` | **MODIFY** | Add `run_f1_2()` function + dispatch cases |

### Tests Required

| File | Test | Class |
| :-- | :-- | :-- |
| `src-tauri/src/slide_state_machine.rs` | `advance_from_last_emits_deck_end` | unit |
| `src-tauri/src/slide_state_machine.rs` | `rewind_from_first_emits_deck_start` | unit |
| `src-tauri/src/slide_state_machine.rs` | `jump_to_valid_index_publishes_slide_update` | unit |
| `src-tauri/src/slide_state_machine.rs` | `jump_to_invalid_index_returns_error` | unit |
| `src-tauri/src/slide_state_machine.rs` | `advance_then_persist_then_restore_preserves_index` | unit |
| `src-tauri/src/slide.rs` | `stub_deck_has_five_slides` | unit |
| `src-tauri/src/slide.rs` | `from_entry_produces_correct_payload` | unit |

### Verification Commands

```bash
cargo test slide              # All SlideStateMachine + SlideDeck unit tests
cargo clippy -- -D warnings   # Zero warnings
npm run typecheck             # Both controller and presentation
bash verify.sh f1.2           # F1.2 AC gate
```


---

## ═══════════════════════════════════════════

## LESSONS LEARNED (F0.x + F1.1 + F1.2 — do not repeat)

## ═══════════════════════════════════════════

```
L1  generate_handler![] requires full paths to defining modules — no pub use re-exports
L2  No manual From<AppError> for InvokeError — Tauri v2 blanket impl covers it
L3  app.rs must stay in its own gated module — #[cfg(not(test))] mod app; in lib.rs
L4  cargo build/clippy require frontend dist/ — cargo test never does
L5  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
L6  notify watcher must be kept alive in managed state — dropping it stops watching silently
L7  #[serde(default)] on every config struct is required for forward-compat migration
L8  Arc<EventBus> must be created before .manage() — never reconstruct from app.state()
L9  Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy::io_other_error
L10 verify.sh file_contains uses raw grep — natural code satisfies checks; no artificial comments
L11 Prefer real acceptance tests in code over hardcoded grep patterns in verify.sh
L12 Empty TOML and v0 TOML behave differently in migration tests — use explicit "version = 0"
L13 PathBuf does not serialise cleanly across Tauri IPC — use String for all IPC-crossing paths
L14 Presentation Window created hidden (visible: false) at startup — hide/show not destroy/recreate
L15 Presentation Window must never call invoke() — capability files grant listen-only access
L16 Global shortcut unregister must NOT be called from inside the shortcut callback —
    use tauri::async_runtime::spawn to defer it, avoiding re-entrant deadlock on plugin mutex
L17 Use Weak<EventBus> (not Arc) in shortcut callback closures — Arc clone keeps Tokio
    runtime alive after app exit, causing cargo tauri dev to hang without exiting
L18 Use app_handle.exit(0) not std::process::exit(0) — the latter skips WebView2 cleanup,
    causing "Failed to unregister class Chrome_WidgetWin_0" on Windows (benign but noisy)
L19 AtomicBool guard on presentation_show/hide prevents double shortcut registration
    and double-hide — both lead to either a deadlock or a no-op crash on Windows
L20 PresentationVisibility event on the bus is the only reliable way to sync button state —
    window focus events and local useState guessing both produce race conditions
```
