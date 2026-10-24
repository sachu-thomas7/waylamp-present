# StreamDeck

Cross-platform desktop app (macOS/Windows/Linux) presentation control (import PPTX/PDF, projector output, controller with
notes/timer/scene media, multi-display); live production is on `feature/live-production`. Built on Tauri v2 + Rust/Tokio +
React/TypeScript. Goal: zero dependency on Mevo Studio / ProPresenter by Phase 5.

Full spec: `dev-docs/streamdeck_product_roadmap_2026.md` — the authoritative phase/feature
list with acceptance criteria. Read it before starting any new feature. Don't paste it
wholesale into context — reference the specific phase/feature section needed.

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
dev-docs/                  Roadmap + one "AI handoff" doc per shipped feature (historical record)
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

## Current status (2026-10-24)
- **2026-10-23 "stop the bleeding"** (WHIP/keyring/preview items below describe code now only on `feature/live-production`; fixes from `dev-docs/review/architecture_review_2026-10-23.md`, one commit each): **F-01 was real** (backgrounds/scene media never loaded: `naturalWidth 0`, video error 4, checked in the running app) and is fixed by the `sdmedia` URI scheme (`media_protocol.rs`: serves only files under the app data dir plus the one staged video, with Range; frontends build URLs via `lib/mediaUrl.ts`; CSP `img-src`/`media-src` allow it). `node scripts/check-media-render.mjs <repo> <video>` re-checks it, and that the projector cannot emit/invoke, against a running `cargo tauri dev` started with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`. F-08: presentation is listen-only for real, unused grants/shell plugin/`devtools` feature/script `unsafe-inline` removed; **in release builds the projector could also invoke every app command** (dev hid it: remote origin), so `build.rs` now declares an app manifest from the `state.rs` handler list and `permissions/controller-commands.toml` grants those commands to the controller only (adding a command means adding its `allow-…` line there; `invariants.rs` fails otherwise). F-03/F-10: `persist.rs` (temp+fsync+rename, `.bak`, damaged files set aside as `*.broken-<ts>`; startup never aborts; banner via `get_startup_notices`), slide timer restored only if saved <6 h ago, confirm-on-close while presenting. F-04: `pptx.rs` (zip-bomb/size/slide caps, `ImportLimits`), imports are staged (`deck_import_stage` → `deck_import_commit`/`discard`; `deck_import` = both, kept for automation), `src-tauri/fuzz` cargo-fuzz target run in Docker. F-05: WHIP always has a keychain token (random on first enable, clearing rotates), binds the LAN interface (`bind_address = "auto"`), rejects any request with `Origin`, no CORS. F-07: bridge replays slide/scene/presentation state after `BusLag` (`state::resync_events`), controller re-pulls the rest (`lib/resync.ts`). Deps: tokio and serde_with bumped (RUSTSEC-2026-0386 appeared in a fresh DB), deny.toml pruned and set to `unmaintained/unsound = "all"`, `scripts/osv-scan.sh` + CI job, `--locked`, nextest in CI, `rust-version` set. Still owed (needs the owner/hardware): see `dev-docs/unverified_items.md`.
- **Unverified/owed checks live in `dev-docs/unverified_items.md` (prioritized) — update it each session; a feature's "done" claim should name which items it leaves open.**
- **Release step 1 (2026-10-19) — PDFium bundling**: `release/deps.json` pins bblanchon/pdfium-binaries `chromium/8100` (157.0.8100.0) with per-platform SHA-256 (windows-x64, linux-x64; no macOS). `bash scripts/fetch-deps.sh [platform]` downloads, verifies (hard fail on mismatch), and stages the library + license into gitignored `src-tauri/resources/pdfium/`; idempotent. Bundling uses the overlay `release/bundle.pdfium.conf.json`, passed only to builds: `cargo tauri build --config release/bundle.pdfium.conf.json` (after fetch-deps). It is deliberately NOT in `tauri.conf.json` or a `tauri.<platform>.conf.json`, because tauri-build errors on a resource glob that matches nothing and would break `cargo test`/clippy/`cargo tauri dev` on a clean checkout. `bash scripts/check-bundle.sh <installer.exe|.deb|dir>` extracts (7z/dpkg-deb, never executes) and asserts required files, no `.pdb`/fixtures/tests leaks, and size vs `release/bundle-budget.json` (Windows 10 MB; Linux unset). Gate: `bash scripts/verify.sh release`. Found and fixed a real bug the ignored test had hidden: PDF import failed every page with "image format Png is not supported" (pdfium-render enables `image` without PNG) — added `image` 0.25 with `png`. Windows NSIS build verified (installer 6.8 MB); Linux layout/fetch is untested (no Linux machine). Later steps (GStreamer, NDI) extend `REQUIRED` in check-bundle.sh.
- **2026-10-18**: jump-to-slide takes a 1-based number (matches the "n / total" badge; Go is disabled outside 1..total); the selected thumbnail scrolls into view; no-notes shows "No notes for this slide"; Tauri crates bumped to 2.12 to match the npm `@tauri-apps/api` 2.12 (the CLI warns on mismatch). Manually verified in `cargo tauri dev` via UI Automation (navigate/jump/present/stop).
- **T0.2 (2026-10)**: `cargo fmt --check` is now in `verify.sh core` (and `verify.sh t0`); tree was reformatted once. CI runs it too (2026-10-21).
- **T0.3 (2026-10)**: test directory contract documented in the test-authoring skill (§2b), `e2e/{specs,pages,fixtures}` created with READMEs, `verify.sh t0` asserts all dirs exist.

- **Phase 0** (F0.1–F0.4: bootstrap, IPC layer, event bus, TOML config) — done.
- **Phase 1 — all of F1.1–F1.5 done**: presentation window + slide renderer, slide state
  machine, controller slide panel + timer, PPTX/PDF import + scene media push, and
  multi-display assignment (`display.rs` — monitor enumeration, hotplug polling every 2s,
  `DisplaysChanged`/`DisplayLost`/`DisplayAssigned` events, assignment persisted to
  `streamdeck.toml`, fallback to primary display on disconnect). 49/49 Rust tests passing,
  clippy clean, typecheck clean in both frontends. `bash scripts/verify.sh f1` is green.
- **Frontend tests are thin** — Vitest store + event-wiring tests only (controller 12,
  presentation 4; baseline coverage in `dev-docs/test_plan.md` §12), no component render
  tests yet. Rust side has solid coverage (49 `#[test]`s across bus, config,
  config_loader, slide, slide_state_machine, deck_importer, display, commands::system).
- `deck_importer.rs` now has fixture-based integration tests
  (`src-tauri/tests/deck_import_fixtures.rs`) against real generated `.pptx`/`.pdf` files in
  `src-tauri/tests/fixtures/` (same files as `dev-docs/test-assets/`, regenerate both via
  `node dev-docs/test-assets/make_test_assets.cjs`). This caught and fixed a real bug: the
  `.pptx` parser only handled `<p:ph type="title">` when quick-xml emitted it as an
  `Event::Start`, but real PowerPoint always writes placeholders as self-closing tags
  (`Event::Empty`) — title/body extraction was silently broken for every real-world file.
  Fixed via a shared `classify_ph()` helper called from both event arms.
- **PDF import needs a native `pdfium` library** — `load_pdfium` searches a pure candidate list (`pdfium_candidate_dirs`: next to exe, `resources/pdfium`, `pdfium/`, `../lib/StreamDeck` for deb/AppImage, cargo `deps/` parent and the staging dir in debug builds), then the system, and returns an `AppError` instead of panicking. The real PDF fixture test now runs when pdfium is found and prints SKIPPED otherwise. See the Release bundling entry below.
- Git history was ~5 commits behind the actual working tree until 2026-09-18 — F1.1–F1.4
  and the scene/deck-import system existed only as uncommitted/untracked changes. Now
  clean and committed; keep commits current going forward — don't let multiple shipped
  features pile up uncommitted.
- **Controller UI redesign (2026-09-19)**: the design-system pass above — replaced every
  hand-rolled inline-style panel with the `ui/` primitives, added `SlidePayload.
  layout_unsupported` end-to-end (was on the internal `SlideEntry` since F1.4 but never
  reached the frontend, despite F1.4's own AC requiring it visible in the controller),
  moved Displays/Build Info behind a Settings popover and Shortcuts behind a Help popover
  (progressive disclosure — they're configured once, not touched during a show), and
  fixed three UX issues found in manual testing: an unstyled native scrollbar on the
  thumbnail strip, the Scene Media active-source toggle having no visible on-screen
  consequence (pushing media only stages it — the toggle is what actually puts it on the
  projector, and nothing said so), and Prev/Next being squeezed into tiny icon buttons
  sandwiched inside the Jump-to-slide group instead of being the prominent, adjacent pair
  an operator clicks constantly.
- Removed a dead scaffold leftover at the repo root (`tsconfig.json`, `tsconfig.node.json`,
  `package.json` from before the repo split into `controller/`/`presentation/` — the
  tsconfig's `include: ["src"]` pointed at a `src/` containing only an icon `assets/`
  folder, no TypeScript; confirmed unused by both `verify.sh` and CI, which always use
  `--prefix controller`/`--prefix presentation` explicitly).

- **T2.1 (2026-10)**: `src/test/mockTauri.ts` in both frontends (`installMockIPC` with call log, `emitAppEvent` via real `listen`/`emit`, `renderApp`), setup runs `clearMocks` after each test. Gate: `bash scripts/verify.sh t2.1` (`t2`).
- **T2.2 (2026-10)**: controller component tests (`App`, `SlideDeckPanel`, `PresentButton`, `SceneMediaPanel`, `DisplayPanel` `*.test.tsx`) over `installMockIPC` + real stores (`test/componentHelpers.ts` resets them); `data-testid`s added: `preview-current`, `preview-next`; thumbnails are real `<button aria-label="Slide N">`s and `SegmentedControl` exposes `aria-pressed`. Two bugs it found are fixed (keyboard shortcuts ignore input fields; failed display assign no longer shows Assigned); gate asserts no `it.fails` remain. Gate: `bash scripts/verify.sh t2.2` (`t2`).
- **T2.4 (2026-10)**: `src/lib/events.exhaustive.test.ts` in both frontends: typed `Record<AppEvent['type'], true>` table (typecheck fails on union drift) plus runtime parse of `events.rs` (via `?raw`; `vitest.config.ts` gained `server.fs.allow` for the project dir + `src-tauri/src`; the parser throws on discriminants/raw idents/unbalanced input) failing on a new Rust variant, TS-only variant, or stale `NOT_MODELLED` allow-list entry (presentation only;: 11 controller-only variants, each with a reason). Gate: `bash scripts/verify.sh t2.4` (`t2`). Phase T2 complete.
- **T2.3 (2026-10)**: `presentation/src/App.test.tsx` (27 tests) + `App.lifecycle.test.tsx` (StrictMode unlisten check; found and fixed a real listener leak: `App.tsx` now uses one `onAppEvent` listener with safe cleanup) over `installMockIPC` + real `emit`: slide/background/media/empty rendering, staged-vs-active media gating, a typed table asserting every non-rendered `AppEvent` variant is ignored (adding a variant breaks typecheck), and `ipc.calls` empty after every test (listen-only). `data-testid`s added: `slide-canvas`, `slide-background`, `scene-media-image`, `scene-media-video`. Gate: `bash scripts/verify.sh t2.3` (`t2`).
- **Coverage visibility (T0.4)**: `npm run test:coverage` in `controller/` and `presentation/` (Vitest v8, report-only, no thresholds, output in gitignored `coverage/`); baseline in `dev-docs/test_plan.md` §12. `verify.sh t0` checks the scripts exist.

## Verification — `scripts/verify.sh`

This is the actual Definition-of-Done gate, not `cargo test` alone. It bootstraps
npm installs + frontend builds (hash-cached), then runs phase/feature-scoped checks:
structural (`grep`-based file/pattern presence) + real `cargo test`/`clippy`/`typecheck`.

```
bash scripts/verify.sh              # full check, current phase
bash scripts/verify.sh f1.4         # one feature's checks (+ its prerequisites)
bash scripts/verify.sh f1           # all of Phase 1
bash scripts/verify.sh invariants   # architecture invariants only
bash scripts/verify.sh core         # build + test + lint only, no structural checks
bash scripts/verify.sh --no-frontend  # skip the frontend build bootstrap step
```

`cargo test` is always independent of frontend state. `cargo build`/`cargo clippy` are
**not** — the default `custom-protocol` Cargo feature embeds `controller/dist` and
`presentation/dist` via rust-embed at compile time, so those dirs must exist first
(verify.sh's bootstrap step handles this; CI now does too — see below).

When adding a new feature: add its `run_fX_Y()` block to `verify.sh` following the
existing pattern (file existence → `file_contains` structural checks → a `cargo test`
filter), and wire it into the `case` dispatch, including the `all|*)` fallthrough — that
fallthrough is easy to forget and was itself out of date (missing `run_f1_4`) until fixed.

## CI (`.github/workflows/ci.yml`)

Two jobs: `rust` (test + clippy, matrix over ubuntu/windows; macOS dropped until an Apple account exists) and `typecheck`
(both frontends). The rust job now builds both frontend `dist/` directories before
running cargo, for the reason above — this was previously missing and would have failed
`cargo test`/`clippy` on any clean checkout since `dist/` is gitignored.

## Local environment note (Windows)

`cargo`/`rustup` tools live in `/c/Users/<you>/.cargo/bin`; Node is managed via `fnm` —
neither is on the default non-interactive shell PATH. **`$HOME` in this Git Bash/MSYS
environment resolves to the MSYS home (`/c/msys64/home/<you>`), not the Windows user
profile** — `$HOME/.cargo/bin` silently doesn't exist. Use the explicit Windows path:
```
export PATH="/c/Users/<you>/.cargo/bin:$PATH"
eval "$(fnm env --shell bash)"
```
(`.claude/hooks/*.sh` fall back through `/c/Users/sachu/.cargo/bin` → `$HOME/.cargo/bin` →
`$USERPROFILE/.cargo/bin` for exactly this reason — don't "simplify" that to just `$HOME`.)

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

## Hard-won lessons (curated — see individual `dev-docs/*-handoff.md` for the full running list)

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

- `/verify` skill (`.claude/skills/verify/`) — wraps `scripts/verify.sh`, the real
  Definition-of-Done gate. Use it instead of running `cargo test` alone and calling
  something done.
- `feature-builder` subagent (`.claude/agents/feature-builder.md`) — implements one roadmap
  feature end-to-end (Rust + frontend + verify.sh gate + tests) following this file's
  conventions. Invoke it for "implement FX.Y"-shaped requests; it never runs git commands.
- A `PreToolUse` hook (`.claude/settings.json` → `.claude/hooks/pre-push-verify.sh`) runs
  `verify.sh core` before any `git push` and blocks the push if it fails (the script
  itself early-exits unless stdin contains `git push`, since the `if:` filter misfires). A `PostToolUse`
  hook (`.claude/hooks/fmt-rust.sh`) auto-runs `cargo fmt` after editing any `.rs` file.
- T1.1 done: mock-runtime harness in `src-tauri/tests/common/mod.rs` (`TestApp`, `collect_events`) over `state::manage_core`/`mock_invoke_handler` (shared with `app.rs`); only runtime-generic commands are reachable in the mock until the `tauri::AppHandle` commands are made generic. Gate: `bash scripts/verify.sh t1`.
- T1.2 done: every command is runtime-generic and on the shared `handler!` list; deck/scene commands read the managed `state::AppDataDir` (not `app.path()`); deck restore, timer spawn and display-state creation moved from `app.rs` into `state.rs` (`restore_on_startup`, `spawn_timer`, `manage_display`). `tests/deck_journeys.rs` covers import/nav/boundaries/clear/restore. Gate: `bash scripts/verify.sh t1.2`. **Manual `cargo tauri dev` launch required** (app.rs wiring changed). Fixed product bug found by T1.2: startup restore applied the saved slide index/timer to the stub deck before `load_deck` reset them; `restore_on_startup` now loads the deck first.
- T1.3 done: display commands take an injected `display::DisplayProvider` (production `TauriDisplayProvider`, harness `FakeDisplays`) and a managed `state::ConfigPath` (never `app_config_dir`); the 2s poll body is `display::reconcile_displays`; `error` module is now `pub`. `tests/{scene,display,timer,config}_journeys.rs` (34 tests). Gate: `bash scripts/verify.sh t1.3`. **Manual `cargo tauri dev` launch required** (app.rs/display wiring changed: confirm display list, assignment, hotplug fallback).
- T1.4 done: `src-tauri/tests/invariants.rs` (28 tests, hardened after review: all capability files + inline, string/comment-aware panic scan incl. `state.rs`, `.emit(` allow-list, structural Cargo/conf parsing) parses the real repo files for invariants: no unwrap/expect in `commands/`, `mod app` gated, capability files (presentation listen-only + dev `remote.urls`), window labels covered by capabilities, controller/presentation TS `AppEvent` unions vs `events.rs`, no secret-like config fields, `tauri/test` dev-only, no `withGlobalTauri`. Gate: `bash scripts/verify.sh t1.4`. Test-only; no product change.
- Testing: `dev-docs/test_framework_roadmap.md` (build order, items T0–T7) and
  `dev-docs/test_plan.md` (coverage, journeys, upgrade protocol, manual checklist). The
  `test-authoring` skill (`.claude/skills/test-authoring/`) is the how-to; the `test-engineer`
  subagent implements one roadmap test item (never runs git); `test-reviewer` is a read-only
  false-green reviewer. Real-app E2E (Playwright over WebView2 CDP) is local-only — keep it
  out of CI `pull_request` runs (metered minutes).
- `scripts/uia-dump.ps1` drives the running real app via Windows UI Automation (no args: list controller controls; `-Click "<name substring>"`: invoke one) — launch with `cargo tauri dev`.
- The built-in `/run` skill can launch the app for manual verification; `/code-review` for
  an independent review pass on a diff.

## Working agreement for AI-assisted sessions

- Treat `dev-docs/streamdeck_product_roadmap_2026.md` as the spec of record for scope;
  treat this file as the spec of record for *how* to work in this repo day to day.
- `dev-docs/*-handoff.md` are historical, feature-scoped session logs (pre-Claude-Code
  workflow) — useful for lessons/context on a specific past feature, not a template to keep
  producing per-feature going forward now that context persists across a session.
- A feature isn't done until `bash scripts/verify.sh <feature>` passes — not just
  `cargo test`.
- Commit as you go. Don't let multiple completed features accumulate uncommitted — see
  "Current status" above for what that already cost once.
- Windows/CRLF: `.gitattributes` now normalizes line endings (`eol=lf`) — don't fight it
  by re-adding per-file `core.autocrlf` overrides.
