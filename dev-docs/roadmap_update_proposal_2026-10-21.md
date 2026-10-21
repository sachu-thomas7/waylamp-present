# Roadmap update proposal (2026-10-21) — for review, not yet applied

Source: `dev-docs/research/competitor-analysis-2026-10-21.md` (OBS, FreeShow, ProPresenter, Mevo, plus
source reads of CasparCG, voctomix, MediaMTX, Companion, OpenLP, gst-plugins-rs, Sunshine, Cap,
DistroAV; raw notes in `dev-docs/research/notes/`). The spec of record stays
`streamdeck_product_roadmap_2026.md` until you approve edits below.

## Decisions needed from you

1. **Audience.** Talks/events only, or worship services too? Songs/lyrics/scripture is the largest
   ProPresenter gap and the roadmap names no audience. If out of scope, say "no ProPresenter
   dependency" means decks, not songs.
2. **Independence wording.** Pick one phase for "zero dependency" (spec says Phase 5 in the summary,
   Phase 4/5/6 in the gate table) and define it as a test: a full service run, streamed and
   recorded, with Mevo Studio and ProPresenter uninstalled. Write down the permanent exceptions
   (NDI runtime, GStreamer plugin set, cameras, OS keychain/signing).
3. **GStreamer baseline.** Roadmap targets 1.26; this dev machine runs 1.28.3 (adds webrtcsink
   renegotiation, VA encoders, WHEP). Choose one.
4. **F3.1 latency.** Accept "about one output frame" (CPU compositor 33.4 ms p50 for the oldest
   input; d3d12compositor ~49.5 ms) in place of "< 33 ms", or keep the target open until real
   sources exist.

## Proposed sequencing

1. Hygiene already done: CI runs `cargo fmt`; F3.1 shipped (`1a1cdf8`, `226d5db`).
2. **F3.2 spike (1 week, two arms):** frame injection of rasterised slides via `appsrc`, and a
   dedicated program-render window at stream resolution. OS capture (WGC / ScreenCaptureKit /
   PipeWire) stays a fallback behind a `CaptureTarget` enum. Windows WGC is verified available
   locally (`d3d12screencapturesrc window-handle`).
3. **F3.3 spike first:** voctomix-style timestamped control-source transitions with inactive pads
   parked at alpha 0 / zorder -1, tested on `d3d12compositor`. Cut, fade, fade-to-black (its own
   transition), slide/push (rename "side-wipe"). Measure fade smoothness at 30 fps before building
   scene UI. Fallback if frame pacing fails: revisit a wgpu mixer.
4. **F3.4:** static PNG alpha fade, then Rust-rendered RGBA lower thirds. Transparent webview / CEF
   only as a time-boxed spike. Defer SlideClassifier auto-overlay.
5. **Phase 4 re-sequenced:** encoder probe (test-encode, isolated, vendor-aware, TOML override) →
   RTMP and SRT egress with per-destination leaky queues, reconnect and health → local recording →
   WHIP fan-out as an extra transport. Pull send-queue congestion + dropped-frame % + basic bitrate
   reduction from F6.2 into this phase.

## New or moved items

| Item | Change |
|---|---|
| F4.4 SRT ingest | Move from optional/Phase 4 to Phase 2; verify with Mevo Start/Core/Plus (SRT is the only output on every model; NDI only on Start/Core) |
| New F3.5 Audio | Capture (NDI, phones, local device), mix, meters, A/V sync, Opus for WebRTC, AAC for RTMP |
| New F3.6 Control API | Rust-owned WebSocket + HTTP bridge on the event bus, scoped bearer tokens in keychain, state on connect, first-party Companion module (booleans: presenting, blackout, stream live, source lost) |
| New F1.6 Stage display + countdown | Bus-fed; we already hold the data |
| New F4.5 Local recording | Crash-safe fragmented MP4 off the encoded stream |
| New phase: Distribution | GStreamer bundling + size budget, NDI runtime policy, macOS notarisation + CI, signing, AV exclusions |
| F1.4 / F5.1 PPTX | Replace "indistinguishable from PowerPoint" with a documented fidelity matrix (PDFium raster + structured text); drop `.key` or route via PDF export |
| F5.2 / F5.3 / F6.1 | Defer until the above ship |
| Maybe later | NDI program output with key/fill (`ndisink` exists), PTZ, PJLink, Planning Center import |

## Corrections to our own docs

- CLAUDE.md lists Rust 1.78; verify against `rustc --version` and the lockfile (only blocks us if we
  build gst-plugins-rs 1.92 or vendor Cap 1.88).
- `scap-direct3d` (MIT) depends on `cap-d3d-adapter`, which is AGPL by Cap's root text: reimplement
  the ~60-line DXGI adapter pick or use the `windows` crate directly.
- `advertises_new_format` in `ndi.rs` can compare width/height/fps/FourCC (what gst `ndisrc` and
  DistroAV do); expose NDI `extra_ips` and groups in config (mDNS does not cross subnets).
- Research caveat: much is code-verified from shallow clones, but nothing was benchmarked, Mevo and
  ProPresenter rest on docs/reports, and NDI runtime redistribution terms are unread.

## Licensing rule of thumb

Reference only (copyleft): OBS, FreeShow, CasparCG, OpenLP, Sunshine, DistroAV, Cap outside its MIT
crates. Usable with notices: voctomix, MediaMTX, Companion core (MIT), gst-plugins-rs (MPL-2.0,
file-level). Implement importers and protocols from public specs, never from GPL importer code.
