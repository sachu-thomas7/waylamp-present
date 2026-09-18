# StreamDeck — AI Handoff Document
## Feature F1.3 · Controller Slide Panel

**Document Type:** AI Session Handoff
**Phase:** Phase 1 — Presentation Engine
**Feature:** F1.3 — Controller Slide Panel
**Prerequisite:** F0.1, F0.2, F0.3, F0.4, F1.1, F1.2 must be complete and CI green before starting
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
Task    : F1.3 — Controller Slide Panel
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
   Timer state (start timestamp, elapsed) lives in Rust — frontend only renders
   what it receives from AppEvent::TimerTick.

3. CONFIG AS CODE
   Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
   No hard-coded behaviour anywhere.
   WHIP bearer tokens go in OS keychain (keyring crate), NEVER in TOML plaintext.

4. COEXISTENCE FIRST
   New subsystems are additive. Mevo Studio and ProPresenter continue operating.
   F1.3 extends SlidePanel (F1.2) — it does not replace or remove any existing
   commands, window lifecycle behaviour, or bus events.

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

8. SLIDE DECK PANEL IS AN ISOLATED SECTION
   SlideDeckPanel is a self-contained, independently collapsible section in the
   controller UI. It has no layout coupling to PresentButton or any future panel.
   Its resize/collapse state is local UI state only — not persisted in F1.3.
   This isolation is the foundation for future hide/resize/reorder capabilities.
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
                #[error("Config error: {0}")]      Config(String),
                #[error("Keychain error: {0}")]    Keychain(String),
            }
        - Commands: src-tauri/src/commands/mod.rs, info.rs, system.rs, config.rs
        - ipc.ts in controller/src/lib/ and presentation/src/lib/
        - No manual From<AppError> impl — Tauri v2 blanket covers it
        - ping dispatch latency < 2ms verified ✓

F0.3 ✓  Event Bus Foundation complete
        - src-tauri/src/bus.rs — EventBus struct, broadcast::channel(256)
            pub fn new() -> (Self, Receiver<AppEvent>)
            pub fn publish(&self, event: AppEvent)
            pub fn subscribe(&self) -> Receiver<AppEvent>
            NOTE: EventBus is Arc-wrapped before .manage() — do NOT reconstruct Arc from app.state()
        - src-tauri/src/app.rs — Arc<EventBus> managed, run_event_bridge task running
            Uses handle.emit() — NOT emit_all() (removed in Tauri v2)
        - All AC passing CI ✓

F0.4 ✓  TOML Configuration Loader complete
        - src-tauri/src/config.rs — StreamDeckConfig { version: u32, app: AppConfig }
        - src-tauri/src/config_loader.rs — load_or_default(), spawn_watcher()
        - notify v6 hot-reload, keyring for WHIP tokens
        - AppEvent::ConfigReloaded published on file change ✓

F1.1 ✓  Presentation Window & Slide Renderer complete
        - Presentation window created hidden at startup (visible: false) in app.rs
        - presentation_show / presentation_hide commands registered in generate_handler![]
        - Global Escape shortcut registered on show, unregistered on hide
          Unregister spawned via tauri::async_runtime::spawn to avoid re-entrant deadlock
          Weak<EventBus> used in shortcut callback — prevents Arc keeping runtime alive on exit
        - PRESENTING: AtomicBool in commands/presentation.rs guards double-registration
        - app.rs on_window_event(Destroyed on "controller") calls app_handle.exit(0)
        - AppEvent::PresentationVisibility { visible: bool } drives controller button label
        - verify.sh f1.1 passing CI ✓

F1.2 ✓  Slide State Machine complete
        - src-tauri/src/slide.rs — SlideEntry, SlideDeck (stub() with 5 slides + real notes
          added in F1.3 — see stub update below), SlidePayload::from_entry()
        - src-tauri/src/slide_state_machine.rs — SlideStateMachine { deck, current_index,
          data_dir: PathBuf }, StateMachineHandle = Arc<RwLock<SlideStateMachine>>
          advance(), rewind(), jump_to() — each calls persist_index() after state change
          persist_state(), restore_state() — slide_state.json in app_data_dir
        - src-tauri/src/commands/slide.rs — slide_advance, slide_rewind, slide_jump_to,
          slide_get_current
        - src-tauri/src/events.rs — VERIFIED CONTENT:
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
                SlideAdvance,
                SlideRewind,
                DeckEnd,
                DeckStart,
            }
        - src-tauri/src/lib.rs — VERIFIED CONTENT:
            mod error;
            pub mod commands;
            pub mod events;
            pub mod bus;
            pub mod config;
            pub mod config_loader;
            pub mod slide;
            pub mod slide_state_machine;

            #[cfg(not(test))]
            mod app;

            #[cfg(not(test))]
            pub fn run() {
                app::run();
            }
        - src-tauri/src/commands/mod.rs — VERIFIED CONTENT:
            pub mod config;
            pub mod info;
            pub mod system;
            pub mod presentation;
            pub mod slide;
        - src-tauri/src/app.rs — VERIFIED CONTENT (F1.2 additions):
            - SlideDeck::stub() loaded, new_handle(deck, data_dir) created
            - restore_state + initial SlideUpdate published via spawned async task
            - StateMachineHandle managed via app.manage()
            - generate_handler![]: ..., slide_advance, slide_rewind,
              slide_jump_to, slide_get_current
        - controller/src/lib/ipc.ts — VERIFIED CONTENT:
            SlidePayload interface: { index, total, title, body, notes, background_path }
            slideAdvance(), slideRewind(), slideJumpTo(index), slideGetCurrent()
        - controller/src/lib/events.ts — VERIFIED CONTENT (full discriminated union):
            export type AppEvent =
              | { type: "BusLag";                 payload: { receiver_id: string; dropped: number } }
              | { type: "Ping";                   payload: { message: string } }
              | { type: "ConfigReloaded";         payload: null }
              | { type: "SlideUpdate";            payload: SlidePayload }
              | { type: "PresentationVisibility"; payload: { visible: boolean } }
              | { type: "SlideAdvance";           payload: null }
              | { type: "SlideRewind";            payload: null }
              | { type: "DeckEnd";                payload: null }
              | { type: "DeckStart";              payload: null }
        - controller/src/store/slideStore.ts — useControllerSlideStore
            { current: SlidePayload | null, atDeckStart, atDeckEnd,
              setSlide, setAtDeckEnd, atDeckStart }
            Imports SlidePayload from ../lib/ipc (NOT from ../lib/events)
        - controller/src/components/SlidePanel.tsx — minimal nav panel (F1.2 stub,
          replaced in full by SlideDeckPanel in F1.3)
        - verify.sh f1.2 passing CI ✓

        LESSONS LEARNED in F1.2 session (do not repeat):
        L21  AppEvent TypeScript union must be a fully discriminated union from the start —
             every variant needs explicit { type, payload } shape. Appending bare union
             fragments breaks type narrowing in all existing consumers.
        L22  When replacing a stub call, remove the now-unused `use` import immediately —
             clippy -D warnings will fail the build.
        L23  SlideStateMachine must own data_dir: PathBuf at construction time —
             advance/rewind/jump_to cannot accept data_dir as a parameter since they are
             called from command handlers that have no path context.
        L24  DeckEnd/DeckStart are emitted INSTEAD OF SlideUpdate at boundaries, not
             alongside — frontend must not expect a SlideUpdate when a boundary event fires.
```

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F1.3 — Controller Slide Panel

**Description:**
Replace the minimal F1.2 `SlidePanel` with a fully featured `SlideDeckPanel` — an isolated,
collapsible section in the controller UI. It shows: a thumbnail grid of all slides (CSS-scaled
`SlideCanvas` at 160×90), current slide full preview, next slide preview, speaker notes,
slide counter, and a presentation elapsed timer. Keyboard shortcuts (Space / ArrowRight /
ArrowLeft) are handled via a hybrid approach: React `keydown` listeners when the controller
window is focused, and Tauri global shortcuts scoped to the lifetime of `presentation_show`
for when the presentation window steals focus.

The Rust-side timer starts on the first `slide_advance` call, emits
`AppEvent::TimerTick { elapsed_secs: u64 }` every second via a Tokio interval task, persists
its start timestamp in `slide_state.json`, and can be reset via `slide_timer_reset` command.

**Stub deck notes update:**
The 5-slide stub deck in `SlideDeck::stub()` must be updated to include real speaker notes
so the notes panel is exercisable without waiting for F1.4. Update notes fields as follows:
```
Slide 0 notes: "Welcome the audience. Mention the event name and your role."
Slide 1 notes: "Explain the three-layer architecture: Rust core, Tauri bridge, React UI."
Slide 2 notes: "Demo advance and rewind live. Show the boundary events in the log."
Slide 3 notes: "Point to the event log in the controller — every action is visible."
Slide 4 notes: "Preview F1.3 features: thumbnails, notes panel, keyboard shortcuts."
```

**Keyboard shortcut hybrid design:**
- React `keydown` on the controller window (via `useEffect` on `document`):
  Space / ArrowRight → `slideAdvance()`, ArrowLeft → `slideRewind()`
  These fire when controller window has focus.
- Tauri global shortcuts registered in `presentation_show`, unregistered in `presentation_hide`
  (same scoped pattern as Escape in F1.1, same async-spawn unregister pattern):
  Space → slide_advance, ArrowRight → slide_advance, ArrowLeft → slide_rewind
  These fire when the presentation window has focus (single-screen use case).
- Both paths call the same Tauri commands — no duplicate logic.
- `keydown` listener must call `event.preventDefault()` for Space to prevent page scroll.

**Timer behaviour:**
- Timer starts on the FIRST call to `slide_advance` (not on `presentation_show`)
- Start timestamp stored as `timer_started_at: Option<u64>` (Unix ms) in `slide_state.json`
- `AppEvent::TimerTick { elapsed_secs: u64 }` emitted every second from a Tokio interval task
- Timer does NOT reset on rewind — measures total presentation elapsed time
- `slide_timer_reset` command resets start timestamp to now and publishes `TimerTick { elapsed_secs: 0 }`
- Timer task spawned in `.setup()` — if persisted start time exists, ticks resume from
  correct elapsed value immediately
- Timer task exits cleanly when controller window is destroyed (listen for Destroyed event)
- If timer has not started (no first advance yet), TimerTick is NOT emitted — frontend shows "--:--"

### Technologies

| Technology | Version | Role |
|---|---|---|
| `tokio::time::interval` | 1.38 | 1-second tick for timer broadcast |
| `std::time::SystemTime` | stdlib | Unix ms timestamp for timer_started_at |
| React `keydown` event | 18.x | Controller-focused keyboard shortcuts |
| `tauri-plugin-global-shortcut` | 2 (already installed) | Presentation-focused keyboard shortcuts |
| Zustand | 5.0.14 (already in package.json) | slideStore extension for timer + thumbnails |
| CSS `transform: scale()` | - | Thumbnail rendering at 160×90 from full SlideCanvas |

### Acceptance Criteria

| # | Criterion | Testability Class | Verification |
|---|---|---|---|
| AC1 | Thumbnail grid renders all 5 stub slides at 160×90; clicking a thumbnail calls slideJumpTo with correct index | integration | Manual: launch app, verify 5 thumbnails, click thumbnail 3, assert slide 3 shown |
| AC2 | Current slide preview and next slide preview are rendered using the same SlideCanvas component as the Presentation Window, CSS-scaled | structural | Code review: single SlideCanvas component used in both windows and controller |
| AC3 | Speaker notes for the current slide are shown in the notes panel and update on every SlideUpdate event | integration | Manual: advance slides, verify notes text changes |
| AC4 | Elapsed timer starts on first slide_advance, increments every second, does not reset on rewind, shows "--:--" before first advance | integration | Manual: verify "--:--" at start, advance once, watch timer count up, rewind, confirm timer continues |
| AC5 | Space and ArrowRight advance the slide; ArrowLeft rewinds — both when controller window is focused (React keydown) and when presentation window is focused (global shortcut) | integration | Manual: focus controller, press Space; then focus presentation window, press Space |
| AC6 | slide_timer_reset command resets elapsed to 0 and frontend updates immediately | unit + integration | Rust unit test + manual: click Reset, verify timer restarts |
| AC7 | SlideDeckPanel is a self-contained collapsible section; collapse/expand toggle hides/shows the entire panel content without affecting PresentButton or other sections | integration | Manual: toggle collapse, verify isolation |
| AC8 | TimerTick events are emitted at 1-second intervals with correct elapsed_secs value after crash-restore | unit | Rust unit test: persist start time 30s ago, restore, assert first tick >= 30 |

### Definition of Done

All unit AC (AC6 elapsed logic, AC8 timer restore) passing `cargo test`, AC1–AC5, AC7
manually verified on macOS and Windows before F1.4 begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Cargo.toml — NO new dependencies for F1.3

All required crates (tokio, serde_json, std::time) are already present.
No new entries in Cargo.toml.

### slide_state.json — EXTENDED schema

```json
// app_data_dir/slide_state.json — extended for F1.3
// F1.2 format was: { "current_index": N }
// F1.3 format is:  { "current_index": N, "timer_started_at": <unix_ms> | null }
// Backwards compatible: missing timer_started_at treated as null (timer not started)
{
  "current_index": 2,
  "timer_started_at": 1717789200000
}
```

### AppEvent — ADD two variants to events.rs

```rust
// src-tauri/src/events.rs — ADD two variants only, preserve all existing variants exactly

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag { receiver_id: String, dropped: u64 },
    Ping { message: String },
    ConfigReloaded,
    SlideUpdate(SlidePayload),
    PresentationVisibility { visible: bool },
    SlideAdvance,
    SlideRewind,
    DeckEnd,
    DeckStart,
    // ADD for F1.3
    TimerTick { elapsed_secs: u64 },
    TimerReset,
}
```

### SlideState — EXTENDED persistence struct

```rust
// src-tauri/src/slide_state_machine.rs — ADD SlideState struct for JSON persistence
// Replace the inline serde_json::json!() call in persist_index() with this typed struct.
// Backwards compatible: timer_started_at is Option<u64>, missing field deserialises as None.

#[derive(serde::Serialize, serde::Deserialize)]
struct SlideState {
    current_index: usize,
    #[serde(default)]
    timer_started_at: Option<u64>,  // Unix milliseconds; None = timer not yet started
}
```

### SlideStateMachine — EXTENDED with timer fields

```rust
// src-tauri/src/slide_state_machine.rs — ADD two fields to SlideStateMachine struct
// Preserve all existing fields and methods exactly.

pub struct SlideStateMachine {
    deck: SlideDeck,
    current_index: usize,
    data_dir: std::path::PathBuf,
    // ADD for F1.3:
    timer_started_at: Option<u64>,  // Unix ms; set on first advance, never reset by rewind
    timer_task_abort: Option<tokio::task::AbortHandle>,  // held to cancel task on shutdown
}
```

### New function: start_timer_if_needed (internal)

```rust
// src-tauri/src/slide_state_machine.rs — INTERNAL helper, called from advance() only
// Sets timer_started_at if not already set, persists state, spawns tick task.
// Must be idempotent: if timer_started_at is already Some, does nothing.

async fn start_timer_if_needed(
    handle: &StateMachineHandle,
    bus: &Arc<EventBus>,
) -> Result<(), AppError> { todo!() }
```

### New function: spawn_timer_task

```rust
// src-tauri/src/slide_state_machine.rs
// Spawns a Tokio interval task that:
//   1. Reads timer_started_at from the handle every second
//   2. Computes elapsed_secs = (now_unix_ms - timer_started_at) / 1000
//   3. Publishes AppEvent::TimerTick { elapsed_secs } on the bus
//   4. Task exits when AbortHandle is aborted (controller window destroyed)
// Called from app.rs .setup() after restore_state completes.
// If timer_started_at is None after restore, task still runs but emits nothing
// until start_timer_if_needed sets the value.

pub fn spawn_timer_task(
    handle: StateMachineHandle,
    bus: Arc<EventBus>,
) -> tokio::task::AbortHandle { todo!() }
```

### Updated advance() contract

```rust
// advance() must now call start_timer_if_needed() after mutating current_index.
// Sequence:
//   1. Write-lock: increment current_index, capture payload + data_dir
//   2. persist_index (now persists timer_started_at too via SlideState struct)
//   3. start_timer_if_needed (idempotent — only fires on first call)
//   4. bus.publish(AppEvent::SlideAdvance)
//   5. bus.publish(AppEvent::SlideUpdate(payload))
```

### New Tauri Command: slide_timer_reset

```rust
// src-tauri/src/commands/slide.rs — ADD one command only

#[tauri::command]
pub async fn slide_timer_reset(
    bus: tauri::State<'_, Arc<crate::bus::EventBus>>,
    machine: tauri::State<'_, crate::slide_state_machine::StateMachineHandle>,
) -> Result<(), AppError> {
    // Set timer_started_at = now_unix_ms in the state machine
    // Persist updated SlideState to disk
    // Publish AppEvent::TimerReset
    // Publish AppEvent::TimerTick { elapsed_secs: 0 }
    todo!()
}
```

### app.rs — REQUIRED MODIFICATIONS

```rust
// src-tauri/src/app.rs — REQUIRED MODIFICATIONS FOR F1.3

// 1. ADD slide_timer_reset to generate_handler![]:
//    crate::commands::slide::slide_timer_reset,

// 2. In the spawned restore task (already exists from F1.2), AFTER restore_state:
//    let abort = slide_state_machine::spawn_timer_task(
//        Arc::clone(&machine_for_restore),
//        Arc::clone(&bus_for_restore),
//    );
//    Store abort handle in managed state so it can be aborted on shutdown:
//    app_handle.manage(TimerAbortHandle(abort));
//    NOTE: define TimerAbortHandle as a newtype: pub struct TimerAbortHandle(tokio::task::AbortHandle);

// 3. In on_window_event, when controller is Destroyed, BEFORE app_handle.exit(0):
//    if let Ok(abort) = app_handle.try_state::<TimerAbortHandle>() {
//        abort.0.abort();
//    }

// 4. In presentation_show — ADD global shortcut registrations for Space, ArrowRight, ArrowLeft
//    following the EXACT same pattern as Escape (async spawn for unregister, Weak<EventBus>).
//    Shortcuts call slide_advance / slide_rewind commands via bus publish.
//    Space and ArrowRight → publish advance intent; ArrowLeft → publish rewind intent.
//    NOTE: register all three in a single on_shortcut block or three separate registrations —
//    either is fine as long as each uses the async-spawn unregister pattern.

// 5. In presentation_hide — ADD unregister for Space, ArrowRight, ArrowLeft
//    in the same async spawn as Escape unregister (batch them in one spawn).

// DO NOT change Arc<EventBus> ownership, run_event_bridge, or WebviewWindowBuilder.
```

### SlideDeck::stub() — UPDATE notes fields

```rust
// src-tauri/src/slide.rs — UPDATE stub() only, preserve all other content exactly
// Change notes field for all 5 entries:

SlideEntry { index: 0, notes: "Welcome the audience. Mention the event name and your role.".into(), .. }
SlideEntry { index: 1, notes: "Explain the three-layer architecture: Rust core, Tauri bridge, React UI.".into(), .. }
SlideEntry { index: 2, notes: "Demo advance and rewind live. Show the boundary events in the log.".into(), .. }
SlideEntry { index: 3, notes: "Point to the event log in the controller — every action is visible.".into(), .. }
SlideEntry { index: 4, notes: "Preview F1.3 features: thumbnails, notes panel, keyboard shortcuts.".into(), .. }
```

### slideStore — EXTENDED

```typescript
// controller/src/store/slideStore.ts — EXTEND, preserve all existing fields and actions

// ADD to ControllerSlideStore interface:
interface ControllerSlideStore {
  current: SlidePayload | null;
  atDeckStart: boolean;
  atDeckEnd: boolean;
  elapsedSecs: number | null;  // null = timer not started; renders as "--:--"
  isCollapsed: boolean;         // SlideDeckPanel collapse state
  setSlide: (payload: SlidePayload) => void;
  setAtDeckEnd: (val: boolean) => void;
  setAtDeckStart: (val: boolean) => void;
  setElapsedSecs: (secs: number) => void;
  resetTimer: () => void;       // sets elapsedSecs to 0
  toggleCollapse: () => void;
}

// ADD to create() initial state:
//   elapsedSecs: null,
//   isCollapsed: false,

// ADD actions:
//   setElapsedSecs: (secs) => set({ elapsedSecs: secs }),
//   resetTimer: () => set({ elapsedSecs: 0 }),
//   toggleCollapse: () => set((s) => ({ isCollapsed: !s.isCollapsed })),
```

### events.ts — ADD two variants

```typescript
// controller/src/lib/events.ts — ADD two variants to AppEvent union only
// Preserve all existing variants and onAppEvent() wrapper exactly

| { type: 'TimerTick'; payload: { elapsed_secs: number } }
| { type: 'TimerReset'; payload: null }
```

### ipc.ts — ADD slide_timer_reset

```typescript
// controller/src/lib/ipc.ts — ADD one function only

export async function slideTimerReset(): Promise<void> {
  return invoke<void>('slide_timer_reset');
}
```

### SlideCanvas — SHARED component

```typescript
// controller/src/components/SlideCanvas.tsx  ← NEW SHARED FILE
// This component is the single source of truth for slide rendering.
// Used in THREE places:
//   1. presentation/src/App.tsx (or SlideCanvas.tsx) — full 1920×1080
//   2. SlideDeckPanel current preview — CSS scaled to fit panel width
//   3. SlideDeckPanel thumbnail grid — CSS scaled to 160×90
//
// Props:
//   slide: SlidePayload — the slide to render
//   scale?: number — CSS scale factor (default 1.0 for presentation window)
//
// Rendering contract:
//   - Renders at a fixed internal size of 1920×1080 (set via style width/height)
//   - CSS transform: scale(scale) applied to the outer wrapper
//   - Caller controls the wrapper dimensions via the scale prop
//   - background_path: if non-null, render as background-image; if null, solid #1a1a2e
//   - Title: large, top zone; Body: smaller, middle zone; both white text
//   - This component MUST be importable by both controller/ and presentation/ workspaces
//     OR duplicated per workspace if they share no build context.
//
// IMPORTANT: Check whether controller/ and presentation/ share a common src/ or are
// fully separate Vite projects. If separate, duplicate SlideCanvas.tsx in both.
// Do NOT create a shared package — that is Phase 3+ scope.
//
// wrapper style example for 160×90 thumbnail:
//   scale = 160/1920 = 0.0833
//   container: width: 160px; height: 90px; overflow: hidden
//   inner div: width: 1920px; height: 1080px; transform: scale(0.0833); transform-origin: top left

interface SlideCanvasProps {
  slide: SlidePayload;
  scale?: number;
}
```

### SlideDeckPanel — REPLACE SlidePanel

```typescript
// controller/src/components/SlideDeckPanel.tsx  ← NEW FILE (replaces SlidePanel.tsx)
// SlidePanel.tsx is DELETED — all its logic moves here with full features.
//
// Layout (top to bottom inside the panel):
//   ┌─────────────────────────────────────────┐
//   │ [▼ Slide Deck]          [▲ Collapse]    │  ← panel header + collapse toggle
//   ├─────────────────────────────────────────┤
//   │  Thumbnail grid (horizontal scroll)     │  ← 160×90 per slide, active highlighted
//   ├───────────────────┬─────────────────────┤
//   │  Current preview  │  Next preview        │  ← scaled SlideCanvas components
//   │  (larger)         │  (smaller, "Next:")  │
//   ├───────────────────┴─────────────────────┤
//   │  Speaker notes (read-only textarea)     │
//   ├─────────────────────────────────────────┤
//   │  ⏱ 00:02:34   [Reset]   2 / 5          │  ← timer, reset button, slide counter
//   ├─────────────────────────────────────────┤
//   │  [← Prev]  [Next →]  [idx input][Jump] │  ← navigation buttons
//   └─────────────────────────────────────────┘
//
// State source:
//   - useControllerSlideStore for all display state
//   - onAppEvent in useEffect:
//       SlideUpdate  → setSlide
//       DeckEnd      → setAtDeckEnd(true)
//       DeckStart    → setAtDeckStart(true)
//       TimerTick    → setElapsedSecs(payload.elapsed_secs)
//       TimerReset   → resetTimer()
//   - On mount: invoke slideGetCurrent() to populate store
//
// Keyboard shortcuts (React keydown — controller window focus):
//   useEffect on mount: document.addEventListener('keydown', handler)
//   Space / ArrowRight → slideAdvance(); event.preventDefault()
//   ArrowLeft          → slideRewind()
//   Cleanup: document.removeEventListener on unmount
//
// Timer display:
//   elapsedSecs === null → "--:--"
//   elapsedSecs >= 0    → format as "HH:MM:SS" or "MM:SS" if < 1 hour
//
// Collapse behaviour:
//   isCollapsed = true → render only the panel header, hide all content
//   isCollapsed = false → render full panel
//   toggleCollapse() from store
//
// Error handling: console.error only — full error UI is F1.4 scope
// DeckEnd / DeckStart banners: same as F1.2 SlidePanel
```

### App.tsx — UPDATE import

```typescript
// controller/src/App.tsx — MODIFY
// REMOVE: import { SlidePanel } from './components/SlidePanel';
// REMOVE: <SlidePanel />
// ADD:    import { SlideDeckPanel } from './components/SlideDeckPanel';
// ADD:    <SlideDeckPanel /> below <PresentButton />
```

---

## ═══════════════════════════════════════════
## BLOCK 6 — SCOPE BOUNDARY
## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  Slide deck import (.pptx / .pdf / .key) — that is F1.4
  ✗  Multi-display assignment UI — that is F1.5
  ✗  GStreamer, NDI, or media pipeline — Phase 2+
  ✗  background_path image loading from disk — stub null; real loading is F1.4
  ✗  Persistent panel size/position preferences — TOML config integration is F1.5+ scope
  ✗  Drag-to-resize panel — F1.5+ scope
  ✗  Per-slide timer (time spent on each slide) — Phase 6 automation scope
  ✗  Thumbnail generation from anything other than CSS-scaled SlideCanvas — Phase 3+
  ✗  Presentation Window keyboard shortcut handling — presentation window is listen-only

DO NOT modify:
  ✗  src-tauri/src/bus.rs
  ✗  src-tauri/src/config.rs or config_loader.rs
  ✗  src-tauri/src/commands/system.rs, info.rs, config.rs
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  presentation/src/lib/events.ts
  ✗  presentation/src/lib/ipc.ts
  ✗  controller/src/components/PresentButton.tsx
  ✗  src-tauri/src/slide_state_machine.rs advance()/rewind()/jump_to() signatures
  ✗  The existing SlidePayload struct in slide.rs

CRITICAL CONSTRAINTS:
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
  ✗  Do NOT construct Arc from app.state().inner()
  ✗  Presentation Window must never call invoke() — pure event listener
  ✗  background_path must remain Option<String> not Option<PathBuf> — PathBuf breaks IPC serde
  ✗  Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy on 1.78
  ✗  Do NOT remove PRESENTING AtomicBool from commands/presentation.rs
  ✗  Do NOT remove Weak<EventBus> pattern in shortcut callback — prevents exit hang
  ✗  Global shortcuts for Space/ArrowRight/ArrowLeft must use async-spawn unregister
     pattern — same as Escape in F1.1. Do NOT unregister from inside the shortcut callback.
  ✗  SlideCanvas must render at fixed 1920×1080 internally and scale via CSS transform only —
     do not change internal dimensions per use-site
  ✗  Timer task AbortHandle must be stored in managed state — dropping it does not abort the task,
     but losing the handle means it cannot be cancelled on shutdown
  ✗  Do not emit TimerTick before first advance — frontend shows "--:--" until timer starts
  ✗  SlidePanel.tsx must be deleted — SlideDeckPanel.tsx is its full replacement
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
| :-- | :-- | :-- |
| `src-tauri/src/slide.rs` | **MODIFY** | Update `stub()` notes fields only |
| `src-tauri/src/slide_state_machine.rs` | **MODIFY** | Add `timer_started_at`, `timer_task_abort` fields; `SlideState` struct; `start_timer_if_needed()`; `spawn_timer_task()`; update `advance()`, `persist_index()`, `restore_state()` |
| `src-tauri/src/events.rs` | **MODIFY** | Add `TimerTick { elapsed_secs: u64 }` and `TimerReset` variants |
| `src-tauri/src/commands/slide.rs` | **MODIFY** | Add `slide_timer_reset` command |
| `src-tauri/src/app.rs` | **MODIFY** | Add timer spawn, `TimerAbortHandle` newtype, abort on shutdown, global shortcuts for Space/ArrowRight/ArrowLeft in `presentation_show`/`presentation_hide` |
| `src-tauri/src/lib.rs` | **NO CHANGE** | Already correct from F1.2 |
| `src-tauri/src/commands/mod.rs` | **NO CHANGE** | Already correct from F1.2 |
| `controller/src/lib/events.ts` | **MODIFY** | Add `TimerTick` and `TimerReset` variants to AppEvent union |
| `controller/src/lib/ipc.ts` | **MODIFY** | Add `slideTimerReset()` |
| `controller/src/store/slideStore.ts` | **MODIFY** | Add `elapsedSecs`, `isCollapsed`, `setElapsedSecs`, `resetTimer`, `toggleCollapse` |
| `controller/src/components/SlideCanvas.tsx` | **CREATE** | Shared slide renderer, scale prop, CSS transform |
| `controller/src/components/SlideDeckPanel.tsx` | **CREATE** | Full panel: thumbnails, previews, notes, timer, nav buttons, collapse |
| `controller/src/components/SlidePanel.tsx` | **DELETE** | Replaced by SlideDeckPanel |
| `controller/src/App.tsx` | **MODIFY** | Replace `<SlidePanel />` with `<SlideDeckPanel />` |
| `verify.sh` | **MODIFY** | Add `run_f1_3()` function + dispatch cases |

### Tests Required

| File | Test | Class |
| :-- | :-- | :-- |
| `src-tauri/src/slide_state_machine.rs` | `timer_starts_on_first_advance_only` | unit |
| `src-tauri/src/slide_state_machine.rs` | `timer_does_not_reset_on_rewind` | unit |
| `src-tauri/src/slide_state_machine.rs` | `timer_reset_command_sets_new_start_time` | unit |
| `src-tauri/src/slide_state_machine.rs` | `timer_persists_and_restores_correctly` | unit |
| `src-tauri/src/slide_state_machine.rs` | `timer_tick_emits_correct_elapsed_after_restore` | unit |
| `src-tauri/src/slide.rs` | `stub_deck_notes_are_non_empty` | unit |

### verify.sh — run_f1_3() checks decoded

| verify.sh label | File | grep pattern | Natural code that satisfies it |
|---|---|---|---|
| TimerTick variant | `src-tauri/src/events.rs` | `TimerTick` | variant name |
| TimerReset variant | `src-tauri/src/events.rs` | `TimerReset` | variant name |
| timer_started_at field | `src-tauri/src/slide_state_machine.rs` | `timer_started_at` | field name |
| spawn_timer_task fn | `src-tauri/src/slide_state_machine.rs` | `spawn_timer_task` | fn name |
| slide_timer_reset cmd | `src-tauri/src/commands/slide.rs` | `slide_timer_reset` | fn name |
| SlideCanvas component | `controller/src/components/SlideCanvas.tsx` | `SlideCanvas` | component name |
| SlideDeckPanel component | `controller/src/components/SlideDeckPanel.tsx` | `SlideDeckPanel` | component name |
| SlidePanel deleted | `controller/src/components/SlidePanel.tsx` | (file must NOT exist) | file_not_exists check |
| TimerTick in events.ts | `controller/src/lib/events.ts` | `TimerTick` | union variant |
| isCollapsed in store | `controller/src/store/slideStore.ts` | `isCollapsed` | field name |
| keydown listener | `controller/src/components/SlideDeckPanel.tsx` | `keydown` | event listener |

```bash
run_f1_3() {
  section "F1.3 — Controller Slide Panel"
  if [ ! -f "controller/src/components/SlideDeckPanel.tsx" ]; then
    skip "SlideDeckPanel.tsx not created — F1.3 not started yet"
    return
  fi

  # Rust checks
  file_contains "TimerTick variant"       "src-tauri/src/events.rs"                    "TimerTick"
  file_contains "TimerReset variant"      "src-tauri/src/events.rs"                    "TimerReset"
  file_contains "timer_started_at"        "src-tauri/src/slide_state_machine.rs"       "timer_started_at"
  file_contains "spawn_timer_task"        "src-tauri/src/slide_state_machine.rs"       "spawn_timer_task"
  file_contains "slide_timer_reset cmd"   "src-tauri/src/commands/slide.rs"            "slide_timer_reset"
  file_contains "stub notes non-empty"    "src-tauri/src/slide.rs"                     "Welcome the audience"

  # Frontend checks
  file_contains "SlideCanvas"             "controller/src/components/SlideCanvas.tsx"  "SlideCanvas"
  file_contains "SlideDeckPanel"          "controller/src/components/SlideDeckPanel.tsx" "SlideDeckPanel"
  file_contains "TimerTick events.ts"     "controller/src/lib/events.ts"               "TimerTick"
  file_contains "isCollapsed store"       "controller/src/store/slideStore.ts"         "isCollapsed"
  file_contains "keydown listener"        "controller/src/components/SlideDeckPanel.tsx" "keydown"
  file_contains "slideTimerReset ipc"     "controller/src/lib/ipc.ts"                  "slideTimerReset"
  file_not_exists "SlidePanel deleted"    "controller/src/components/SlidePanel.tsx"

  # Unit tests
  file_contains "timer_starts_on_first_advance_only test"  "src-tauri/src/slide_state_machine.rs" "timer_starts_on_first_advance_only"
  file_contains "timer_does_not_reset_on_rewind test"      "src-tauri/src/slide_state_machine.rs" "timer_does_not_reset_on_rewind"
  file_contains "timer_reset_command test"                  "src-tauri/src/slide_state_machine.rs" "timer_reset_command_sets_new_start_time"
  file_contains "timer_persists_restores test"              "src-tauri/src/slide_state_machine.rs" "timer_persists_and_restores_correctly"
  file_contains "stub_notes_non_empty test"                 "src-tauri/src/slide.rs"               "stub_deck_notes_are_non_empty"

  check "F1.3 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide 2>&1 | tee /tmp/sd-f13-test.log \
    && ! grep -q "FAILED" /tmp/sd-f13-test.log'
}

# In the dispatch case block, ADD:
# f1.3)
#   run_invariants; run_f1_1; run_f1_2; run_f1_3
#   ;;
# f1|phase1)
#   run_invariants; run_f1_1; run_f1_2; run_f1_3; run_core
#   ;;
```

### Verification Commands

```bash
cargo test slide              # All timer + state machine unit tests
cargo clippy -- -D warnings   # Zero warnings
npm run typecheck             # Both controller and presentation
bash verify.sh f1.3           # F1.3 AC gate
```

---

## ═══════════════════════════════════════════
## LESSONS LEARNED (F0.x + F1.x — do not repeat)
## ═══════════════════════════════════════════

```
L1   generate_handler![] requires full paths to defining modules — no pub use re-exports
L2   No manual From<AppError> for InvokeError — Tauri v2 blanket impl covers it
L3   app.rs must stay in its own gated module — #[cfg(not(test))] mod app; in lib.rs
L4   cargo build/clippy require frontend dist/ — cargo test never does
L5   Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
L6   notify watcher must be kept alive in managed state — dropping it stops watching silently
L7   #[serde(default)] on every config struct is required for forward-compat migration
L8   Arc<EventBus> must be created before .manage() — never reconstruct from app.state()
L9   Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy::io_other_error
L10  verify.sh file_contains uses raw grep — natural code satisfies checks; no artificial comments
L11  Prefer real acceptance tests in code over hardcoded grep patterns in verify.sh
L12  Empty TOML and v0 TOML behave differently in migration tests — use explicit "version = 0"
L13  PathBuf does not serialise cleanly across Tauri IPC — use String for all IPC-crossing paths
L14  Presentation Window created hidden (visible: false) at startup — hide/show not destroy/recreate
L15  Presentation Window must never call invoke() — capability files grant listen-only access
L16  Global shortcut unregister must NOT be called from inside the shortcut callback —
     use tauri::async_runtime::spawn to defer it, avoiding re-entrant deadlock on plugin mutex
L17  Use Weak<EventBus> (not Arc) in shortcut callback closures — Arc clone keeps Tokio
     runtime alive after app exit, causing cargo tauri dev to hang without exiting
L18  Use app_handle.exit(0) not std::process::exit(0) — skips WebView2 cleanup on Windows
L19  AtomicBool guard on presentation_show/hide prevents double shortcut registration
L20  PresentationVisibility event on the bus is the only reliable way to sync button state
L21  AppEvent TypeScript union must be a fully discriminated union — every variant needs
     explicit { type, payload } shape; appending bare fragments breaks narrowing everywhere
L22  When replacing a stub call, remove the now-unused `use` import — clippy -D warnings fails
L23  SlideStateMachine must own data_dir: PathBuf at construction — free functions cannot
     accept path context since command handlers have none
L24  DeckEnd/DeckStart are emitted INSTEAD OF SlideUpdate at boundaries — frontend must not
     expect a SlideUpdate when a boundary event fires
L25  AbortHandle for background Tokio tasks must be stored in managed state — losing the
     handle means the task cannot be cancelled on shutdown, causing ghost ticks after exit
L26  SlideCanvas must render at fixed 1920×1080 internally; scale via CSS transform: scale()
     only — changing internal dimensions per use-site breaks pixel-identical rendering
L27  Timer task must NOT emit TimerTick before first advance — check timer_started_at is Some
     before computing and publishing elapsed; frontend renders "--:--" until first tick arrives
L28  Global shortcuts for navigation (Space/Arrow) follow the same scoped lifetime pattern
     as Escape: registered in presentation_show, unregistered in presentation_hide via
     async-spawn to avoid re-entrant deadlock. Do NOT register them at app startup.
```
