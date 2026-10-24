#!/usr/bin/env bash
# Waylamp Present verification tiers. CI, the pre-push hook and humans all call this, so they cannot drift.
#
#   bash scripts/verify.sh quick       fmt, cargo check, fast Rust tests + typecheck/Vitest for what changed (every edit)
#   bash scripts/verify.sh push        fmt, clippy -D warnings, ONE nextest run (incl. invariants), both frontends
#                                      typecheck + Biome + Vitest once (pre-push hook and CI). `core` is an alias.
#   bash scripts/verify.sh invariants  tests/invariants.rs only
#   bash scripts/verify.sh deps        cargo deny, osv-scanner, fuzz smoke (needs Docker; skipped without it)
#   bash scripts/verify.sh release [bundle]  delegates to scripts/build-release.sh / check-bundle.sh <bundle> if present
# Options: --only rust|frontend (push) or deny|osv|fuzz (deps); --no-frontend skips the frontend lane and dist build.
# Exit code = number of failed steps. Per-step logs live in a fresh temp dir (path printed on failure).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
for d in "$(cygpath -u "${USERPROFILE:-}" 2>/dev/null)/.cargo/bin" "$HOME/.cargo/bin" "${USERPROFILE:-}/.cargo/bin"; do [ -d "$d" ] && PATH="$d:$PATH" && break; done
command -v fnm >/dev/null 2>&1 && eval "$(fnm env --shell bash 2>/dev/null)"

TIER=""; ONLY=""; NO_FRONTEND=false; ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --no-frontend) NO_FRONTEND=true ;;
    --only) ONLY="${2:-}"; shift ;;
    -*) echo "unknown option $1"; exit 2 ;;
    *) [ -z "$TIER" ] && TIER="$1" || ARGS+=("$1") ;;
  esac
  shift
done
[ "$TIER" = core ] && TIER=push
[ -f src-tauri/Cargo.toml ] || { echo "run from the repo root"; exit 2; }

LOGS="$(mktemp -d)"; CACHE=".verify-cache"; mkdir -p "$CACHE"
win() { cygpath -m "$1" 2>/dev/null || echo "$1"; }
RC_DIR="$LOGS/rc"; mkdir -p "$RC_DIR"
FRONTENDS=(controller presentation)
CARGO=(cargo)
MANIFEST=(--manifest-path src-tauri/Cargo.toml)

# step <label> <cmd...>: run one step, log to $LOGS/<label>.log, append a result line to $OUT (lane output file).
OUT="$LOGS/main.out"
step() {
  local label="$1"; shift
  local log="$LOGS/${label//[^A-Za-z0-9._-]/_}.log" t0=$SECONDS
  if "$@" >"$log" 2>&1; then
    echo "  ok    $label ($((SECONDS - t0))s)" >>"$OUT"
  else
    { echo "  FAIL  $label ($((SECONDS - t0))s) - log: $log"; tail -n 25 "$log" | sed 's/^/        /'; } >>"$OUT"
    echo "$label" >>"$RC_DIR/failed"
  fi
}
skipped() { echo "  skip  $1" >>"$OUT"; }

# lane <name> <function>: run a function with its own output file, so lanes can run concurrently.
lane() { ( OUT="$LOGS/$1.out"; : >"$OUT"; "$2" ); }
show() { for f in "$@"; do [ -f "$LOGS/$f.out" ] && cat "$LOGS/$f.out"; done; }

# --- bootstrap: npm ci + dist build, each skipped while a content hash is unchanged ---------------
hash_of() { { find "$@" -type f -not -path '*/node_modules/*' -not -path '*/dist/*' 2>/dev/null | sort | xargs md5sum 2>/dev/null; } | md5sum | cut -d' ' -f1; }
npm_install() {
  local w="$1" h; h="$(hash_of "$w/package.json" "$w/package-lock.json")"
  [ "$h" = "$(cat "$CACHE/$w-npm.hash" 2>/dev/null)" ] && [ -d "$w/node_modules" ] && { echo "  ok    $w: node_modules cached" >>"$OUT"; return; }
  step "$w npm ci" npm ci --prefix "$w" && echo "$h" >"$CACHE/$w-npm.hash"
}
dist_build() {
  local w="$1" h; h="$(hash_of "$w/src" "$w/public" "$w/index.html" "$w/package.json" "$w"/vite.config.* "$w"/tsconfig*.json)"
  [ "$h" = "$(cat "$CACHE/$w-build.hash" 2>/dev/null)" ] && [ -d "$w/dist" ] && { echo "  ok    $w: dist cached" >>"$OUT"; return; }
  step "$w build" npm run build --prefix "$w" && echo "$h" >"$CACHE/$w-build.hash"
}
bootstrap() { # $1 = true when cargo needs dist/ (rust lane will run)
  for w in "${FRONTENDS[@]}"; do lane "boot-$w" "npm_install_$w" & done; wait
  show boot-controller boot-presentation
  [ -s "$RC_DIR/failed" ] && return 1
  if [ "$1" = true ] && [ "$NO_FRONTEND" = false ]; then
    for w in "${FRONTENDS[@]}"; do lane "dist-$w" "dist_build_$w" & done; wait
    show dist-controller dist-presentation
  fi
  [ -s "$RC_DIR/failed" ] && return 1 || return 0
}
npm_install_controller() { npm_install controller; }; npm_install_presentation() { npm_install presentation; }
dist_build_controller() { dist_build controller; };   dist_build_presentation() { dist_build presentation; }

# --- building blocks ------------------------------------------------------------------------------
fmt()     { step "cargo fmt --check" "${CARGO[@]}" fmt --check "${MANIFEST[@]}"; }
nextest() { # extra args pass through; fails on zero tests and on anything skipped/ignored
  step "cargo nextest $*" bash -c '"$@" >"$0" 2>&1; rc=$?; cat "$0"; [ $rc -eq 0 ] && ! grep -Eq "[1-9][0-9]* skipped" "$0"' \
    "$(win "$LOGS")/nextest.raw" cargo nextest run --locked --no-fail-fast --no-tests=fail "${MANIFEST[@]}" "$@"
}
vitest_check() { # Vitest JSON report: no failed/pending/todo tests, at least one test ran
  local w="$1" j; j="$(win "$LOGS")/vitest-$w.json"
  step "$w vitest" bash -c 'npm test --prefix "$1" -- --reporter=default --reporter=json --outputFile.json="$2" && node scripts/check-vitest-json.cjs "$2"' _ "$w" "$j"
}
frontend_checks() { # $@ = windows
  for w in "$@"; do
    step "$w typecheck" npm run typecheck --prefix "$w"
    [ "$TIER" = push ] && step "$w biome" npm run lint --prefix "$w"
    vitest_check "$w"
  done
}
rust_push_lane() { fmt; step "cargo clippy" cargo clippy --locked "${MANIFEST[@]}" --all-targets -- -D warnings; nextest; }
# One lane per window so they run concurrently; each logs into its own .out file.
fe_lane_controller() { frontend_checks controller; };     fe_lane_presentation() { frontend_checks presentation; }

finish() {
  echo; local n=0; [ -f "$RC_DIR/failed" ] && n=$(wc -l <"$RC_DIR/failed")
  if [ "$n" -eq 0 ]; then echo "verify $TIER: all steps passed in ${SECONDS}s"; else echo "verify $TIER: $n step(s) failed in ${SECONDS}s (logs: $LOGS)"; fi
  exit "$n"
}

# --- tiers ----------------------------------------------------------------------------------------
echo "verify $TIER (logs: $LOGS)"
case "$TIER" in
  push)
    want_rust=true; want_fe=true
    [ "$ONLY" = rust ] && want_fe=false; [ "$ONLY" = frontend ] && want_rust=false
    [ "$NO_FRONTEND" = true ] && want_fe=false
    bootstrap "$want_rust" || { cat "$LOGS/main.out" 2>/dev/null; finish; }
    [ "$want_rust" = true ] && { lane rust rust_push_lane & }
    if [ "$want_fe" = true ]; then
      for w in "${FRONTENDS[@]}"; do lane "fe-$w" "fe_lane_$w" & done
    fi
    wait; show rust fe-controller fe-presentation ;;
  quick)
    base="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main 2>/dev/null || echo HEAD)"
    changed="$({ git diff --name-only "$base"; git ls-files -o --exclude-standard; } | sort -u)"
    bootstrap true || { finish; }
    fmt
    step "cargo check" cargo check --locked "${MANIFEST[@]}" --all-targets
    if grep -Eq '^(src-tauri/|Cargo\.|rust-toolchain)' <<<"$changed"; then nextest --lib --test invariants; else skipped "nextest (no Rust changes vs $base)"; fi
    for w in "${FRONTENDS[@]}"; do
      if grep -q "^$w/" <<<"$changed" && [ "$NO_FRONTEND" = false ]; then frontend_checks "$w"; else skipped "$w typecheck/vitest (unchanged vs $base)"; fi
    done
    cat "$OUT" ;;
  invariants)
    bootstrap true || { finish; }
    nextest --test invariants; cat "$OUT" ;;
  deps)
    if [ -z "$ONLY" ] || [ "$ONLY" = deny ]; then
      if command -v cargo-deny >/dev/null 2>&1; then step "cargo deny" cargo deny --locked "${MANIFEST[@]}" check; else skipped "cargo deny (install: cargo install cargo-deny --locked)"; fi
    fi
    [ -z "$ONLY" ] || [ "$ONLY" = osv ] && step "osv-scanner" bash scripts/osv-scan.sh
    if [ -z "$ONLY" ] || [ "$ONLY" = fuzz ]; then
      if docker info >/dev/null 2>&1; then
        step "fuzz smoke (30s)" bash -c 'docker build -q -t waylamp-present-fuzz src-tauri/fuzz && MSYS_NO_PATHCONV=1 docker run --rm -v "$(pwd -W 2>/dev/null || pwd):/work" -v waylamp-present-fuzz-target:/fuzz-target -e CARGO_TARGET_DIR=/fuzz-target -w /work/src-tauri waylamp-present-fuzz cargo +nightly fuzz run pptx_import fuzz/seeds/pptx_import -- -max_total_time=30'
      else skipped "fuzz smoke (Docker unavailable)"; fi
    fi
    cat "$OUT" ;;
  release)
    if [ -f scripts/build-release.sh ]; then step "build-release.sh" bash scripts/build-release.sh; else skipped "scripts/build-release.sh not present yet (owned by the release work)"; fi
    if [ -f scripts/check-bundle.sh ] && [ -n "${ARGS[0]:-}" ]; then step "check-bundle.sh ${ARGS[0]}" bash scripts/check-bundle.sh "${ARGS[0]}"; else skipped "check-bundle.sh (pass a bundle path: verify.sh release <installer|dir>)"; fi
    cat "$OUT" ;;
  *) sed -n 2,11p "$0"; exit 2 ;;
esac
finish
