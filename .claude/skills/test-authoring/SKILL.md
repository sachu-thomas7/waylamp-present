---
name: test-authoring
description: How to plan, write, run, and debug automated tests in the Waylamp Present repo across all layers (Rust unit/integration with the mock Tauri runtime, Vitest component tests with mockIPC, golden fixtures, and real-app Playwright-over-WebView2 E2E). Use whenever adding or changing tests, building an item from dev-docs/test_framework_roadmap.md, writing E2E journeys, diagnosing a failing or flaky test, or deciding which test layer a bug belongs in.
---

# Test authoring (Waylamp Present)

Specs of record: `dev-docs/test_framework_roadmap.md` (what to build, in order) and
`dev-docs/test_plan.md` (what must be covered and why). `CLAUDE.md` holds repo conventions
and architecture invariants. This skill is the **how**. Read the roadmap item and the
relevant `test_plan.md` rows first; read `references/` only when you need the detail below.

## 1. Pick the lowest layer that can see the bug

| The behaviour is… | Layer | Where | Pattern |
|---|---|---|---|
| pure logic (state machine, parser fn, store reducer) | L1 | `#[cfg(test)]` / `*.test.ts` next to the code | plain asserts |
| a command's effect on state + bus events | L2 Rust | `src-tauri/tests/<area>.rs` | mock runtime + real `EventBus`, collect `AppEvent`s |
| what a component renders / which command it invokes | L2 JS | `controller/src/**` or `presentation/src/**` | Testing Library + `mockIPC` + event registry (copy `controller/src/test/eventWiring.integration.test.tsx`) |
| parser/serialization output must not drift | L3 | fixtures + snapshots | golden JSON |
| windows talk to each other / what the projector shows | L4 | `e2e/specs/` | Playwright over CDP, page objects |
| hardware/OS (displays, global keys, native dialog) | L5 | `test_plan.md` §9 | manual — do not fake it |

If a layer-4 test fails and a layer-1/2 test could have caught it, add the lower test too.

## 2. Non-negotiable rules

1. **Prove it can fail.** After writing a test, break the behaviour on purpose (invert the
   assertion, comment out the production line, change a payload field), see it fail for the
   *right reason*, restore. State what you broke in your report. A test never seen red is
   not done.
2. **Assert outcomes, not implementation.** Check the published `AppEvent` payload, the
   rendered text, the window state — not that a function was called.
3. **Wait on state, never on time.** No `sleep`, `waitForTimeout`, or fixed delays. Use
   event collection with a deadline, `waitFor`, `expect.poll`, auto-waiting locators.
4. **Real Rust, real bus; mock only the OS boundary.** Don't mock `EventBus` or the state
   machine in Rust tests. In JS, mock `invoke` via `mockIPC` and events via the registry
   pattern; never mock the stores under test.
5. **Isolation.** Each test owns its data (temp dir, fresh stores, fresh app instance for
   E2E). No shared mutable state, no order dependence, never touch the real app-data dir.
6. **Locators.** Role/label/text first; add a `data-testid` to the component if needed.
   Never CSS classes or DOM position. In E2E all locators live in `e2e/pages/*`.
7. **Presentation is listen-only.** Any test that makes the presentation window `invoke` is
   wrong; add the opposite test (it never invokes) when touching it.
8. **No weakening the product for tests.** Test seams are compiled out of release
   (`cfg(debug_assertions)` / feature `e2e`); no `withGlobalTauri`, no extra permissions.
9. **Deterministic fixtures.** Add via `dev-docs/test-assets/make_test_assets.cjs` (then
   copy to `src-tauri/tests/fixtures/`); never hand-edit binaries.
10. **No retries in the gate.** Fix the cause. Quarantine only with owner + expiry.

## 2b. Directory contract

```
src-tauri/tests/                  Rust integration tests (one file per area) + fixtures/
controller/src/**/*.test.ts(x)    co-located unit/component tests
controller/src/test/              shared setup, mockIPC helpers, render helpers
presentation/src/test/            same
e2e/                              L4 harness (own package.json), specs/, pages/, fixtures/
```
`verify.sh t0` asserts these dirs exist; add new top-level test dirs there too.

## 3. Workflow

1. **Plan** (anything beyond one small test): list the scenarios and the layer for each,
   map them to `test_plan.md` rows, and state which existing tests you will not duplicate.
   Show the plan before generating many files.
2. **Discover** conventions: read the neighbouring tests and the code under test. Reuse
   helpers; do not invent a new pattern when one exists.
3. **Write** the smallest test that expresses one behaviour. One reason to fail per test.
4. **Run narrow, then wide**: single test → file → `bash scripts/verify.sh <gate>`.
5. **Prove it can fail** (rule 1).
6. **Scan**: no `.only`/`.skip`, no sleeps, no empty catches, every test asserts
   (`references/false-green-rules.md`; `scripts/scan-tests.sh` once T6.1 exists).
7. **Wire the gate**: add/extend the roadmap item's `run_*` block in `scripts/verify.sh`
   (file-exists → content check → scoped test command) and the `case` dispatch, including
   the `all|*)` fallthrough.
8. **Update docs**: the `test_plan.md` §4 traceability row, `CLAUDE.md` "Current status",
   and `references/lessons.md` if you learned something non-obvious.

## 4. Commands (Windows / Git Bash)

```bash
export PATH="/c/Users/<you>/.cargo/bin:$PATH"; eval "$(fnm env --shell bash)"
cargo test --manifest-path src-tauri/Cargo.toml <filter>
npm test --prefix controller -- <file-or-pattern>      # vitest run
npm test --prefix presentation -- <file-or-pattern>
bash scripts/verify.sh core | t1 | t2 | t3 | e2e       # gates
```
`$HOME` is the MSYS home — use the explicit `/c/Users/<you>/...` paths. Frontend `dist/`
must exist before cargo builds (the `custom-protocol` feature embeds it); `verify.sh`
bootstraps this. PowerShell is the escape hatch if a broken tree blocks Bash via the hook.

## 5. Debugging a failure

1. Identify the layer from the failing test's name/path. Read the *first* error, not the last.
2. L4: open the Playwright trace and **both** windows' screenshots, then the app log and the
   event log. Decide: product regression, test bug, or environment (port/user-data folder/
   leaked process).
3. Reproduce one layer lower. If you can, the fix and the permanent test belong there.
4. Product bug → failing test first, then fix. Test bug → fix the test and say why it was
   wrong. Never "fix" by loosening an assertion without stating what behaviour you stopped
   checking.
5. Flaky = a real race or a missing readiness signal. Find it; do not add retries or sleeps.

## 6. Things that bite in this repo

- `AppEvent`'s TS union must stay `{ type, payload }`-discriminated or narrowing breaks.
- `DeckEnd`/`DeckStart` are emitted **instead of** `SlideUpdate` at boundaries.
- quick-xml distinguishes `Start` vs `Empty` events; real PowerPoint writes `<p:ph/>`
  self-closing — fixtures must include both forms. quick-xml 0.42 returns `str` names/values
  and emits entity references separately; check `&amp;` handling explicitly.
- `tokio::spawn` from `.setup()` panics only at runtime; unit tests won't show it — L4 will.
- jsdom has no WebCrypto: polyfill in setup before `mockIPC`.
- Presentation URL differs dev (`http://localhost:5174`, needs `remote.urls` capability) vs
  release (bundled) — test the production-like build as the default for E2E.
- PDF import panics without pdfium; keep that test `#[ignore]`d until the library is bundled.

## 7. Boundaries

Never run git write commands. Never edit hooks, `.claude/settings.json`, or CI workflows
without explicit human approval. Do not start another roadmap item; report it instead.
Final report: files touched, exact gate command + result, what you broke to prove each new
test can fail, any `CLAUDE.md`/plan discrepancies found, and anything deliberately left out.

References (read on demand): `references/layer-templates.md` (copy-ready skeletons),
`references/false-green-rules.md` (review checklist), `references/lessons.md` (running log).
