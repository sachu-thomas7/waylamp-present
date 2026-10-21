# Unverified items and owed checks (prioritized)

Single list of things shipped but not proven. Update on every session: move an item to "Verified"
with the date and how, or add new ones. Priority = risk to the product x cost of finding out late.
Last updated: 2026-10-21.

## P0 — verify before building more on top

| # | Item | Why it matters | How to verify | Blocked by |
|---|---|---|---|---|
| 1 | **Real NDI end to end** (F2.2/F2.3): discovery, add source, "New Format" renegotiation, 25 fps preview, source lost/return | Mevo Start/Core and most cameras depend on it; everything so far ran on simulation/`videotestsrc`. `advertises_new_format` is always false and `format` does not alter `ndisrc` | Install NDI SDK/runtime + gst-plugin-ndi; run `cargo tauri dev --features ndi,gstreamer` with an NDI test source (NDI Tools "Test Patterns") | NDI SDK install (user). Also read the NDI SDK license for runtime redistribution |
| 2 | **Keychain token persists across restart** (F2.4) | `keyring` silently falls back to a non-persistent mock if mis-featured; a lost token leaves the WHIP endpoint closed or open unexpectedly | Set token in Settings, quit, relaunch, confirm it is still required (curl 401 without it); repeat on Linux when available | none (10 min) |
| 3 | **Real WHIP backend in the app** (F2.4): `GstWhipBackend` with a real phone (Larix), real network drop, 3 s connect target | Today's in-app run used `STREAMDECK_WHIP_SIMULATE=1`; the real path is only proven by the `#[ignore]` loopback test (session registered in ~4 s in a debug build) | Run without the env var and `--features whip,gstreamer`; connect Larix over LAN; pull Wi-Fi; time connect in a release build | a phone; release build |
| 4 | **Installed app never run** (release step 1): NSIS installer on a clean machine, PDF import with bundled PDFium, AV false positives | Dev launches hide packaging bugs (this is how the PDFium PNG bug and the presentation wiring bug hid) | Install the 6.8 MB installer in a clean Windows VM/user, import a PDF and PPTX, present, check Defender/Kaspersky | clean VM |

## P1 — decide or check soon

| # | Item | Why it matters | How to verify | Blocked by |
|---|---|---|---|---|
| 5 | **Slide timer shows 214:51:51 after restart** | Either persisted dev state counting across closed time (product bug) or stale data | Launch with an empty app data dir; start/stop the deck; relaunch and read the timer | none |
| 6 | **F3.1 latency target (<33 ms)**: CPU compositor oldest input 33.4 ms p50, d3d12compositor ~49.5 ms; CPU <60% depends on one-core vs whole-machine reading | Acceptance criterion unmet as written; needs a decision (see roadmap proposal) | Decide wording; then re-measure with real sources (NDI/WHIP) and a real display sink's `processing-deadline` | decision; real sources (#1, #3) |
| 7 | **F3.1 `glvideomixer` path and GPU utilisation** | Only `d3d12compositor` and CPU `compositor` were ever run; Linux/macOS selection is untested | Force `glvideomixer` via the element filter on this machine; sample GPU load in Task Manager/nvidia-smi during 4x1080p | none for GL on Windows; Linux box for real target |
| 8 | **Linux: nothing has ever run there** (fetch-deps, bundle layout, `.deb`, `check-bundle.sh`, GStreamer plugin set, PipeWire) | Linux is a stated target; CI only proves it compiles on Ubuntu | Ubuntu 24.04 VM or WSL2: `fetch-deps.sh linux-x64`, `cargo tauri build`, install the `.deb`, run | a Linux environment |
| 9 | **GStreamer baseline** (roadmap 1.26 vs dev machine 1.28.3) and encoder licensing (x264 GPL, OpenH264) | Packaging step 3 and Phase 4 depend on the plugin set | Decision + audit of bundled plugins' licenses | decision |
| 10 | **Rust toolchain claim** (CLAUDE.md says 1.78; CI pins 1.96.0) | Stale docs mislead; matters if we build gst-plugins-rs (1.92) | `rustc --version`, `rust-toolchain.toml`; fix the Stack row | none |

## P2 — worth tracking

| # | Item | Verify by |
|---|---|---|
| 11 | No real-app E2E suite (T4.x). Manual UI Automation (`scripts/uia-dump.ps1`) is the only real-app coverage and is not run by CI | Build the Playwright-over-WebView2 item, or script the UIA smoke into `verify.sh e2e` (local-only) |
| 12 | PPTX/PDF fidelity on real decks (only generated fixtures tested) | Collect 5-10 real decks; render and compare by eye; record a fidelity matrix |
| 13 | F2.1 Valgrind/leak check not done; one flaky-looking `cargo test` seen once | Run leak check on the gstreamer feature; rerun `cargo test` 20x to catch the flake |
| 14 | Norton exclusion for `target/` unconfirmed (Kaspersky done) | Confirm no deleted exes after a clean build |
| 15 | Open product decisions: repo public/private (SignPath), Windows signing route, AppImage vs deb | Decide; affects release.yml |
| 16 | Display hotplug and single-display mirrored output (ClickShare-style) on real hardware | Manual: unplug/replug a monitor while presenting |

## Verified (keep for the record)

| Date | What | How |
|---|---|---|
| 2026-10-21 | Full `verify.sh`: 448 checks, 0 failed; CI on `main` green | local gate + `gh run list` |
| 2026-10-21 | Real-app smoke with `--features gstreamer`: displays IPC, test pipeline Playing/Paused/Stopped, grid layouts, Present/Stop, clean exit, no panic in log | `scripts/uia-dump.ps1` |
| 2026-10-21 | Simulated-WHIP endpoint: HTTP POST -> 201 -> live tile in grid (1920x1080, ~27 fps) | curl + UIA (simulated backend) |
| 2026-10-21 | Compositor in the real app: `d3d12compositor` selected, demo 3 sources, runtime add/remove (3->2->3->4), stop | UIA |
