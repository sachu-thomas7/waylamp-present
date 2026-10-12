# False-green review checklist

A test that cannot fail is worse than no test. Use this when writing and when reviewing
(also the rule set for `scripts/scan-tests.sh`, roadmap T6.1). Pattern ideas are adapted from
public E2E-quality catalogs; severities are ours.

## P0 — silently always passes

| ID | Smell | Fix |
|---|---|---|
| G1 | Test name promises behaviour X; assertions check Y | Rename or assert X |
| G2 | No assertion (action only) | Assert the observable outcome |
| G3 | Error swallowed: `try/catch` without rethrow, `.catch(() => {})`, `let _ =` on a Result being tested | Let it fail; assert the error explicitly if expected |
| G4 | Vacuous assertion: `expect(locator).toBeTruthy()`, `toBeDefined()` on something always defined, `assert!(true)`, `len() >= 0` | Assert a concrete value |
| G5 | Conditional assertion (`if (visible) expect…`) | Assert unconditionally; make state deterministic |
| G6 | `test.only` / `describe.only` / `fit` left in | Remove; scanner blocks |
| G7 | Playwright `expect(...)` or action without `await` | Add `await` |
| G8 | Assertion on the mock you just configured (tests the mock) | Assert real output of the code under test |
| G9 | Event collection that returns empty and the test only checks "no panic" | Assert the expected events and their order |

## P1 — fragile or uninformative

| ID | Smell | Fix |
|---|---|---|
| G10 | `sleep`, `waitForTimeout`, fixed `Duration` waits | Wait on state/event with a deadline |
| G11 | `force: true` / `{ force }` clicks without a comment | Fix the real obstruction |
| G12 | CSS-class or positional selectors | role/label/text/`data-testid`, via page object |
| G13 | `test.skip` / `#[ignore]` without a linked reason | Add reason + owner + expiry, or delete |
| G14 | Shared mutable state / order dependence between tests | Per-test setup; run with shuffled order |
| G15 | Reads or writes real `%APPDATA%` / real config | Temp dir + `STREAMDECK_DATA_DIR` |
| G16 | `expect.soft` overuse hiding the first failure | Use hard expects |
| G17 | Snapshot auto-updated in the same change as the code it guards | Review the diff by hand |
| G18 | Test passes both before and after the production change | It doesn't test the change — redo |

## P2 — maintenance

- Duplicate coverage of an existing lower-layer test (move up/down instead).
- Locators duplicated across specs (belongs in the page object).
- Fixtures hand-edited instead of generated.

## Prove-it-can-fail recipe

1. Run the new test: green.
2. Introduce the smallest fault that should break it (flip the expected value; comment the
   production line; change one payload field; for E2E, break the DOM text).
3. Run again: it must fail, and the message must point at the right cause.
4. Revert the fault; run again: green. Record the fault in your final report.

For parsers and the state machine, `cargo mutants` (roadmap T6.2) does this at scale;
surviving mutants are missing tests.
