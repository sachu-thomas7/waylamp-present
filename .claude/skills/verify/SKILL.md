---
name: verify
description: Run this repo's scripts/verify.sh acceptance gate (feature/phase-scoped or full) and report pass/fail. Use whenever a feature, fix, or chunk of work needs to be checked as actually done — not just "cargo test passed locally" — before saying so or before committing/pushing.
---

# Verify

`scripts/verify.sh` is this repo's real Definition-of-Done, documented in `CLAUDE.md`. It's
more than `cargo test`: it bootstraps npm installs + frontend builds (hash-cached), then
runs phase/feature-scoped structural checks (`file_contains`-style grep assertions that the
right types/commands/components/tests exist) plus real `cargo test` / `cargo clippy -- -D
warnings` / `npm run typecheck` in both frontends. A `PreToolUse` hook already runs
`verify.sh core` automatically before any `git push` — this skill is for checking a specific
feature or the full suite on demand, mid-work.

## Usage

```
bash scripts/verify.sh              # full check, current phase
bash scripts/verify.sh f1.5         # one feature + its prerequisites
bash scripts/verify.sh f1           # all of Phase 1
bash scripts/verify.sh invariants   # architecture invariants only (fast, no build)
bash scripts/verify.sh core         # build + test + lint only, no structural checks
bash scripts/verify.sh --no-frontend  # skip the frontend build bootstrap (faster iteration)
```

Pick the narrowest scope that answers the actual question — `invariants` or a single `fX.Y`
while iterating, `f1`/full only when you need the whole-phase picture.

**Environment note (Windows/Git Bash):** `cargo` and `node` are not on the default PATH,
and `$HOME` resolves to the MSYS home, *not* the Windows user profile, so `$HOME/.cargo/bin`
silently doesn't exist. Before running manually:
```
export PATH="/c/Users/<you>/.cargo/bin:$PATH"
eval "$(fnm env --shell bash)"
```

## Interpreting the result

- **All passed** → the feature/change is actually done; safe to report as complete or commit.
- **Any failed** → do not report the work as done. Read the failing check names, fix them,
  re-run the same scope. Don't move on to the next feature with known failures.
- **Adding a new feature's gate:** follow the existing `run_fX_Y()` pattern in
  `scripts/verify.sh` — file existence (`file_exists`/`file_contains`) checks, then a scoped
  `cargo test <filter>` call — and wire the new function into the `case` dispatch, including
  the `f1|phase1)`-style aggregate *and* the `all|*)` fallthrough (easy to forget; it was
  itself out of date once already).
