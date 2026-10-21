# StreamDeck: internal roadmap and implementation digest

Sources (all local, no web research): [Roadmap](file:///C:/Users/sachu/Projects/streamdeck/dev-docs/streamdeck_product_roadmap_2026.md) (v1.0, May 2026, "Authoritative Specification"); [CLAUDE.md](file:///C:/Users/sachu/Projects/streamdeck/CLAUDE.md) (working tree as of 2026-10-21, includes uncommitted edits; this is the status record); [test roadmap](file:///C:/Users/sachu/Projects/streamdeck/dev-docs/test_framework_roadmap.md) (headings only). "Roadmap" and "CLAUDE.md" below refer to these. Note: roadmap says F-numbers F0.1-F6.3; the roadmap has Phase 6, though CLAUDE.md's wording and the user's prompt say "by Phase 5" for the independence goal.

## 1. Per-phase feature list and status

### Takeaway
Phases 0-1 shipped; Phase 2 (F2.1-F2.4) and F3.1 shipped but camera/phone paths are mostly verified only with simulation or test sources; F3.2-F3.4 and all of Phase 4, 5, 6 are not started. Real NDI has not been tested, because the NDI SDK is absent.

### Cited Findings
Status key: SHIPPED = implemented and verified in repo; SIM = implemented, real hardware/SDK path unverified; NOT STARTED = no code (CLAUDE.md says none or doesn't mention it).

**Phase 0 (skeleton)** - all SHIPPED (CLAUDE.md "Phase 0 ... done")
- F0.1 Tauri v2 bootstrap, two windows, Vite/React/TS, CI (test, clippy, typecheck). CI matrix is ubuntu + windows only; macOS dropped "until an Apple account exists". Roadmap AC of "signed installer on macOS and Windows" is therefore NOT met (see section 5).
- F0.2 IPC command layer (`Result<T, AppError>`, no unwrap in handlers; enforced by `verify.sh invariants`).
- F0.3 Event bus (`broadcast`, cap 256, `AppEvent`).
- F0.4 TOML config + `notify` hot-reload + keychain for secrets. Caveat: hot-reload only publishes `ConfigReloaded`; "nothing re-reads it yet" (CLAUDE.md Stack row). So the roadmap AC "config change reflected within 500 ms" is only partly met.

**Phase 1 (presentation engine)** - all SHIPPED
- F1.1 Presentation window + slide renderer (fullscreen always; `always_on_top` only with 2+ displays; Esc always registered; OS-close hides instead of destroying).
- F1.2 Slide state machine (advance/rewind/jump/timer, DeckEnd/DeckStart, persisted/restored).
- F1.3 Controller slide panel (thumbnails, current/next, notes, timer, shortcuts).
- F1.4 Deck import: `.pptx` (own XML parse via quick-xml) and `.pdf` (pdfium-render). Roadmap also lists `.key` (Keynote); CLAUDE.md does not mention Keynote import, so treat it as absent/ambiguous. `layout_unsupported` flag end-to-end.
- F1.5 Multi-display assignment (enumeration, 2s hotplug polling, persisted, primary fallback).

**Phase 2 (ingest)**
- F2.1 GStreamer PipelineService - SHIPPED but behind the non-default `gstreamer` cargo feature; `UnavailableBackend` in default builds. Needs manual `cargo tauri dev --features gstreamer`.
- F2.2 NDI discovery/ingest - SIM. `SimulatedDiscovery` for tests; `UnavailableDiscovery` by default; optional `ndi` feature. Unverified without NDI SDK: real discovery, "New Format" detection (`advertises_new_format` always false), 25 fps preview.
- F2.3 Camera preview grid - SIM (test patterns/`videotestsrc`). 320x180 PNG+base64 at 8 fps via dedicated `PreviewHub` channel (deliberate deviation from "event bus only"). Layouts 2x2/2x3/2x4. Synthetic 8-stream CPU about 5% of one core (release); excludes webview decode and real NDI decode.
- F2.4 Phone WHIP ingest (axum 0.8 `POST /whip/{slot}`, 409/503/401, QR, token in keychain, 4 slots, H.264 Baseline else VP8, ICE-state LinkMachine with 10s grace) - partly verified. Real WebRTC loopback test passes (`tests/whip_gst_e2e.rs`, ignored by default). NOT verified: Larix/real phones, real network drop (only simulated), 3s connect target (loopback debug build about 4s). Server is disabled by default (0.0.0.0:8088 once enabled). Roadmap says it binds 127.0.0.1 by default; implementation is disabled-by-default instead (minor spec deviation).

**Phase 3 (compositor and scenes)**
- F3.1 GStreamer compositor - SHIPPED with caveats (commit 1a1cdf8; `compositor.rs` has uncommitted edits). Pure geometry/validation (<=8 sources), element selection (d3d12compositor on Windows; glvideomixer+gldownload on macOS/Linux; CPU `compositor` fallback), runtime add/remove via BLOCK_DOWNSTREAM probe. Sources are `videotestsrc` stand-ins (or untested `ndisrc`). Measured on author's box (Windows, RTX 2060 + Intel UHD, d3d12compositor installed): 3 sources 30.0 fps, max inter-frame gap 48 ms steady / 53-60 ms across runtime add+remove. See section 3 for the latency/CPU numbers. glvideomixer path untested.
- F3.2 Presentation slide capture source - NOT STARTED.
- F3.3 Scene definition system (TOML scenes, SceneManager, 4 transitions, switch IPC) - NOT STARTED. Partial seed only: `[[scene.<id>.sources]]` TOML shape exists from F3.1, and CLAUDE.md says F3.3 will add overlays/transition beside `sources`. Do not confuse with the unrelated, already-shipped "scene media source" in `scene.rs` (image/video push to the projector).
- F3.4 Adaptive overlay engine - NOT STARTED (CLAUDE.md Stack row).

**Phase 4 (streaming egress)** - all NOT STARTED: F4.1 WHIP encode pipeline + hardware encoder chain; F4.2 multi-platform fan-out; F4.3 stream control panel; F4.4 SRT ingest relay (optional). (CLAUDE.md Stack row says "compositor/WHIP (F2.4+) not started" in older text; the more recent status entries supersede it for F2.4/F3.1 only.) Note "WHIP" in shipped F2.4 is ingest only; there is no egress code.

**Phase 5 (presentation independence)** - all NOT STARTED: F5.1 native slide renderer (6 transitions, theme TOML, 4K); F5.2 live slide editing with undo/redo; F5.3 slide library/templates (8 templates, `.streamdeck` ZIP); F5.4 ProPresenter 7 network bridge.

**Phase 6 (intelligence/automation)** - all NOT STARTED: F6.1 automated scene switching (Manual/SuggestOnly/FullAuto); F6.2 adaptive bitrate via TWCC; F6.3 event runsheet (timed cues).

**Non-roadmap shipped work (not feature ids)**
- Release step 1 (2026-10-19): PDFium bundling via `release/deps.json` (pinned, SHA-256 verified), `scripts/fetch-deps.sh`, `scripts/check-bundle.sh`, `cargo tauri build --config release/bundle.pdfium.conf.json`. Windows NSIS installer built and verified (6.8 MB); Linux layout untested; no macOS.
- Test framework T0-T2 done (see section 7).
- Roadmap's Phase Gate table: Phase 4 retires "Mevo Studio (streaming only)"; Phase 5 retires ProPresenter "(optional)"; Phase 6 "All external tools optional".

### Inferences
- Zero-dependency goal is far off: only the projector-slide half of ProPresenter's job is done (import and display). Nothing of Mevo Studio's streaming job exists yet (no encode/egress).
- The shipped product today is effectively a PPTX/PDF slide controller + projector output with a simulated camera-monitor/compositor scaffold behind a feature flag.

### Gaps
- CLAUDE.md does not say whether F1.4 handles `.key`. Roadmap lists it.
- Roadmap Phase 6 exists, but the user prompt's "F0-F5+" and CLAUDE.md's "Phase 5" goal wording mean the "zero dependency" claim is stated as Phase 5 in the Executive Summary, while the Phase Gate table says Phase 6 is when "all external tools optional". Internal inconsistency in the spec.

## 2. Architecture in a nutshell

### Takeaway
Tauri v2 shell with a Rust/Tokio backend, an event-bus-centric design, two independent React windows (full-permission controller; listen-only presentation), and GStreamer as an optional compile-time feature running on dedicated threads.

### Cited Findings
- Stack: Tauri 2.12 crates (roadmap says 2.11.x), Rust 1.78 + Tokio, two separate Vite+React18+TS projects (`controller/`, `presentation/`), Zustand per window, TOML config, `keyring` for secrets. GStreamer targeted 1.26.x in roadmap. CLAUDE.md does not state "1.28.3"; the brief mentioned it, but I found no such string in CLAUDE.md. The F3.1 section only mentions `d3d12compositor` installed on the author's Windows box.
- Backend modules (CLAUDE.md repo map): `app.rs` (builder, window creation, capability wiring, event bridge), `bus.rs`, `events.rs` (AppEvent, "single source of truth"), `error.rs`, `config*.rs`, `slide*.rs`, `pipeline.rs` (PipelineService actor + `PipelineBackend` trait: SimulatedBackend / UnavailableBackend / feature-gated GstBackend), `scene.rs` (media source state), `deck_importer.rs` (spawn_blocking), `display.rs`, `ndi.rs`, `preview.rs`, `whip.rs` + `whip_server.rs`, `compositor.rs`, `commands/` (thin wrappers).
- Invariants (CLAUDE.md, 7): (1) pipeline in dedicated Tokio task, UI never blocks it; (2) event bus only for state changes; (3) config as code, secrets in OS keychain; (4) coexistence first (additive); (5) capability minimalism (controller full; presentation listen-only, `core:event:default` only; every dynamic window needs its own capability file); (6) no `.unwrap()` in command handlers; (7) `app.rs` gated `#[cfg(not(test))]`. Roadmap adds "Observability from Day One" as invariant 5 (GStreamer bus messages to UI).
- Known, documented deviation: preview frames (up to 64/s) use a dedicated `PreviewHub` channel + `emit_to("controller","preview-frame")` instead of the bus.
- Roadmap's phase feature-flag scheme (`phase1..phase6`) differs from the implementation's non-default `gstreamer`, `ndi`, `whip` cargo features (CLAUDE.md).
- Presentation window dev/prod wiring: External `http://localhost:5174` in dev (needs `remote.urls` capability), `WebviewUrl::App` in release; `controller/dist` embeds `presentation/dist` via merge script. Five manual-launch bugs in 2026-09 that `verify.sh` could not catch; policy: manual `cargo tauri dev` launch required for multi-window/IPC changes.
- Gate: `scripts/verify.sh` per-feature + phase; CI = rust job (ubuntu, windows) + typecheck; pre-push hook runs `verify.sh core`.

### Inferences
- Because default builds have no GStreamer, anyone running a stock build sees none of Phase 2/3 working; shipping those requires bundling GStreamer (Release "later steps", not done).

### Gaps
- Where the CLAUDE.md "GStreamer 1.28.3 element availability" claim comes from is unclear; not in the file.

## 3. Complexity and risk hot spots (with roadmap ids)

### Takeaway
Risk concentrates in the not-yet-started capture/overlay/transition/encode work (F3.2, F3.3, F3.4, F4.x) and in the NDI/phone paths that are implemented but untested on real hardware.

### Cited Findings
- **Presentation webview capture into GStreamer (F3.2)**: three platform-specific paths (macOS `avfvideosrc` window ID, Windows `d3d12screencapturesrc` DXGI duplication, Linux `pipewiresrc` portal) plus an IPC `appsrc` fallback; must be lossless-ish (no JPEG), 30 fps, within 2 frames of slide change; OS permission flows (macOS Screen Recording). NOT STARTED, highest uncertainty.
- **Transparent-webview overlay capture (F3.4)**: transparent always-on-top Tauri webview captured with the same F3.2 mechanism and alpha-composited; a transparent always-on-top window also interacts with the fullscreen/Alt+Tab issues documented in F1.1. Plus `SlideClassifier` heuristics and WCAG 4.5:1 check. NOT STARTED.
- **Transitions on GStreamer compositor (F3.3)**: Cut/Fade/Fade-to-Black/Side-wipe in 0.5 s with re-patching in 2 frames (66 ms) while keeping glitch-free runtime add/remove (F3.1 shows pad add/remove already causes 53-60 ms max gaps). NOT STARTED.
- **Compositor latency/CPU vs ACs (F3.1)**, measured and documented in CLAUDE.md: latency AC (<33 ms) "only partly met". CPU compositor with zero-deadline sink: 33.4 ms p50, ~35 ms p95, ~37 ms max (oldest of 3 inputs); d3d12compositor about 49.5 ms p50, about 53 ms p95; with the default 20 ms sink deadline every figure is about 16 ms worse. CPU AC (<60%) "ambiguous": 4 NV12 1080p sources, whole machine about 8% (CPU compositor) / 5.6-6.6% of 12 cores (d3d12), but per-core: compositor 90-99% of one core (CPU) or 67-80% (d3d12) including source generation. glvideomixer path, real display sink, GPU utilisation, real source decode all untested.
- **Cross-platform capture/encode differences (F3.1, F3.2, F4.1)**: compositor element differs per OS; encoders `vtenc_h264` / `mfh264enc` / `vaapih264enc` / `x264enc` chain (F4.1, not started); GStreamer installation methods differ (Homebrew / MSVC installer / apt). Linux and macOS are untested (no Linux machine; no Apple account/macOS CI).
- **NDI SDK dependency (F2.2/F2.3)**: SDK and gst-plugin-ndi not available in dev/CI; real discovery, New Format, 25 fps all unverified; also a licensing/redistribution question for bundling (not addressed in the files).
- **WHIP phone ingest (F2.4)**: real phones, ICE restart/5 s network interruption, 3 s connect target (about 4 s measured on loopback debug) unverified; gst-plugin-webrtc + openh264 availability; known pitfall in `gst::parse::launch` empty property values (`stun-server=""`).
- **Hardware encoders / egress (F4.1-F4.3, F6.2)**: encode-once fan-out via `tee` to multiple `whipclientsink` (<=1.15x bandwidth); per-destination state machine with back-off; Facebook/YouTube WHIP acceptance asserted in roadmap ("as of 2025") but unverified; TWCC-driven bitrate control. NOT STARTED.
- **PPTX/PDF fidelity (F1.4, F5.1)**: own PPTX parser (quick-xml), placeholder-only text extraction, background image only; custom masters flagged `layout_unsupported`; a real bug (Event::Empty placeholders) hid for a while; PDF requires bundled native pdfium (and PNG feature bug found). F5.1 demands "visually indistinguishable from PowerPoint" with theme fonts/transitions: very high fidelity risk. Keynote import unclear.
- **Multi-window/IPC (F1.1, F1.5)**: dev vs release URL wiring; remote-origin IPC allowlist; fullscreen/always-on-top/Escape/close-interception trap-avoidance; mirrored single-display (e.g., ClickShare) cannot be fully solved ("hardware limitation").
- **Antivirus/code-signing friction**: Windows AV flagged the dev binary (`VHO:Trojan-PSW`, keyring + unsigned); Kaspersky/Norton delete fresh exes.
- **Verification gap**: `verify.sh` is structural + unit; real-app problems found only by manual launch; E2E (T4) is not yet built.

### Inferences
- The hardest single dependency is F3.2: F3.4, scene transitions' previews, and a "slide in the stream" feature all hang off it, and no platform implementation exists.

### Gaps
- No measured numbers exist for real NDI, real phone ingest, encode CPU, or any macOS/Linux behaviour.

## 4. Capabilities a presentation+production operator would expect: roadmap coverage

### Takeaway
The roadmap is a pipeline-and-slides roadmap. It covers scenes, multi-destination streaming, stream health and a runsheet; it has no audio at all, no recording, no remote-control API, and no lyrics/scripture/songs.

### Cited Findings (status = present / partial / absent in the ROADMAP; implementation status in brackets)
| Capability | Roadmap | Detail |
|---|---|---|
| Audio mixing / monitoring / meters | ABSENT | No audio feature or AC anywhere; roadmap and CLAUDE.md mention only video elements. Mevo cameras provide their own audio in practice but the roadmap does not say how it is routed. |
| Stream health | PRESENT (F4.1, F4.3, Observability table) | `StreamHealth` every 2 s (bitrate, dropped frames, rtt), sparkline, dropped-frame thresholds, per-destination green/amber/red (F4.2). [not started] |
| Multi-destination streaming | PRESENT (F4.2) | Encode once, `tee` to multiple WHIP endpoints; add/remove live. WHIP only: no RTMP egress ("replaces all RTMP egress"), so platforms without WHIP are uncovered (F4.2's "RTMP-over-WHIP relay" is vague). [not started] |
| Recording to disk | ABSENT | No local recording/ISO recording feature. |
| Remote control API / Companion / Stream Deck hardware | PARTIAL/ABSENT | Only `invoke('scene_switch')` "enables future automation" (F3.3); no HTTP/WebSocket/OSC API, no Companion module, no MIDI. F5.4 is inbound from ProPresenter only. |
| Timers / stage display | PARTIAL | Presentation elapsed timer (F1.3, shipped), runsheet countdown (F6.3). No countdown-to-event, no stage display / confidence monitor output for speakers. |
| Lyrics / scripture / songs | ABSENT | Nothing; only title/bullet/two-column/quote templates (F5.3). This is the biggest ProPresenter feature gap for church use. |
| Media library / playlists | PARTIAL | Scene media push (image/video, 10 MB/500 MB limits) shipped outside roadmap ids; "Slide Library" F5.3 is templates, not a media library; no playlists, looks, or video loops management. |
| Templates / themes | PRESENT (F5.3, F5.1) | 8 built-in templates, `SlideTheme` TOML, `.streamdeck` ZIP. [not started] |
| Lower thirds | PARTIAL | Automatic slide-driven overlay (F3.4); manual lower-third show/hide only as runsheet cue (F6.3 `LowerThirdShow/Hide`); no operator-authored lower-third/graphics editor. |
| Undo/redo | PARTIAL | Only inside the slide editor (F5.2). |
| Backups / autosave / crash recovery | PARTIAL | Slide position persistence (F1.2, shipped); scene/config in TOML; F5.2 "edits lost on restart unless saved". No backup/restore of library or settings. |
| Licensing / updates / installers / signing | PARTIAL | See section 5. |
| Multi-camera switching with transitions | PRESENT (F3.1, F3.3) | 4 transition types; no ATEM-style preview/program (T-bar, preview bus) workflow is specified. Ambiguous. |
| PTZ camera control | ABSENT | Not mentioned. |
| NDI output (send) | ABSENT | Roadmap only ingests NDI; Phase 5 text says "NDI output still used" referring to Mevo cameras. Program NDI/virtual-camera output not planned. |
| Stage/confidence monitor, multi-screen audience looks | ABSENT | Single presentation output (plus display assignment). |
| Slide formats | PARTIAL | `.pptx`, `.pdf` (shipped), `.key` (roadmap; unclear in CLAUDE.md); no Google Slides, images folder, or video import beyond scene media. |
| Automation / triggers | PRESENT (F6.1, F6.3) | Slide-type scene rules, runsheet cues. [not started] |
| Adaptive bitrate | PRESENT (F6.2) | TWCC; [not started] |
| Live graphics / countdown / chat / multi-viewer comments | ABSENT | Not mentioned. |
| Security | PRESENT | Keychain tokens, listen-only presentation window, capability minimalism, WHIP LAN opt-in; WHIP ingest token is optional. |

### Inferences
- Audio is the largest hole for a "zero dependency on Mevo Studio" claim, since any streaming replacement must carry sound; the roadmap's Phase 4 acceptance criteria are video-only.
- "Recording" and "RTMP egress" are both absent, so the product cannot replace tools that churches/events use for archival or for non-WHIP platforms.

### Gaps
- The roadmap's silence is not proof of intent; some items may be assumed (e.g., audio passthrough in `whipclientsink`) but nothing is written.

## 5. Installers, auto-update, code signing, licensing

### Takeaway
No phase is dedicated to distribution. Only F0.1 mentions a signed installer; the only shipped distribution work is the unnumbered "Release step 1" (PDFium bundling, Windows NSIS).

### Cited Findings
- F0.1 AC: `cargo tauri build` "produces a signed installer on macOS and Windows without warnings" (Roadmap). CLAUDE.md: macOS dropped from CI "until an Apple account exists"; "code-sign for real distribution" is flagged as to-do after AV false positives; no signing evidence in the files.
- Release step 1 (CLAUDE.md): Windows NSIS installer 6.8 MB verified; Linux (.deb) layout untested; macOS not covered; bundle-budget file (Windows 10 MB). "Later steps (GStreamer, NDI) extend `REQUIRED` in check-bundle.sh": GStreamer and NDI bundling are planned but not done.
- No auto-update (Tauri updater), no licensing/activation, no telemetry/crash reporting, no onboarding/first-run beyond UI empty states: none appear in roadmap or CLAUDE.md. Roadmap's Observability table includes memory usage (`sysinfo`) only.

### Inferences
- Bundling GStreamer (with the right plugins) and the NDI runtime is a large unplanned workstream: size budget, licensing (NDI redistribution, openh264, x264 GPL), per-OS packaging.

### Gaps
- No stated policy on licensing/pricing or distribution channel.

## 6. Target users and use cases

### Takeaway
The roadmap names no audience (no mention of churches, events companies, or solo streamers as segments). Use is inferable only from the reference tools and features.

### Cited Findings
- Stated: "unifies local presentation control, multi-camera production, and live-streaming egress"; target is "zero dependency on proprietary production tools like Mevo Studio or ProPresenter" (Roadmap Executive Summary). Streaming targets: Facebook Live, YouTube, any WHIP platform. Cameras: Mevo (Start, Core, Go), NDI sources, phones via WHIP. Single-operator framing: "a single operator to run a complex multi-hour event" (F6.3); "reduce cognitive load during live events" (Phase 6 goal). Projector output + speaker notes imply speaker/presenter-led talks (slides + keynote cam), "keynote-cam", "presenter-wide" scene names, "Title/Visual/Content/Transition" slide classes (F3.4/F6.1). CLAUDE.md memory: solo dev moving manual to agentic.

### Inferences
- Implied segment: small venues/events/talks with Mevo cameras, ProPresenter or PowerPoint, streaming to Facebook/YouTube. A church (ProPresenter + Mevo + Facebook Live is a classic church stack) is plausible but unstated; lyrics/songs absence argues against worship-service as a primary target.

### Gaps
- No personas, pricing, competitive positioning, or market sizing in the files.

## 7. Quality and process facts (useful context)

### Cited Findings
- Tests: Rust integration harness (mock runtime), invariants-as-tests (28), controller/presentation Vitest component tests (T2.2-T2.4; presentation has 27+ tests), event-union exhaustiveness checks; E2E via Playwright over WebView2 (T4) is planned and local-only; visual regression (T5), false-green scanner (T6), extension points for Phase 2+ (T7) not done (test roadmap headings).
- Last commit: 1a1cdf8 "feat(F3.1) ..."; working tree has uncommitted `CLAUDE.md` and `src-tauri/src/compositor.rs` changes (git status). Memory note says earlier commits were unpushed as of 2026-10-19.
- Roadmap dependency table lists `chart.js`, `@dnd-kit/core`, `criterion`, `sysinfo`: none are confirmed used in CLAUDE.md status.

### Gaps
- Not checked: actual source code; all statuses come from CLAUDE.md prose.
