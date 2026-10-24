# Unverified items and owed checks (prioritized)

Single list of things shipped but not proven. Update on every session: move an item to "Verified"
with the date and how, or add new ones. Priority = risk to the product x cost of finding out late.
Item numbers are stable (cited elsewhere); gaps are items that belong to live production.
Last updated: 2026-10-24.

Live-production items (NDI, WHIP, compositor, streaming, GStreamer) are tracked on `feature/live-production`.

## P0 — verify before building more on top

| # | Item | Why it matters | How to verify | Blocked by |
|---|---|---|---|---|
| 4 | **Installed app never run** (release step 1): NSIS installer on a clean machine, PDF import with bundled PDFium, AV false positives | Dev launches hide packaging bugs (this is how the PDFium PNG bug and the presentation wiring bug hid) | Install the installer in a clean Windows VM/user, import a PDF and PPTX, present, check Defender and any other AV in use | clean VM |
| 29 | **Release installer beyond the dev machine** (added 2026-10-24; narrows #4): the NSIS installer was proven only on the dev PC (WebView2 already present) | Not proven: clean Windows 10/11 VM without WebView2 (offline runtime install path, `minimumWebview2Version` update), Smart App Control / SmartScreen behaviour, AV verdicts, signed build (`WAYLAMP_SIGN_COMMAND` path never ran), upgrade-in-place keeps `%APPDATA%` data, the GitHub `release.yml` run itself (only actionlint-checked), `SOURCE_DATE_EPOCH` reproducibility, the WebView2 runtime installer is fetched from Microsoft's fwlink at build time and not hash-pinned | Clean VM + second machine; tag a `v0.0.0-test` on a throwaway repo copy; `dev-docs/release_process.md` | signing route, VM |

## P1 — decide or check soon

| # | Item | Why it matters | How to verify | Blocked by |
|---|---|---|---|---|
| 5 | **Slide timer shows 214:51:51 after restart** — *fix landed 2026-10-23, real relaunch still owed* | Root cause was a persisted wall-clock `timer_started_at` restored from any past session; now only restored if the state was saved <6 h ago (older or pre-`saved_at` files drop the timer). Proven by unit/journey tests, not by a real relaunch | Advance a slide, quit, relaunch within 6 h (timer continues), then edit `slide_state.json` `saved_at` to a day ago and relaunch (timer shows `--:--`) | none (5 min) |
| 8 | **Linux: nothing has ever run there** (fetch-deps, bundle layout, `.deb`, `check-bundle.sh`) | Linux is a stated target; CI only proves it compiles on Ubuntu | Ubuntu 24.04 VM or WSL2: `fetch-deps.sh linux-x64`, `cargo tauri build`, install the `.deb`, run | a Linux environment |
| 19 | **Mirrored/cast display: window share vs whole screen** (added 2026-10-23) | Decides what the operator sees on the laptop while the presentation window is shared through a casting dongle | Try sharing only the presentation window with a real casting dongle | a casting dongle |
| 20 | **Installed/bundled release** (added 2026-10-23): the un-bundled release exe was run and passed (see Verified), but not the installer, and not with `devtools` confirmed off in the UI (only pinned by a Cargo-feature test) | CSP and capability changes behave differently in dev and release (they did: see the app-manifest row below) | Install the NSIS build on a clean VM (#4); try F12/right-click Inspect in the installed app | clean VM (#4) |
| 21 | **Corrupt-file recovery banner and stage-then-confirm import dialog, seen in the real window** (added 2026-10-23) | Both are covered by component tests and real-IPC journeys, but nobody has looked at them in the real app (the file dialog is native) | Break `waylamp-present.toml` (and/or `slide_state.json`) while the app is closed, launch, read the banner, check `*.broken-<ts>` was kept; import a deck and see "Ready to import N slides" before Use this deck / Cancel | none (10 min) |
| 22 | **Confirm-on-close: the "Quit" answer** (added 2026-10-23) | Verified live that closing the controller while presenting shows "Quit while presenting?" and that dismissing it keeps the app running; the Quit button path (`destroy()` into the existing exit cleanup) was not clicked | Present, close the controller, press Quit, check the presentation window closes and the process exits | none (2 min) |
| 24 | **PPTX fuzzing is a 3-minute smoke, not a campaign** (added 2026-10-23) | 556k runs, no crash, in `src-tauri/fuzz` (Docker); PDF import (native pdfium) and `deck.json` loading are not fuzzed | Run longer (`-max_total_time=3600`) occasionally; add a `SlideDeck` JSON target | none |
| 25 | **Test data left in the developer's real app data dir** (added 2026-10-23) | While verifying F-01 against the real dev app, imports and a pushed image landed in the developer's real app data dir (the dev build shares it): extra `decks/<uuid>` folders, `scene_media/waylamp-present-test-scene-image.png`, and `decks/active_deck_id.txt` pointing at a test deck | Delete the test deck folders created that day, restore the previous id in `active_deck_id.txt` (or use Clear Deck) | none (2 min) |

## P2 — worth tracking

| # | Item | Verify by |
|---|---|---|
| 11 | No real-app E2E suite (T4.x). Manual UI Automation (`scripts/uia-dump.ps1`) is the only real-app coverage and is not run by CI | Build the Playwright-over-WebView2 item, or script the UIA smoke into `verify.sh e2e` (local-only) |
| 12 | PPTX/PDF fidelity on real decks (only generated fixtures tested) | Collect 5-10 real decks; render and compare by eye; record a fidelity matrix |
| 13 | One flaky-looking `cargo test` seen once (and once more on 2026-10-23: a full `verify.sh` run reported `cargo test` failed with the log cut off; the same suite then passed 3 times; likely a timing test such as `ping_dispatches_under_2ms` while the machine was loaded) | Rerun `cargo test` 20x to catch the flake; nextest (per-test processes) is now in CI |
| 14 | Antivirus exclusion for `target/` unconfirmed on some AV products | Confirm no deleted exes after a clean build |
| 15 | Open product decisions: Windows signing route (the repo is going public, so SignPath Foundation may qualify), AppImage vs deb | Decide; affects release.yml |
| 16 | Display hotplug and single-display mirrored output (casting-dongle style) on real hardware | Manual: unplug/replug a monitor while presenting |

## Verified (keep for the record)

| Date | What | How |
|---|---|---|
| 2026-10-23 | **F-01 was real** and is fixed: before, a real import in the running dev app gave `naturalWidth 0` for PDF page backgrounds and scene images and `MediaError 4` for a video (raw `file://` path from an `http://localhost` origin); after the `sdmedia` scheme all three render | CDP script `scripts/check-media-render.mjs` against `cargo tauri dev` (WebView2 remote debugging) |
| 2026-10-23 | F-08: the presentation window can no longer `emit` (was allowed through `core:event:default`) or invoke app commands; `listen` still works; the controller is unaffected | same script, `plugin:event|emit` denied with the ACL message, `listen` OK |
| 2026-10-23 | Confirm-on-close: with `presentation_show` active, closing the controller shows "Quit while presenting?" and dismissing keeps the app alive; not presenting closes immediately | real dev app + Win32 `WM_CLOSE` |
| 2026-10-23 | Rust 1.95 (the new `rust-version`, = highest dependency floor) builds the app: `cargo +1.95.0 check --locked` passes | `RUSTUP_TOOLCHAIN=1.95.0 cargo check --locked` |
| 2026-10-23 | `osv-scanner` (Docker, v2.7.0) on Cargo.lock + both npm lockfiles: no issues after bumping tokio (RUSTSEC-2026-0386 appeared in a fresh advisory DB) and serde_with (3.24.0); glib 0.18.5 / proc-macro-error are documented Linux-only exceptions; `cargo deny check` all four ok | `bash scripts/osv-scan.sh`, `cargo deny check` |
| 2026-10-23 | `cargo fuzz` PPTX target: 556,717 runs in 181 s, no crash or hang | Docker image from `src-tauri/fuzz/Dockerfile` |
| 2026-10-23 | **New finding, fixed:** in a *release* build the projector window could invoke every app command (Tauri allows app commands to all local windows unless an app manifest exists); dev builds hid it because the dev-server origin is remote. Now `build.rs` declares the app manifest (command list read from `state.rs`), `permissions/controller-commands.toml` grants them to the controller only, and `invariants.rs` checks the set equals the handler list | release exe + CDP: `presentation cannot invoke app commands` failed before, passes after, in release and dev |
| 2026-10-23 | Un-bundled **release** exe (`cargo tauri build --no-bundle`) with the new CSP: controller UI renders; PDF page background, scene image and WebM render in the presentation window over `http://sdmedia.localhost`; PPTX imports; projector cannot emit or invoke | `scripts/check-media-render.mjs` against `target/release/waylamp-present.exe` (pdfium.dll copied next to it) |
| 2026-10-24 | `scripts/build-release.sh` end to end on Windows (unsigned): installer built; silent per-user install (no admin, exit 0, 18 s) to a temp dir; app launched and showed a window with WebView2 child processes; silent uninstall removed exe/dir/Start-menu and desktop shortcuts/uninstall key. Left behind by uninstall: the `HKCU` install-dir key (removed by hand) and `%APPDATA%`/`%LOCALAPPDATA%` `app.waylamp.present` user data (by design). The local AV did not delete the exes | local run |
