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
