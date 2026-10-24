---
name: verify
description: Run this repo's scripts/verify.sh tiers (quick, push, deps, release, invariants) and report pass/fail. Use whenever a feature, fix, or chunk of work needs to be checked as actually done — not just "cargo test passed locally" — before saying so or before committing/pushing.
---

# Verify

`scripts/verify.sh` is this repo's Definition-of-Done (see `CLAUDE.md` "Verification"). CI and the pre-push hook call the
same tiers. It bootstraps npm + frontend dist (hash-cached), then runs real commands; no grep-for-a-word checks.

```
bash scripts/verify.sh quick        # while iterating: fmt, cargo check, lib+invariants tests, typecheck+Vitest for changed windows
bash scripts/verify.sh push         # before saying "done"/pushing: fmt, clippy -D warnings, full nextest, both frontends (Biome+Vitest)
bash scripts/verify.sh invariants   # tests/invariants.rs only
bash scripts/verify.sh deps         # cargo deny + osv + fuzz smoke (Docker optional)
bash scripts/verify.sh release [bundle]  # delegates to the release scripts if present
```

Use `quick` mid-work and `push` to finish (the hook runs `push` before any `git push`). `core` = `push`; `--no-frontend`
skips the frontend lane.

**Environment (Windows/Git Bash):** the script adds `/c/Users/sachu/.cargo/bin` and fnm itself. Set
`CARGO_TARGET_DIR` to the shared target dir if you build in a worktree.

## Interpreting the result

- **All steps passed** (exit 0) → done for that tier. **Any FAIL** → exit code = failed steps; read the tail printed under it
  (full log path is in the output), fix, re-run the same tier. Never report work done with failures.
- A skipped/ignored test is a failure, not a warning.
- **Adding a regression guard:** write a test in `src-tauri/tests/invariants.rs` (parse the real file, comment/string-aware) or a
  Vitest/nextest test; do not add grep checks to `verify.sh`.
