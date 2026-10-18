# Lessons log (append; newest last)

Keep entries to one or two lines: what happened → what to do. Date them.

- 2026-09 — `verify.sh` green but app broken five times (dev/build wiring, no fullscreen
  handling, no remote-origin IPC capability, close destroying window, always-on-top). →
  Real-launch E2E (L4) is the only layer that sees these; unit tests cannot.
- 2026-09 — quick-xml `Event::Start` vs `Event::Empty`: hand-written fixtures hid a bug that
  real PowerPoint files triggered. → Fixtures must come from realistic producers and cover
  both forms.
- 2026-10 — Dependency uplift: coupled sets (vite+plugin-react+vitest; react+react-dom+types)
  fail `npm ci` individually. → Test them as one unit.
- 2026-10 — A failed `npm ci` leaves stale `node_modules`; later checks then test the wrong
  tree. → Trials run in an isolated worktree/copy, and treat a failed install as an
  immediate stop.
- 2026-10 — quick-xml 0.42 compiled after mechanical changes and all tests passed, yet entity
  handling (`&amp;`) was never asserted. → Green means "covered", not "correct"; add the
  fixture before trusting a parser bump.
- 2026-10 — The pre-push hook's `if` filter did not match compound commands, so verify ran on
  every Bash call and blocked fixes. → Hooks must re-check the command themselves; keep a
  non-Bash fallback (PowerShell) in mind when a broken tree locks the shell.
- 2026-10 — React 19 / Vite 8 were checked by mounting in a plain browser; Tauri IPC errors
  (`transformCallback`) are expected there. → Don't treat those as failures in browser mode;
  assert with `mockIPC` instead.
- 2026-10 — A fault injected to prove a gate can fail also trips the pre-push-style Bash hook (it runs `verify.sh core` on every call), blocking the revert via Bash. → Revert with the Edit tool; prove fmt/lint faults with `verify.sh t0` (cheap) and revert before any further Bash call.
- 2026-10 — Integration test binaries that touch the Tauri mock runtime exit with `STATUS_ENTRYPOINT_NOT_FOUND` on Windows: tauri-build's Common Controls v6 manifest is only linked into the bin. → `build.rs` emits `rustc-link-arg-tests` manifest args; also commands taking a concrete `tauri::AppHandle` can't register on `MockRuntime` (make them `AppHandle<R>` before testing them).
- 2026-10 — Commands that resolved `app.path().app_data_dir()` would write to the real app-data dir from the mock runtime; mock `available_monitors` is `unimplemented!` (panics -> `RecvError` from `get_ipc_response`). → Commands read a managed `AppDataDir`; display commands need a seam before T1.3 can test them.
- 2026-10 — Sync `get_ipc_response` returns only after the command finished, so `EventCollector::drain()` gives a deterministic, wait-free event sequence for exact-order assertions.
- 2026-10 — A restore-order bug (saved index applied to the stub deck, then `load_deck` reset it) hid behind a test that pinned the buggy event order. → Assert final restored state (index, timer), not the intermediate event sequence; mutation-swap the ordering to prove it.
- 2026-10 — Mock-runtime `available_monitors` panics and `app_config_dir` hits the real user dir. → Inject OS boundaries as a trait (`DisplayProvider`, faked in the harness) and managed paths (`ConfigPath`); put poll bodies in a callable fn (`reconcile_displays`) so hotplug is testable without a 2s timer.
- 2026-10 — Absence assertions on async file watchers can't be proven by time. → Use a probe watcher on a sibling file as a readiness signal (wait for the probe, then assert the main bus is empty); size-limit tests use sparse `set_len` files (no real data written).
- 2026-10 — The mock window's `set_position/set_size` are no-ops, so display repositioning is unobservable at L2 → keep it in the manual checklist (§9).
- 2026-10 — Structural invariant tests that hand-parse source (TS unions, Rust enums) silently rot if the parser matches nothing. → Assert the parser finds known anchors (first/last variant, `presentation` dynamic window) and mutation-prove each; split TS union members by line, not by `;` (payload types contain `;`).
- 2026-10 — Source-scanning invariants need a sanitiser (strip comments/strings, drop only `#[cfg(test)] mod` blocks, not any `#[cfg(test)]`) and whole-file matching (rustfmt splits `.unwrap\n()`); scan every capability file/inline conf, not just the named one; parse TS unions to the depth-0 `;`, not by line (this supersedes the earlier line-split advice).
- 2026-10 — `mockIPC(..., { shouldMockEvents: true })` handles `plugin:event|*` itself, so real `listen`/`emit` work and event-plugin calls never reach your handler (call logs stay clean). → Emit app events with `emit('app-event', e)` inside `act`, not a `vi.mock` of `onAppEvent`; presentation tests can assert `calls` is empty. Frontend tsconfigs have no node types: don't import `node:*` in test setup.
- 2026-10 — Component tests share module-level Zustand stores across tests in a file. → Reset every store in `beforeEach` (`test/componentHelpers.ts`); the native file dialog is just `plugin:dialog|open` over IPC, so stub it as a handler. Global keydown handlers (Space) fire while typing in inputs: `user.type` with a space exposed a real bug; use `it.fails` to pin a known product bug without a skip.
