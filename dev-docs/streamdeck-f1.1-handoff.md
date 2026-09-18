# StreamDeck — AI Handoff Document
## Feature F1.1 · Presentation Window & Slide Renderer

**Document Type:** AI Session Handoff
**Phase:** Phase 1 — Presentation Engine
**Feature:** F1.1 — Presentation Window & Slide Renderer
**Prerequisite:** F0.1, F0.2, F0.3, F0.4 must be complete and CI green before starting
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
Task    : F1.1 — Presentation Window & Slide Renderer
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
   The Presentation Window MUST receive slide content via AppEvent::SlideUpdate only —
   never via direct IPC invoke from the frontend.

3. CONFIG AS CODE
   Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
   No hard-coded behaviour anywhere.
   WHIP bearer tokens go in OS keychain (keyring crate), NEVER in TOML plaintext.

4. COEXISTENCE FIRST
   New subsystems are additive. Mevo Studio and ProPresenter continue operating.
   The Presentation Window is additive — it does not remove or replace existing windows.

5. OBSERVABILITY FROM DAY ONE
   All errors surface as typed AppEvent variants within 100 ms.
   Command handlers return Result<T, AppError> — never unwrap() or panic in handlers.

6. PRESENTATION WINDOW CONTAINS ZERO BUSINESS LOGIC
   The Presentation Window is a pure renderer. It holds no slide state.
   It only reacts to AppEvent::SlideUpdate events and renders what it receives.
   All slide state lives in Rust (F1.2 onwards). In F1.1 a stub payload is sufficient.
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
        - src-tauri/src/events.rs — AppEvent enum (#[non_exhaustive], serde::Serialize)
            Current variants: BusLag, Ping, ConfigReloaded
        - src-tauri/src/bus.rs — EventBus struct, broadcast::channel(256)
            pub fn new() -> (Self, Receiver<AppEvent>)
            pub fn publish(&self, event: AppEvent)
            pub fn subscribe(&self) -> Receiver<AppEvent>
            NOTE: EventBus is Arc-wrapped before .manage() — do NOT reconstruct Arc from app.state()
        - src-tauri/src/app.rs — Arc<EventBus> managed, run_event_bridge task running
            Uses handle.emit() — NOT emit_all() (removed in Tauri v2)
        - Presentation window currently auto-spawned in .setup() — F1.1 CHANGES THIS (see Block 5)
        - All AC passing CI ✓

F0.4 ✓  TOML Configuration Loader complete
        - src-tauri/src/config.rs — StreamDeckConfig { version: u32, app: AppConfig }
        - src-tauri/src/config_loader.rs — load_or_default(), spawn_watcher()
        - notify v6 hot-reload, keyring for WHIP tokens
        - AppEvent::ConfigReloaded published on file change
        - Config v0→v1 migration via #[serde(default)] on all structs
        - All AC passing CI ✓
```

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F1.1 — Presentation Window & Slide Renderer

**Description:**
The Presentation Window renders slide content (title, body, background colour, slide number)
in a full-screen React canvas. It is hidden at startup — the operator launches it by clicking
"Present" in the Controller Window. Content is pushed from Rust via the event bus. The window
contains zero business logic. A fixed 1920×1080 logical canvas is scaled via CSS transform
to fill any physical display resolution, mirroring how PowerPoint and Google Slides handle
fullscreen projection.

**Window Lifecycle (confirmed):**
- Created ONCE at app startup with visible: false — avoids WebView re-init cost
- "Present" button in Controller calls invoke('presentation_show') → window.show() + emit current slide
- Closing/stopping presentation calls invoke('presentation_hide') → window.hide()
- Window is never destroyed and recreated during a session
- On re-open, Rust re-emits the current SlidePayload so the window renders the correct slide instantly

**CSS Scaling Strategy (confirmed):**
- Inner canvas is a fixed 1920×1080px div — all slide content authored in these logical units
- Outer container is 100vw × 100vh, black background, overflow: hidden, display: flex, centered
- scale factor = min(windowWidth / 1920, windowHeight / 1080)
- Applied as: transform: scale(factor), transform-origin: top left, then centred via margin calc
- On window resize a ResizeObserver (or window resize event) recomputes scale
- This works identically on laptop screen, ClickShare, HDMI direct, and 4K projector

### Technologies

| Technology | Version | Role |
|---|---|---|
| Tauri WebviewWindowBuilder | v2.11.x | Create hidden presentation window at startup |
| Tauri Window API | v2.11.x | show(), hide(), set_fullscreen(), set_focus() |
| `@tauri-apps/api` | 2.x | listen('app-event') in presentation frontend |
| React + TypeScript | 18.x / 5.x | SlideCanvas component, useSlideStore (Zustand) |
| Zustand | 5.x | Frontend slide state store — introduced in F1.1 |
| CSS transform: scale() | — | Resolution-independent canvas scaling |

### Acceptance Criteria

| # | Criterion | Testability Class | Verification |
|---|---|---|---|
| AC1 | Presentation Window is hidden at startup; only becomes visible after invoke('presentation_show') | integration | Manual: launch app, confirm no presentation window; click Present, confirm it appears |
| AC2 | Window opens on a secondary display if one is connected; falls back to floating on primary | integration | Manual: test with and without external display connected |
| AC3 | Slide transitions complete in < 16 ms from event emit to DOM paint, measured via performance.now() | integration | Vitest component test with mocked event + performance.now() delta assertion |
| AC4 | Presentation Window has no visible browser chrome, no scrollbars, no DevTools shortcut in production build | integration | Manual: production build inspection |
| AC5 | Slide content renders correctly at 1920×1080 logical resolution regardless of physical display size | unit | Vitest: mount SlideCanvas at various container sizes, assert canvas scale factor |
| AC6 | Window is crash-isolated: a JS error in Presentation Window does not crash Controller Window | integration | Manual: throw error in presentation, confirm controller remains operational |

### Definition of Done

All six AC verified (unit AC via Vitest, integration AC manually on macOS and Windows) before F1.2 begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Cargo.toml — NO new dependencies for F1.1

F1.1 uses only Tauri window management APIs already present. No new crates needed.
Frontend only: add zustand to both package.json files (controller and presentation).

```
presentation/package.json  — ADD: "zustand": "^5.0.0"
controller/package.json    — ADD: "zustand": "^5.0.0"
```

### AppEvent — ADD one variant to events.rs

```rust
// src-tauri/src/events.rs — ADD SlideUpdate only, preserve all existing variants exactly

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    BusLag { receiver_id: String, dropped: u64 },
    Ping { message: String },
    ConfigReloaded,

    // ADD for F1.1
    SlideUpdate(SlidePayload),
}
```

### SlidePayload — NEW type in src-tauri/src/slide.rs

```rust
// src-tauri/src/slide.rs  ← NEW FILE
// SlidePayload is the only data the Presentation Window ever receives.
// background_path is Option<String> not Option<PathBuf> — PathBuf does not serialise
// cleanly across the Tauri IPC boundary to TypeScript.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidePayload {
    pub index: u32,
    pub total: u32,
    pub title: String,
    pub body: String,
    pub notes: String,
    pub background_path: Option<String>,
}

impl SlidePayload {
    pub fn stub() -> Self {
        Self {
            index: 0,
            total: 1,
            title: "StreamDeck".into(),
            body: "Ready to present".into(),
            notes: String::new(),
            background_path: None,
        }
    }
}
```

### New Tauri Commands (F1.1)

```rust
// src-tauri/src/commands/presentation.rs  ← NEW FILE

// presentation_show:
//   1. Gets the "presentation" window by label
//   2. Calls window.show()
//   3. Calls window.set_fullscreen(true) on the current display
//   4. Publishes AppEvent::SlideUpdate(SlidePayload::stub()) on the bus
//      so the window renders immediately on show (F1.2 will replace stub with real state)
//   Returns AppError::NotFound if the window label "presentation" does not exist.

// presentation_hide:
//   1. Gets the "presentation" window by label
//   2. Calls window.set_fullscreen(false)
//   3. Calls window.hide()
//   Returns AppError::NotFound if the window label does not exist.

#[tauri::command]
pub async fn presentation_show(
    app: tauri::AppHandle,
    bus: tauri::State<'_, std::sync::Arc<crate::bus::EventBus>>,
) -> Result<(), AppError> { todo!() }

#[tauri::command]
pub async fn presentation_hide(
    app: tauri::AppHandle,
) -> Result<(), AppError> { todo!() }
```

### app.rs — REQUIRED MODIFICATIONS

```rust
// src-tauri/src/app.rs — REQUIRED MODIFICATIONS FOR F1.1:
//
// 1. REMOVE the existing WebviewWindowBuilder spawn for the "presentation" window
//    that currently lives inside .setup(). It was auto-launched in F0.3 — F1.1 replaces this.
//
// 2. ADD a new WebviewWindowBuilder call that creates the presentation window with:
//    .visible(false)
//    .decorations(false)
//    .always_on_top(true)
//    .fullscreen(false)           ← starts windowed, set_fullscreen(true) called on show
//    .inner_size(1920.0, 1080.0)  ← logical size before any OS scaling
//    .resizable(false)
//
// 3. ADD presentation_show and presentation_hide to generate_handler![]
//
// 4. DO NOT change the Arc<EventBus> ownership pattern established in F0.4
//
// CLIPPY NOTES (rust-version = "1.78" active):
//   Use std::io::Error::other(msg) NOT Error::new(ErrorKind::Other, msg)
//   Collapse nested if/if → clippy::collapsible_if
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
pub mod slide;        // ADD — SlidePayload type

#[cfg(not(test))]
mod app;
```

### commands/mod.rs — ADD presentation module

```rust
// src-tauri/src/commands/mod.rs — ADD one line only

pub mod config;
pub mod info;
pub mod system;
pub mod presentation;  // ADD
```

### Frontend: Presentation Window

```typescript
// presentation/src/lib/events.ts — ADD SlidePayload type and SlideUpdate handler
// DO NOT modify the existing onAppEvent() wrapper structure

export interface SlidePayload {
  index: number;
  total: number;
  title: string;
  body: string;
  notes: string;
  background_path: string | null;
}

// SlideUpdate is received as: { type: "SlideUpdate", payload: SlidePayload }
// Use the existing onAppEvent() wrapper to listen — never call listen() directly
```

```typescript
// presentation/src/store/slideStore.ts  ← NEW FILE
// Zustand store — introduced in F1.1, extended by F1.2+
// Holds only what the Presentation Window needs to render.

import { create } from 'zustand';
import type { SlidePayload } from '../lib/events';

interface SlideStore {
  current: SlidePayload | null;
  setSlide: (payload: SlidePayload) => void;
}

export const useSlideStore = create<SlideStore>((set) => ({
  current: null,
  setSlide: (payload) => set({ current: payload }),
}));
```

```typescript
// presentation/src/components/SlideCanvas.tsx  ← NEW FILE
// Pure renderer — zero business logic.
// Reads from useSlideStore, renders at 1920×1080 logical units, scales to fill window.

// Scale strategy:
//   outerContainer: width=100vw, height=100vh, background=#000, overflow=hidden,
//                   display=flex, alignItems=center, justifyContent=center
//   innerCanvas:    width=1920px, height=1080px, position=relative,
//                   transform=`scale(${scale})`, transformOrigin='top left'
//   scale computed in a useEffect watching window resize:
//     const scale = Math.min(window.innerWidth / 1920, window.innerHeight / 1080)
//   Inner canvas absolute position centred via:
//     marginLeft = (window.innerWidth - 1920 * scale) / 2
//     marginTop  = (window.innerHeight - 1080 * scale) / 2

// Zones inside the 1920×1080 canvas (CSS Grid or absolute):
//   background zone: position absolute, inset 0, z-index 0  (background image or solid)
//   title zone:      position absolute, top 10%, left 5%, width 90%, z-index 1
//   body zone:       position absolute, top 30%, left 5%, width 90%, height 55%, z-index 1
//   slide number:    position absolute, bottom 3%, right 3%, z-index 2
```

```typescript
// controller/src/components/PresentButton.tsx  ← NEW FILE
// Minimal "Present" / "Stop Presenting" toggle button.
// Calls invoke('presentation_show') or invoke('presentation_hide').
// No slide state here — that belongs to F1.3 Controller Slide Panel.

import { invoke } from '../lib/ipc';

// State: presenting boolean (local useState)
// On click when not presenting: invoke('presentation_show')
// On click when presenting:     invoke('presentation_hide')
// Shows error via console.error on invoke failure — full error UI is F1.3 scope
```

### ipc.ts — ADD two commands to both windows

```typescript
// controller/src/lib/ipc.ts — ADD these two functions only, do not touch existing functions

export async function presentationShow(): Promise<void> {
  return invoke<void>('presentation_show');
}

export async function presentationHide(): Promise<void> {
  return invoke<void>('presentation_hide');
}
```

### verify.sh — run_f1_1() block to ADD

```bash
# ADD this function to verify.sh and add it to the dispatch case block

run_f1_1() {
  section "F1.1 — Presentation Window & Slide Renderer"
  if [ ! -f "src-tauri/src/slide.rs" ]; then
    skip "slide.rs not created — F1.1 not started yet"
    return
  fi

  # Rust checks
  file_contains "SlidePayload struct" "src-tauri/src/slide.rs" "SlidePayload"
  file_contains "background_path Option<String>" "src-tauri/src/slide.rs" "background_path"
  file_contains "SlideUpdate variant" "src-tauri/src/events.rs" "SlideUpdate"
  file_contains "presentation_show command" "src-tauri/src/commands/presentation.rs" "presentation_show"
  file_contains "presentation_hide command" "src-tauri/src/commands/presentation.rs" "presentation_hide"
  file_contains "visible(false) in app.rs" "src-tauri/src/app.rs" "visible"
  file_not_contains "No auto-spawn on setup" "src-tauri/src/app.rs" "presentation.*index.html.*spawn"

  # Frontend checks
  file_contains "SlideCanvas component" "presentation/src/components/SlideCanvas.tsx" "SlideCanvas"
  file_contains "useSlideStore" "presentation/src/store/slideStore.ts" "useSlideStore"
  file_contains "scale" "presentation/src/components/SlideCanvas.tsx" "scale"
  file_contains "PresentButton in controller" "controller/src/components/PresentButton.tsx" "PresentButton"
  file_contains "zustand in presentation" "presentation/package.json" "zustand"
  file_contains "zustand in controller" "controller/package.json" "zustand"

  check "F1.1 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide 2>&1 | tee /tmp/sd-test.log \
    && ! grep -q "FAILED" /tmp/sd-test.log'
}

# In the dispatch case block, ADD:
# f1.1)
#   run_invariants; run_f1_1
#   ;;
# f1|phase1)
#   run_invariants; run_f1_1; run_core
#   ;;
```

---

## ═══════════════════════════════════════════
## BLOCK 6 — SCOPE BOUNDARY
## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  SlideStateMachine — that is F1.2; use SlidePayload::stub() in F1.1
  ✗  Controller slide panel (thumbnail grid, speaker notes, timer) — that is F1.3
  ✗  Slide deck import (.pptx / .pdf) — that is F1.4
  ✗  Multi-display assignment UI — that is F1.5
  ✗  GStreamer, NDI, or media pipeline — Phase 2+
  ✗  AppEvent::SlideAdvance, SlideRewind, DeckEnd, DeckStart — F1.2 variants, not yet
  ✗  Keyboard shortcuts for slide navigation — F1.3 scope
  ✗  background_path image loading from disk — stub null for F1.1; real loading is F1.4

DO NOT modify:
  ✗  src-tauri/src/bus.rs
  ✗  src-tauri/src/config.rs or config_loader.rs
  ✗  src-tauri/src/commands/system.rs, info.rs, config.rs
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  controller/src/lib/events.ts (existing onAppEvent wrapper)
  ✗  presentation/src/lib/events.ts (existing onAppEvent wrapper — only ADD types)
  ✗  controller/src/lib/ipc.ts (only ADD the two new presentation functions)

CRITICAL CONSTRAINTS:
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
  ✗  Do NOT construct Arc from app.state().inner() — yields Arc<&T> not Arc<T>
  ✗  Presentation Window must never call invoke() — it is a pure event listener
  ✗  background_path must be Option<String> not Option<PathBuf> — PathBuf breaks IPC serde
  ✗  Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy fires on 1.78
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/slide.rs` | **CREATE** | `SlidePayload` struct + `stub()`, `#[cfg(test)]` unit tests |
| `src-tauri/src/events.rs` | **MODIFY** | Add `SlideUpdate(SlidePayload)` variant only |
| `src-tauri/src/commands/presentation.rs` | **CREATE** | `presentation_show`, `presentation_hide` |
| `src-tauri/src/commands/mod.rs` | **MODIFY** | Add `pub mod presentation;` |
| `src-tauri/src/app.rs` | **MODIFY** | Remove auto-spawn, add hidden window creation, add commands to handler |
| `src-tauri/src/lib.rs` | **MODIFY** | Add `pub mod slide;` |
| `presentation/src/store/slideStore.ts` | **CREATE** | Zustand store with `current: SlidePayload \| null` |
| `presentation/src/components/SlideCanvas.tsx` | **CREATE** | Fixed 1920×1080 canvas, CSS scale, zone layout |
| `presentation/src/App.tsx` | **MODIFY** | Wire `onAppEvent` → `useSlideStore.setSlide`, render `<SlideCanvas />` |
| `presentation/src/lib/events.ts` | **MODIFY** | ADD `SlidePayload` type and `SlideUpdate` variant type only |
| `controller/src/components/PresentButton.tsx` | **CREATE** | Present / Stop Presenting toggle button |
| `controller/src/App.tsx` | **MODIFY** | Render `<PresentButton />` |
| `controller/src/lib/ipc.ts` | **MODIFY** | ADD `presentationShow()` and `presentationHide()` only |
| `presentation/package.json` | **MODIFY** | Add `"zustand": "^5.0.0"` |
| `controller/package.json` | **MODIFY** | Add `"zustand": "^5.0.0"` |
| `verify.sh` | **MODIFY** | Add `run_f1_1()` function + dispatch cases |

### Tests Required

| File | Test | Class |
|---|---|---|
| `src-tauri/src/slide.rs` | `stub_has_expected_defaults` — stub() index=0, total=1 | unit |
| `src-tauri/src/slide.rs` | `round_trip_serialise_deserialise` — serde JSON round-trip | unit |
| `src-tauri/src/slide.rs` | `background_path_serialises_as_null_when_none` — IPC serde compat | unit |
| `presentation/src/components/SlideCanvas.tsx` | Scale factor computed correctly at various container sizes | Vitest |
| `presentation/src/store/slideStore.ts` | setSlide updates current correctly | Vitest |

### Verification Commands

```bash
cargo test                    # SlidePayload unit tests
cargo clippy -- -D warnings   # zero warnings
npm run typecheck             # both controller and presentation
bash verify.sh f1.1           # F1.1 AC gate
```

---

## ═══════════════════════════════════════════
## LESSONS LEARNED (F0.2 → F0.4 + F1.1 decisions)
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
L10 verify.sh file_contains uses raw grep — natural code satisfies checks; no artificial comments needed
L11 Prefer real acceptance tests in code over hardcoded grep patterns in verify.sh
L12 Empty TOML and v0 TOML behave differently in migration tests — use explicit "version = 0\n"
L13 PathBuf does not serialise cleanly across Tauri IPC to TypeScript — use String for all paths
    in types that cross the IPC boundary (SlidePayload.background_path: Option<String>)
L14 Presentation Window must be created hidden (visible: false) at startup, not auto-shown.
    Destroy/recreate incurs 300-800ms WebView re-init cost — hide/show is the correct pattern.
L15 Presentation Window must never call invoke() — capability files grant listen-only access.
    All slide content arrives exclusively via AppEvent::SlideUpdate on the event bus.
```
