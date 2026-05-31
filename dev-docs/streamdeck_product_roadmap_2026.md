# StreamDeck — Product & Architecture Roadmap 2026
## Live Presentation + Multi-Camera Streaming Platform

**Document Version:** 1.0  
**Date:** May 2026  
**Status:** Authoritative Specification  

---

## Executive Summary

StreamDeck is a cross-platform desktop application (macOS, Windows, Linux) that unifies local presentation control, multi-camera production, and live-streaming egress into a single orchestrated system. It is built ground-up in Rust using Tauri v2, GStreamer 1.26, and the NDI 6 SDK, targeting zero dependency on proprietary production tools like Mevo Studio or ProPresenter by the end of Phase 5.

The roadmap is structured so that **every phase is independently shippable and stable**. External tools are retired only after their replacement is proven in production. Each feature is defined by a clear acceptance criterion that specifies the exact technology boundary, testable condition, and pass/fail gate.

---

## Technology Foundation

### Core Runtime

| Layer | Technology | Version | Rationale |
|---|---|---|---|
| Application shell | Tauri | v2.11.x | Multi-window WebView, OS-native IPC, capability sandbox |
| Application language | Rust + Tokio | 1.78 / 1.38 | Memory safety, async pipeline, zero-cost abstractions |
| Frontend language | TypeScript + React | 5.x / 18.x | Controller and Presentation UIs |
| Media pipeline | GStreamer | 1.26.x | NDI, WebRTC, WHIP, hardware encode, compositing |
| NDI ingest/output | NDI SDK + gst-plugin-ndi | 6.3 / 0.14.x | Sub-frame latency (NDI 6.3 New Format Stream) |
| WebRTC ingest | gst-plugin-webrtc (whipserversrc) | 0.14.x | RFC 9725 WHIP, phone camera ingest |
| WebRTC egress | gst-plugin-webrtc (whipclientsink) | 0.14.x | RFC 9725 WHIP, direct to Facebook/YouTube |
| Event bus | tokio::sync::broadcast | built-in | Single-producer multi-consumer slide/scene state |
| Configuration | TOML (serde) | - | Declarative pipelines, scenes, overlay zones |
| Observability | GStreamer Bus + Tauri events | built-in | Real-time pipeline metrics to controller UI |

### Architecture Invariants (Never Break These)

1. **Process Separation by Tier** — GStreamer pipeline runs in a dedicated Tokio task. Tauri UI events never block the media pipeline thread pool.
2. **Event Bus is the Only Channel** — all state transitions (slide change, scene cut, stream start/stop) flow through a `tokio::sync::broadcast` channel. No direct function calls across subsystem boundaries.
3. **Configuration as Code** — pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files. No hard-coded behaviour.
4. **Coexistence First** — every new subsystem is additive. Mevo Studio and ProPresenter continue operating until the phase that explicitly replaces them. No breaking changes to external tools until the replacement passes all acceptance criteria.
5. **Observability from Day One** — GStreamer bus messages (error, warning, latency, dropped frames) are translated to Tauri events visible in the controller UI from Phase 1 onward.

---

## Phase 0 — Project Skeleton & Toolchain

**Goal:** Establish the build system, repository structure, CI pipeline, and validated multi-window shell with no media functionality.

**External tools still active:** Mevo Studio (all streaming), ProPresenter (all presentation)

### Features

#### F0.1 — Tauri v2 Project Bootstrap
**Description:** Initialise a Tauri v2 workspace with a Rust backend (`src-tauri/`) and two separate frontend entry points: `controller/` (operator UI) and `presentation/` (projector screen). Wire Vite + React + TypeScript for both frontends.

**Technologies:**
- Tauri v2.11.x (`tauri`, `tauri-build` crates)
- Rust workspace (`Cargo.toml` with `[workspace]`)
- Vite 5.x + React 18.x + TypeScript 5.x
- `tauri.conf.json` defining two windows: `controller` (default) and `presentation` (secondary)

**Acceptance Criteria:**
- `cargo tauri dev` launches two windows simultaneously: a controller window (1280×800) and a presentation window (1920×1080)
- Both windows render their respective React apps and show the build timestamp
- `cargo tauri build` produces a signed installer on macOS and Windows without warnings
- `core:webview:allow-create-webview-window` capability declared in `src-tauri/capabilities/default.json`
- CI (GitHub Actions) runs `cargo test`, `cargo clippy -- -D warnings`, and `npm run typecheck` on every PR

#### F0.2 — IPC Command Layer
**Description:** Define the Tauri command interface that all subsequent features will extend. Establish typed request/response envelopes so the frontend never calls raw Rust functions.

**Technologies:**
- `#[tauri::command]` with `serde` serialise/deserialise
- TypeScript `invoke<T>()` wrapper with generated types (ts-rs or manual)
- Error enum translated to TypeScript discriminated union

**Acceptance Criteria:**
- `ping` command returns `{ ok: true, version: string }` from Rust to both windows
- All commands return `Result<T, AppError>` — no `unwrap()` in command handlers
- TypeScript types are generated or maintained in sync with Rust structs (enforced by CI typecheck)
- Command dispatch latency < 2 ms measured in unit test

#### F0.3 — Event Bus Foundation
**Description:** Instantiate the central `tokio::sync::broadcast` channel with a typed `AppEvent` enum. Wire Tauri's event emitter so all broadcast events are forwarded to the frontend.

**Technologies:**
- `tokio::sync::broadcast` (capacity 256)
- `AppEvent` Rust enum with `serde::Serialize`
- Tauri `app.emit_all()` bridge task

**Acceptance Criteria:**
- `AppEvent` variants compile with `#[non_exhaustive]` to enforce forward compatibility
- Any event emitted in Rust appears in both windows' `listen()` handlers within 5 ms
- Bus backpressure: if a receiver lags > 128 events behind, it is force-dropped and re-subscribed with a `BusLag` warning event emitted
- Unit test: 1000 events dispatched under load, zero deadlocks, zero panics

#### F0.4 — TOML Configuration Loader
**Description:** Load `streamdeck.toml` from the user's app data directory on startup. Hot-reload on file change using `notify` crate.

**Technologies:**
- `serde` + `toml` crates
- `notify` v6 for file-system watching
- `config::StreamDeckConfig` struct with validation

**Acceptance Criteria:**
- Invalid TOML on startup shows a user-facing error dialog (not a Rust panic)
- Valid config change on disk is reflected in the running app within 500 ms
- Config version field enables migration: loading a v0 config in a v1 app applies default values for new fields without error
- Sensitive fields (WHIP bearer tokens) are stored in OS keychain via `keyring` crate, not in TOML plaintext

---

## Phase 1 — Presentation Engine (Projector Screen)

**Goal:** Replace the external presentation tool's projector output with a Tauri-managed Presentation Window. The operator drives slide changes from the Controller Window. The Presentation Window is mapped to the projector via OS display assignment.

**External tools still active:** Mevo Studio (all streaming), ProPresenter (fully active but now mirrored by StreamDeck)

### Features

#### F1.1 — Presentation Window & Slide Renderer
**Description:** The Presentation Window renders slide content (title, body, background image, slide number) in a full-screen React canvas. Content is pushed from Rust via the event bus — the window contains zero business logic.

**Technologies:**
- Tauri secondary window with `fullscreen: false`, `decorations: false`, `always_on_top: true`
- React + CSS Grid layout for slide zones (title zone, body zone, background zone)
- Tauri `listen('slide:update')` event in the Presentation Window frontend
- `AppEvent::SlideUpdate(SlidePayload)` variant on the bus

**Acceptance Criteria:**
- Window opens on a secondary display if one is connected; falls back to a floating window on the primary display
- Slide transitions complete in < 16 ms (one frame at 60 Hz) measured from event emit to DOM paint via `performance.now()`
- Presentation Window has no visible browser chrome, no scrollbars, no DevTools shortcut in production build
- Slide content is fully rendered at 1920×1080 regardless of physical display resolution (CSS `transform: scale()` applied based on window size)
- Window is crash-isolated: a Presentation Window JS error does not crash the Controller Window

#### F1.2 — Slide State Machine
**Description:** Rust-side state machine managing current slide index, total slides, section names, and slide metadata. The state machine is the single source of truth — both windows read from it, neither writes directly.

**Technologies:**
- `SlideStateMachine` Rust struct with `tokio::sync::RwLock`
- `AppEvent::SlideUpdate`, `AppEvent::SlideAdvance`, `AppEvent::SlideRewind` variants
- JSON slide deck format (loaded from file or pushed from controller)

**Acceptance Criteria:**
- Advancing past the last slide emits `AppEvent::DeckEnd` instead of wrapping around
- Rewinding before slide 1 is a no-op with a `AppEvent::DeckStart` event
- Concurrent reads from Controller and Presentation windows never block more than 1 ms
- State is serialised to disk on every change so a crash-restart restores the last slide position
- Unit tests cover: advance, rewind, jump-to, deck load, deck end, state restore

#### F1.3 — Controller Slide Panel
**Description:** The Controller Window displays a slide thumbnail grid, current slide preview, next slide preview, speaker notes, elapsed time, and a progress bar. The operator navigates using keyboard shortcuts or on-screen buttons.

**Technologies:**
- React component: `SlideDeck`, `SlideThumb`, `SlidePreview`, `SpeakerNotes`, `Timer`
- Tauri `invoke('slide_advance')`, `invoke('slide_rewind')`, `invoke('slide_jump_to', { index })`
- Keyboard shortcuts: Space/Right Arrow (advance), Left Arrow (rewind), number keys (jump)

**Acceptance Criteria:**
- Thumbnail grid renders all slides at 160×90 px; clicking a thumbnail jumps to that slide
- Current and next previews are pixel-identical to what the Presentation Window renders
- Speaker notes are shown in the controller but never on the Presentation Window
- Presentation elapsed timer starts on first advance and persists across slide changes
- Keyboard shortcuts work when the Controller Window has focus; they do not trigger in the Presentation Window (focus isolation)

#### F1.4 — Slide Deck Import (PowerPoint / Keynote / PDF)
**Description:** Import a `.pptx`, `.key`, or `.pdf` file and extract per-slide data: title text, body text, background image, slide notes. Store as the internal `SlideDeck` JSON format.

**Technologies:**
- `.pptx`: `pptx` Rust crate (xml parsing) or LibreOffice headless subprocess for image export
- `.pdf`: `pdfium-render` Rust crate for rasterisation
- Background images extracted at 1920×1080 PNG and stored in app cache directory
- `SlideDeck` JSON schema v1: `{ slides: [{ index, title, body, notes, background_path }] }`

**Acceptance Criteria:**
- A 50-slide `.pptx` file imports in < 10 seconds on a mid-range laptop
- Background images exported at correct aspect ratio with no distortion
- Title and body text extracted for slides that use standard text placeholders
- Slides with unsupported layouts (custom masters) import with a blank body and a `layout_unsupported: true` flag visible in the controller
- Import errors surface as user-facing dialogs listing which slides failed and why
- Imported deck passes through the same slide state machine as a manually entered deck

#### F1.5 — Multi-Display Assignment
**Description:** Detect connected displays at startup and when displays are added/removed. Allow the operator to assign the Presentation Window to a specific display from the controller UI.

**Technologies:**
- Tauri `tauri::window::Window::set_position()` and `get_all_windows()`
- System display enumeration via `winit` (bundled in Tauri) or OS API via Tauri plugin
- `AppEvent::DisplaysChanged(Vec<DisplayInfo>)` on connect/disconnect

**Acceptance Criteria:**
- Controller shows a display map matching physical layout (position, resolution, name)
- Operator can drag or select which display the Presentation Window occupies; change takes effect within one second
- Projector assigned display retains the Presentation Window across sleep/wake cycles
- If the assigned display is disconnected, a `DisplayLost` warning appears in the controller; the window falls back to the primary display
- Setting persists in `streamdeck.toml` and is restored on app restart

---

## Phase 2 — NDI Camera Ingest & Local Preview

**Goal:** Ingest Mevo cameras and any NDI source over the local network. Display live camera previews in the Controller Window. No streaming yet — this phase proves the GStreamer pipeline foundation.

**External tools still active:** Mevo Studio (all streaming), ProPresenter (presentation — now running in parallel with StreamDeck Phase 1)

### Features

#### F2.1 — GStreamer Pipeline Service
**Description:** Initialise GStreamer inside a dedicated Tokio runtime task. Expose a `PipelineService` handle that accepts commands (add source, remove source, set output) and emits bus messages as `AppEvent`s.

**Technologies:**
- `gstreamer`, `gstreamer-app`, `gstreamer-video` crates
- Dedicated `tokio::task::spawn_blocking` for GStreamer main loop
- `PipelineService` Rust actor (channels-based, no shared state)
- GStreamer bus watcher thread forwarding `Error`, `Warning`, `Latency`, `Qos` messages to `AppEvent`

**Acceptance Criteria:**
- GStreamer initialises without error on macOS (ARM + Intel), Windows 10+, and Ubuntu 22.04+
- A `videotestsrc ! autovideosink` pipeline starts and stops cleanly via `PipelineService` commands
- GStreamer errors surface as `AppEvent::PipelineError { element, message }` within 100 ms of occurrence
- `PipelineService` is shut down cleanly on app exit: all elements reach `NULL` state, no zombie processes
- Unit test: start, pause, resume, stop pipeline 100 times with no memory leak (checked via Valgrind / heaptrack)

#### F2.2 — NDI Source Discovery & Ingest
**Description:** Enumerate NDI sources on the local network. For each selected source, create a GStreamer NDI source element and decode the video to a local preview texture.

**Technologies:**
- `gst-plugin-ndi` v0.14.x (`ndisrc` element)
- NDI SDK 6.3 (NDI 6.3 New Format Stream for sub-frame latency where hardware supports it)
- `ndisrc ! queue ! video/x-raw ! videoconvert ! appsink` pipeline segment
- NDI discovery via `ndi::Find` API called on a 2-second polling interval

**Acceptance Criteria:**
- All Mevo cameras (Start, Core, Go) with NDI mode enabled appear in the source list within 5 seconds of appearing on the network
- Source list updates without restarting the pipeline when a camera appears or disappears
- Video preview renders in the Controller Window at ≥ 25 fps for 1080p NDI sources
- NDI 6.3 New Format Stream is selected automatically when the source advertises it; falls back to standard NDI gracefully
- Removing a camera from the network triggers `AppEvent::SourceLost` and the preview shows a "Source Lost" placeholder — no crash

#### F2.3 — Camera Preview Grid in Controller
**Description:** Display live video thumbnails for all active NDI sources in the Controller Window. Each preview shows source name, resolution, frame rate, and latency.

**Technologies:**
- GStreamer `appsink` → `gdk-pixbuf` or raw RGBA frames → Tauri `app.emit()` as base64 PNG at 8 fps
- React `CameraPreview` component using `<img>` tag updated on event
- Overlay metadata rendered as CSS over the preview image

**Acceptance Criteria:**
- Up to 8 simultaneous NDI preview streams render in the controller at 8 fps each without CPU usage exceeding 40% on a modern 8-core CPU
- Preview resolution is 320×180 (thumbnail) — not full resolution — to minimise CPU/bandwidth
- Latency figure shown per preview is the NDI timestamp delta (source to render), updated every second
- Preview grid layout is configurable in the controller: 2×2, 2×3, 2×4 grid options
- Previews survive display sleep/wake without requiring a restart

#### F2.4 — Phone Camera Ingest (WHIP Server)
**Description:** Expose a local WHIP server endpoint so phones running any WHIP-capable app (Larix Broadcaster, OBS Mobile, browser-based) can push video to StreamDeck over WiFi.

**Technologies:**
- `whipserversrc` GStreamer element (gst-plugin-webrtc 0.14.x, RFC 9725)
- Embedded HTTP server (`axum` 0.8 crate) for WHIP signalling endpoint (`POST /whip/{slot}`)
- ICE candidates resolved on the local network (mDNS + LAN IP, no STUN/TURN required for LAN use)
- `whipserversrc ! queue ! videoconvert ! appsink` for preview

**Acceptance Criteria:**
- WHIP endpoint `http://{local-ip}:8088/whip/camera1` accepts a WebRTC push from Larix Broadcaster within 3 seconds of connection
- Negotiation completes using H.264 Baseline or VP8 (auto-selected by capability exchange)
- Phone camera appears in the Controller preview grid identical to NDI sources
- Up to 4 simultaneous WHIP phone sources active concurrently
- QR code displayed in Controller UI encodes the WHIP URL for quick phone connection
- WHIP session survives a 5-second network interruption (ICE restart) without requiring operator action

---

## Phase 3 — Composite Pipeline & Scene System

**Goal:** Composite multiple camera sources and the presentation slide into a single output canvas. Define named scenes. Switch scenes from the Controller. No streaming output yet.

**External tools still active:** Mevo Studio (all streaming), ProPresenter (running in parallel)

### Features

#### F3.1 — GStreamer Compositor Pipeline
**Description:** Build a GStreamer compositor pipeline that takes N camera sources plus the slide capture source and blends them onto a 1920×1080 output canvas. Position, size, and Z-order of each source is defined per scene.

**Technologies:**
- `compositor` GStreamer element (hardware-accelerated via `d3d12compositor` on Windows, `glvideomixer` on macOS/Linux)
- `videoscale`, `videocrop`, `videobox` for geometry transforms
- `capsfilter` enforcing `video/x-raw,width=1920,height=1080,framerate=30/1`
- Pipeline described in TOML `[scene.X.sources]` array

**Acceptance Criteria:**
- Compositor produces a stable 1920×1080 @ 30 fps output with 3 simultaneous sources without frame drops on a modern 8-core CPU with integrated GPU
- Source-to-compositor latency (input frame to composite frame) < 33 ms (one frame)
- GPU compositor variant is selected automatically on macOS (Metal via `glvideomixer`), Windows (D3D12 via `d3d12compositor`), falling back to CPU `compositor` if neither is available
- CPU usage stays below 60% with 4 simultaneous 1080p sources composited
- Adding or removing a source from the compositor does not produce a visible glitch on the output canvas

#### F3.2 — Presentation Slide Capture Source
**Description:** Capture the Presentation Window's rendered content as a GStreamer video source, so the slide is available as an input to the compositor. This is the bridge between the Tauri presentation layer and the media pipeline.

**Technologies:**
- macOS: `AVFoundation` window capture via `avfvideosrc` with window ID targeting the Presentation Window
- Windows: `d3d12screencapturesrc` (DXGIOutputDuplication, window-specific)
- Linux: `pipewiresrc` with portal-requested screen share of the Presentation Window
- `ScreenCaptureSource` Rust trait with platform-specific implementations
- GStreamer `appsrc` fallback: Tauri frontend sends frames via IPC for platforms without window-specific capture

**Acceptance Criteria:**
- Presentation Window contents appear in the compositor as a video source within 2 frames of a slide change
- Capture runs at 30 fps with < 5 ms additional latency beyond the GStreamer source-to-compositor path
- Slide text is legible (no JPEG artefacts) at 1920×1080: capture must use lossless or high-quality encoding in the pipeline (not JPEG)
- Platform implementations compile and pass smoke tests on macOS 14+, Windows 11, and Ubuntu 24.04
- If screen capture permission is denied by the OS, a clear error dialog explains the required permission step (Screen Recording on macOS, no action on Windows, portal grant on Linux)

#### F3.3 — Scene Definition System
**Description:** A scene is a named configuration of: which sources are active, their position/size on the canvas, which overlay zones are enabled, and transition type. Scenes are defined in TOML and editable from the Controller.

**Technologies:**
- `Scene` Rust struct serialisable to/from TOML
- `SceneManager` with `switch_to(scene_id)` method
- `AppEvent::SceneChanged(SceneId)` on the bus
- Controller UI: scene card grid with live preview thumbnails

**Acceptance Criteria:**
- Scene switch completes (compositor re-patched) within 2 frames (66 ms) at 30 fps output
- At least 4 transition types defined: Cut (instant), Fade (0.5s), Fade-to-Black (0.5s), Side-wipe (0.5s)
- Scene definitions survive app restart (TOML persisted)
- Controller UI shows a live thumbnail of each scene's expected layout (not live video, just the layout diagram)
- Programmatic scene switch via IPC command: `invoke('scene_switch', { id: 'keynote' })` — enables future automation

#### F3.4 — Adaptive Overlay Engine
**Description:** Render slide metadata as a stream-safe overlay on the compositor output. The overlay adapts its zone, font size, and content density based on the classified slide type. This solves the dimension mismatch and readability problem between the full local slide and the stream overlay.

**Technologies:**
- Overlay rendered as HTML/CSS in a transparent Tauri WebView window (`AlwaysOnTop`, `transparent: true`) sized to 1920×1080
- `OverlayCapture` Rust trait (same platform-specific capture as F3.2) feeding overlay as GStreamer `appsrc`
- GStreamer `compositor` alpha-composites overlay on top of camera feed
- `SlideClassifier` Rust function: classifies each slide as `Title | Visual | Content | Transition` based on word count, bullet count, and image presence
- Overlay zones: `LowerThird` (bottom 12%), `SidePanel` (right 28%), `CornerBadge` (top-right 6%)

**Acceptance Criteria:**
- `Title` slide type: shows only the slide title in the Lower Third zone at ≥ 32px rendered font size, camera feed full canvas
- `Visual` slide type (chart/image dominant): shows only a Corner Badge with slide number — no text overlay obstructs the visual
- `Content` slide type: shows the first bullet point (max 40 characters, truncated with ellipsis) in the Side Panel, slide title in Lower Third
- `Transition` slide type: no overlay shown, clean camera feed
- All overlay text rendered on a semi-transparent backing (`rgba(0,0,0,0.72)`) — WCAG AA contrast ratio ≥ 4.5:1 verified by automated colour contrast check at build time
- Overlay zones never overlap each other (geometric exclusion enforced in layout logic)
- Overlay updates within one frame of a `SlideUpdate` event
- Operator can disable any overlay zone per scene from the controller

---

## Phase 4 — Live Streaming Egress (WHIP)

**Goal:** Stream the composite pipeline output to Facebook Live, YouTube, and any WHIP-compatible platform simultaneously. Replace Mevo Studio's streaming function entirely.

**External tools still active after phase:** ProPresenter (still optional), Mevo Studio cameras (still used), Mevo Studio **streaming is retired**

### Features

#### F4.1 — WHIP Encoding Pipeline
**Description:** Encode the compositor output to H.264 (hardware-accelerated where available) and push to one or more WHIP ingest endpoints. Facebook Live and YouTube both accept RFC 9725 WHIP as of 2025.

**Technologies:**
- `whipclientsink` GStreamer element (gst-plugin-webrtc 0.14.x, replaces all RTMP egress)
- Hardware encoder selection chain: `vtenc_h264` (macOS), `mfh264enc` (Windows), `vaapih264enc` (Linux VA-API), fallback to `x264enc`
- `EncoderSelector` Rust struct with platform detection at runtime
- Bitrate ladder: 720p@3Mbps (default), 1080p@6Mbps (configurable)

**Acceptance Criteria:**
- Stream reaches the Facebook Live ingest within 8 seconds of operator pressing "Go Live"
- Encoder selection is automatic: hardware encoder chosen if available, CPU encoder as verified fallback
- End-to-end latency (camera sensor to viewer) ≤ 8 seconds over a stable 20 Mbps uplink
- Encoding CPU overhead with hardware encoder < 10% on a modern laptop
- Encoding CPU overhead with software encoder (x264enc, `veryfast` preset) < 40% on a modern 8-core laptop
- `AppEvent::StreamHealth { platform, bitrate_kbps, dropped_frames, rtt_ms }` emitted every 2 seconds

#### F4.2 — Multi-Platform Fan-Out
**Description:** Push the same stream to multiple WHIP destinations simultaneously (Facebook, YouTube, custom RTMP-over-WHIP relay) from a single encoded output via GStreamer `tee`.

**Technologies:**
- GStreamer `tee` element splitting encoded RTP to multiple `whipclientsink` instances
- `StreamDestination` TOML array: `[[destinations]]` with `platform`, `whip_url`, `bearer_token` (keychain ref)
- `StreamManager` Rust actor managing per-destination state machines

**Acceptance Criteria:**
- Three simultaneous WHIP destinations active with total upstream bandwidth usage ≤ 1.15× a single destination (stream is encoded once, not re-encoded per destination)
- A single destination failure (network drop) does not interrupt other destinations
- Failed destination auto-reconnects within 30 seconds with exponential back-off (1s, 2s, 4s, max 30s)
- Controller UI shows per-destination health: green (live), amber (reconnecting), red (failed) indicators
- Operator can add or remove destinations while streaming without interrupting active destinations

#### F4.3 — Stream Control Panel
**Description:** Operator-facing streaming controls in the Controller Window: Go Live, End Stream, per-destination mute/unmute, stream timer, bitrate graph, and dropped-frame counter.

**Technologies:**
- React `StreamPanel` component
- `AppEvent::StreamHealth` consumed for live metrics
- Tauri `invoke('stream_start')`, `invoke('stream_stop')`, `invoke('stream_mute_destination', { id })`
- Chart.js sparkline for bitrate history (last 60 seconds)

**Acceptance Criteria:**
- "Go Live" button is protected by a two-step confirmation (click → confirm dialog) to prevent accidental streams
- Stream timer shows hours:minutes:seconds elapsed, updating every second
- Bitrate sparkline shows the last 60-second history, updating every 2 seconds
- Dropped frame counter shows cumulative drops since stream start; turns amber at > 0.1% drop rate, red at > 1%
- "End Stream" sends a graceful GStreamer EOS before closing WHIP sessions; no abrupt disconnects

#### F4.4 — SRT Ingest Relay (Optional Intermediate)
**Description:** For environments where phone cameras cannot reach the WHIP server directly (corporate WiFi isolation), provide an SRT listener that relays into the GStreamer pipeline alongside the WHIP server.

**Technologies:**
- `srtsrc` GStreamer element (GStreamer 1.26 built-in)
- SRT caller mode on camera (Mevo, Larix), listener mode on StreamDeck
- `srt://0.0.0.0:9000?mode=listener` URI configured per source slot in TOML
- ARQ error recovery configured: `latency=120ms`, tolerates up to 30% packet loss

**Acceptance Criteria:**
- An SRT source from Larix Broadcaster connects and appears in the source list within 5 seconds
- Video plays smoothly at 1080p with up to 10% simulated packet loss (netem tc test)
- SRT source slots are configured independently from WHIP/NDI slots — operator chooses per source
- SRT and NDI sources can coexist simultaneously in the compositor without interference
- SRT latency parameter is configurable per source in TOML (range: 40 ms to 4000 ms)

---

## Phase 5 — Presentation Engine Independence

**Goal:** Load and drive presentations natively without relying on PowerPoint, Keynote, or ProPresenter. The Presentation Window becomes a first-class slide renderer. ProPresenter is fully optional.

**External tools still active after phase:** Mevo Studio cameras (NDI output still used) — Mevo Studio as a software tool is now fully optional

### Features

#### F5.1 — Native Slide Renderer
**Description:** Render slides natively in the Presentation Window using a structured slide format, with full support for background images, text layers, font sizes, colour themes, and slide transitions.

**Technologies:**
- React `SlideCanvas` component with CSS absolute positioning for text/image layers
- `SlideTheme` TOML schema: defines fonts, colours, background, text zone sizes
- `SlideTransition` enum: `Cut | Fade | FadeBlack | SlideLeft | SlideRight | ZoomIn` with CSS animation
- Background images loaded from app cache (exported from `.pptx` import in F1.4)
- Font loading via `@font-face` with embedded WOFF2 in the app bundle for the bundled theme fonts

**Acceptance Criteria:**
- A slide rendered natively is visually indistinguishable from the same slide in PowerPoint when viewed on the projector (verified by side-by-side screenshot comparison test)
- All 6 transition types animate at 60 fps with no jank on a mid-range laptop
- Text zones respect font size, weight, alignment, and colour from the imported `.pptx` theme
- Slide background images render at exactly the Presentation Window resolution with no letterboxing
- Presentation Window renders correctly at 1280×720, 1920×1080, and 3840×2160 (4K projector mode)

#### F5.2 — Live Slide Editing from Controller
**Description:** Operator can edit slide text in the controller while presenting (for live correction), and the change appears on the Presentation Window immediately.

**Technologies:**
- `SlideEditor` React component in Controller Window with contentEditable fields
- `invoke('slide_update_text', { index, field, value })` Tauri command
- `AppEvent::SlideUpdate` re-emitted immediately to Presentation Window
- Dirty state tracked: unsaved changes flagged in controller UI

**Acceptance Criteria:**
- Text edit appears on the Presentation Window within one frame (< 16 ms) of the operator finishing a keystroke
- Edits do not modify the original imported file — stored as a delta layer in the `SlideDeck` JSON
- Undo/redo within the editor (Ctrl+Z / Ctrl+Y) works correctly
- Edits are lost on app restart unless explicitly saved ("Save as working copy" button)

#### F5.3 — Slide Library & Template System
**Description:** A library of built-in slide templates (title, bullet, two-column, quote, image-full, section break) that the operator can instantiate and populate without importing an external file.

**Technologies:**
- `templates/` directory in app bundle with JSON template definitions
- React `TemplatePicker` modal in Controller Window
- `invoke('deck_create_from_template', { template_id, variables })` command
- Variables system: `{{ presenter_name }}`, `{{ event_date }}`, `{{ title }}` interpolated on deck load

**Acceptance Criteria:**
- At least 8 built-in templates available at launch
- Operator can create a complete 5-slide deck from templates in under 2 minutes
- Template variables are editable from a structured form (not free text) so formatting is preserved
- Created deck is saved as a `.streamdeck` file (ZIP of JSON + assets) that can be re-opened

#### F5.4 — ProPresenter / Renewed Vision Protocol Bridge
**Description:** For events that continue using ProPresenter, StreamDeck can receive slide change notifications via ProPresenter's Network protocol (port 50001 TCP) and synchronise the overlay engine with ProPresenter's active slide.

**Technologies:**
- ProPresenter 7 Network API (TCP JSON protocol on port 50001)
- `ProPresenterBridge` Rust actor connecting as a Network Listener client
- Incoming `currentSlide` messages mapped to `AppEvent::SlideUpdate`
- Bridge mode is mutually exclusive with native presentation mode (TOML flag `[presentation] mode = "propresenter_bridge"`)

**Acceptance Criteria:**
- StreamDeck receives ProPresenter slide change events within 200 ms of a slide advance in ProPresenter
- Slide title and notes from ProPresenter are used to drive the Adaptive Overlay Engine (Phase 3.4)
- Bridge reconnects automatically if the ProPresenter machine restarts (with 5-second polling retry)
- If ProPresenter is not reachable, `AppEvent::BridgeLost` is emitted and overlay engine shows the last known slide title

---

## Phase 6 — Intelligence & Automation

**Goal:** Add automated scene switching, stream quality optimisation, and operator assistance features that reduce cognitive load during live events.

**External tools still active after phase:** None required — all tools are optional

### Features

#### F6.1 — Automated Scene Switching
**Description:** Define trigger rules that automatically switch scenes based on slide type, slide index, or elapsed time. The operator sets rules in advance; StreamDeck executes them during the event.

**Technologies:**
- `TriggerRule` TOML schema: `when = { slide_type = "Title" }`, `then = { scene = "presenter-wide" }`
- Rule engine evaluated on every `AppEvent::SlideUpdate`
- `AutomationMode` enum: `Manual | SuggestOnly | FullAuto`
- In `SuggestOnly` mode: overlay in controller shows "Suggested: switch to keynote-cam" with one-click confirm

**Acceptance Criteria:**
- Trigger rule evaluates and fires within one frame (33 ms) of the triggering event
- Rule conflicts (two rules match the same event) are detected at load time and reported as a warning, not silently resolved
- `SuggestOnly` mode suggestion appears in the controller with a 3-second auto-dismiss countdown
- Automation history is logged (timestamp, rule, action taken) and viewable in the controller
- Rules can be enabled/disabled individually at runtime without restarting

#### F6.2 — Adaptive Bitrate Streaming
**Description:** Monitor real-time network conditions and adjust encoder bitrate dynamically to maintain stream quality within the available uplink bandwidth.

**Technologies:**
- GStreamer `whipclientsink` built-in TWCC (Transport-Wide Congestion Control) feedback
- `BitrateController` Rust struct consuming TWCC reports
- Bitrate ladder: 6000, 4500, 3000, 2000, 1500 Kbps steps with hysteresis (3 degradations before step-down, 10 stable readings before step-up)
- `AppEvent::BitrateAdapted { from_kbps, to_kbps, reason }` emitted on each change

**Acceptance Criteria:**
- Bitrate adapts within 3 seconds of a simulated network constriction to 50% of baseline (netem test)
- Step-down is faster than step-up (avoid oscillation): 3 bad readings → step-down, 10 good readings → step-up
- Minimum floor bitrate configurable in TOML (default: 1000 Kbps); stream stops with `AppEvent::StreamQualityUnsustainable` if floor is breached
- Controller shows the current bitrate target and a reason indicator ("network congestion" / "stable")
- Adaptation does not cause visible stream interruption (no IDR frame storm)

#### F6.3 — Event Runsheet Integration
**Description:** Import a structured event runsheet (CSV or JSON) that pre-programs scene switches, slide advances, and lower-third titles at specific times or cues, enabling a single operator to run a complex multi-hour event.

**Technologies:**
- `Runsheet` JSON schema: `[{ time_offset_ms, cue_type, payload }]`
- `RunsheetEngine` Rust actor with `tokio::time::sleep` scheduling
- Cue types: `SceneSwitch`, `SlideAdvance`, `LowerThirdShow`, `LowerThirdHide`, `StreamStart`, `StreamStop`
- Controller UI: timeline view showing upcoming cues with 10-second warning

**Acceptance Criteria:**
- Cue fires within ±100 ms of scheduled time offset
- Runsheet can be paused (freezing the clock) and resumed from the current position
- Missed cues (operator paused for longer than the gap) are skipped with a log entry, not batch-fired
- Controller timeline highlights the next 3 upcoming cues, with a 10-second countdown for the immediate next
- Runsheet can be edited live (add/remove/edit cues) while running

---

## Cross-Cutting Concerns (All Phases)

### Observability

| Signal | Source | Destination | Update Rate |
|---|---|---|---|
| Pipeline dropped frames | GStreamer `QoS` bus message | `AppEvent::QosWarning` | Per drop |
| Encoder bitrate | `whipclientsink` stats | `AppEvent::StreamHealth` | Every 2 s |
| Source latency | NDI timestamp delta | Controller preview badge | Every 1 s |
| Bus lag | `broadcast::Receiver::len()` | `AppEvent::BusLag` | Per detection |
| Memory usage | `sysinfo` crate | Controller status bar | Every 5 s |

### Security Boundaries

- WHIP bearer tokens stored in OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service) via `keyring` crate — never written to TOML plaintext
- Tauri capability files grant the minimum required permissions per window: Presentation Window receives `listen` events only, no `invoke` permissions
- WHIP server (F2.4) binds to `127.0.0.1` by default; LAN binding `0.0.0.0` requires explicit TOML opt-in
- Screen capture permissions (F3.2) are requested once at startup with OS permission prompt; never silently requested

### Testing Strategy

| Test Type | Tool | Scope |
|---|---|---|
| Unit tests | `cargo test` | Rust state machines, parsers, classifiers |
| Integration tests | GStreamer test pipelines | Pipeline segments (ingest, compositor, egress) |
| Frontend tests | Vitest + Testing Library | React components, event handler logic |
| E2E tests | Tauri `WebviewWindow` in test mode | Full user flows (import deck, switch scene, go live) |
| Performance tests | `criterion` benchmarks | IPC latency, compositor throughput, bitrate |
| Platform smoke tests | CI matrix (macOS, Windows, Ubuntu) | Build, launch, basic pipeline start/stop |

### Feature Flag System

All phases are gated by Rust feature flags:

```toml
# Cargo.toml features
[features]
default = ["phase1", "phase2"]
phase1 = []
phase2 = ["dep:gstreamer", "dep:gst-plugin-ndi"]
phase3 = ["phase2"]
phase4 = ["phase3"]
phase5 = ["phase4"]
phase6 = ["phase5"]
```

This ensures a Phase 2 build compiles and runs without Phase 4 code being present, simplifying testing and allowing incremental releases.

---

## Dependency Manifest

### Rust Crates

| Crate | Version | Phase First Used | Purpose |
|---|---|---|---|
| `tauri` | 2.11.x | P0 | Application shell, multi-window, IPC |
| `tokio` | 1.38 | P0 | Async runtime, broadcast channels, timers |
| `serde` + `serde_json` + `toml` | latest | P0 | Serialisation, config |
| `notify` | 6.x | P0 | Config file hot-reload |
| `keyring` | 3.x | P0 | OS keychain for secrets |
| `gstreamer` | 0.23.x | P2 | Media pipeline |
| `gstreamer-app` | 0.23.x | P2 | appsrc/appsink bridges |
| `gstreamer-video` | 0.23.x | P2 | Video caps, format conversion |
| `gst-plugin-ndi` | 0.14.x | P2 | NDI 6 source/sink |
| `gst-plugin-webrtc` | 0.14.x | P2 | whipserversrc, whipclientsink |
| `axum` | 0.8 | P2 | WHIP HTTP signalling server |
| `pdfium-render` | 0.8.x | P1 | PDF slide rasterisation |
| `sysinfo` | 0.30.x | P0 | Memory/CPU observability |
| `criterion` | 0.5 | P0 | Performance benchmarks |

### Frontend Packages

| Package | Version | Phase | Purpose |
|---|---|---|---|
| `react` + `react-dom` | 18.x | P0 | UI framework |
| `typescript` | 5.x | P0 | Type safety |
| `vite` | 5.x | P0 | Build tool |
| `@tauri-apps/api` | 2.x | P0 | Tauri IPC bindings |
| `zustand` | 5.x | P1 | Frontend state (slide deck, scene list) |
| `chart.js` | 4.x | P4 | Bitrate sparkline |
| `@dnd-kit/core` | 6.x | P3 | Scene drag-and-drop reorder |

### External SDKs

| SDK | Version | Phase | Platform |
|---|---|---|---|
| NDI SDK | 6.3 | P2 | macOS, Windows, Linux |
| GStreamer runtime | 1.26.x | P2 | macOS (Homebrew), Windows (MSVC installer), Linux (apt) |

---

## Phase Gate Summary

| Phase | Milestone | External Tools Retired | Key Deliverable |
|---|---|---|---|
| 0 | Skeleton + CI | None | Two-window app, event bus, config loader |
| 1 | Presentation Engine | None (parallel) | Projector screen driven from controller |
| 2 | NDI + WHIP Ingest | None | Camera previews in controller |
| 3 | Compositor + Scenes | None | Live composited preview, adaptive overlay |
| 4 | Streaming Egress | Mevo Studio (streaming only) | Go Live to Facebook/YouTube via WHIP |
| 5 | Native Presentation | ProPresenter (optional) | Full slide authoring and rendering |
| 6 | Intelligence | All external tools optional | Automated scene switching, adaptive bitrate |

