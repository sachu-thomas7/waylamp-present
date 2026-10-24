# Roadmap

Feature IDs (F0.x, F1.x) are stable and cited by code comments, tests and other docs.

## Vision

A cross-platform desktop app (Windows, macOS, Linux) for controlling presentations: import a PPTX or PDF, show it on a projector output, and run it from a separate controller window with speaker notes, a timer and scene media (image/video). Multiple displays are supported. Built on Tauri v2, Rust/Tokio and React/TypeScript.

Live production features (camera ingest, compositing, streaming) are not part of `main`; see "Live production" below.

## Architecture invariants

1. Long-running work runs in its own Tokio task; UI events never block it.
2. State changes flow only through the `tokio::sync::broadcast` `AppEvent` bus.
3. Behaviour is configured in TOML (`streamdeck.toml`); secrets never go in TOML.
4. New subsystems are additive and must not break existing flows.
5. Capability minimalism: the controller window can invoke commands; the presentation window can only listen.
6. No `unwrap()` in command handlers; every command returns `Result<T, AppError>`.

## Done

### Phase 0 - Skeleton and toolchain

- **F0.1 Tauri v2 bootstrap**: a Rust backend plus two separate Vite/React/TypeScript frontends (`controller/`, `presentation/`); CI runs tests, clippy and typecheck on every PR.
- **F0.2 IPC command layer**: typed commands returning `Result<T, AppError>`, mirrored as TypeScript types; dispatch latency under 2 ms.
- **F0.3 Event bus**: a typed `AppEvent` broadcast channel bridged to both windows; lagging receivers resync and a `BusLag` event is emitted.
- **F0.4 TOML config**: `streamdeck.toml` loaded at startup, hot-reload watcher, forward-compatible defaults; invalid TOML never panics.

### Phase 1 - Presentation engine

- **F1.1 Presentation window and slide renderer**: a listen-only window renders title, body, background and slide number from bus events, scaled to its size; a JS error in it does not affect the controller.
- **F1.2 Slide state machine**: Rust owns current index, deck, timer and metadata; advancing past the end emits `DeckEnd` and rewinding before the start emits `DeckStart` (no wrap-around); state is persisted so a restart restores position.
- **F1.3 Controller slide panel**: thumbnail grid, current and next previews, speaker notes (never shown on the projector), elapsed timer, progress and keyboard shortcuts (Space/arrows/number jump) that apply only in the controller.
- **F1.4 Deck import (PPTX/PDF)**: extracts title, body, notes and a background image per slide; unsupported layouts import with a blank body and a `layout_unsupported` flag shown in the controller; failures are reported per slide. PDF import needs the bundled PDFium library.
- **F1.5 Multi-display assignment**: enumerates displays with hotplug detection, lets the operator assign the presentation window to a display, falls back to the primary display (with a `DisplayLost` warning) when it disconnects, and persists the choice in `streamdeck.toml`.

Hardening since then (media protocol, capability tightening, atomic persistence, import limits, fuzzing) is described in `CLAUDE.md` and tracked in `unverified_items.md`.

## Planned (presentation side)

- **Release**: signed Windows installer, a clean-machine install test, Linux packages (`.deb`/AppImage) and macOS once signing is possible. See `release_process.md` and `release_bundling_plan.md`.
- **Import fidelity**: verify PPTX/PDF rendering against real-world decks and record a fidelity matrix; broaden supported layouts.
- **Accessibility**: keyboard-only operation of the controller, screen-reader labels on every control, and sufficient contrast.
- **Testing**: real-app end-to-end coverage (local only), more frontend component tests and longer fuzzing campaigns. See `test_framework_roadmap.md`.
- **Native slide rendering (candidate)**: richer text/theme rendering and slide transitions (Cut, Fade, FadeBlack, SlideLeft, SlideRight, ZoomIn), tested at 1280x720, 1920x1080 and 3840x2160.
- **Live slide editing (candidate)**: correct slide text from the controller while presenting; edits stored as a delta layer and never written to the imported file.
- **Templates (candidate)**: built-in slide templates and a re-openable working-copy file format.

Candidates are not committed scope.

## Live production

Camera ingest (NDI, WHIP phone cameras), the compositor, scenes and multi-destination streaming are developed on the long-lived branch `feature/live-production`. It will be merged into `main` only when ready; until then `main` stays presentation-only. Items that depend on it are tracked on that branch.
