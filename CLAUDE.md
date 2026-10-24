# StreamDeck

Cross-platform desktop app (Windows primary; Linux builds in CI, macOS not built) for presentation control: import PPTX/PDF,
projector output, controller with notes/timer/scene media, multi-display. Built on Tauri v2 + Rust/Tokio + React/TypeScript.
GPL-3.0-or-later. Live production is developed on `feature/live-production`, not on `main`.

Scope and plans: `dev-docs/streamdeck_product_roadmap_2026.md`. Read it before starting any new feature, and
reference only the section you need.

## Stack

| Layer | Technology |
|---|---|
| Shell | Tauri v2.11.x — two windows: `controller` (operator UI, static in `tauri.conf.json`) and `presentation` (projector output, created dynamically in `app.rs::run()`) |
| Backend | Rust (`rust-version` 1.95, toolchain pinned to 1.96.0 in `rust-toolchain.toml`) + Tokio (`src-tauri/`) |
| Frontend | Two **independent** Vite + React 18 + TypeScript projects: `controller/` and `presentation/` |
| State sync | `tokio::sync::broadcast::<AppEvent>` — the only channel for cross-subsystem state. No direct function calls across subsystem boundaries. |
| Config | TOML (`streamdeck.toml` in app config dir), watched via `notify` (publishes `ConfigReloaded` only; nothing re-reads it yet — `get_config` is startup-time), secrets in OS keychain via `keyring` — never in TOML |
| Frontend state | Zustand (`slideStore.ts`, `sceneStore.ts`, one per window) |
| Media | None on this branch. Live production (GStreamer pipeline, NDI, WHIP phone ingest, compositor, RTMP streaming) lives on `feature/live-production`; see "Branch split" below |

## Multi-window dev/build wiring

Tauri only has one `devUrl`/`frontendDist` in config (controller's). The presentation
window's URL is set explicitly in `app.rs::run()`, branching on `cfg!(debug_assertions)`:
`WebviewUrl::External("http://localhost:5174")` in dev, `WebviewUrl::App("presentation/index.html")`
in release. This requires:
- `beforeDevCommand`/`beforeBuildCommand` use the object form with `"cwd": ".."`: without a root
  `package.json` (deliberately absent) the Tauri CLI otherwise runs them from `controller/` and
  `node scripts/dev-both.cjs` isn't found.
- `scripts/dev-both.cjs` runs both windows' Vite dev servers (`beforeDevCommand` in
  `tauri.conf.json`) — controller's alone isn't enough.
- `scripts/merge-presentation-dist.cjs` copies `presentation/dist/` into
  `controller/dist/presentation/` after both builds (`beforeBuildCommand`), since
  `frontendDist` only points at `controller/dist`. `presentation/vite.config.ts` uses
  `base: "./"` so its asset references still resolve once relocated there.

This was broken from F1.1 through 2026-09-18 — the presentation window silently loaded
controller's own dev server/SPA fallback instead. `verify.sh` structurally cannot catch
this class of bug: it builds each frontend independently and never exercises
`beforeDevCommand`/`beforeBuildCommand`, does a real IPC round-trip, or launches the app.
**A manual `cargo tauri dev` launch is required before considering any multi-window or
IPC change done** — five rounds of manual launch testing on 2026-09-18/19 each found a
real bug `verify.sh` passed anyway (chronological detail in git history / this section's
prior revisions if needed):

1. Dev/build wiring above (presentation loaded controller's own dev server).
2. `presentation_show` had no single-display handling at all, trapping the operator in
   fullscreen — fixed with a floating-window fallback, later superseded (below).
3. Remote dev-server content (`WebviewUrl::External`) gets zero IPC access unless a
   capability explicitly allowlists the origin via `remote.urls` (separate from the
   `windows` label-matching) — `onAppEvent()`'s `listen()` silently never attached.
   Fixed by a runtime capability (`presentation-dev-remote`, listen/unlisten only) that
   `app.rs` adds under `cfg!(debug_assertions)`; no shipped capability file carries
   `remote.urls` (invariants.rs enforces both). Production loads local bundled content.
4. The floating-window fallback from #2 was reverted: presenting should always go
   fullscreen, single display or several, matching PowerPoint's own behavior — not a
   mitigation, a product decision. Operator-trapping is now prevented by two safety nets
   instead of avoiding fullscreen: Escape is defensively cleared and re-registered on
   every `presentation_show` call (unconditionally, not just for 2+ displays), and
   closing the presentation window via Alt+F4/OS close no longer destroys it —
   `app.rs`'s `on_window_event` intercepts `CloseRequested` for the `presentation` label
   with `api.prevent_close()` and routes it through the same `hide_presentation()` used
   by "Stop Presenting".
5. `always_on_top(true)` was still unconditional after #4, which blocks Alt+Tab from
   ever bringing another window to the front regardless of focus — a real complaint on a
   single display mirrored to a projector via a casting dongle (e.g. ClickShare), which
   Windows reports as 1 display, same as a laptop-only setup. Fixed: `always_on_top` is
   now only set when `display::list_displays` reports 2+ real displays; fullscreen stays
   unconditional. This doesn't (can't) fix the mirrored case fully — whatever the
   operator switches to also appears on the mirrored output, since mirroring duplicates
   the same pixels; that's a hardware limitation.

**Current behavior**: `presentation_show` always goes fullscreen; `always_on_top` only
with 2+ displays; Escape always registered; OS-close always hides, never destroys.

## Repo map

```
src-tauri/src/
  app.rs                  Builder setup: window creation, capability wiring, .manage() state, event bridge
  bus.rs                  EventBus (broadcast channel wrapper)
  events.rs               AppEvent enum — single source of truth for what can flow over the bus
  error.rs                AppError — discriminated union returned by every command
  config.rs / config_loader.rs   StreamDeckConfig, TOML load + hot-reload watcher
  slide.rs / slide_state_machine.rs   Slide deck model + advance/rewind/jump/timer state machine
  scene.rs                 Scene media source state (image/video push, active source)
  deck_importer.rs         .pptx/.pdf import → SlideDeck (spawn_blocking — CPU-bound)
  commands/                 One #[tauri::command] module per feature area — thin wrappers, no business logic
controller/src/            Operator UI — the only window with invoke() permissions
controller/src/components/ui/  Design-system primitives (Button, IconButton, Badge, Card,
                            SegmentedControl, Popover, icons.tsx) — reuse these for any new
                            controller panel instead of hand-rolled inline styles; see
                            "Design system" below
presentation/src/          Projector output — listen-only, zero invoke permissions
dev-docs/                  Roadmap, test plan, release process, unverified-items list
scripts/verify.sh          Acceptance-gate script — see "Verification" below
```

`controller/` and `presentation/` intentionally duplicate small shared files
(`SlideCanvas.tsx`, `ipc.ts`, `events.ts` shape) — they are separate Vite projects with
separate `package.json`s, not a monorepo with shared packages. This is a deliberate
choice, not drift; don't "fix" it by extracting a shared package unless asked.

## Design system (controller only)

`controller/src/index.css` defines the token set (`--color-bg`, `--color-surface`,
`--color-primary`, `--color-live` [presenting, violet — deliberately distinct from the
red/green/amber the roadmap reserves for streaming health in Phase 4, see F4.2],
`--color-warn`, `--slide-bg`, `--font-mono`, `--radius-*`) and
`controller/src/components/ui/` has the primitives built on them (`Button`, `IconButton`
— requires a `title` prop, every icon-only control needs a stated label, `Badge`,
`Card`, `SegmentedControl`, `Popover`, `icons.tsx`). Every controller panel
(`SlideDeckPanel`, `SceneMediaPanel`, `DisplayPanel`, `PresentButton`) is built from
these — new panels should be too,
rather than hand-rolling inline styles again. `presentation/` doesn't get this kit — it
has no buttons/panels, and `controller`/`presentation` are deliberately separate
projects (see above), not sharing a package.

A design canvas exploring this system, including a Phase 2/3/4 direction sketch
(cameras + scenes + streaming coexisting) and empty/first-run states, exists as a
published Claude artifact — ask the user for the link if picking this back up, since it
isn't tracked in the repo.

## Architecture invariants (do not violate)

1. **Process separation** — any future long-running media pipeline runs in a dedicated
   Tokio task; UI events never block it.
2. **Event bus only** — all state transitions (slide change, scene cut, stream start/stop)
   flow through `AppEvent` on the broadcast bus. No direct cross-module function calls for
   state changes.
3. **Config as code** — pipeline/scene/overlay config lives in TOML, no hard-coded
   behaviour. Secrets (tokens, stream keys): OS keychain only, never TOML plaintext.
4. **Coexistence first** — new subsystems are additive; don't break existing flows to add
   a new phase's feature.
5. **Capability minimalism** — `controller` window has `invoke` + full permissions
   (`src-tauri/capabilities/default.json`); `presentation` window has **listen-only**
   access (`src-tauri/capabilities/presentation.json`, `core:event:allow-listen` + `allow-unlisten` only — `core:event:default` also grants
   emit — and no invoke permissions at all). Any new dynamically-created window needs its own capability
   file — a window not listed in any capability gets **zero** permissions, including
   `listen()`. (This was actually broken until 2026-09-18 — the presentation window had no
   capability file at all and its `listen()` calls would have been silently denied at
   runtime; fixed by adding `presentation.json`.)
6. **No `.unwrap()` in command handlers** — return `Result<T, AppError>`. Enforced by
   `verify.sh invariants`.
7. **`app.rs` stays gated** — `#[cfg(not(test))] mod app;` in `lib.rs`, so `cargo test`
   never needs to build the Tauri runtime wiring.

## Branch split (2026-10-24)

`main` is the **presentation-only product** (import PPTX/PDF, present on the projector, controller with notes/timer/scene media, multi-display). All live-production code
(F2.1-F2.4 NDI/WHIP/camera previews/pipeline, F3.1 compositor, F4.1 multi-destination streaming) lives on the long-lived branch `feature/live-production`
(forked at 71275fe) and was removed from `main` in a single commit (`chore: remove live-production code ...`). To bring it back: `git revert <that commit>` on `main`,
then merge `feature/live-production`. Keep edits to shared files (`events.rs`, `state.rs`, `app.rs`, `config.rs`, controller `App.tsx`) small meanwhile. Full plan: `dev-docs/handover_presentation_release_2026-10-24.md`.
`verify.sh` still has its old per-feature structure for the remaining features; a tiered rebuild is a separate follow-on.

## Current status

- **Phase 0 and Phase 1 (F1.1–F1.5) are done**: window/event/config foundations, slide renderer and state machine,
  controller panel and timer, PPTX/PDF import with scene media, multi-display assignment with hotplug fallback.
- **Hardening done** (see `dev-docs/public_release_audit.md` and git history): the `sdmedia` URI scheme serves
  backgrounds/scene media (`media_protocol.rs`, only files under the app data dir plus the one staged video, with Range;
  frontends build URLs via `lib/mediaUrl.ts`); the presentation window is listen-only and in release builds
  `build.rs` declares an app manifest from the `state.rs` handler list so only the controller may invoke commands
  (adding a command means adding its `allow-…` line to `permissions/controller-commands.toml`; `invariants.rs` fails
  otherwise); `persist.rs` does atomic writes with `.bak` and sets damaged files aside as `*.broken-<ts>`; imports are
  staged (`deck_import_stage` → `deck_import_commit`/`discard`) with size/zip-bomb caps in `pptx.rs` and a cargo-fuzz
  target in `src-tauri/fuzz`; the bridge replays state after `BusLag` (`state::resync_events`, `lib/resync.ts`).
  `node scripts/check-media-render.mjs <repo> <video>` re-checks media rendering and the projector's lack of
  emit/invoke against a running `cargo tauri dev` started with
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
- **PDF import needs a native `pdfium` library**: `load_pdfium` searches a candidate list (next to the exe,
  `resources/pdfium`, `pdfium/`, `../lib/StreamDeck` for deb/AppImage, the staging dir in debug builds), then the system,
  and returns an `AppError` instead of panicking. `release/deps.json` pins the checksummed binaries and
  `bash scripts/fetch-deps.sh [platform]` stages them; installers bundle them via `release/bundle.pdfium.conf.json`
  (deliberately not in `tauri.conf.json`: tauri-build errors on a resource glob matching nothing).
- **Tests**: Rust unit, integration (mock Tauri runtime harness in `src-tauri/tests/common`), fixture-based importer
  tests, `invariants.rs`; Vitest store/component/event-wiring tests in both frontends (`src/test/mockTauri.ts`);
  `src/lib/events.exhaustive.test.ts` keeps the TS `AppEvent` unions in step with `events.rs`. Coverage:
  `npm run test:coverage` in each frontend. Plans: `dev-docs/test_plan.md`, `dev-docs/test_framework_roadmap.md`.
- **Owed manual checks** are listed (prioritized) in `dev-docs/unverified_items.md`; update it when you change behaviour
  that only a real launch can confirm.
- The product name and identifier are due to change before the first release; keep edits that mention the name minimal.

## Verification — `scripts/verify.sh`

Purpose/speed tiers; CI and the pre-push hook call the same commands, so they cannot drift. Features are test
filters, not scripts. The script bootstraps `npm ci` + frontend `dist/` (hash-cached in `.verify-cache/`), runs the
Rust and frontend lanes concurrently, and keeps per-step logs in a fresh temp dir (path printed on failure).

```
bash scripts/verify.sh quick        # every edit: fmt, cargo check, fast Rust tests, typecheck+Vitest for changed frontends
bash scripts/verify.sh push         # pre-push + CI: fmt, clippy -D warnings, ONE nextest (incl. invariants), both frontends typecheck+Biome+Vitest
bash scripts/verify.sh invariants   # src-tauri/tests/invariants.rs only
bash scripts/verify.sh deps         # cargo deny, osv-scanner, fuzz smoke (Docker); --only deny|osv|fuzz
bash scripts/verify.sh release [bundle]  # delegates to scripts/build-release.sh / check-bundle.sh when present
```

`core` is an alias of `push`; `--no-frontend` skips the frontend lane and dist build; `push --only rust|frontend` is what CI jobs run.
"Nothing unexpectedly skipped" replaces hard-coded counts: nextest runs with `--no-tests=fail` and fails on any skipped/ignored test;
`scripts/check-vitest-json.cjs` fails on failed/skipped/todo Vitest tests. Regression greps live in `tests/invariants.rs` as real tests.
`cargo build`/`clippy` need `controller/dist` and `presentation/dist` (rust-embed, `custom-protocol`); bootstrap builds them.

## CI (`.github/workflows/ci.yml`)

Jobs: `rust` (matrix ubuntu/windows: `verify.sh push --only rust`), `frontend` (`verify.sh push --only frontend`), `cargo-deny`
(action) and `osv` (`verify.sh deps --only osv`). macOS dropped until an Apple account exists.

## Release (2026-10-24)

Windows installer: `bash scripts/build-release.sh` (no args; optional `STREAMDECK_SIGN_COMMAND`), tag `v*` runs `.github/workflows/release.yml` to a draft release. Licence GPL-3.0-or-later. Name/identifier/publisher live in `src-tauri/tauri.conf.json`. Full process, signing routes and AV checklist: `dev-docs/release_process.md`.

## Local environment note (Windows)

`cargo`/`rustup` live in `%USERPROFILE%\.cargo\bin`; Node is managed via `fnm`. Neither is on the default
non-interactive Git Bash PATH, and **`$HOME` in Git Bash/MSYS resolves to the MSYS home, not the Windows profile**, so
`$HOME/.cargo/bin` may not exist. Use:
```
export PATH="$(cygpath -u "$USERPROFILE")/.cargo/bin:$PATH"
eval "$(fnm env --shell bash)"
```
`.claude/hooks/*.sh` and `scripts/verify.sh` try the `cygpath`-converted profile first, then `$HOME`, then
`$USERPROFILE`; don't simplify that to just `$HOME`.

## Editor / LSP setup

No special config needed, and don't add a root-level tsconfig/package.json to "unify"
the two frontends — that's exactly the dead scaffold pattern just removed (see Current
status). `controller/` and `presentation/` each carry their own `tsconfig.json`; every
editor's TypeScript server resolves the nearest one per file automatically. For
rust-analyzer: no extra settings required beyond what `cargo build`/`clippy` already
need — `controller/dist`/`presentation/dist` must exist first (the default
`custom-protocol` feature embeds them via `rust-embed` at compile time), or
rust-analyzer's background `cargo check` will show spurious errors around the embed
macros the same way a bare `cargo build` would fail. Run `bash scripts/verify.sh
--no-frontend` once (it bootstraps both frontend builds) before expecting a clean
rust-analyzer state on a fresh checkout.

## Hard-won lessons (curated)

- `generate_handler![]` needs full module paths — no `pub use` re-exports.
- No manual `From<AppError> for InvokeError` — Tauri v2's blanket impl covers it.
- Use `handle.emit()`, not `emit_all()` (removed in Tauri v2).
- A `notify` watcher must be kept in managed state — dropping it silently stops watching.
- Every config struct needs `#[serde(default)]` for forward-compatible migration.
- `Arc<EventBus>` must be constructed *before* `.manage()` — never rebuild an `Arc` from
  `app.state()`.
- `PathBuf` doesn't serialize cleanly over Tauri IPC — use `Option<String>`.
- `AppEvent`'s TypeScript union must stay a fully discriminated union (`{ type, payload }`
  per variant) or type narrowing breaks.
- `DeckEnd`/`DeckStart` are emitted **instead of** `SlideUpdate` at deck boundaries, not
  alongside it.
- Tauri global-shortcut callbacks can't `await` directly — spawn onto
  `tauri::async_runtime::spawn`, including for unregistering (never unregister from inside
  the callback itself — deadlocks).
- CPU-bound work (PPTX/PDF parsing) must run via `tokio::task::spawn_blocking` or it stalls
  the whole async runtime.
- Don't poll `deckGetAllSlides()` on every `DeckImportProgress` tick — IPC overhead scales
  with slide count; fetch once on completion.
- File-size limits (10 MB image / 500 MB video) are validated in the Rust command, before
  any copy or path storage — never trust the frontend to pre-validate.
- Windows AV may flag an unsigned dev binary (`VHO:Trojan-PSW` was seen when `keyring` was linked; main no longer links it) —
  add the project/`target` dirs to AV exclusions locally; code-sign for real distribution.
  Kaspersky/Norton also delete freshly built exes (looks like missing `target/debug/*.exe` or `os error 5/32`).
- `target/` hit 96 GB (full MSVC debuginfo × ~20 test binaries × feature combos): root `Cargo.toml`
  now sets `line-tables-only` for the workspace, no debuginfo for deps, no incremental for tests.
  If it creeps up again, `cargo clean` (it's all rebuildable).
- quick-xml distinguishes `Event::Start`/`Event::End` (has children) from `Event::Empty`
  (self-closing `<tag/>`) — real-world XML producers (PowerPoint included) favor
  self-closing tags for childless elements. A `match` that only handles one of these two
  event kinds for the same element name will silently miss real files while still passing
  hand-written tests that happen to use the other form. Handle both, or share the logic via
  a helper called from both arms (see `classify_ph` in `deck_importer.rs`).
- A dynamically-created Tauri window (`WebviewWindowBuilder` inside `.setup()`, not declared
  in `tauri.conf.json`) is invisible to capability matching unless its label appears in some
  capability file's `windows` array — including for `listen()`, not just `invoke()`. Every
  new dynamic window needs its own capability entry from the start.
- `tokio::spawn` called synchronously from `.setup()` panics ("no reactor running") —
  `.setup()` isn't inside a Tokio runtime context yet. Use `tauri::async_runtime::spawn`
  for anything spawned directly in `.setup()`. `cargo test`/clippy don't catch this; only
  actually running the app does (see `display::spawn_watch_task`).

## Agentic tooling in this repo

- `/verify` skill (`.claude/skills/verify/`) wraps `scripts/verify.sh`, the real Definition-of-Done gate. Use it instead
  of running `cargo test` alone and calling something done.
- A `PreToolUse` hook (`.claude/settings.json` → `.claude/hooks/pre-push-verify.sh`) runs `verify.sh push` before any
  `git push` and blocks the push if it fails (the script early-exits unless stdin contains `git push`, since the `if:`
  filter misfires). A `PostToolUse` hook (`.claude/hooks/fmt-rust.sh`) runs `cargo fmt` after editing any `.rs` file.
- Testing: `dev-docs/test_framework_roadmap.md` (items T0–T7) and `dev-docs/test_plan.md` (coverage, journeys, manual
  checklist). The `test-authoring` skill is the how-to; the `test-engineer` subagent implements one roadmap test item
  (never runs git); `test-reviewer` is a read-only false-green reviewer. Real-app E2E (Playwright over WebView2 CDP) is
  local-only; keep it out of CI `pull_request` runs.
- `scripts/uia-dump.ps1` drives the running real app via Windows UI Automation (no args: list controller controls;
  `-Click "<name substring>"`: invoke one); launch with `cargo tauri dev`.

## Working agreement

- `dev-docs/streamdeck_product_roadmap_2026.md` is the spec of record for scope; this file is the spec of record for
  *how* to work in this repo day to day.
- A change isn't done until `bash scripts/verify.sh push` passes (not just `cargo test`), plus a manual
  `cargo tauri dev` launch for window/IPC wiring changes.
- Commit as you go; sign off commits (`git commit -s`, see `CONTRIBUTING.md`). Don't let completed work pile up uncommitted.
- Windows/CRLF: `.gitattributes` normalizes line endings (`eol=lf`); don't fight it with per-file `core.autocrlf`.
