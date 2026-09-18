# StreamDeck — AI Handoff Document
## Feature F1.4 · Slide Deck Import (PPTX / PDF)

**Document Type:** AI Session Handoff
**Phase:** Phase 1 — Presentation Engine
**Feature:** F1.4 — Slide Deck Import
**Prerequisite:** F0.1, F0.2, F0.3, F0.4, F1.1, F1.2, F1.3 must be complete and CI green before starting
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
Task    : F1.4 — Slide Deck Import (PPTX / PDF)

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
Deck import progress and completion MUST flow through the bus.
SlideStateMachine remains the single source of truth for active slide state.
3. CONFIG AS CODE
Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
No hard-coded behaviour anywhere.
WHIP bearer tokens go in OS keychain (keyring crate), NEVER in TOML plaintext.
4. COEXISTENCE FIRST
New subsystems are additive. Mevo Studio and ProPresenter continue operating.
F1.4 replaces SlideDeck::stub() with real imported decks — it does NOT remove stub()
(stub() remains for tests). All existing commands, bus events, and window lifecycle
behaviour from F1.1–F1.3 are preserved exactly.
5. OBSERVABILITY FROM DAY ONE
All errors surface as typed AppEvent variants within 100 ms.
Command handlers return Result<T, AppError> — never unwrap() or panic in handlers.
Import failures surface per-slide via DeckImportFailed — not as a single fatal error.
6. PRESENTATION WINDOW CONTAINS ZERO BUSINESS LOGIC
The Presentation Window remains a pure renderer. It holds no slide state.
It only reacts to AppEvent::SlideUpdate events. All navigation logic lives
exclusively in SlideStateMachine on the Rust side.
7. STATE MACHINE IS THE SINGLE SOURCE OF TRUTH
Both the Controller and Presentation windows read slide state only from
AppEvent::SlideUpdate published by the state machine.
No frontend component stores a slide index independently.
8. FRONTEND NEVER TOUCHES THE FILESYSTEM DIRECTLY
The file path returned by tauri-plugin-dialog is passed to a Rust command via invoke().
Rust reads, parses, and caches all file content. The frontend receives only serialised
SlidePayload data via the event bus — never raw file bytes or OS paths.
9. DECK PERSISTENCE IS SELF-CONTAINED (Option B — confirmed)
On import, the full SlideDeck is serialised to JSON in app_data_dir/decks/{deck_id}/.
The original source file path is NOT stored or re-read on restart.
This makes the deck resilient to USB drives being unplugged or files being moved.
Background images are copied into app_data_dir/decks/{deck_id}/media/ at import time.
10. SCENE MEDIA PANEL IS AN ISOLATED SECTION
SceneMediaPanel is a self-contained, independently collapsible section in the controller
UI. It has no layout coupling to SlideDeckPanel, PresentButton, or any future panel.
All state flows from the event bus — no prop drilling between panels.
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
- app.rs on_window_event(Destroyed on "controller") calls app_handle.exit(0)
- AppEvent::PresentationVisibility { visible: bool } drives controller button label
- verify.sh f1.1 passing CI ✓

F1.2 ✓  Slide State Machine complete
- src-tauri/src/slide.rs — SlideEntry, SlideDeck (stub() with 5 slides + real notes),
SlidePayload::from_entry()
- src-tauri/src/slide_state_machine.rs — SlideStateMachine { deck, current_index,
data_dir: PathBuf, timer_started_at: Option<u64> }
StateMachineHandle = Arc<RwLock<SlideStateMachine>>
advance(), rewind(), jump_to(), reset_timer() — each calls persist_index()
persist_state(), restore_state() — slide_state.json in app_data_dir
spawn_timer_task() — returns AbortHandle, stored as TimerAbortHandle in managed state
- src-tauri/src/commands/slide.rs — slide_advance, slide_rewind, slide_jump_to,
slide_get_current, slide_timer_reset
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
                TimerTick { elapsed_secs: u64 },
                TimerReset,
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
            pub fn run() { app::run(); }
        - src-tauri/src/commands/mod.rs — VERIFIED CONTENT:
            pub mod config;
            pub mod info;
            pub mod system;
            pub mod presentation;
            pub mod slide;
        - controller/src/lib/ipc.ts — VERIFIED CONTENT:
            SlidePayload interface: { index, total, title, body, notes, background_path }
            slideAdvance(), slideRewind(), slideJumpTo(index), slideGetCurrent(),
            slideTimerReset()
        - controller/src/lib/events.ts — VERIFIED CONTENT (full discriminated union):
            export type AppEvent =
              | { type: 'BusLag';                 payload: { receiver_id: string; dropped: number } }
              | { type: 'Ping';                   payload: { message: string } }
              | { type: 'ConfigReloaded';         payload: null }
              | { type: 'SlideUpdate';            payload: SlidePayload }
              | { type: 'PresentationVisibility'; payload: { visible: boolean } }
              | { type: 'SlideAdvance';           payload: null }
              | { type: 'SlideRewind';            payload: null }
              | { type: 'DeckEnd';                payload: null }
              | { type: 'DeckStart';              payload: null }
              | { type: 'TimerTick';              payload: { elapsed_secs: number } }
              | { type: 'TimerReset';             payload: null }
        - controller/src/store/slideStore.ts — useControllerSlideStore
            { current, atDeckStart, atDeckEnd, elapsedSecs, isCollapsed,
              setSlide, setAtDeckEnd, setAtDeckStart, setElapsedSecs,
              resetTimer, toggleCollapse }
        - controller/src/components/SlideDeckPanel.tsx — full panel with thumbnails,
          previews, notes, timer, nav buttons, collapse toggle ✓
        - controller/src/components/SlideCanvas.tsx — prop-driven, scale prop,
          CSS transform-based rendering at fixed 1920×1080 ✓
        - presentation/src/components/SlideCanvas.tsx — store-driven, ResizeObserver
          layout, zero props — DO NOT MODIFY ✓
        - verify.sh f1.2 + f1.3 passing CI ✓
    
        LESSONS LEARNED in F1.3 session (do not repeat):
        L31  timer_task_abort field on SlideStateMachine struct causes clippy dead_code —
             AbortHandle is managed exclusively via TimerAbortHandle newtype in app.rs.
             Do not add abort handles to SlideStateMachine struct fields.
        L32  start_timer_if_needed() must NOT take a bus parameter — it only sets
             timer_started_at and persists. Publishing is the caller's (advance()) job.
        L33  controller/ and presentation/ are fully separate Vite projects with separate
             package.json and build contexts. SlideCanvas.tsx must be duplicated per workspace.
             Do NOT create a shared package — that is Phase 3+ scope.
        L34  Tauri global shortcut callbacks cannot directly await async functions.
             Use tauri::async_runtime::spawn inside the callback to call command functions.
        L35  Unregistering global shortcuts must use async-spawn pattern — same as Escape
             in F1.1. Never unregister from inside the shortcut callback itself.
        L36  AV software triggering VHO:Trojan-PSW heuristic on Windows is a false positive
             caused by keyring (OS keychain access) + freshly compiled unsigned binary.
             Resolution: add project folder + cargo/target dirs to AV exclusions during dev;
             code-sign the binary for distribution builds.
    ```

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F1.4 — Slide Deck Import (PPTX / PDF)

**Description:**
Replace `SlideDeck::stub()` as the active deck source with real imported slide decks.
Operators select a `.pptx` or `.pdf` file via the system file picker (tauri-plugin-dialog).
The Rust backend parses the file, extracts per-slide data (title, body as plain text,
rich text runs, speaker notes, background images), and serialises the full deck to
`app_data_dir/decks/{deck_id}/` (Option B persistence — self-contained, no original
file path dependency). The imported deck loads into `SlideStateMachine` and publishes
an initial `SlideUpdate`. Import progress is emitted per-slide via `DeckImportProgress`.

Additionally, F1.4 introduces the **Scene Media Panel** — a second independent panel in
the controller UI that allows operators to push ad-hoc images or video files to the
Presentation Window independently from the active slide deck. A `SceneSource` toggle
controls which track (slide deck or scene media) the Presentation Window displays.

**File formats confirmed for F1.4:**
- `.pptx` — parsed via `zip` + `quick-xml` crates (pure Rust, no LibreOffice dependency)
- `.pdf` — rasterized via `pdfium-render` crate (PDFium shared library bundled with installer)
- `.ppt` (legacy binary) — NOT supported, deferred indefinitely
- `.key` (Keynote) — NOT supported, deferred to Phase 5+

**Deck persistence model (Option B — confirmed):**
- On import: full `SlideDeck` serialised to `app_data_dir/decks/{deck_id}/deck.json`
- Background images copied to `app_data_dir/decks/{deck_id}/media/slide_{n}_bg.{ext}`
- On app restart: `deck_restore` reads from `app_data_dir/decks/active_deck_id.txt`
- Original source file path is NOT stored and NOT required after import

**PPTX text extraction depth:**
- Per-slide, per-shape text is extracted at the **run level** (captures mixed formatting)
- `SlideEntry` gains `rich_body: Vec<SlideTextBlock>` alongside the existing plain `body: String`
- Plain `body` = all bullet text joined with `\n` (consumed by F3.4 classifier)
- `rich_body` = structured runs with font, size, color, bold, italic (consumed by F5.1 renderer)
- `word_count: u32` and `has_background_image: bool` pre-computed at import (consumed by F3.4)

**Background image extraction rule (confirmed):**
- Extract ONLY for slides with a non-default background (embedded PNG/JPEG in the PPTX media folder)
- Solid color fills → `background_path: null`, no file written
- Gradient/theme backgrounds → `background_path: null` in F1.4 (F3.4+ scope)
- PDF pages → always rasterized at 1920×1080 PNG (PDF is always visual)

**Scene media (F1.4 scope):**
- Images: PNG, JPEG, WebP — max 10 MB — copied to `app_data_dir/scene_media/`
- Video: MP4 (H.264), WebM (VP8/VP9) — max 500 MB — path-referenced, NOT copied
- `SceneSource` toggle: `SlideDeck` | `SceneMedia` — controls Presentation Window output
- GStreamer compositor integration is Phase 3 — in F1.4 the Presentation Window switches
  between rendering `SlidePayload` and `SceneMediaPayload` via React conditional rendering

**Code slide detection:**
- Detected heuristically: monospace font family in any `SlideTextRun`
- Families: "Courier New", "Consolas", "Monaco", "Fira Code", "JetBrains Mono", "Source Code Pro"
- Detection result stored as `is_code_slide: bool` on `SlideEntry` for F3.4 classifier use
- F1.4 does not change rendering for code slides — flag only; F3.4 acts on it

### Technologies

| Technology | Version | Role |
|---|---|---|
| `zip` crate | 2.x | Read PPTX ZIP archive |
| `quick-xml` crate | 0.36.x | Parse OOXML slide XML (zero-copy, namespace-aware) |
| `pdfium-render` crate | 0.8.x | PDF rasterization + text extraction (PDFium engine) |
| `tauri-plugin-dialog` | 2 | System file picker for PPTX/PDF selection |
| `uuid` crate | 1.x | Generate stable `deck_id` per imported deck |
| `std::fs` | stdlib | Copy background images to app cache directory |
| Zustand | 5.0.14 (already present) | `useSceneStore` for scene media panel state |
| React conditional render | 18.x | Switch Presentation Window between slide and scene media |

### Acceptance Criteria

| # | Criterion | Testability Class | Verification |
|---|---|---|---|
| AC1 | Operator clicks "Import Deck", system file picker opens filtered to .pptx and .pdf; selecting a file triggers import | integration | Manual: click button, verify picker opens, select file, verify import starts |
| AC2 | A 50-slide .pptx imports in < 10 seconds on a mid-range laptop; DeckImportProgress events emitted per slide | integration | Manual: import 50-slide deck, verify progress updates and completion < 10s |
| AC3 | After import, thumbnail grid in SlideDeckPanel updates to show all imported slides | integration | Manual: import deck, verify correct slide count and titles in thumbnails |
| AC4 | Title, body (plain text), and speaker notes are correctly extracted from PPTX standard text placeholders | unit + integration | Rust unit test with fixture .pptx + manual verification |
| AC5 | Background images are extracted only for slides with embedded images; solid-fill slides have background_path null | unit | Rust unit test with fixture .pptx containing mixed slide backgrounds |
| AC6 | Imported deck persists across app restart without requiring the original file (Option B) | integration | Manual: import deck, quit app, relaunch, verify deck loads from app_data_dir |
| AC7 | PDF import rasterizes each page at 1920×1080 PNG; title = first line ≤ 60 chars; notes = empty | unit | Rust unit test with fixture .pdf |
| AC8 | Slides with unsupported PPTX layouts import with blank body and layout_unsupported: true visible in controller | integration | Manual: import .pptx with custom master, verify flag shown in controller |
| AC9 | Import errors surface per-slide as DeckImportFailed events listing which slides failed; import continues for remaining slides | integration | Manual: import .pptx with one corrupt slide, verify partial import + error shown |
| AC10 | Operator can push an image via SceneMediaPanel; Presentation Window switches to show the image when SceneSource = SceneMedia | integration | Manual: push image, toggle SceneSource, verify Presentation Window switches |
| AC11 | Operator can push a video file (MP4/WebM); Presentation Window plays it via HTML5 video element when SceneSource = SceneMedia | integration | Manual: push video, verify playback in Presentation Window |
| AC12 | SceneSource toggle resets to SlideDeck when a new deck is imported | integration | Manual: set SceneMedia active, import new deck, verify toggle resets |
| AC13 | word_count and has_background_image fields are present and correct on all imported SlideEntry records | unit | Rust unit test with fixture .pptx |

### Definition of Done

All unit AC (AC4, AC5, AC7, AC13) passing `cargo test`, AC1–AC3, AC6, AC8–AC12
manually verified on macOS and Windows before F1.5 begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Cargo.toml — ADD four new dependencies

```toml
# src-tauri/Cargo.toml — ADD for F1.4 only; preserve all existing entries exactly
zip             = "2"
quick-xml       = { version = "0.36", features = ["serialize"] }
pdfium-render   = { version = "0.8", features = ["sync"] }
uuid            = { version = "1", features = ["v4"] }
tauri-plugin-dialog = "2"
```


### capabilities/default.json — ADD dialog permission

```json
// src-tauri/capabilities/default.json — ADD one permission only
{
  "permissions": [
    "dialog:allow-open"
  ]
}
```


### SlideEntry — EXTENDED for F1.4

```rust
// src-tauri/src/slide.rs — EXTEND SlideEntry struct only
// Preserve SlidePayload, SlideDeck, stub(), from_entry() exactly.
// ADD fields to SlideEntry:

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideEntry {
    pub index: u32,
    pub title: String,
    pub body: String,                      // plain text, bullets joined with \n
    pub notes: String,
    ```
    pub background_path: Option<String>,   // Option<String> NOT Option<PathBuf> — IPC serde
    ```
    // ADD for F1.4:
    pub rich_body: Vec<SlideTextBlock>,    // structured runs; empty vec for stub() slides
    pub word_count: u32,                   // pre-computed at import; 0 for stub()
    pub has_background_image: bool,        // pre-computed at import; false for stub()
    pub is_code_slide: bool,               // heuristic; false for stub()
    pub layout_unsupported: bool,          // true if slide used custom master
}
```


### SlideTextBlock and SlideTextRun — NEW types

```rust
// src-tauri/src/slide.rs — ADD below SlideEntry, before SlideDeck

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideTextBlock {
    pub runs: Vec<SlideTextRun>,
    pub alignment: TextAlignment,
    pub indent_level: u8,    // 0 = title/heading, 1–5 = bullet depth
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideTextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub font_size_pt: Option<u32>,    // None = inherited from slide theme
    pub color_hex: Option<String>,    // None = inherited; format: "RRGGBB" (no #)
    pub font_family: Option<String>,  // None = theme default font
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}
```


### SlideDeck — ADD load_from_json / save_to_json

```rust
// src-tauri/src/slide.rs — ADD to SlideDeck impl
// stub() is preserved exactly and still used in all tests.

impl SlideDeck {
    pub fn stub() -> Self { /* unchanged */ }

    /// Deserialise a deck previously saved by save_to_json().
    pub fn load_from_json(path: &std::path::Path) -> Result<Self, crate::error::AppError> {
        todo!()
    }

    /// Serialise the full deck to JSON at the given path.
    /// Creates parent directories if they do not exist.
    pub fn save_to_json(&self, path: &std::path::Path) -> Result<(), crate::error::AppError> {
        todo!()
    }
}
```


### AppEvent — ADD five variants to events.rs

```rust
// src-tauri/src/events.rs — ADD five variants only, preserve all existing variants exactly

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
#[non_exhaustive]
pub enum AppEvent {
    // ... all existing variants preserved ...
    // ADD for F1.4:
    DeckImportProgress { slide: u32, total: u32 },
    DeckImportComplete { deck_id: String },
    DeckImportFailed { slide: u32, reason: String },  // per-slide; import continues
    SceneMediaUpdate(SceneMediaPayload),
    SceneSourceChanged { source: SceneSource },
}
```


### SceneMediaPayload and SceneSource — NEW types

```rust
// src-tauri/src/scene.rs  ← NEW FILE

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SceneSource {
    SlideDeck,
    SceneMedia,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SceneMediaPayload {
    pub media_type: SceneMediaType,
    pub path: String,            // absolute path in app_data_dir (images) or original (video)
    pub display_name: String,    // filename shown in controller UI
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SceneMediaType {
    Image,
    Video,
}
```


### New module: deck_importer.rs

```rust
// src-tauri/src/deck_importer.rs  ← NEW FILE
// Owns all PPTX and PDF parsing logic. Imported via pub mod deck_importer in lib.rs.
// Emits DeckImportProgress per slide on the bus during import.
// Returns the completed SlideDeck on success.

use std::path::Path;
use std::sync::Arc;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::slide::SlideDeck;

pub enum DeckFormat {
    Pptx,
    Pdf,
}

impl DeckFormat {
    pub fn detect(path: &Path) -> Result<Self, AppError> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("pptx") => Ok(Self::Pptx),
            Some("pdf")  => Ok(Self::Pdf),
            Some(ext)    => Err(AppError::InvalidInput(
                format!("unsupported format: .{ext} — only .pptx and .pdf are supported")
            )),
            None => Err(AppError::InvalidInput("file has no extension".into())),
        }
    }
}

/// Import a deck from path. Spawned in a Tokio blocking task.
/// Publishes DeckImportProgress per slide and DeckImportComplete or DeckImportFailed.
pub async fn import_deck(
    path: &Path,
    dest_dir: &Path,      // app_data_dir/decks/{deck_id}/
    bus: Arc<EventBus>,
) -> Result<SlideDeck, AppError> {
    todo!()
}

/// Code slide detection: returns true if any run in the slide uses a monospace font family.
pub fn is_code_slide(blocks: &[crate::slide::SlideTextBlock]) -> bool {
    const MONOSPACE_FAMILIES: &[&str] = &[
        "Courier New", "Consolas", "Monaco",
        "Fira Code", "JetBrains Mono", "Source Code Pro",
    ];
    blocks.iter().any(|block| {
        block.runs.iter().any(|run| {
            run.font_family.as_deref()
                .map(|f| MONOSPACE_FAMILIES.contains(&f))
                .unwrap_or(false)
        })
    })
}
```


### New Tauri Commands: commands/deck.rs

```rust
// src-tauri/src/commands/deck.rs  ← NEW FILE

use std::sync::Arc;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::slide::SlidePayload;
use crate::slide_state_machine::StateMachineHandle;

/// Called by the controller after the operator selects a file via tauri-plugin-dialog.
/// Spawns import in a blocking task; progress via bus events.
/// On completion: loads new deck into SlideStateMachine, resets to slide 0,
/// resets timer_started_at to None, resets SceneSource to SlideDeck,
/// publishes SlideUpdate for slide 0.
#[tauri::command]
pub async fn deck_import(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    app: tauri::AppHandle,
) -> Result<String, AppError> {
    // Returns deck_id on success
    todo!()
}

/// Returns all SlidePayload for the active deck (for thumbnail grid population).
#[tauri::command]
pub async fn deck_get_all_slides(
    machine: tauri::State<'_, StateMachineHandle>,
) -> Result<Vec<SlidePayload>, AppError> {
    todo!()
}

/// Restores the most recently imported deck from app_data_dir on startup.
/// Called from app.rs setup task after restore_state. No-op if no prior import exists.
#[tauri::command]
pub async fn deck_restore(
    bus: tauri::State<'_, Arc<EventBus>>,
    machine: tauri::State<'_, StateMachineHandle>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    todo!()
}
```


### New Tauri Commands: commands/scene.rs

```rust
// src-tauri/src/commands/scene.rs  ← NEW FILE

use std::sync::Arc;
use crate::bus::EventBus;
use crate::error::AppError;
use crate::scene::{SceneSource, SceneMediaType};

/// Operator pushes an image (PNG/JPEG/WebP, max 10 MB) to the scene media slot.
/// File is copied to app_data_dir/scene_media/. Publishes SceneMediaUpdate.
#[tauri::command]
pub async fn scene_push_image(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    todo!()
}

/// Operator pushes a video file (MP4/WebM, max 500 MB). Path-referenced, not copied.
/// Publishes SceneMediaUpdate.
#[tauri::command]
pub async fn scene_push_video(
    path: String,
    bus: tauri::State<'_, Arc<EventBus>>,
) -> Result<(), AppError> {
    todo!()
}

/// Switch the Presentation Window source between SlideDeck and SceneMedia.
/// Publishes SceneSourceChanged.
#[tauri::command]
pub async fn scene_set_source(
    source: SceneSource,
    bus: tauri::State<'_, Arc<EventBus>>,
) -> Result<(), AppError> {
    todo!()
}

/// Returns the current active SceneSource.
#[tauri::command]
pub async fn scene_get_active_source(
    // SceneState managed struct holds current source
) -> Result<SceneSource, AppError> {
    todo!()
}
```


### SceneState — new managed struct

```rust
// src-tauri/src/scene.rs — ADD SceneState managed struct

use tokio::sync::RwLock;
use std::sync::Arc;

pub struct SceneState {
    pub active_source: SceneSource,
    pub current_media: Option<SceneMediaPayload>,
}

pub type SceneStateHandle = Arc<RwLock<SceneState>>;

pub fn new_scene_handle() -> SceneStateHandle {
    Arc::new(RwLock::new(SceneState {
        active_source: SceneSource::SlideDeck,
        current_media: None,
    }))
}
```


### app.rs — REQUIRED MODIFICATIONS

```rust
// src-tauri/src/app.rs — REQUIRED MODIFICATIONS FOR F1.4

// 1. ADD plugin in builder chain:
//    .plugin(tauri_plugin_dialog::init())

// 2. ADD to generate_handler![]:
//    crate::commands::deck::deck_import,
//    crate::commands::deck::deck_get_all_slides,
//    crate::commands::deck::deck_restore,
//    crate::commands::scene::scene_push_image,
//    crate::commands::scene::scene_push_video,
//    crate::commands::scene::scene_set_source,
//    crate::commands::scene::scene_get_active_source,

// 3. In .setup(), after app.manage(machine), ADD:
//    let scene_handle = crate::scene::new_scene_handle();
//    app.manage(scene_handle);

// 4. In the spawned restore task, AFTER spawn_timer_task and before the task ends:
//    // Attempt to restore previously imported deck (no-op if none):
//    // deck_restore logic runs here — reads active_deck_id.txt from app_data_dir/decks/

// DO NOT change Arc<EventBus> ownership, run_event_bridge, TimerAbortHandle, or
// WebviewWindowBuilder.
```


### lib.rs — ADD three module declarations

```rust
// src-tauri/src/lib.rs — ADD three lines only, preserve all existing exactly
pub mod scene;
pub mod deck_importer;
// commands/mod.rs additions handled separately
```


### commands/mod.rs — ADD two module declarations

```rust
// src-tauri/src/commands/mod.rs — ADD two lines only
pub mod deck;
pub mod scene;
```


### slide_state_machine.rs — ADD load_deck method

```rust
// src-tauri/src/slide_state_machine.rs — ADD one method to SlideStateMachine impl
// Called by deck_import command after a successful import to hot-swap the active deck.
// Resets current_index to 0 and timer_started_at to None.
// Publishes SlideUpdate for slide 0.

impl SlideStateMachine {
    pub fn load_deck(&mut self, deck: crate::slide::SlideDeck) {
        self.deck = deck;
        self.current_index = 0;
        self.timer_started_at = None;
    }
}
```


### active_deck_id.txt — persistence for Option B

```
// app_data_dir/decks/active_deck_id.txt
// Contains the UUID string of the most recently successfully imported deck.
// Written atomically by deck_import after save_to_json completes.
// Read by deck_restore on startup.
// If file does not exist or deck_id directory is missing: silently fall back to stub().

Format: single line, UUID v4 string, e.g.:
f47ac10b-58cc-4372-a567-0e02b2c3d479
```


### events.ts — ADD five variants

```typescript
// controller/src/lib/events.ts — ADD five variants to AppEvent union only
// Preserve all existing variants and onAppEvent() wrapper exactly.

| { type: 'DeckImportProgress'; payload: { slide: number; total: number } }
| { type: 'DeckImportComplete'; payload: { deck_id: string } }
| { type: 'DeckImportFailed';   payload: { slide: number; reason: string } }
| { type: 'SceneMediaUpdate';   payload: SceneMediaPayload }
| { type: 'SceneSourceChanged'; payload: { source: 'SlideDeck' | 'SceneMedia' } }
```


### ipc.ts — ADD new functions

```typescript
// controller/src/lib/ipc.ts — ADD the following functions only

export interface SceneMediaPayload {
  media_type: 'Image' | 'Video';
  path: string;
  display_name: string;
}

export type SceneSource = 'SlideDeck' | 'SceneMedia';

export async function pickDeckFile(): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const result = await open({
    multiple: false,
    filters: [{ name: 'Presentation', extensions: ['pptx', 'pdf'] }],
  });
  return typeof result === 'string' ? result : null;
}

export async function deckImport(path: string): Promise<string> {
  return invoke<string>('deck_import', { path });
}

export async function deckGetAllSlides(): Promise<SlidePayload[]> {
  return invoke<SlidePayload[]>('deck_get_all_slides');
}

export async function scenePushImage(path: string): Promise<void> {
  return invoke<void>('scene_push_image', { path });
}

export async function scenePushVideo(path: string): Promise<void> {
  return invoke<void>('scene_push_video', { path });
}

export async function sceneSetSource(source: SceneSource): Promise<void> {
  return invoke<void>('scene_set_source', { source });
}

export async function sceneGetActiveSource(): Promise<SceneSource> {
  return invoke<SceneSource>('scene_get_active_source');
}
```


### slideStore.ts — ADD deck import state

```typescript
// controller/src/store/slideStore.ts — ADD to ControllerSlideStore interface and create()
// Preserve all existing fields and actions exactly.

// ADD to interface:
importProgress: { slide: number; total: number } | null;
importError: { slide: number; reason: string } | null;
setImportProgress: (progress: { slide: number; total: number } | null) => void;
setImportError: (error: { slide: number; reason: string } | null) => void;

// ADD to create() initial state:
importProgress: null,
importError: null,

// ADD actions:
setImportProgress: (progress) => set({ importProgress: progress }),
setImportError: (error) => set({ importError: error }),
```


### useSceneStore — NEW Zustand store

```typescript
// controller/src/store/sceneStore.ts  ← NEW FILE

import { create } from 'zustand';
import type { SceneMediaPayload, SceneSource } from '../lib/ipc';

interface SceneStore {
  activeSource: SceneSource;
  currentMedia: SceneMediaPayload | null;
  setActiveSource: (source: SceneSource) => void;
  setCurrentMedia: (media: SceneMediaPayload) => void;
}

export const useSceneStore = create<SceneStore>((set) => ({
  activeSource: 'SlideDeck',
  currentMedia: null,
  setActiveSource: (source) => set({ activeSource: source }),
  setCurrentMedia: (media) => set({ currentMedia: media }),
}));
```


### SceneMediaPanel — NEW component

```typescript
// controller/src/components/SceneMediaPanel.tsx  ← NEW FILE
//
// Layout (top to bottom):
//   ┌─────────────────────────────────────────┐
//   │ [▼ Scene Media]         [▲ Collapse]    │  ← panel header
//   ├─────────────────────────────────────────┤
//   │  Active Source:                         │
//   │  ● Slide Deck   ○ Scene Media           │  ← radio toggle
//   ├─────────────────────────────────────────┤
//   │  [Push Image]   [Push Video]            │  ← file picker triggers
//   ├─────────────────────────────────────────┤
//   │  Current media: [display_name or none]  │
//   │  [preview thumbnail if image]           │
//   └─────────────────────────────────────────┘
//
// State source:
//   - useSceneStore for activeSource and currentMedia
//   - onAppEvent in useEffect:
//       SceneMediaUpdate   → setCurrentMedia
//       SceneSourceChanged → setActiveSource
//
// On mount: invoke sceneGetActiveSource() to populate store
//
// Image picker flow:
//   pickDeckFile() → filtered to png/jpg/jpeg/webp
//   → scenePushImage(path)
//   → SceneMediaUpdate event → store update → UI refresh
//
// Video picker flow:
//   pickDeckFile() → filtered to mp4/webm
//   → scenePushVideo(path)
//   → SceneMediaUpdate event → store update → UI refresh
//
// Source toggle:
//   onChange → sceneSetSource(source)
//   → SceneSourceChanged event → store update → Presentation Window reacts
//
// Error handling: console.error only — full error UI is F1.5 scope
// Collapse state: local React useState (not persisted, not in store)
```


### SlideDeckPanel — ADD import controls

```typescript
// controller/src/components/SlideDeckPanel.tsx — ADD import section only
// Preserve all existing layout and logic exactly.
//
// ADD above the thumbnail grid:
//   ┌─────────────────────────────────────────┐
//   │  [Import Deck]                          │
//   │  Importing… slide 3 / 50               │  ← shown during import
//   │  ⚠ Slide 12: layout_unsupported        │  ← shown on per-slide failure
//   └─────────────────────────────────────────┘
//
// Import button handler:
//   pickDeckFile() → if path !== null → deckImport(path)
//   DeckImportProgress → setImportProgress({ slide, total })
//   DeckImportComplete → setImportProgress(null) + deckGetAllSlides() to refresh thumbnails
//   DeckImportFailed   → setImportError({ slide, reason }) (non-blocking, import continues)
//
// Thumbnail grid: on DeckImportComplete, invoke deckGetAllSlides() to repopulate
// NOTE: do not call deckGetAllSlides() per-progress-tick — only on complete
```


### Presentation Window — ADD scene media rendering

```typescript
// presentation/src/App.tsx (or main presentation component) — ADD conditional render
//
// Listen for SceneSourceChanged to know which track to display:
//   SceneSource = 'SlideDeck'  → render SlideCanvas (existing, unchanged)
//   SceneSource = 'SceneMedia' → render SceneMediaView (new component)
//
// SceneMediaView (presentation/src/components/SceneMediaView.tsx  ← NEW FILE):
//   - Receives SceneMediaPayload via onAppEvent('SceneMediaUpdate')
//   - If media_type = 'Image': <img src={path} style={{ width:'100%', height:'100%',
//     objectFit:'contain' }} />
//   - If media_type = 'Video': <video src={path} autoPlay loop
//     style={{ width:'100%', height:'100%', objectFit:'contain' }} />
//   - Full 1920×1080 container, black background
//   - DO NOT add any controls, overlays, or chrome — pure renderer
//
// presentation/src/lib/events.ts — ADD the same five variants as controller events.ts
// presentation/src/lib/ipc.ts — no changes needed (presentation never calls invoke())
```


---

## ═══════════════════════════════════════════

## BLOCK 6 — SCOPE BOUNDARY

## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  .ppt (legacy binary) import — deferred indefinitely (no pure-Rust parser exists)
  ✗  .key (Keynote) import — deferred to Phase 5+
  ✗  Gradient/theme background rasterization for PPTX — Phase 3+ (F3.4 scope)
  ✗  GStreamer compositor switching between deck and scene media — Phase 3 (F3.1)
  ✗  GIF support for scene media — Phase 3+
  ✗  SVG support — Phase 5+
  ✗  Multi-display assignment UI — F1.5
  ✗  Per-slide timer — Phase 6 automation scope
  ✗  Overlay zone rendering (LowerThird, SidePanel, CornerBadge) — F3.4 scope
  ✗  SlideClassifier acting on is_code_slide flag — F3.4 scope
  ✗  rich_body rendering in Presentation Window — F5.1 scope
  ✗  Slide deck editing from controller — F5.2 scope
  ✗  Streaming platform direct text push — F3.4 / F4.x scope
  ✗  Import from URL or cloud storage — out of scope

DO NOT modify:
  ✗  src-tauri/src/bus.rs
  ✗  src-tauri/src/config.rs or config_loader.rs
  ✗  src-tauri/src/commands/system.rs, info.rs, config.rs
  ✗  src-tauri/src/commands/presentation.rs
  ✗  tauri.conf.json
  ✗  presentation/src/components/SlideCanvas.tsx — store-driven, zero props, DO NOT TOUCH
  ✗  controller/src/components/PresentButton.tsx
  ✗  controller/src/components/SlideDeckPanel.tsx layout/logic (only add import section)
  ✗  The existing SlidePayload struct fields — only SlideEntry gains new fields
  ✗  SlideDeck::stub() — preserved exactly for all tests
  ✗  slide_state_machine.rs advance()/rewind()/jump_to()/reset_timer() signatures

CRITICAL CONSTRAINTS:
  ✗  No manual From<AppError> for InvokeError — Tauri v2 blanket covers it
  ✗  No pub use re-exports in generate_handler![] — use full module paths
  ✗  app.rs stays gated: #[cfg(not(test))] mod app; in lib.rs
  ✗  Use handle.emit() not handle.emit_all() — emit_all removed in Tauri v2
  ✗  Do NOT construct Arc from app.state().inner()
  ✗  background_path must remain Option<String> not Option<PathBuf> — PathBuf breaks IPC serde
  ✗  Use std::io::Error::other() not Error::new(ErrorKind::Other, ...) — clippy on 1.78
  ✗  Presentation Window must never call invoke() — pure event listener
  ✗  deck_import must spawn blocking work via tokio::task::spawn_blocking —
     PPTX/PDF parsing is CPU-bound and must not block the Tokio async runtime
  ✗  Image copy (scene_push_image) must validate file size ≤ 10 MB before copying
  ✗  Video (scene_push_video) must validate extension (mp4/webm) and size ≤ 500 MB
  ✗  pickDeckFile() uses tauri-plugin-dialog — frontend never reads the file directly
  ✗  SlideDeck::stub() rich_body must be Vec::new() and word_count = 0 —
     new fields must not break existing unit tests
  ✗  Do NOT call deckGetAllSlides() on every DeckImportProgress tick —
     only on DeckImportComplete (performance: 50 events × IPC call = unacceptable)
  ✗  SceneSource resets to SlideDeck on every deck_import completion —
     operator must re-select SceneMedia if they want it after a new import
```


---

## ═══════════════════════════════════════════

## BLOCK 7 — OUTPUT CONTRACT

## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
| :-- | :-- | :-- |
| `src-tauri/Cargo.toml` | **MODIFY** | Add zip, quick-xml, pdfium-render, uuid, tauri-plugin-dialog |
| `src-tauri/capabilities/default.json` | **MODIFY** | Add dialog:allow-open |
| `src-tauri/src/slide.rs` | **MODIFY** | Add SlideTextBlock, SlideTextRun, TextAlignment types; extend SlideEntry with 4 new fields; add load_from_json/save_to_json to SlideDeck |
| `src-tauri/src/events.rs` | **MODIFY** | Add 5 new AppEvent variants |
| `src-tauri/src/scene.rs` | **CREATE** | SceneSource, SceneMediaPayload, SceneMediaType, SceneState, SceneStateHandle, new_scene_handle() |
| `src-tauri/src/deck_importer.rs` | **CREATE** | DeckFormat, import_deck(), is_code_slide() |
| `src-tauri/src/slide_state_machine.rs` | **MODIFY** | Add load_deck() method only |
| `src-tauri/src/commands/deck.rs` | **CREATE** | deck_import, deck_get_all_slides, deck_restore |
| `src-tauri/src/commands/scene.rs` | **CREATE** | scene_push_image, scene_push_video, scene_set_source, scene_get_active_source |
| `src-tauri/src/commands/mod.rs` | **MODIFY** | Add pub mod deck; pub mod scene; |
| `src-tauri/src/lib.rs` | **MODIFY** | Add pub mod scene; pub mod deck_importer; |
| `src-tauri/src/app.rs` | **MODIFY** | Add plugin init, new commands to handler, SceneStateHandle managed, deck_restore in setup task |
| `controller/src/lib/events.ts` | **MODIFY** | Add 5 new AppEvent variants |
| `controller/src/lib/ipc.ts` | **MODIFY** | Add SceneMediaPayload, SceneSource types + 7 new functions |
| `controller/src/store/slideStore.ts` | **MODIFY** | Add importProgress, importError state + actions |
| `controller/src/store/sceneStore.ts` | **CREATE** | useSceneStore with activeSource, currentMedia |
| `controller/src/components/SceneMediaPanel.tsx` | **CREATE** | Scene media push + source toggle panel |
| `controller/src/components/SlideDeckPanel.tsx` | **MODIFY** | Add import button + progress/error display section |
| `controller/src/App.tsx` | **MODIFY** | Add `<SceneMediaPanel />` below `<SlideDeckPanel />` |
| `presentation/src/components/SceneMediaView.tsx` | **CREATE** | Image/video renderer for scene media track |
| `presentation/src/App.tsx` | **MODIFY** | Add SceneSource conditional render + SceneMediaView |
| `presentation/src/lib/events.ts` | **MODIFY** | Add same 5 new AppEvent variants as controller |
| `verify.sh` | **MODIFY** | Add run_f1_4() function + dispatch cases |

### Tests Required

| File | Test | Class |
| :-- | :-- | :-- |
| `src-tauri/src/deck_importer.rs` | `pptx_extracts_title_body_notes_from_standard_placeholders` | unit (fixture) |
| `src-tauri/src/deck_importer.rs` | `pptx_background_image_extracted_only_when_present` | unit (fixture) |
| `src-tauri/src/deck_importer.rs` | `pptx_solid_fill_slide_has_null_background_path` | unit (fixture) |
| `src-tauri/src/deck_importer.rs` | `pdf_every_page_has_background_path` | unit (fixture) |
| `src-tauri/src/deck_importer.rs` | `unsupported_extension_returns_invalid_input_error` | unit |
| `src-tauri/src/deck_importer.rs` | `is_code_slide_true_for_monospace_font_run` | unit |
| `src-tauri/src/deck_importer.rs` | `is_code_slide_false_for_sans_serif_run` | unit |
| `src-tauri/src/slide.rs` | `slide_entry_new_fields_default_to_safe_values_in_stub` | unit |
| `src-tauri/src/slide.rs` | `deck_save_and_load_json_round_trips_correctly` | unit |
| `src-tauri/src/slide_state_machine.rs` | `load_deck_resets_index_and_timer` | unit |

> NOTE: Unit tests for deck_importer require small fixture files (.pptx and .pdf).
> Store fixtures at `src-tauri/tests/fixtures/` — this path must be created in F1.4.
> Fixture .pptx must contain: one slide with standard title/body placeholders, one with
> an embedded image background, one with a solid fill, one with a monospace font run.

### verify.sh — run_f1_4() checks

```bash
run_f1_4() {
  section "F1.4 — Slide Deck Import"
  if [ ! -f "src-tauri/src/deck_importer.rs" ]; then
    skip "deck_importer.rs not created — F1.4 not started yet"
    return
  fi

  # Rust checks
  file_contains "DeckImportProgress variant"  "src-tauri/src/events.rs"              "DeckImportProgress"
  file_contains "DeckImportComplete variant"  "src-tauri/src/events.rs"              "DeckImportComplete"
  file_contains "SceneMediaUpdate variant"    "src-tauri/src/events.rs"              "SceneMediaUpdate"
  file_contains "SceneSourceChanged variant"  "src-tauri/src/events.rs"              "SceneSourceChanged"
  file_contains "SlideTextBlock type"         "src-tauri/src/slide.rs"               "SlideTextBlock"
  file_contains "SlideTextRun type"           "src-tauri/src/slide.rs"               "SlideTextRun"
  file_contains "word_count field"            "src-tauri/src/slide.rs"               "word_count"
  file_contains "is_code_slide field"         "src-tauri/src/slide.rs"               "is_code_slide"
  file_contains "deck_import cmd"             "src-tauri/src/commands/deck.rs"       "deck_import"
  file_contains "deck_restore cmd"            "src-tauri/src/commands/deck.rs"       "deck_restore"
  file_contains "scene_push_image cmd"        "src-tauri/src/commands/scene.rs"      "scene_push_image"
  file_contains "scene_set_source cmd"        "src-tauri/src/commands/scene.rs"      "scene_set_source"
  file_contains "spawn_blocking"              "src-tauri/src/deck_importer.rs"       "spawn_blocking"
  file_contains "tauri-plugin-dialog"         "src-tauri/Cargo.toml"                 "tauri-plugin-dialog"
  file_contains "pdfium-render"               "src-tauri/Cargo.toml"                 "pdfium-render"
  file_contains "quick-xml"                   "src-tauri/Cargo.toml"                 "quick-xml"

  # Frontend checks
  file_contains "DeckImportProgress events.ts"  "controller/src/lib/events.ts"                 "DeckImportProgress"
  file_contains "SceneMediaUpdate events.ts"    "controller/src/lib/events.ts"                 "SceneMediaUpdate"
  file_contains "deckImport ipc"                "controller/src/lib/ipc.ts"                    "deckImport"
  file_contains "pickDeckFile ipc"              "controller/src/lib/ipc.ts"                    "pickDeckFile"
  file_contains "sceneStore"                    "controller/src/store/sceneStore.ts"           "useSceneStore"
  file_contains "SceneMediaPanel"               "controller/src/components/SceneMediaPanel.tsx" "SceneMediaPanel"
  file_contains "SceneMediaView"                "presentation/src/components/SceneMediaView.tsx" "SceneMediaView"
  file_contains "DeckImportProgress pres"       "presentation/src/lib/events.ts"               "DeckImportProgress"

  # Unit tests
  file_contains "pptx_extracts_title test"   "src-tauri/src/deck_importer.rs" "pptx_extracts_title_body_notes"
  file_contains "is_code_slide test"         "src-tauri/src/deck_importer.rs" "is_code_slide_true_for_monospace"
  file_contains "load_deck_resets test"      "src-tauri/src/slide_state_machine.rs" "load_deck_resets_index_and_timer"
  file_contains "round_trip_json test"       "src-tauri/src/slide.rs"         "deck_save_and_load_json_round_trips"

  check "F1.4 unit tests pass" \
    bash -c 'cd src-tauri && cargo test 2>&1 | tee /tmp/sd-f14-test.log \
    && ! grep -q "FAILED" /tmp/sd-f14-test.log'
}

# In the dispatch case block, ADD:
# f1.4)
#   run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4
#   ;;
# f1|phase1)
#   run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_core
#   ;;
```


### Verification Commands

```bash
cargo test                       # all unit tests including fixture-based importer tests
cargo clippy -- -D warnings      # zero warnings — spawn_blocking, no PathBuf in IPC
npm run typecheck                # both controller and presentation
bash verify.sh f1.4              # F1.4 AC gate
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
L13  PathBuf does not serialise cleanly across Tauri IPC — use Option<String> for file paths
L21  AppEvent TypeScript union must be a fully discriminated union — every variant needs
     explicit { type, payload } shape; appending bare fragments breaks type narrowing
L22  When replacing a stub call, remove the now-unused `use` import immediately —
     clippy -D warnings will fail the build
L23  SlideStateMachine must own data_dir: PathBuf at construction time
L24  DeckEnd/DeckStart are emitted INSTEAD OF SlideUpdate at boundaries, not alongside
L31  Do not add AbortHandle fields to SlideStateMachine — manage via newtype in app.rs
L32  start_timer_if_needed() must not take a bus parameter — caller publishes
L33  controller/ and presentation/ are separate Vite projects — duplicate shared components
L34  Tauri global shortcut callbacks cannot directly await — use tauri::async_runtime::spawn
L35  Unregistering global shortcuts must use async-spawn pattern — never from inside callback
L36  VHO:Trojan-PSW AV false positive on Windows caused by keyring + unsigned binary —
     add project + cargo/target dirs to AV exclusions; code-sign for distribution
L41  CPU-bound work (PPTX/PDF parsing) must use tokio::task::spawn_blocking —
     blocking the Tokio runtime thread pool causes all async tasks to stall
L42  Do not call deckGetAllSlides() per DeckImportProgress tick —
     IPC overhead × slide count degrades import performance visibly; call only on complete
L43  File size validation (10 MB image / 500 MB video) must happen in the Rust command
     before any copy or path storage — never trust the frontend to pre-validate
L44  SlideDeck::stub() must remain backward compatible with all new SlideEntry fields —
     new fields default to zero/false/empty; existing tests must not require modification
```

```

