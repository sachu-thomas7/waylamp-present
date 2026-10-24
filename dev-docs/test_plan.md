# StreamDeck — Test Plan

**Version:** 1.0 (2026-10-11) · **Scope:** Phase 0–1 product (F0.1–F1.5, empty-deck/Clear Deck)
and the dependency-uplift regression protocol. **Status:** living document — update the
traceability matrix (§4) whenever a test is added.
**Companions:** `dev-docs/test_framework_roadmap.md` (how the framework gets built),
`.claude/skills/test-authoring/SKILL.md` (how to write tests), `CLAUDE.md` (conventions).

---

## 1. Purpose and quality goals

1. Know within minutes, locally, whether a code change or dependency upgrade broke the
   product — including the multi-window/IPC wiring that `verify.sh` historically missed.
2. Localise failures: the lowest layer able to see a bug should be the one that fails.
3. Cost: zero extra CI minutes by default (metered runners; macOS 10x, Windows 2x).
4. Honesty: a green run means something. Tests must be able to fail (roadmap principle 3).

**Out of scope for automation:** physical multi-display/projector behaviour, mirrored-output
(casting dongle) behaviour, OS-level global shortcuts, the native file picker (§9 manual list).
Live-production media (GStreamer/NDI/WHIP) lives on `feature/live-production` and is not covered here.

## 2. Test strategy — layers

| Layer | What | Tooling | Where it runs |
|---|---|---|---|
| L0 Static | fmt, clippy `-D warnings`, `tsc`, Biome, `cargo deny`, invariant tests | cargo, tsc, biome | `verify.sh push`, CI |
| L1 Unit | state machine, parsers, stores, config | `#[test]`, Vitest | `verify.sh push`, CI |
| L2 Integration | Rust: command→bus→payload on `tauri::test` mock runtime. JS: components/stores with Testing Library + `mockIPC` | cargo, Vitest | `verify.sh push`, CI |
| L3 Golden/contract | fixture decks + snapshots, event/payload serialization, capability files | cargo, Vitest | `verify.sh push`, CI |
| L4 Real-app E2E | launch real app, click controller, assert presentation DOM and window state | Playwright over WebView2 CDP (fallback: WebdriverIO embedded) | `verify.sh e2e` locally; optional manual CI dispatch |
| L5 Manual | hardware/OS behaviours | checklist §9 | before release / after Tauri-level bumps |

Why this stack (evaluated 2026-10): CDP attach needs **no changes to the app** and adds no
permissions (invariant 5); WebdriverIO's embedded provider is the cross-platform fallback but
adds an in-app plugin; `tauri-driver`+Selenium needs a matching `msedgedriver`;
`tauri-plugin-playwright` (0.x, needs `withGlobalTauri`, control socket) conflicts with
capability minimalism; `mockIPC` fakes only `invoke`, so it cannot replace L4. Details and
sources: roadmap T4.0 spike decision gate.

## 3. Risk register (drives priority)

| # | Risk | Evidence | Mitigating tests |
|---|---|---|---|
| R1 | Presentation window receives no events (dev/prod URL, capability `remote.urls`) | CLAUDE.md bugs #1, #3 | L4 J2/J3; T1.4 capability test; T2.3 |
| R2 | IPC works in unit tests but not in real app | 5 manual-launch bugs, 2026-09 | L4 J1–J8 |
| R3 | Parser upgrade silently changes text (quick-xml `&amp;`, self-closing tags) | quick-xml 0.42 uplift; earlier `Event::Empty` bug | T3.1 fixture matrix + golden snapshots |
| R4 | React/Vite/TS major breaks rendering | Vite 8 / React 19 / TS 7 uplift (verified by hand only) | T2.2/T2.3 render tests; T4 smoke; T5.1 visual |
| R5 | Event union drifts between Rust and TS | CLAUDE.md lesson | T1.4, T2.4 (done), T3.2 |
| R6 | Presentation window gains invoke permission | invariant 5 | T1.4 capability assertion; T2.3 "never invokes" |
| R7 | Operator trapped by fullscreen / window close destroys presentation | CLAUDE.md bugs #2, #4, #5 | L4 J4 (close → hidden, not destroyed); §9 manual |
| R8 | Tests pollute the user's real app-data deck | active-deck persistence | planned T4.1 data-dir seam (not implemented yet); fixtures own temp dir |
| R9 | Tests that cannot fail (false green) | industry pattern | skill "Prove it can fail"; T6.1 scanner; T6.2 mutation |
| R10 | Dev tooling blocks work (hook runs on every Bash call) | 2026-10 session | T0.1 |
| R11 | PDF import without pdfium must be an error, not a panic | CLAUDE.md | ✅ `load_pdfium` unit test + `pdf_import_never_panics_when_pdfium_is_missing`; real PDF test still ignored until pdfium is bundled |

## 4. Feature inventory and traceability

Legend: ✅ exists today · 🟡 partial · ⬜ planned (roadmap item). Update this table with every
test you add.

| Feature | Behaviour under test | L1 | L2 | L3 | L4 |
|---|---|---|---|---|---|
| F0.1 bootstrap | two windows launch | — | — | — | ⬜ T4.2 smoke (J1) |
| F0.2 IPC layer | typed commands, `AppError` discriminated, no `.unwrap()` | ✅ `commands::system` | 🟡 T1.1 harness (`harness_smoke`: `get_build_info`, `get_config`, unknown cmd, `slide_get_current`/`slide_advance` + events) → T1.2/T1.3 journeys. All commands are runtime-generic and registered on the mock handler; display commands use an injected `DisplayProvider` (faked in the harness) and a managed `ConfigPath` (temp dir), so nothing touches the real app-data/config dirs. | ⬜ T3.2 | ⬜ J1 |
| F0.3 event bus | broadcast, lag handling | ✅ `bus::tests` | ✅ T1.2 real-bus event-sequence assertions | ⬜ T3.2 | ⬜ J2 |
| F0.4 TOML config | load, invalid TOML error, hot reload | ✅ `config`, `config_loader` | ✅ T1.3 `config_journeys` (`get_config` default/non-default; real notify watcher: edit/create/corrupt edit -> `ConfigReloaded`, sibling file ignored, parentless path rejected). Covers `ConfigReloaded` publication only: currently nothing consumes it and `get_config` is not refreshed after a reload (pending product decision) | — | ⬜ (J9 optional) |
| F1.1 presentation window | renders slide from events; listen-only | ✅ presentation `slideStore` | 🟡 `appEventWiring.integration` + T2.1 helper (`mockTauri.test`: events-only render, zero invokes) + ✅ T2.3 `App.test` (27 tests + `App.lifecycle.test` listener-release under StrictMode: empty canvas, title/body/position, background on/off, `DeckEnd`/`DeckStart` keep slide, staged-vs-active media gating, image/video elements, swap, return to latest slide, typed table of every other `AppEvent` variant asserted ignored in three scene modes, `calls` empty after every test). Found+fixed listener leak in `App.tsx` | ⬜ T1.4 | ⬜ J2, J4 |
| F1.2 slide state machine | advance/rewind/jump/timer, boundaries, `DeckEnd`/`DeckStart` instead of `SlideUpdate` | ✅ `slide_state_machine` | ✅ T1.3 `timer_journeys` (timer reset events/persist/restart/persist failure leaves in-memory timer unchanged; `slide_get_current`, `deck_get_all_slides` on stub deck) + `deck_journeys` (advance/rewind/jump, `DeckEnd`/`DeckStart` instead of `SlideUpdate`, out-of-range jump) | ⬜ T3.2 | ⬜ J3, J8 |
| F1.3 controller panel + timer | Prev/Next/Jump, thumbnails, timer, shortcuts | ✅ controller `slideStore` | 🟡 `eventWiring.integration` + T2.1 helper (`mockTauri.test`: real `App` over `mockIPC`) + ✅ T2.2 `SlideDeckPanel.test`/`PresentButton.test`/`App.test` (empty/loaded/`layout_unsupported`, Prev/Next/Jump/thumbnail/keys + args, boundary banners, timer, import progress/error, popovers). Space/arrow shortcuts ignored while typing in inputs (bug found by T2.2, fixed) | ⬜ T5.1 | ⬜ J3, J8 |
| F1.4 PPTX/PDF import | title/body/formatting extraction, progress, failures | ✅ `deck_importer` | ✅ fixture test (pptx); ✅ `deck_journeys` import (progress → `SlideUpdate` 0 → scene → complete, bad path/extension) | ⬜ T3.1 | ⬜ J2 |
| F1.4 scene media | push image/video, size limits, active source toggle | ✅ `scene` | ✅ T1.3 `scene_journeys` (image copy + event, exact 10 MB / 500 MB boundary, reject before copy, bad ext/missing file, staged vs active source, `scene_set_source` event + unknown source); ✅ T2.2 `SceneMediaPanel.test`/`DisplayPanel.test` (source toggle, push image/video + cancel, staged image/video, display list/assign/changed/lost). Failed assign does not show Assigned (bug found by T2.2, fixed) | — | ⬜ J5 |
| F1.5 multi-display | enumerate, assign, fallback on `DisplayLost` | ✅ `display` | ✅ T1.3 `display_journeys` (list, assign persisted to temp `streamdeck.toml`, NotFound, corrupt config, `reconcile_displays`: `DisplaysChanged`/`DisplayLost`, stable, provider failure, no displays); the mock window's `set_position` is a no-op, so window repositioning (assign and the DisplayLost fallback to primary) is manual-only (checklist M1); `display_assign_presentation` persists config first, then moves the window, updates state and publishes (on config failure state and window are unchanged) | — | 🟡 J9 (read-only) + §9 manual |
| Empty deck / Clear Deck (2026-10) | `SlidePayload::empty`, `deck_clear`, Present disabled | ✅ `current_payload_on_empty_deck…` | ✅ `deck_journeys` clear (+idempotent, empty-deck boundaries); ✅ T2.2 `App.test` (Clear Deck -> `deck_clear` -> empty state, Present disabled) | — | ⬜ J6 |
| Persistence | `active_deck_id.txt`, restore on startup | — | ✅ `deck_journeys` restore/no-op/corrupt/clear-then-restart/`restore_on_startup` (saved index + timer survive restart; product bug fixed), re-import resets index/timer, `spawn_timer` ticks | — | ⬜ J7 |
| Architecture invariants (CLAUDE.md) | no unwrap/expect in `commands/`, `mod app` gated, presentation capability listen-only (`core:event:default` only, dev `remote.urls`), controller capability, every window label in a capability, controller TS `AppEvent` union == Rust variants (presentation subset, same shapes), `{type,payload}` tagging, no secret-like config fields, `tauri/test` dev-only, no `withGlobalTauri`, presentation never imports invoke wrappers | — | ✅ T1.4 `tests/invariants.rs` (28 tests incl. scanner self-tests; every capability file + inline conf capabilities scanned for presentation grants, wildcard-aware; `.emit(` only in `app.rs` bridge (#2); unwrap/expect/panic!/todo! ban also covers `state.rs`; mutation-proven). **Not covered / deferred:** #1 process separation (no media pipeline on `main`; it lives on `feature/live-production`) and #4 coexistence (no static check possible). `state::mock_invoke_handler` is a non-gated test seam, accepted as harmless (not reachable from release IPC). | — | — |
| Event-union drift (R5) | every Rust `AppEvent` variant is in each TS union or explicitly allow-listed | ✅ T1.4 `invariants.rs` (source-parsing) | ✅ T2.4 `src/lib/events.exhaustive.test.ts` in both frontends (controller 7, presentation 9 tests). The runtime half (Rust variants vs TS union names) overlaps T1.4 `invariants.rs`; new in T2.4: the type-level `Record<AppEvent['type'], true>` table (typecheck fails on union drift), the TS-side `NOT_MODELLED` allow-list with reasons and stale-entry detection (presentation, 11 controller-only variants), and a fail-loud `events.rs` parser (rejects discriminants, raw idents, unbalanced input; cross-checked against an independent reduction count) | — | — |
| Window safety (Esc, hide-on-close) | Esc re-registered; OS close hides | — | — | — | ⬜ J4 (+ §9 for Esc) |

Current baseline (2026-10-11): Rust 55 unit tests + 1 fixture integration (+1 `#[ignore]`d PDF);
frontend: controller 3 store tests + 1 wiring integration, presentation 1 store test + 1 wiring
integration. No component/render tests, no E2E; coverage baseline in §12.

## 5. Environments and test data

- **Machine:** Windows 11 dev box is the reference for L4 (WebView2). L1–L3 are
  cross-platform and run in CI on ubuntu/macos/windows.
- **Fixtures:** `src-tauri/tests/fixtures/` (and `dev-docs/test-assets/`), regenerated with
  `node dev-docs/test-assets/make_test_assets.cjs`. New fixtures go through that generator so
  they are reproducible, not hand-edited binaries.
- **Isolation:** every L4 run is meant to use its own temp data dir (a seam planned in T4.1; no such env var exists in the code today),
  WebView2 user-data folder, and CDP port. No test reads or writes the real `%APPDATA%`.
- **Build under test:** primary = production-like (`cargo tauri build --no-bundle`), because
  it exercises `beforeBuildCommand` + `merge-presentation-dist` (the bug class that hid for
  weeks). Secondary = dev build against Vite servers for fast loops.
- **Time/randomness:** no real-time sleeps; use event/DOM readiness signals. Timer tests
  assert on `TimerTick` ordering/tolerance, not wall-clock equality.

## 6. End-to-end journeys (L4)

Each journey asserts **both** the controller state **and** the presentation DOM after every
action. "Presentation DOM" = the presentation page's rendered title/body/index/media element.
Window state (visible/fullscreen) is read from the backend, not inferred from pixels.

| ID | Journey | Steps → expected |
|---|---|---|
| J1 | Launch & windows | start app → controller loads, `get_build_info` returns, presentation window exists and is hidden, no console errors beyond allowlist |
| J2 | Import → Present | invoke `deck_import(fixture.pptx)` (no native dialog) → progress then complete; controller shows N slides; click Present → presentation visible and shows slide 1 title/body exactly as fixture |
| J3 | Navigation | Next ×N → presentation index/title track; at end `DeckEnd` (no slide change, controller indicates end); Prev back to `DeckStart`; Jump-to k → exact slide; each step's payload matches the golden JSON |
| J4 | Present lifecycle & safety | Present → visible; Stop → hidden; Present again; simulate OS close of presentation → window **hidden, not destroyed**, Present works again |
| J5 | Scene media | push fixture image → staged, active source unchanged; toggle active source → presentation shows media; toggle back → slide again; oversize file rejected with error and no copy |
| J6 | Clear / empty | Clear Deck → controller "No slides yet", Present disabled, presentation shows empty state; active source back to slide deck |
| J7 | Persistence | import deck → quit app → relaunch with same data dir → deck restored and slide 1 shown |
| J8 | Timer | start presenting → `TimerTick` advances; reset → back to 0; value shown in controller |
| J9 | Displays (read-only) | `display_list` returns ≥1 display with exactly one primary; assignment persists to TOML in the temp data dir |

Assertion style: wait on **state**, not time (`expect.poll` / `toHaveText`). No retries in the
gate. Page objects own all locators; prefer role/label/text, fall back to a `data-testid` added
to the component (never CSS classes).

## 7. Dependency-upgrade regression protocol

Run per upgrade set, locally, on an isolated worktree (never in place):

| Change class | Minimum gate | Plus |
|---|---|---|
| npm minor/patch | `verify.sh push` | — |
| Rust minor/patch | `verify.sh push` | — |
| Rust major (parsers/serializers: quick-xml, zip, serde, ts-rs) | `core` | T3 golden diff reviewed by a human |
| Frontend toolchain (vite, plugin-react, vitest, typescript) | `push` + build both frontends | `verify.sh e2e` + T5.1 visual diff + both dev servers boot |
| React / react-dom / types | `core` | L4 J2–J3 + visual diff |
| `tauri*` / `@tauri-apps/*` | `core` | full `e2e` + §9 manual launch |
| GitHub Actions | read changelog; cannot run locally | first `main` CI run is the test |

Coupled sets move together: `{vite, @vitejs/plugin-react, vitest}`,
`{react, react-dom, @types/react, @types/react-dom}`, `{tauri*, @tauri-apps/*}`.
Outcome of each trial: PASS (merge) / NEEDS-ATTENTION (fix list) / BAD-PR (close). Record in
the PR or commit message. `scripts/update-deps.sh` (T5.2) automates this.

## 8. Entry / exit criteria

- **Per change:** `verify.sh push` green (includes fmt, clippy, all L1–L3). Touching windows,
  IPC, capabilities, dev/build wiring, or any Vite/Tauri dependency → also `verify.sh e2e`.
- **Per roadmap test item:** its gate passes; new tests proven to fail (skill); traceability
  matrix and `CLAUDE.md` status updated.
- **Release candidate:** `core` + `e2e` green 3× consecutively on the reference machine, §9
  checklist executed and recorded, no open quarantined tests past expiry.

## 9. Manual checklist (L5) — cannot be automated

Record date, build, display setup, and result for each. Run before release and after
Tauri/OS-integration bumps.

- [ ] M1 Two physical displays: Present goes fullscreen on the assigned display; always-on-top
      only with 2+ displays; unplug assigned display → fallback to primary + `DisplayLost` shown.
- [ ] M2 Single display mirrored to a projector (casting dongle): Present is fullscreen, Alt+Tab
      reaches other windows.
- [ ] M3 Global shortcuts: arrows advance/rewind from another focused app; Escape exits
      presenting and is re-registered on every Present.
- [ ] M4 Native file picker: Import Deck opens the dialog, cancel is harmless, a real `.pptx`
      from PowerPoint imports (title/body present).
- [ ] M5 Alt+F4 / OS-close on the presentation window hides it; Present works again.
- [ ] M6 Fullscreen visual: no letterbox/black-border surprises on the projector's resolution.
- [ ] M7 Fresh-profile first run: empty-state UX, no errors, config file created.

## 10. Defect, flake, and quarantine policy

- A failing test is a product bug until proven otherwise; diagnose with the failure artifacts
  (traces, both windows' screenshots, app log, event log) before touching the test.
- No auto-retries in the gate. A flaky test is quarantined with owner + expiry date + root
  cause hypothesis; quarantine is tracked in `e2e/QUARANTINE.md` and must be empty before a
  release candidate.
- Product regressions found by manual testing get a failing automated test first, then the fix.

## 11. Tooling decisions (record changes in the roadmap, not here)

| Concern | Decision | Revisit when |
|---|---|---|
| E2E driver | Playwright + WebView2 CDP | T4.0 spike fails, or macOS/Linux E2E needed → WebdriverIO embedded |
| JS unit/component | Vitest 5 + Testing Library + `mockIPC` | — |
| Rust integration | `tauri::test` mock runtime + real `EventBus` | API changes in Tauri minor |
| Snapshots | `insta` (Rust), Vitest snapshots, Playwright `toHaveScreenshot` | — |
| Mutation | `cargo mutants` on state machine + importer; Stryker optional | after T6.1 |

## 12. Metrics (record and review monthly)

| Metric | Baseline 2026-10-11 | Target |
|---|---|---|
| Rust tests | 55 unit + 1 integration + 7 harness smoke + 17 deck journeys + 34 T1.3 journeys (10 scene, 12 display, 5 timer/slide-query, 7 config) + 28 T1.4 invariants (the only `#[ignore]` is the pdfium PDF fixture test) (2026-10-12, T1.4) | grows with T1/T3 |
| Frontend test files | controller 11 files (93 tests), presentation 6 files (49 tests) after T2.4 (2026-10-18) | grows with T5.1 |
| E2E journeys green | 0 / 9 | 9 / 9 (T4.3) |
| Line coverage (frontends) | controller 45.5% lines / 45% branches; presentation 81.4% lines / 83.3% branches (v8, 2026-10-12; `npm run test:coverage`) | floor at T6.3 |
| Line coverage (Rust) | not measured (`cargo-llvm-cov` not installed) | optional |
| Time: `verify.sh push` | ~2–4 min warm | ≤ 6 min |
| Time: `verify.sh e2e` | n/a | ≤ 8 min |
| Flaky/quarantined tests | n/a | 0 at release |

## 13. Maintenance rules

1. Every bug fix and every shipped feature adds or updates a row in §4.
2. New dependency majors follow §7 and leave a one-line result in the commit message.
3. If a journey changes, update §6 first, then the spec, then the page object.
4. Review this plan at the end of each product phase and after each uplift round; fold
   lessons into the skill's `references/lessons.md`.
