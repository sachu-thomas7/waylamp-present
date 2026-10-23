#!/usr/bin/env bash
# ╔══════════════════════════════════════════════════════════════════╗
# ║  StreamDeck — Universal Verification Framework                  ║
# ║  Run from project root.                                         ║
# ║                                                                 ║
# ║  Usage:                                                         ║
# ║    bash verify.sh                → full check (current phase)   ║
# ║    bash verify.sh f0.2           → F0.2 feature checks only     ║
# ║    bash verify.sh f0             → all Phase 0 checks           ║
# ║    bash verify.sh invariants     → architecture invariants only  ║
# ║    bash verify.sh release        → release bundling checks       ║
# ║    bash verify.sh core           → build + test + lint only      ║
# ║    bash verify.sh --no-frontend  → skip frontend build step      ║
# ╚══════════════════════════════════════════════════════════════════╝

set -euo pipefail

# ── Colours ────────────────────────────────────────────────────────
RED='\033[0;31m';   GREEN='\033[0;32m';  YELLOW='\033[1;33m'
CYAN='\033[0;36m';  BLUE='\033[0;34m';   MAGENTA='\033[0;35m'
BOLD='\033[1m';     DIM='\033[2m';        RESET='\033[0m'

PASS="${GREEN}✔${RESET}";  FAIL="${RED}✗${RESET}"
WARN="${YELLOW}⚠${RESET}"; SKIP="${DIM}–${RESET}"; INFO="${CYAN}›${RESET}"

# ── State ──────────────────────────────────────────────────────────
FAILURES=0; WARNINGS=0; SKIPPED=0; PASSED=0
FILTER="all"
SKIP_FRONTEND=false
LOG=/tmp/streamdeck-verify.log
CACHE_DIR=".verify-cache"

# ── Arg parsing ────────────────────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --no-frontend) SKIP_FRONTEND=true ;;
    *) FILTER="$arg" ;;
  esac
done
FILTER=$(echo "$FILTER" | tr '[:upper:]' '[:lower:]')

# ── Helpers ────────────────────────────────────────────────────────
section() {
  echo ""
  echo -e "${BOLD}${BLUE}━━━ $1 ━━━${RESET}"
}
subsection() { echo ""; echo -e "  ${BOLD}${CYAN}$1${RESET}"; }
pass()  { echo -e "  ${PASS}  $1";                         PASSED=$((PASSED+1)); }
fail()  { echo -e "  ${FAIL}  ${RED}$1${RESET}";           FAILURES=$((FAILURES+1)); }
warn()  { echo -e "  ${WARN}  ${YELLOW}$1${RESET}";        WARNINGS=$((WARNINGS+1)); }
skip()  { echo -e "  ${SKIP}  ${DIM}$1 (skipped)${RESET}"; SKIPPED=$((SKIPPED+1)); }
info()  { echo -e "  ${INFO}  ${DIM}$1${RESET}"; }

check() {
  local label="$1"; shift
  if "$@" > "$LOG" 2>&1; then
    pass "$label"
  else
    fail "$label"
    echo -e "${DIM}$(head -n 25 "$LOG" | sed 's/^/      /')${RESET}"
  fi
}
check_warn() {
  local label="$1"; shift
  if "$@" > "$LOG" 2>&1; then
    pass "$label"
  else
    warn "$label (non-blocking)"
    echo -e "${DIM}$(head -n 10 "$LOG" | sed 's/^/      /')${RESET}"
  fi
}

file_exists()      { [ -f "$1" ] && pass "exists: $1"                    || fail "MISSING: $1"; }
file_absent()      { [ ! -f "$1" ] && pass "absent (correct): $1"        || fail "MUST BE DELETED: $1"; }
dir_exists()       { [ -d "$1" ] && pass "exists: $1/"                   || fail "MISSING dir: $1/"; }
file_contains()    {
  grep -q "$3" "$2" 2>/dev/null && pass "$1" || fail "$1 — pattern not found in $2"
}
file_not_contains() {
  ! grep -q "$3" "$2" 2>/dev/null && pass "$1" || fail "$1 — forbidden pattern found in $2"
}

file_not_exists() {
  local label="$1"
  local file="$2"
  if [ -f "$file" ]; then
    fail "$label — $file should NOT exist but does"
  else
    pass "$label"
  fi
}

# ── Guard: project root ────────────────────────────────────────────
if [ ! -f "src-tauri/Cargo.toml" ]; then
  echo -e "${RED}ERROR: Run from the project root (where src-tauri/ lives).${RESET}"
  exit 1
fi

# ══════════════════════════════════════════════════════════════════
# STEP 0 — DEPENDENCY BOOTSTRAP
# Ensures npm packages and frontend dist exist before anything else.
# Uses a hash-based cache so re-runs are instant when sources haven't changed.
# ══════════════════════════════════════════════════════════════════
run_bootstrap() {
  section "Dependency Bootstrap"
  mkdir -p "$CACHE_DIR"

  # ── Node / npm ──────────────────────────────────────────────────
  subsection "Node.js & npm"
  if ! command -v node &>/dev/null; then
    fail "node not found — install Node.js 20+ from https://nodejs.org"
    return
  fi
  if ! command -v npm &>/dev/null; then
    fail "npm not found"
    return
  fi
  pass "node $(node --version)"
  pass "npm  $(npm --version)"

  # ── Rust toolchain ──────────────────────────────────────────────
  subsection "Rust toolchain"
  if ! command -v cargo &>/dev/null; then
    fail "cargo not found — install Rust from https://rustup.rs"
    return
  fi
  pass "cargo $(cargo --version)"
  check_warn "cargo-tauri CLI available" bash -c 'cargo tauri --version'

  # ── npm install (controller) ────────────────────────────────────
  subsection "npm install — controller"
  _npm_install "controller"

  # ── npm install (presentation) ──────────────────────────────────
  subsection "npm install — presentation"
  _npm_install "presentation"

  # ── Frontend build (if needed) ──────────────────────────────────
  if [ "$SKIP_FRONTEND" = true ]; then
    info "Skipping frontend build (--no-frontend)"
  else
    subsection "Frontend build — controller (required for cargo build/clippy)"
    _frontend_build "controller"

    subsection "Frontend build — presentation"
    _frontend_build "presentation"
  fi
}

# Install npm deps only if package.json changed since last install
_npm_install() {
  local win="$1"
  local pkg="$win/package.json"
  local lock="$win/package-lock.json"
  local cache_file="$CACHE_DIR/${win}-npm.hash"

  if [ ! -f "$pkg" ]; then
    skip "$win/package.json not found"
    return
  fi

  # Hash package.json + lockfile
  local current_hash
  current_hash=$(cat "$pkg" "${lock:-/dev/null}" 2>/dev/null | md5sum | cut -d' ' -f1)
  local cached_hash=""
  [ -f "$cache_file" ] && cached_hash=$(cat "$cache_file")

  if [ "$current_hash" = "$cached_hash" ] && [ -d "$win/node_modules" ]; then
    pass "$win: node_modules up to date (cached)"
  else
    info "$win: running npm install..."
    if (cd "$win" && npm install) > "$LOG" 2>&1; then
      echo "$current_hash" > "$cache_file"
      pass "$win: npm install complete"
    else
      fail "$win: npm install failed"
      cat "$LOG"
    fi
  fi
}

# Build frontend only if src/ changed since last build
_frontend_build() {
  local win="$1"
  local src_dir="$win/src"
  local dist_dir="$win/dist"
  local cache_file="$CACHE_DIR/${win}-build.hash"

  if [ ! -d "$src_dir" ]; then
    skip "$win/src not found"
    return
  fi

  # Hash everything the build consumes: src/, public/, index.html, vite/ts config, package.json
  local current_hash
  current_hash=$( { find "$src_dir" "$win/public" -type f 2>/dev/null; ls "$win"/index.html "$win"/vite.config.* "$win"/tsconfig*.json "$win"/package.json 2>/dev/null; }     | sort | xargs md5sum 2>/dev/null | md5sum | cut -d' ' -f1)
  local cached_hash=""
  [ -f "$cache_file" ] && cached_hash=$(cat "$cache_file")

  if [ "$current_hash" = "$cached_hash" ] && [ -d "$dist_dir" ]; then
    pass "$win: dist/ up to date (cached — no source changes)"
  else
    info "$win: source changed or dist missing — building..."
    if (cd "$win" && npm run build) > "$LOG" 2>&1; then
      echo "$current_hash" > "$cache_file"
      pass "$win: frontend build complete → $dist_dir/"
    else
      fail "$win: frontend build failed"
      cat "$LOG"
    fi
  fi
}

# ══════════════════════════════════════════════════════════════════
# ARCHITECTURE INVARIANTS
# ══════════════════════════════════════════════════════════════════
run_invariants() {
  section "Architecture Invariants (all phases)"

  subsection "No unwrap() in command handlers"
  check "No .unwrap() in src-tauri/src/commands/" \
    bash -c '! grep -rn "\.unwrap()" src-tauri/src/commands/ 2>/dev/null \
             | grep -v "#\[cfg(test)\]" | grep -q .'

  subsection "AppError invariants"
  if [ -f "src-tauri/src/error.rs" ]; then
    file_contains "#[non_exhaustive] present"            "src-tauri/src/error.rs" "non_exhaustive"
    file_contains "serde tag+content discriminated union" "src-tauri/src/error.rs" 'serde(tag = "type", content = "message")'
    file_not_contains "No manual From<AppError> impl"    "src-tauri/src/error.rs" "impl From<AppError>"
  else
    skip "error.rs not yet created"
  fi

  subsection "No raw invoke() in frontend components"
  for win in controller presentation; do
    if [ -d "$win/src" ]; then
      check "No raw invoke() outside ipc.ts in $win/" \
        bash -c "! grep -rn \"invoke(\" $win/src \
                 --include='*.ts' --include='*.tsx' \
                 --exclude='ipc.ts' --exclude='*.test.ts' --exclude='*.test.tsx' --exclude-dir=test 2>/dev/null | grep -q ."
    else
      skip "$win/src not yet created"
    fi
  done

  subsection "Tauri wiring gated from test compilation"
  if [ -f "src-tauri/src/app.rs" ]; then
    check "app.rs gated via cfg(not(test)) in lib.rs" \
      bash -c 'grep -q "cfg(not(test))" src-tauri/src/lib.rs \
               && grep -q "mod app" src-tauri/src/lib.rs'
  else
    skip "app.rs not yet created (pre-F0.2)"
  fi
}

# ══════════════════════════════════════════════════════════════════
# F0.1 — Tauri v2 Project Bootstrap
# ══════════════════════════════════════════════════════════════════
run_f0_1() {
  section "F0.1 — Tauri v2 Project Bootstrap"

  subsection "File structure"
  file_exists "src-tauri/Cargo.toml"
  file_exists "src-tauri/tauri.conf.json"
  file_exists "src-tauri/capabilities/default.json"
  file_exists "src-tauri/src/lib.rs"
  dir_exists  "controller/src"
  dir_exists  "presentation/src"

  subsection "Window configuration"
  file_contains "controller window defined"   "src-tauri/tauri.conf.json" "controller"
  file_contains "presentation window defined" "src-tauri/tauri.conf.json" "presentation"

  subsection "Capabilities"
  file_not_contains "controller does not grant unused create-webview-window" \
    "src-tauri/capabilities/default.json" "create-webview-window"

  subsection "CI pipeline"
  if [ -d ".github/workflows" ]; then
    check       "CI workflow file exists"     bash -c 'ls .github/workflows/*.yml 2>/dev/null | grep -q .'
    check_warn  "CI runs cargo test"          bash -c 'grep -r "cargo test"   .github/workflows/ | grep -q .'
    check_warn  "CI runs cargo clippy"        bash -c 'grep -r "cargo clippy" .github/workflows/ | grep -q .'
    check_warn  "CI runs npm run typecheck"   bash -c 'grep -r "typecheck"    .github/workflows/ | grep -q .'
  else
    warn ".github/workflows/ not found — CI not configured yet"
  fi
}

# ══════════════════════════════════════════════════════════════════
# F0.2 — IPC Command Layer
# ══════════════════════════════════════════════════════════════════
run_f0_2() {
  section "F0.2 — IPC Command Layer"

  subsection "File structure"
  file_exists  "src-tauri/src/error.rs"
  file_exists  "src-tauri/src/app.rs"
  file_exists  "src-tauri/src/commands/mod.rs"
  file_exists  "src-tauri/src/commands/info.rs"
  file_exists  "src-tauri/src/commands/system.rs"
  file_absent  "src-tauri/src/commands.rs"
  file_exists  "controller/src/lib/ipc.ts"
  file_exists  "presentation/src/lib/ipc.ts"

  subsection "error.rs"
  file_contains "derive Debug"               "src-tauri/src/error.rs" "derive.*Debug"
  file_contains "thiserror::Error"           "src-tauri/src/error.rs" "thiserror::Error"
  file_contains "serde::Serialize"           "src-tauri/src/error.rs" "serde::Serialize"
  file_contains "#[non_exhaustive]"          "src-tauri/src/error.rs" "non_exhaustive"
  file_contains "Internal variant"           "src-tauri/src/error.rs" "Internal"
  file_contains "NotFound variant"           "src-tauri/src/error.rs" "NotFound"
  file_contains "InvalidInput variant"       "src-tauri/src/error.rs" "InvalidInput"

  subsection "commands/system.rs"
  file_contains "PingResponse struct"              "src-tauri/src/commands/system.rs" "PingResponse"
  file_contains "pub async fn ping"                "src-tauri/src/commands/system.rs" "pub async fn ping"
  file_contains "returns Result<PingResponse>"     "src-tauri/src/commands/system.rs" "Result<PingResponse"
  file_contains "#[tauri::command]"                "src-tauri/src/commands/system.rs" "#\[tauri::command\]"
  file_contains "#[cfg(test)] block present"       "src-tauri/src/commands/system.rs" "#\[cfg(test)\]"
  file_contains "latency assertion (as_millis)"    "src-tauri/src/commands/system.rs" "as_millis"

  subsection "lib.rs / app.rs wiring"
  file_contains "mod commands declared"      "src-tauri/src/lib.rs" "mod commands"
  file_contains "mod error declared"         "src-tauri/src/lib.rs" "mod error"
  file_contains "ping registered"            "src-tauri/src/state.rs" "system::ping"
  file_contains "get_build_info registered"  "src-tauri/src/state.rs" "info::get_build_info"

  subsection "ipc.ts — both windows"
  for win in controller presentation; do
    local f="$win/src/lib/ipc.ts"
    if [ -f "$f" ]; then
      file_contains "$win: AppError type"          "$f" "AppError"
      file_contains "$win: PingResponse interface" "$f" "PingResponse"
      if [ "$win" = controller ]; then
        file_contains "$win: ping() exported"        "$f" "export async function ping"
        file_contains "$win: @tauri-apps/api import" "$f" "@tauri-apps/api"
      else
        check "presentation ipc.ts has no invoke wrappers (listen-only)" bash -c '! grep -q "invoke" presentation/src/lib/ipc.ts'
      fi
    else
      fail "MISSING: $f"
    fi
  done

  subsection "cargo test (independent — no frontend needed)"
  check "All Rust unit tests pass" \
    bash -c 'cd src-tauri && cargo test 2>&1 | tee /tmp/sd-test.log \
             && ! grep -q "FAILED" /tmp/sd-test.log'

  subsection "TypeScript typecheck"
  for win in controller presentation; do
    if grep -q '"typecheck"' "$win/package.json" 2>/dev/null; then
      check "$win: npm run typecheck" bash -c "cd $win && npm run typecheck"
    else
      warn "$win: no typecheck script in package.json"
    fi
  done
}

# ══════════════════════════════════════════════════════════════════
# F0.3 — Event Bus Foundation  (scaffolded — skips gracefully)
# ══════════════════════════════════════════════════════════════════
run_f0_3() {
  section "F0.3 — Event Bus Foundation"
  if [ ! -f "src-tauri/src/events.rs" ]; then
    skip "events.rs not created — F0.3 not started yet"
    return
  fi
  file_contains "pub enum AppEvent"     "src-tauri/src/events.rs" "pub enum AppEvent"
  file_contains "#[non_exhaustive]"     "src-tauri/src/events.rs" "non_exhaustive"
  file_contains "serde::Serialize"      "src-tauri/src/events.rs" "serde::Serialize"
  file_contains "broadcast channel"     "src-tauri/src/bus.rs" "broadcast"
  file_contains "BusLag variant"        "src-tauri/src/events.rs" "BusLag"
  check "Event bus unit tests pass" \
    bash -c 'cd src-tauri && cargo test events 2>&1 | tee /tmp/sd-test.log \
             && ! grep -q "FAILED" /tmp/sd-test.log'
}

# ══════════════════════════════════════════════════════════════════
# F0.4 — TOML Config Loader  (scaffolded — skips gracefully)
# ══════════════════════════════════════════════════════════════════
run_f0_4() {
  section "F0.4 — TOML Config Loader"
  if [ ! -f "src-tauri/src/config.rs" ]; then
    skip "config.rs not created — F0.4 not started yet"
    return
  fi
  file_contains "StreamDeckConfig struct" "src-tauri/src/config.rs" "StreamDeckConfig"
  file_contains "serde::Deserialize"       "src-tauri/src/config.rs" "serde::Deserialize"
  file_contains "notify hot-reload"       "src-tauri/src/config_loader.rs" "notify"
  file_contains "version field"           "src-tauri/src/config.rs" "version"
  file_not_contains "No plaintext tokens" "src-tauri/src/config.rs" "bearer_token.*String"
}

run_f1_1() {
  section "F1.1 — Presentation Window & Slide Renderer"
  if [ ! -f "src-tauri/src/slide.rs" ]; then
    skip "slide.rs not created — F1.1 not started yet"
    return
  fi

  file_contains "SlidePayload struct" "src-tauri/src/slide.rs" "SlidePayload"
  file_contains "background_path Option<String>" "src-tauri/src/slide.rs" "background_path"
  file_contains "SlideUpdate variant" "src-tauri/src/events.rs" "SlideUpdate"
  file_contains "presentation_show command" "src-tauri/src/commands/presentation.rs" "presentation_show"
  file_contains "presentation_hide command" "src-tauri/src/commands/presentation.rs" "presentation_hide"
  file_contains "visible(false) in app.rs" "src-tauri/src/app.rs" "visible"
  file_not_contains "No auto-spawn on setup" "src-tauri/src/app.rs" "presentation.*index.html.*spawn"

  file_contains "SlideCanvas component" "presentation/src/components/SlideCanvas.tsx" "SlideCanvas"
  file_contains "useSlideStore" "presentation/src/store/slideStore.ts" "useSlideStore"
  file_contains "scale" "presentation/src/components/SlideCanvas.tsx" "scale"
  file_contains "PresentButton in controller" "controller/src/components/PresentButton.tsx" "PresentButton"
  file_contains "zustand in presentation" "presentation/package.json" "zustand"
  file_contains "zustand in controller" "controller/package.json" "zustand"

  subsection "Presentation always fullscreen, PowerPoint-style (regression check — see CLAUDE.md)"
  file_contains "presentation_show goes fullscreen unconditionally" "src-tauri/src/commands/presentation.rs" "set_fullscreen(true)"
  file_not_contains "no single-display windowed fallback branch" "src-tauri/src/commands/presentation.rs" "single_display"
  file_contains "always-on-top gated on genuine multi-display (Alt+Tab must work on mirrored/single-display rigs)" "src-tauri/src/commands/presentation.rs" "multi_display"
  file_contains "Escape registration failure is non-fatal" "src-tauri/src/commands/presentation.rs" "non-fatal"
  file_contains "presentation window close is intercepted, not destroyed" "src-tauri/src/app.rs" "prevent_close"

  subsection "Multi-window dev/build wiring (regression check — see CLAUDE.md)"
  file_contains "presentation window uses dev server URL in debug builds" "src-tauri/src/app.rs" "localhost:5174"
  file_contains "presentation dev-server IPC is granted at runtime, debug builds only" "src-tauri/src/app.rs" "presentation-dev-remote"
  file_contains "beforeDevCommand runs both dev servers" "src-tauri/tauri.conf.json" "dev-both.cjs"
  file_contains "beforeBuildCommand merges presentation dist" "src-tauri/tauri.conf.json" "merge-presentation-dist.cjs"
  file_contains "presentation build uses relative asset base" "presentation/vite.config.ts" 'base: "./"'
  file_exists "scripts/dev-both.cjs"
  file_exists "scripts/merge-presentation-dist.cjs"

  check "F1.1 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide 2>&1 | tee /tmp/sd-test.log \
    && ! grep -q "FAILED" /tmp/sd-test.log'
}

run_f1_2() {
  section "F1.2 — Slide State Machine"
  if [ ! -f "src-tauri/src/slide_state_machine.rs" ]; then
    skip "slide_state_machine.rs not created — F1.2 not started yet"
    return
  fi
  
  file_contains "advance_from_last test"          "src-tauri/src/slide_state_machine.rs" "advance_from_last_emits_deck_end"
  file_contains "rewind_from_first test"          "src-tauri/src/slide_state_machine.rs" "rewind_from_first_emits_deck_start"
  file_contains "jump_to_valid_index test"        "src-tauri/src/slide_state_machine.rs" "jump_to_valid_index_publishes_slide_update"
  file_contains "jump_to_invalid_index test"      "src-tauri/src/slide_state_machine.rs" "jump_to_invalid_index_returns_error"
  file_contains "persist_then_restore test"       "src-tauri/src/slide_state_machine.rs" "advance_then_persist_then_restore_preserves_index"
  file_contains "stub_deck_has_five_slides test"  "src-tauri/src/slide.rs"               "stub_deck_has_five_slides"
  file_contains "from_entry_correct_payload test" "src-tauri/src/slide.rs"               "from_entry_produces_correct_payload"

  check "F1.2 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide_state_machine 2>&1 | tee /tmp/sd-test.log \
    && ! grep -q "FAILED" /tmp/sd-test.log'
}

# ── ADD this function before the dispatch block ──────────────────────────────

run_f1_3() {
  section "F1.3 — Controller Slide Panel"
  if [ ! -f "controller/src/components/SlideDeckPanel.tsx" ]; then
    skip "SlideDeckPanel.tsx not created — F1.3 not started yet"
    return
  fi

  # Rust checks
  file_contains "TimerTick variant"       "src-tauri/src/events.rs"                      "TimerTick"
  file_contains "TimerReset variant"      "src-tauri/src/events.rs"                      "TimerReset"
  file_contains "timer_started_at"        "src-tauri/src/slide_state_machine.rs"         "timer_started_at"
  file_contains "spawn_timer_task"        "src-tauri/src/slide_state_machine.rs"         "spawn_timer_task"
  file_contains "slide_timer_reset cmd"   "src-tauri/src/commands/slide.rs"              "slide_timer_reset"
  file_contains "stub notes non-empty"    "src-tauri/src/slide.rs"                       "Welcome the audience"

  # Frontend checks
  file_contains "SlideCanvas"             "controller/src/components/SlideCanvas.tsx"    "SlideCanvas"
  file_contains "SlideDeckPanel"          "controller/src/components/SlideDeckPanel.tsx" "SlideDeckPanel"
  file_contains "TimerTick events.ts"     "controller/src/lib/events.ts"                 "TimerTick"
  file_contains "isCollapsed store"       "controller/src/store/slideStore.ts"           "isCollapsed"
  file_contains "keydown listener"        "controller/src/components/SlideDeckPanel.tsx" "keydown"
  file_contains "slideTimerReset ipc"     "controller/src/lib/ipc.ts"                    "slideTimerReset"
  file_not_exists "SlidePanel deleted"    "controller/src/components/SlidePanel.tsx"

  # Unit test presence checks
  file_contains "timer_starts_on_first_advance_only test"  "src-tauri/src/slide_state_machine.rs" "timer_starts_on_first_advance_only"
  file_contains "timer_does_not_reset_on_rewind test"      "src-tauri/src/slide_state_machine.rs" "timer_does_not_reset_on_rewind"
  file_contains "timer_reset_command test"                  "src-tauri/src/slide_state_machine.rs" "timer_reset_command_sets_new_start_time"
  file_contains "timer_persists_restores test"              "src-tauri/src/slide_state_machine.rs" "timer_persists_and_restores_correctly"
  file_contains "stub_notes_non_empty test"                 "src-tauri/src/slide.rs"               "stub_deck_notes_are_non_empty"

  check "F1.3 unit tests pass" \
    bash -c 'cd src-tauri && cargo test slide 2>&1 | tee /tmp/sd-f13-test.log \
    && ! grep -q "FAILED" /tmp/sd-f13-test.log'
}

run_f1_4() {
  section "F1.4 — Slide Deck Import"
  if [ ! -f "src-tauri/src/deck_importer.rs" ]; then
    skip "deck_importer.rs not created — F1.4 not started yet"
    return
  fi

  file_contains "DeckImportProgress variant"  "src-tauri/src/events.rs"                       "DeckImportProgress"
  file_contains "DeckImportComplete variant"  "src-tauri/src/events.rs"                       "DeckImportComplete"
  file_contains "SceneMediaUpdate variant"    "src-tauri/src/events.rs"                       "SceneMediaUpdate"
  file_contains "SceneSourceChanged variant"  "src-tauri/src/events.rs"                       "SceneSourceChanged"
  file_contains "SlideTextBlock type"         "src-tauri/src/slide.rs"                        "SlideTextBlock"
  file_contains "SlideTextRun type"           "src-tauri/src/slide.rs"                        "SlideTextRun"
  file_contains "word_count field"            "src-tauri/src/slide.rs"                        "word_count"
  file_contains "is_code_slide field"         "src-tauri/src/slide.rs"                        "is_code_slide"
  file_contains "deck_import cmd"             "src-tauri/src/commands/deck.rs"                "deck_import"
  file_contains "deck_restore cmd"            "src-tauri/src/commands/deck.rs"                "deck_restore"
  file_contains "deck_clear cmd (empty-deck support)" "src-tauri/src/commands/deck.rs"        "deck_clear"
  file_contains "scene_push_image cmd"        "src-tauri/src/commands/scene.rs"               "scene_push_image"
  file_contains "scene_set_source cmd"        "src-tauri/src/commands/scene.rs"               "scene_set_source"
  file_contains "spawn_blocking"              "src-tauri/src/deck_importer.rs"                "spawn_blocking"
  file_contains "tauri-plugin-dialog"         "src-tauri/Cargo.toml"                          "tauri-plugin-dialog"
  file_contains "pdfium-render"               "src-tauri/Cargo.toml"                          "pdfium-render"
  file_contains "quick-xml"                   "src-tauri/Cargo.toml"                          "quick-xml"

  file_contains "DeckImportProgress events.ts"  "controller/src/lib/events.ts"                "DeckImportProgress"
  file_contains "SceneMediaUpdate events.ts"    "controller/src/lib/events.ts"                "SceneMediaUpdate"
  file_contains "deckImport ipc"                "controller/src/lib/ipc.ts"                   "deckImport"
  file_contains "pickDeckFile ipc"              "controller/src/lib/ipc.ts"                   "pickDeckFile"
  file_contains "sceneStore"                    "controller/src/store/sceneStore.ts"           "useSceneStore"
  file_contains "SceneMediaPanel"               "controller/src/components/SceneMediaPanel.tsx" "SceneMediaPanel"
  file_contains "SceneMediaView"                "presentation/src/components/SceneMediaView.tsx" "SceneMediaView"
  file_contains "DeckImportProgress pres"       "presentation/src/lib/events.ts"              "DeckImportProgress"

  file_contains "pptx_extracts_title test"   "src-tauri/src/deck_importer.rs" "pptx_extracts_title_body_notes"
  file_contains "is_code_slide test"         "src-tauri/src/deck_importer.rs" "is_code_slide_true_for_monospace"
  file_contains "load_deck_resets test"      "src-tauri/src/slide_state_machine.rs" "load_deck_resets_index_and_timer"
  file_contains "round_trip_json test"       "src-tauri/src/slide.rs"         "deck_save_and_load_json_round_trips"

  check "F1.4 unit tests pass" \
    bash -c 'cd src-tauri && cargo test 2>&1 | tee /tmp/sd-f14-test.log \
    && ! grep -q "FAILED" /tmp/sd-f14-test.log'
}

run_f1_5() {
  section "F1.5 — Multi-Display Assignment"
  if [ ! -f "src-tauri/src/display.rs" ]; then
    skip "display.rs not created — F1.5 not started yet"
    return
  fi

  file_contains "DisplayInfo struct"        "src-tauri/src/display.rs"              "pub struct DisplayInfo"
  file_contains "list_displays fn"          "src-tauri/src/display.rs"              "pub fn list_displays"
  file_contains "displays_changed fn"       "src-tauri/src/display.rs"              "pub fn displays_changed"
  file_contains "is_assigned_display_lost"  "src-tauri/src/display.rs"              "pub fn is_assigned_display_lost"
  file_contains "pick_fallback fn"          "src-tauri/src/display.rs"              "pub fn pick_fallback"
  file_contains "spawn_watch_task fn"       "src-tauri/src/display.rs"              "pub fn spawn_watch_task"
  file_contains "DisplaysChanged variant"   "src-tauri/src/events.rs"               "DisplaysChanged"
  file_contains "DisplayLost variant"       "src-tauri/src/events.rs"               "DisplayLost"
  file_contains "display_list cmd"          "src-tauri/src/commands/display.rs"     "display_list"
  file_contains "display_assign cmd"        "src-tauri/src/commands/display.rs"     "display_assign_presentation"
  file_contains "display_id in config"      "src-tauri/src/config.rs"               "display_id"
  file_contains "ConfigLoader::save"        "src-tauri/src/config_loader.rs"        "pub fn save"

  file_contains "DisplayPanel component"    "controller/src/components/DisplayPanel.tsx" "DisplayPanel"
  file_contains "displayStore"              "controller/src/store/displayStore.ts"  "useDisplayStore"
  file_contains "displayList ipc"           "controller/src/lib/ipc.ts"             "displayList"
  file_contains "displayAssignPresentation ipc" "controller/src/lib/ipc.ts"         "displayAssignPresentation"
  file_contains "DisplaysChanged events.ts" "controller/src/lib/events.ts"          "DisplaysChanged"

  check "F1.5 unit tests pass" \
    bash -c 'cd src-tauri && cargo test display 2>&1 | tee /tmp/sd-f15-test.log \
    && ! grep -q "FAILED" /tmp/sd-f15-test.log'
}

run_f2_1() {
  section "F2.1 — GStreamer Pipeline Service"

  file_exists "src-tauri/src/pipeline.rs"
  file_exists "src-tauri/src/commands/pipeline.rs"
  file_exists "src-tauri/tests/pipeline_journeys.rs"
  file_contains "PipelineService actor handle"   "src-tauri/src/pipeline.rs" "pub struct PipelineService"
  file_contains "PipelineBackend seam"           "src-tauri/src/pipeline.rs" "pub trait PipelineBackend"
  file_contains "actor runs in spawn_blocking"   "src-tauri/src/pipeline.rs" "spawn_blocking"
  file_contains "GStreamer backend behind a cargo feature" "src-tauri/src/pipeline.rs" 'cfg(feature = "gstreamer")'
  file_contains "gstreamer optional dependency"  "src-tauri/Cargo.toml" 'gstreamer.*optional = true'
  file_contains "bus watcher forwards Qos"       "src-tauri/src/pipeline.rs" "MessageView::Qos"
  file_contains "PipelineError event"            "src-tauri/src/events.rs" "PipelineError"
  file_contains "PipelineStateChanged event"     "src-tauri/src/events.rs" "PipelineStateChanged"
  file_contains "pipeline commands registered"   "src-tauri/src/state.rs" "pipeline_start_test"
  file_contains "shutdown to NULL on app exit"   "src-tauri/src/app.rs" "shutdown_blocking"
  file_contains "PipelineError in controller events.ts" "controller/src/lib/events.ts" "PipelineError"
  file_contains "pipelineStartTest ipc"          "controller/src/lib/ipc.ts" "pipelineStartTest"
  file_contains "pipelineStore"                  "controller/src/store/pipelineStore.ts" "usePipelineStore"
  file_contains "PipelinePanel component"        "controller/src/components/PipelinePanel.tsx" "PipelinePanel"

  check "F2.1 unit + harness tests pass without GStreamer" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --lib pipeline 2>&1 | tee /tmp/sd-f21-lib.log \
      && cargo test --test pipeline_journeys 2>&1 | tee /tmp/sd-f21-j.log \
      && ! grep -q "FAILED" /tmp/sd-f21-lib.log /tmp/sd-f21-j.log \
      && grep -q "test result: ok. 3 passed" /tmp/sd-f21-j.log'

  check "PipelinePanel + store tests and controller typecheck" \
    bash -c 'cd controller && npx vitest run src/components/PipelinePanel.test.tsx src/store/pipelineStore.test.ts src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'
  check "presentation exhaustive-event test + typecheck" \
    bash -c 'cd presentation && npx vitest run src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'

  # Real GStreamer tests need the SDK; CI machines without it skip them.
  if [ -n "${GSTREAMER_1_0_ROOT_MSVC_X86_64:-}" ]; then
    export PKG_CONFIG_PATH="${GSTREAMER_1_0_ROOT_MSVC_X86_64}/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    export PATH="${GSTREAMER_1_0_ROOT_MSVC_X86_64}/bin:$PATH"
  fi
  if pkg-config --exists gstreamer-1.0 2>/dev/null; then
    check "real GStreamer: 100x start/pause/resume/stop, error surfacing, shutdown" \
      bash -c 'set -o pipefail; cd src-tauri && STREAMDECK_PIPELINE_LOOPS=100 cargo test --features gstreamer --lib gst_tests -- --test-threads=1 2>&1 | tee /tmp/sd-f21-gst.log \
        && ! grep -q "FAILED" /tmp/sd-f21-gst.log && grep -q "test result: ok. 5 passed" /tmp/sd-f21-gst.log'
    check "clippy with --features gstreamer" \
      bash -c 'cd src-tauri && cargo clippy --features gstreamer -- -D warnings'
  else
    warn "GStreamer SDK not found (pkg-config gstreamer-1.0) — real-pipeline tests skipped"
  fi
}

run_f2_2() {
  section "F2.2 — NDI Source Discovery & Ingest"

  file_exists "src-tauri/src/ndi.rs"
  file_exists "src-tauri/src/commands/ndi.rs"
  file_exists "src-tauri/tests/ndi_journeys.rs"
  file_contains "NdiDiscovery trait seam"          "src-tauri/src/ndi.rs" "pub trait NdiDiscovery"
  file_contains "SimulatedDiscovery for tests"     "src-tauri/src/ndi.rs" "pub struct SimulatedDiscovery"
  file_contains "pure stream-format selection"     "src-tauri/src/ndi.rs" "pub fn select_stream_format"
  file_contains "pure source diff"                 "src-tauri/src/ndi.rs" "pub fn diff_sources"
  file_contains "2s discovery poll"                "src-tauri/src/ndi.rs" "from_secs(2)"
  file_contains "poll spawned via tauri runtime"   "src-tauri/src/ndi.rs" "tauri::async_runtime::spawn"
  file_contains "ndi cargo feature"                "src-tauri/Cargo.toml" "^ndi "
  file_contains "production discovery behind feature" "src-tauri/src/ndi.rs" 'cfg(feature = "ndi")'
  file_contains "backend add_ndi_source"           "src-tauri/src/pipeline.rs" "fn add_ndi_source"
  file_contains "gst ndisrc segment"               "src-tauri/src/ndi.rs" "ndisrc ndi-name"
  file_contains "SourceLost event"                 "src-tauri/src/events.rs" "SourceLost"
  file_contains "SourceListChanged event"          "src-tauri/src/events.rs" "SourceListChanged"
  file_contains "ndi commands registered"          "src-tauri/src/state.rs" "ndi_add_source"
  file_contains "ndi watch task wired in app.rs"   "src-tauri/src/app.rs" "ndi::spawn_watch_task"
  file_contains "SourceLost in controller events.ts" "controller/src/lib/events.ts" "SourceLost"
  file_contains "ndiAddSource ipc"                 "controller/src/lib/ipc.ts" "ndiAddSource"
  file_contains "ndiStore"                         "controller/src/store/ndiStore.ts" "useNdiStore"
  file_contains "Source Lost placeholder"          "controller/src/components/NdiSourcesPanel.tsx" "Source Lost"
  file_contains "SourceLost NOT_MODELLED in presentation" "presentation/src/lib/events.exhaustive.test.ts" "SourceLost"

  check "F2.2 unit + harness tests pass without NDI or GStreamer"     bash -c 'set -o pipefail; cd src-tauri && cargo test --lib ndi 2>&1 | tee /tmp/sd-f22-lib.log       && cargo test --lib pipeline 2>&1 | tee /tmp/sd-f22-pl.log       && cargo test --test ndi_journeys 2>&1 | tee /tmp/sd-f22-j.log       && cargo test --test invariants 2>&1 | tee /tmp/sd-f22-inv.log       && ! grep -q "FAILED" /tmp/sd-f22-lib.log /tmp/sd-f22-pl.log /tmp/sd-f22-j.log /tmp/sd-f22-inv.log       && grep -q "test result: ok. 6 passed" /tmp/sd-f22-j.log'

  check "NdiSourcesPanel + store tests and controller typecheck"     bash -c 'cd controller && npx vitest run src/components/NdiSourcesPanel.test.tsx src/store/ndiStore.test.ts src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'
  check "presentation exhaustive-event test + typecheck"     bash -c 'cd presentation && npx vitest run src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'

  # The ndi feature (gst device provider) only compiles where the GStreamer SDK is present.
  if pkg-config --exists gstreamer-1.0 2>/dev/null; then
    check "clippy with --features ndi"       bash -c 'cd src-tauri && cargo clippy --features ndi -- -D warnings'
  else
    warn "GStreamer SDK not found — ndi feature compile check skipped (real NDI needs the NDI SDK + gst-plugin-ndi)"
  fi
}

run_f2_3() {
  section "F2.3 — Camera Preview Grid"

  file_exists "src-tauri/src/preview.rs"
  file_exists "src-tauri/src/commands/preview.rs"
  file_exists "src-tauri/tests/preview_journeys.rs"
  file_exists "controller/src/components/CameraPreview.tsx"
  file_exists "controller/src/components/CameraPreviewGrid.tsx"
  file_contains "pure per-source frame throttle"     "src-tauri/src/preview.rs" "pub struct FrameThrottle"
  file_contains "8 fps preview cap"                  "src-tauri/src/preview.rs" "PREVIEW_FPS: u32 = 8"
  file_contains "320x180 thumbnails"                 "src-tauri/src/preview.rs" "PREVIEW_WIDTH: u32 = 320"
  file_contains "PNG encode off the actor thread"    "src-tauri/src/preview.rs" "spawn_blocking"
  file_contains "simulated test-pattern feeds"       "src-tauri/src/preview.rs" "pub fn test_pattern"
  file_contains "simulated backend produces frames"  "src-tauri/src/pipeline.rs" "pub fn with_preview"
  file_contains "gst appsink frame sender"           "src-tauri/src/pipeline.rs" "AppSinkCallbacks"
  file_contains "scale to 320x180 in the pipeline"   "src-tauri/src/ndi.rs" "width=320,height=180"
  file_contains "SourceStats event"                  "src-tauri/src/events.rs" "SourceStats"
  file_contains "preview commands registered"        "src-tauri/src/state.rs" "preview_set_layout"
  file_contains "stats task wired in app.rs"         "src-tauri/src/app.rs" "preview::spawn_stats_task"
  file_contains "frame bridge targets the controller" "src-tauri/src/app.rs" 'emit_to("controller", "preview-frame"'
  file_contains "layout persisted in config"         "src-tauri/src/config.rs" "preview_grid"
  file_contains "SourceStats in controller events.ts" "controller/src/lib/events.ts" "SourceStats"
  file_contains "onPreviewFrame listener"            "controller/src/lib/events.ts" "onPreviewFrame"
  file_contains "previewSetLayout ipc"               "controller/src/lib/ipc.ts" "previewSetLayout"
  file_contains "previewStore"                       "controller/src/store/previewStore.ts" "usePreviewStore"
  file_contains "Source Lost placeholder"            "controller/src/components/CameraPreview.tsx" "Source Lost"
  file_contains "sleep/wake resubscribe"             "controller/src/components/CameraPreviewGrid.tsx" "visibilitychange"
  file_contains "SourceStats NOT_MODELLED in presentation" "presentation/src/lib/events.exhaustive.test.ts" "SourceStats"

  check "F2.3 unit + harness tests pass without NDI or GStreamer" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --lib preview 2>&1 | tee /tmp/sd-f23-lib.log \
      && cargo test --lib pipeline 2>&1 | tee /tmp/sd-f23-pl.log \
      && cargo test --test preview_journeys 2>&1 | tee /tmp/sd-f23-j.log \
      && cargo test --test ndi_journeys 2>&1 | tee /tmp/sd-f23-ndi.log \
      && cargo test --test invariants 2>&1 | tee /tmp/sd-f23-inv.log \
      && ! grep -q "FAILED" /tmp/sd-f23-lib.log /tmp/sd-f23-pl.log /tmp/sd-f23-j.log /tmp/sd-f23-ndi.log /tmp/sd-f23-inv.log \
      && grep -q "test result: ok. 6 passed" /tmp/sd-f23-j.log'

  check "CameraPreviewGrid + store tests and controller typecheck" \
    bash -c 'cd controller && npx vitest run src/components/CameraPreviewGrid.test.tsx src/store/previewStore.test.ts src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'
  check "presentation exhaustive-event test + typecheck" \
    bash -c 'cd presentation && npx vitest run src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'

  if pkg-config --exists gstreamer-1.0 2>/dev/null; then
    check "clippy with --features gstreamer (appsink frame sender compiles)" \
      bash -c 'cd src-tauri && cargo clippy --features gstreamer --all-targets -- -D warnings'
  else
    warn "GStreamer SDK not found — appsink frame sender compile check skipped"
  fi
}

run_f2_4() {
  section "F2.4 — Phone Camera Ingest (WHIP Server)"

  file_exists "src-tauri/src/whip.rs"
  file_exists "src-tauri/src/whip_server.rs"
  file_exists "src-tauri/src/commands/whip.rs"
  file_exists "src-tauri/tests/whip_journeys.rs"
  file_exists "src-tauri/tests/whip_gst_e2e.rs"
  file_exists "controller/src/components/WhipPanel.tsx"
  file_exists "controller/src/components/QrCode.tsx"
  file_exists "controller/src/store/whipStore.ts"
  file_contains "axum 0.8 dependency"                 "src-tauri/Cargo.toml" 'axum = { version = "0.8"'
  file_contains "optional whip cargo feature"         "src-tauri/Cargo.toml" 'whip  *= '
  file_contains "native keyring stores enabled"       "src-tauri/Cargo.toml" 'windows-native'
  file_contains "POST /whip/{slot} route"             "src-tauri/src/whip_server.rs" '"/whip/{slot}"'
  file_contains "DELETE + PATCH resource route"       "src-tauri/src/whip_server.rs" '"/whip/{slot}/{session}"'
  file_contains "pure codec selection"                "src-tauri/src/whip.rs" "pub fn select_codec"
  file_contains "pure ICE link state machine"         "src-tauri/src/whip.rs" "pub struct LinkMachine"
  file_contains "10s ICE grace period"                "src-tauri/src/whip.rs" "GRACE_PERIOD: Duration = Duration::from_secs(10)"
  file_contains "injected local-IP provider"          "src-tauri/src/whip.rs" "pub trait LocalIpProvider"
  file_contains "token lives in the OS keychain"      "src-tauri/src/whip.rs" "pub struct KeyringTokenStore"
  file_contains "whip backend seam"                   "src-tauri/src/whip.rs" "pub trait WhipBackend"
  file_contains "real whipserversrc backend gated"    "src-tauri/src/whip.rs" 'cfg(feature = "whip")'
  file_contains "[whip] config section"               "src-tauri/src/config.rs" "pub struct WhipConfig"
  file_contains "whip disabled by default"            "src-tauri/src/config.rs" "enabled: false"
  file_contains "whip events"                         "src-tauri/src/events.rs" "WhipSessionChanged"
  file_contains "whip commands registered"            "src-tauri/src/state.rs" "whip_set_enabled"
  file_contains "whip tick task wired in app.rs"      "src-tauri/src/app.rs" "whip::spawn_tick_task"
  file_contains "whipGetInfo ipc"                     "controller/src/lib/ipc.ts" "whipGetInfo"
  file_contains "WhipSessionChanged in controller events.ts" "controller/src/lib/events.ts" "WhipSessionChanged"
  file_contains "WhipPanel mounted in Settings"       "controller/src/App.tsx" "<WhipPanel />"
  file_contains "WhipSessionChanged NOT_MODELLED in presentation" "presentation/src/lib/events.exhaustive.test.ts" "WhipSessionChanged"
  if grep -qiE "token|secret|password" src-tauri/src/config.rs; then
    fail "config.rs must not contain secret-like fields (token lives in the keychain)"
  else
    pass "no secret-like fields in config.rs"
  fi

  check "F2.4 unit tests (pure logic + real HTTP against the axum server)"     bash -c 'set -o pipefail; cd src-tauri && cargo test --lib whip 2>&1 | tee /tmp/sd-f24-lib.log       && cargo test --lib config 2>&1 | tee /tmp/sd-f24-cfg.log       && ! grep -q "FAILED" /tmp/sd-f24-lib.log /tmp/sd-f24-cfg.log       && grep -q "test result: ok. 3[0-9] passed" /tmp/sd-f24-lib.log'

  check "F2.4 journeys over real IPC + HTTP, preview reuse, invariants"     bash -c 'set -o pipefail; cd src-tauri && cargo test --test whip_journeys 2>&1 | tee /tmp/sd-f24-j.log       && cargo test --test preview_journeys 2>&1 | tee /tmp/sd-f24-pj.log       && cargo test --test ndi_journeys 2>&1 | tee /tmp/sd-f24-nj.log       && cargo test --test invariants 2>&1 | tee /tmp/sd-f24-inv.log       && ! grep -q "FAILED" /tmp/sd-f24-j.log /tmp/sd-f24-pj.log /tmp/sd-f24-nj.log /tmp/sd-f24-inv.log       && grep -q "test result: ok. 8 passed" /tmp/sd-f24-j.log'

  check "WhipPanel + store + grid tests and controller typecheck"     bash -c 'cd controller && npx vitest run src/components/WhipPanel.test.tsx src/store/whipStore.test.ts src/components/CameraPreviewGrid.test.tsx src/components/NdiSourcesPanel.test.tsx src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'
  check "presentation exhaustive-event test + typecheck"     bash -c 'cd presentation && npx vitest run src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'

  # The real whipserversrc path compiles wherever GStreamer is present; it is only exercised end to end if the plugin is installed.
  if pkg-config --exists gstreamer-1.0 2>/dev/null; then
    check "clippy with --features whip"       bash -c 'cd src-tauri && cargo clippy --features whip --all-targets -- -D warnings'
    if gst-inspect-1.0 whipserversrc >/dev/null 2>&1 && gst-inspect-1.0 whipclientsink >/dev/null 2>&1 && gst-inspect-1.0 openh264enc >/dev/null 2>&1; then
      check "real WebRTC loopback: whipclientsink -> endpoint -> whipserversrc -> preview"         bash -c 'cd src-tauri && cargo test --features whip --test whip_gst_e2e -- --ignored 2>&1 | grep -q "test result: ok. 1 passed"'
    else
      warn "gst-plugin-webrtc/openh264 not found — real WHIP loopback test skipped"
    fi
  else
    warn "GStreamer SDK not found — whipserversrc backend compile check and loopback test skipped"
  fi
}

run_f3_1() {
  section "F3.1 — GStreamer Compositor Pipeline"

  file_exists "src-tauri/src/compositor.rs"
  file_exists "src-tauri/src/commands/compositor.rs"
  file_exists "src-tauri/tests/compositor_journeys.rs"
  file_exists "controller/src/components/CompositorPanel.tsx"
  file_exists "controller/src/store/compositorStore.ts"
  file_contains "pure compositor element selection"  "src-tauri/src/compositor.rs" "pub fn select_compositor"
  file_contains "d3d12compositor on Windows"         "src-tauri/src/compositor.rs" '"d3d12compositor"'
  file_contains "glvideomixer on macOS/Linux"        "src-tauri/src/compositor.rs" '"glvideomixer"'
  file_contains "CPU compositor fallback"            "src-tauri/src/compositor.rs" 'element: "compositor"'
  file_contains "geometry to sink-pad properties"    "src-tauri/src/compositor.rs" "pub fn pad_properties"
  file_contains "layout validation"                  "src-tauri/src/compositor.rs" "pub fn validate_layout"
  file_contains "1920x1080@30 output caps"           "src-tauri/src/compositor.rs" "video/x-raw,width=1920,height=1080,framerate=30/1"
  file_contains "GStreamer compositor behind a cargo feature" "src-tauri/src/compositor.rs" 'cfg(feature = "gstreamer")'
  file_contains "pad block before unlink"            "src-tauri/src/compositor.rs" "BLOCK_DOWNSTREAM"
  file_contains "backend compositor seam"            "src-tauri/src/pipeline.rs" "fn compositor_set_layout"
  file_contains "[scene.X.sources] config"           "src-tauri/src/config.rs" "pub scene: BTreeMap"
  file_contains "CompositorStateChanged event"       "src-tauri/src/events.rs" "CompositorStateChanged"
  file_contains "CompositorLayoutChanged event"      "src-tauri/src/events.rs" "CompositorLayoutChanged"
  file_contains "compositor commands registered"     "src-tauri/src/state.rs" "compositor_set_layout"
  file_contains "CompositorStateChanged in controller events.ts" "controller/src/lib/events.ts" "CompositorStateChanged"
  file_contains "compositorStart ipc"                "controller/src/lib/ipc.ts" "compositorStart"
  file_contains "CompositorPanel mounted in Settings" "controller/src/App.tsx" "<CompositorPanel />"
  file_contains "CompositorStateChanged NOT_MODELLED in presentation" "presentation/src/lib/events.exhaustive.test.ts" "CompositorStateChanged"

  check "F3.1 pure + actor unit tests pass without GStreamer" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --lib compositor 2>&1 | tee /tmp/sd-f31-lib.log \
      && cargo test --lib config 2>&1 | tee /tmp/sd-f31-cfg.log \
      && ! grep -q "FAILED" /tmp/sd-f31-lib.log /tmp/sd-f31-cfg.log \
      && grep -qE "test result: ok. (1[5-9]|[2-9][0-9]) passed" /tmp/sd-f31-lib.log'

  check "F3.1 journeys over real IPC + invariants" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --test compositor_journeys 2>&1 | tee /tmp/sd-f31-j.log \
      && cargo test --test pipeline_journeys 2>&1 | tee /tmp/sd-f31-pj.log \
      && cargo test --test invariants 2>&1 | tee /tmp/sd-f31-inv.log \
      && ! grep -q "FAILED" /tmp/sd-f31-j.log /tmp/sd-f31-pj.log /tmp/sd-f31-inv.log \
      && grep -q "test result: ok. 4 passed" /tmp/sd-f31-j.log'

  check "CompositorPanel + store tests and controller typecheck" \
    bash -c 'cd controller && npx vitest run src/components/CompositorPanel.test.tsx src/store/compositorStore.test.ts src/lib/events.exhaustive.test.ts src/App.test.tsx >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'
  check "presentation exhaustive-event test + typecheck" \
    bash -c 'cd presentation && npx vitest run src/lib/events.exhaustive.test.ts >/dev/null 2>&1 && npm run typecheck >/dev/null 2>&1'

  if [ -n "${GSTREAMER_1_0_ROOT_MSVC_X86_64:-}" ]; then
    export PKG_CONFIG_PATH="${GSTREAMER_1_0_ROOT_MSVC_X86_64}/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    export PATH="${GSTREAMER_1_0_ROOT_MSVC_X86_64}/bin:$PATH"
  fi
  if pkg-config --exists gstreamer-1.0 2>/dev/null; then
    check "real GStreamer: 1080p30 with 3 sources, runtime add/remove, CPU fallback, GPU variant" \
      bash -c 'set -o pipefail; cd src-tauri && cargo test --features gstreamer --lib gst_compositor_tests -- --test-threads=1 --nocapture 2>&1 | tee /tmp/sd-f31-gst.log \
        && ! grep -q "FAILED" /tmp/sd-f31-gst.log && grep -q "test result: ok. 7 passed" /tmp/sd-f31-gst.log'
    if grep -q "SKIPPED: no GPU compositor" /tmp/sd-f31-gst.log 2>/dev/null; then
      warn "no GPU compositor element installed — only the CPU compositor path was exercised"
    fi
    check "clippy with --features gstreamer (compositor compiles, all targets)" \
      bash -c 'cd src-tauri && cargo clippy --features gstreamer --all-targets -- -D warnings'
  else
    warn "GStreamer SDK not found (pkg-config gstreamer-1.0) — real compositor tests skipped"
  fi
}

# ══════════════════════════════════════════════════════════════════
# CORE — Build + Lint (requires frontend dist to exist)
# ══════════════════════════════════════════════════════════════════
run_core() {
  section "Core Build & Lint"

  subsection "cargo fmt --check (always independent)"
  check "cargo fmt --check" \
    bash -c 'cd src-tauri && cargo fmt --check'

  # Check frontend dist exists before attempting cargo build/clippy
  local frontend_ready=true
  for win in controller presentation; do
    if [ ! -d "$win/dist" ]; then
      warn "$win/dist not found — run bootstrap first (or remove --no-frontend)"
      frontend_ready=false
    fi
  done

  if [ "$frontend_ready" = true ]; then
    subsection "cargo build (requires frontend dist)"
    check "cargo build --debug" \
      bash -c 'cd src-tauri && cargo build'

    subsection "cargo clippy (requires frontend dist)"
    check "cargo clippy -- -D warnings" \
      bash -c 'cd src-tauri && cargo clippy -- -D warnings'
  else
    skip "cargo build  — skipped (frontend dist missing)"
    skip "cargo clippy — skipped (frontend dist missing)"
  fi

  # cargo test is ALWAYS independent — never needs frontend dist
  subsection "cargo test (always independent)"
  check "cargo test" \
    bash -c 'cd src-tauri && cargo test 2>&1 | tee /tmp/sd-test.log \
             && ! grep -q "FAILED" /tmp/sd-test.log'

  subsection "TypeScript typecheck (always independent)"
  for win in controller presentation; do
    if grep -q '"typecheck"' "$win/package.json" 2>/dev/null; then
      check "$win: npm run typecheck" bash -c "cd $win && npm run typecheck"
    else
      skip "$win: no typecheck script"
    fi
  done

  subsection "Frontend lint — Biome (always independent)"
  for win in controller presentation; do
    if grep -q '"lint"' "$win/package.json" 2>/dev/null; then
      check "$win: npm run lint" bash -c "cd $win && npm run lint"
    else
      skip "$win: no lint script"
    fi
  done

  subsection "Frontend unit tests — Vitest (always independent)"
  for win in controller presentation; do
    if grep -q '"test"' "$win/package.json" 2>/dev/null; then
      check "$win: npm test" bash -c "cd $win && npm test"
    else
      skip "$win: no test script"
    fi
  done

  subsection "cargo-deny — advisories/licenses/bans/sources (non-blocking locally)"
  if command -v cargo-deny >/dev/null 2>&1; then
    check_warn "cargo deny check" bash -c 'cd src-tauri && cargo deny check'
  else
    skip "cargo-deny not installed locally — runs in CI via cargo-deny-action (cargo install cargo-deny to check here)"
  fi
}

# ══════════════════════════════════════════════════════════════════
# T0 — Test-framework foundations (T0.2: fmt gate)
# ══════════════════════════════════════════════════════════════════
run_t0() {
  section "T0 — Test framework foundations"

  subsection "T0.2 — rustfmt gate"
  file_contains "run_core runs cargo fmt --check" "scripts/verify.sh" "cargo fmt --check"
  check "cargo fmt --check" \
    bash -c 'cd src-tauri && cargo fmt --check'

  subsection "T0.3 — test directory contract"
  local d
  for d in src-tauri/tests src-tauri/tests/fixtures controller/src/test presentation/src/test \
           e2e e2e/specs e2e/pages e2e/fixtures; do
    dir_exists "$d"
  done
  file_contains "skill documents the layout" ".claude/skills/test-authoring/SKILL.md" "Directory contract"

  subsection "T0.4 — coverage visibility"
  local f
  for f in controller presentation; do
    file_contains "$f has test:coverage script" "$f/package.json" '"test:coverage"'
    file_contains "$f has @vitest/coverage-v8" "$f/package.json" '"@vitest/coverage-v8"'
    file_contains "$f vitest config uses v8 coverage" "$f/vitest.config.ts" 'provider: "v8"'
  done
  file_contains "coverage output is gitignored" ".gitignore" "coverage"
}

# ══════════════════════════════════════════════════════════════════
# T1 — Rust integration tests (L2, mock runtime, no webview)
# ══════════════════════════════════════════════════════════════════
run_t1_1() {
  section "T1.1 — Mock-runtime harness"

  file_exists "src-tauri/src/state.rs"
  file_exists "src-tauri/tests/common/mod.rs"
  file_exists "src-tauri/tests/harness_smoke.rs"
  file_contains "state.rs exposes manage_core" "src-tauri/src/state.rs" "pub fn manage_core"
  grep -v '^[[:space:]]*//' src-tauri/src/app.rs | grep -q "state::manage_core" \
    && pass "app.rs uses shared manage_core" || fail "app.rs must call state::manage_core (non-comment)"
  grep -v '^[[:space:]]*//' src-tauri/src/app.rs | grep -q "state::invoke_handler" \
    && pass "app.rs uses shared invoke_handler" || fail "app.rs must call state::invoke_handler (non-comment)"
  file_contains "app.rs stays gated" "src-tauri/src/lib.rs" "cfg(not(test))"
  file_contains "harness uses tauri mock runtime" "src-tauri/tests/common/mod.rs" "mock_builder"
  file_contains "harness provides collect_events" "src-tauri/tests/common/mod.rs" "fn collect_events"
  local cargo_toml="src-tauri/Cargo.toml"
  local dev_deps prod_deps
  dev_deps=$(awk '/^\[dev-dependencies\]/{f=1;next} /^\[/{f=0} f' "$cargo_toml")
  prod_deps=$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f' "$cargo_toml")
  echo "$dev_deps" | grep -q '"test"' && ! echo "$prod_deps" | grep -q '"test"'     && pass "tauri test feature is dev-only" || fail "tauri test feature must be in [dev-dependencies] only"

  local t11_log
  t11_log=$(mktemp)
  check "T1.1 harness smoke tests pass (>=6, none failed)" \
    bash -c 'cd src-tauri && cargo test --test harness_smoke 2>&1 | tee "$1" \
      && ! grep -q "FAILED" "$1" \
      && n=$(grep -o "test result: ok\. [0-9]* passed" "$1" | grep -o "[0-9]*" | head -1) \
      && [ "${n:-0}" -ge 6 ]' _ "$t11_log"
  rm -f "$t11_log"
}

run_t1_2() {
  section "T1.2 — Deck & slide command journeys"

  file_exists "src-tauri/tests/deck_journeys.rs"
  file_contains "state.rs extracts restore_deck" "src-tauri/src/state.rs" "pub async fn restore_deck"
  file_contains "state.rs extracts restore_on_startup" "src-tauri/src/state.rs" "pub async fn restore_on_startup"
  file_contains "state.rs extracts spawn_timer" "src-tauri/src/state.rs" "pub fn spawn_timer"
  file_contains "state.rs extracts manage_display" "src-tauri/src/state.rs" "pub fn manage_display"
  file_contains "mock handler registers deck_import" "src-tauri/src/state.rs" "commands::deck::deck_import"
  file_contains "mock handler registers display_list" "src-tauri/src/state.rs" "commands::display::display_list"
  grep -v '^[[:space:]]*//' src-tauri/src/app.rs | grep -q "state::restore_on_startup"     && pass "app.rs uses shared restore_on_startup" || fail "app.rs must call state::restore_on_startup"
  grep -v '^[[:space:]]*//' src-tauri/src/app.rs | grep -q "state::manage_display"     && pass "app.rs uses shared manage_display" || fail "app.rs must call state::manage_display"
  if grep -rnE 'tauri::AppHandle[,)>;]|: AppHandle[,)>;]' src-tauri/src/commands >/dev/null 2>&1; then
    fail "commands must take AppHandle<R>, not a concrete AppHandle"
  else
    pass "commands are runtime-generic"
  fi
  file_contains "app.rs stays gated" "src-tauri/src/lib.rs" "cfg(not(test))"

  local t12_log
  t12_log=$(mktemp)
  check "T1.2 deck journeys pass (>=17, none failed)"     bash -c 'set -o pipefail; cd src-tauri && cargo test --test deck_journeys 2>&1 | tee "$1"       && ! grep -q "FAILED" "$1"       && n=$(grep -o "test result: ok. [0-9]* passed" "$1" | grep -o "[0-9]*" | head -1)       && [ "${n:-0}" -ge 17 ]' _ "$t12_log"
  rm -f "$t12_log"
}

run_t1_3() {
  section "T1.3 — Scene, timer, display, config commands"

  file_exists "src-tauri/tests/scene_journeys.rs"
  file_exists "src-tauri/tests/display_journeys.rs"
  file_exists "src-tauri/tests/timer_journeys.rs"
  file_exists "src-tauri/tests/config_journeys.rs"
  file_contains "display seam: DisplayProvider trait" "src-tauri/src/display.rs" "pub trait DisplayProvider"
  file_contains "display seam: reconcile_displays is testable" "src-tauri/src/display.rs" "pub async fn reconcile_displays"
  file_contains "state.rs manages ConfigPath" "src-tauri/src/state.rs" "pub struct ConfigPath"
  file_contains "harness fakes the monitor list" "src-tauri/tests/common/mod.rs" "impl DisplayProvider for FakeDisplays"
  if grep -rnE 'resolve_path|app_config_dir|app_data_dir|available_monitors' src-tauri/src/commands >/dev/null 2>&1; then
    fail "commands must use managed ConfigPath/AppDataDir/DisplayProvider, not OS lookups"
  else
    pass "commands never resolve OS paths or monitors directly"
  fi

  local t13_log
  t13_log=$(mktemp)
  check "T1.3 journeys pass (scene>=10, display>=12, timer>=5, config>=7, none failed)" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --test scene_journeys --test display_journeys --test timer_journeys --test config_journeys 2>&1 | tee "$1" \
      && ! grep -q "FAILED" "$1" \
      && [ "$(grep -c "test result: ok" "$1")" -eq 4 ] \
      && n=$(grep -o "test result: ok\. [0-9]* passed" "$1" | grep -o "[0-9]*" | awk "{s+=\$1} END {print s}") \
      && [ "${n:-0}" -ge 34 ]' _ "$t13_log"
  rm -f "$t13_log"
}

run_t1_4() {
  section "T1.4 — Architecture invariants as tests"

  file_exists "src-tauri/tests/invariants.rs"
  file_contains "invariants test covers capabilities" "src-tauri/tests/invariants.rs" "fn presentation_capability_is_listen_only"
  file_contains "invariants test covers event union parity" "src-tauri/tests/invariants.rs" "fn controller_event_union_mirrors_every_rust_variant"
  file_contains "invariants test covers unwrap ban" "src-tauri/tests/invariants.rs" "fn command_handlers_never_unwrap_or_expect"
  file_contains "invariants test covers every capability file" "src-tauri/tests/invariants.rs" "fn no_capability_grants_presentation_more_than_listen_and_unlisten"
  file_contains "invariants test covers emit allow-list" "src-tauri/tests/invariants.rs" "fn emit_is_only_called_from_the_app_event_bridge"

  local t14_log
  t14_log=$(mktemp)
  check "T1.4 invariant tests pass (>=28, none failed)"     bash -c 'set -o pipefail; cd src-tauri && cargo test --test invariants 2>&1 | tee "$1"       && ! grep -q "FAILED" "$1"       && n=$(grep -o "test result: ok. [0-9]* passed" "$1" | grep -o "[0-9]*" | head -1)       && [ "${n:-0}" -ge 28 ]' _ "$t14_log"
  rm -f "$t14_log"
}

run_t1() {
  run_t1_1
  run_t1_2
  run_t1_3
  run_t1_4
}

# ══════════════════════════════════════════════════════════════════
# T2 — Frontend render & wiring tests (L2, JS)
# ══════════════════════════════════════════════════════════════════
run_t2_1() {
  section "T2.1 — Shared frontend test utilities"

  local f
  for f in controller presentation; do
    file_exists "$f/src/test/mockTauri.ts"
    file_exists "$f/src/test/mockTauri.test.tsx"
    file_contains "$f helper exports installMockIPC" "$f/src/test/mockTauri.ts" "export function installMockIPC"
    file_contains "$f helper exports emitAppEvent" "$f/src/test/mockTauri.ts" "export async function emitAppEvent"
    file_contains "$f helper exports renderApp" "$f/src/test/mockTauri.ts" "export function renderApp"
    local out
    out=$(mktemp)
    check "$f vitest: 0 failed, 0 skipped, helper tests >=8 passed, typecheck" \
      bash -c 'cd "$1" && npx vitest run --reporter=json --outputFile="$2" >/dev/null 2>&1; \
        node ../scripts/check-vitest-json.cjs "$2" src/test/mockTauri.test.tsx 8 && npm run typecheck >/dev/null 2>&1' _ "$f" "$out"
    rm -f "$out"
  done
}

run_t2_2() {
  section "T2.2 — Controller component tests"

  local f
  for f in src/App.test.tsx src/components/SlideDeckPanel.test.tsx src/components/PresentButton.test.tsx \
           src/components/SceneMediaPanel.test.tsx src/components/DisplayPanel.test.tsx; do
    file_exists "controller/$f"
    check "controller/$f uses user-event, not fireEvent" bash -c '! sed "s#//.*##" "controller/$1" | grep -qw fireEvent && grep -q "@testing-library/user-event" "controller/$1"' _ "$f"
    check "controller/$f asserts no unhandled IPC" grep -q "ipc.unhandled" "controller/$f"
  done
  file_exists controller/src/test/componentHelpers.ts

  # All known product-bug pins are fixed; no it.fails may remain.
  check "no it.fails product-bug pins" bash -c     '[ "$(cd controller && sed "s#//.*##" src/components/*.test.tsx src/App.test.tsx | grep -c "it.fails(")" = 0 ]'

  local out
  out=$(mktemp)
  check "controller vitest: 0 failed/skipped, component tests >= App 10, Deck 31, Present 7, Scene 10, Display 8, typecheck" \
    bash -c 'cd controller && npx vitest run --reporter=json --outputFile="$1" >/dev/null 2>&1; r=$1; \
      node ../scripts/check-vitest-json.cjs "$r" src/App.test.tsx 10 && \
      node ../scripts/check-vitest-json.cjs "$r" src/components/SlideDeckPanel.test.tsx 31 && \
      node ../scripts/check-vitest-json.cjs "$r" src/components/PresentButton.test.tsx 7 && \
      node ../scripts/check-vitest-json.cjs "$r" src/components/SceneMediaPanel.test.tsx 10 && \
      node ../scripts/check-vitest-json.cjs "$r" src/components/DisplayPanel.test.tsx 8 && npm run typecheck >/dev/null 2>&1' _ "$out"
  rm -f "$out"
}

run_t2_3() {
  section "T2.3 — Presentation app tests"

  file_exists presentation/src/App.test.tsx
  file_exists presentation/src/App.lifecycle.test.tsx
  check "presentation/App.test.tsx asserts listen-only (ipc.calls empty) and no unhandled IPC" \
    bash -c 'grep -q "expect(ipc.calls).toEqual(\[\])" presentation/src/App.test.tsx && grep -q "ipc.unhandled" presentation/src/App.test.tsx'
  check "presentation/App.test.tsx covers every non-rendered AppEvent via a typed ignored-events table" \
    grep -q "IgnoredType = Exclude<AppEvent" presentation/src/App.test.tsx
  check "presentation/App.test.tsx has no CSS-class queries" bash -c '! grep -Eq "querySelector|getByClassName" presentation/src/App.test.tsx'

  local out
  out=$(mktemp)
  check "presentation vitest: 0 failed/skipped, App.test >= 27, lifecycle >= 1, typecheck" \
    bash -c 'cd presentation && npx vitest run --reporter=json --outputFile="$1" >/dev/null 2>&1; \
      node ../scripts/check-vitest-json.cjs "$1" src/App.test.tsx 27 && \
      node ../scripts/check-vitest-json.cjs "$1" src/App.lifecycle.test.tsx 1 && npm run typecheck >/dev/null 2>&1' _ "$out"
  rm -f "$out"
}

run_t2_4() {
  section "T2.4 — Event-union exhaustiveness"

  local f
  for f in controller presentation; do
    file_exists "$f/src/lib/events.exhaustive.test.ts"
    file_contains "$f has typed TS_VARIANTS table" "$f/src/lib/events.exhaustive.test.ts" "const TS_VARIANTS: Record<AppEvent"
    file_contains "$f reads events.rs" "$f/src/lib/events.exhaustive.test.ts" "src-tauri/src/events.rs?raw"
    local out
    out=$(mktemp)
    check "$f vitest: 0 failed/skipped, exhaustiveness tests >=7 passed, typecheck"       bash -c 'cd "$1" && npx vitest run --reporter=json --outputFile="$2" >/dev/null 2>&1;         node ../scripts/check-vitest-json.cjs "$2" src/lib/events.exhaustive.test.ts 7 && npm run typecheck >/dev/null 2>&1' _ "$f" "$out"
    rm -f "$out"
  done
}

run_t2() {
  run_t2_1
  run_t2_2
  run_t2_3
  run_t2_4
}

# ══════════════════════════════════════════════════════════════════
# RELEASE — step 1: PDFium bundling + bundle-contents assertion
# (no real `cargo tauri build` here; it is too slow — run it manually, then scripts/check-bundle.sh)
# ══════════════════════════════════════════════════════════════════
run_release_pdfium() {
  section "Release — PDFium bundling + bundle check"

  file_exists "release/deps.json"
  file_exists "release/bundle-budget.json"
  file_exists "release/bundle.pdfium.conf.json"
  file_exists "scripts/fetch-deps.sh"
  file_exists "scripts/check-bundle.sh"
  file_contains "deps.json pins windows-x64 sha256"   "release/deps.json" '"windows-x64"'
  file_contains "deps.json pins linux-x64 sha256"     "release/deps.json" '"linux-x64"'
  file_contains "deps.json only uses bblanchon releases" "release/deps.json" 'github.com/bblanchon/pdfium-binaries/releases/download/'
  file_contains "fetch-deps verifies SHA-256"         "scripts/fetch-deps.sh" 'SHA-256 mismatch'
  file_contains "staging dir is gitignored"           ".gitignore" 'src-tauri/resources/'
  file_contains "bundle overlay maps pdfium resources" "release/bundle.pdfium.conf.json" 'resources/pdfium/\*'
  # Missing resource globs fail tauri-build, so resources must live only in the overlay passed to `tauri build --config`.
  file_not_contains "tauri.conf.json has no bundle.resources (empty staging dir must still build)" "src-tauri/tauri.conf.json" '"resources"'
  file_contains "loader searches ../lib/<product>"    "src-tauri/src/deck_importer.rs" 'fn pdfium_candidate_dirs'
  file_not_contains "PDF fixture test is no longer ignored" "src-tauri/tests/deck_import_fixtures.rs" '#\[ignore'
  if git ls-files --error-unmatch src-tauri/resources >/dev/null 2>&1 && [ -n "$(git ls-files src-tauri/resources)" ]; then
    fail "fetched binaries under src-tauri/resources must not be tracked"
  else
    pass "no fetched binaries tracked"
  fi

  check "pdfium loader unit tests + PDF fixture (skips if pdfium not fetched)" \
    bash -c 'set -o pipefail; cd src-tauri && cargo test --lib deck_importer 2>&1 | tail -3 && cargo test --test deck_import_fixtures 2>&1 | grep -q "test result: ok. 3 passed"'

  # check-bundle.sh against synthetic trees: a good one passes, a leaky/pdfium-less one fails.
  local tmp; tmp="$(mktemp -d)"
  mkdir -p "$tmp/good" "$tmp/bad/tests"
  head -c 1100000 /dev/zero > "$tmp/good/pdfium.dll"
  echo x > "$tmp/good/PDFIUM-LICENSE.txt"; echo x > "$tmp/good/streamdeck.exe"
  echo x > "$tmp/bad/streamdeck.exe"; echo x > "$tmp/bad/app.pdb"
  check "check-bundle.sh accepts a complete tree" bash scripts/check-bundle.sh "$tmp/good" windows-x64
  if bash scripts/check-bundle.sh "$tmp/bad" windows-x64 >/dev/null 2>&1; then
    fail "check-bundle.sh must reject a tree without pdfium / with .pdb and tests/"
  else
    pass "check-bundle.sh rejects a leaky tree without pdfium"
  fi
  rm -rf "$tmp"
}

# ══════════════════════════════════════════════════════════════════
# DISPATCH
# ══════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}StreamDeck — Verification Framework${RESET}"
echo -e "${DIM}Filter: ${FILTER}  |  Frontend build: $( [ "$SKIP_FRONTEND" = true ] && echo "skipped" || echo "auto" )  |  $(date '+%Y-%m-%d %H:%M:%S')${RESET}"
echo -e "══════════════════════════════════════════════════════"

# Bootstrap always runs first (handles npm install + frontend build)
run_bootstrap

case "$FILTER" in
  invariants)
    run_invariants
    ;;
  f0.1)
    run_invariants; run_f0_1
    ;;
  f0.2)
    run_invariants; run_f0_2
    ;;
  f0.3)
    run_invariants; run_f0_3
    ;;
  f0.4)
    run_invariants; run_f0_4
    ;;
  f0|phase0)
    run_invariants; run_f0_1; run_f0_2; run_f0_3; run_f0_4; run_core
    ;;
  f1.1)
    run_invariants; run_f1_1
    ;;
  f1.2)
    run_invariants; run_f1_2; run_core
    ;;
  f1.3)
    run_invariants; run_f1_1; run_f1_2; run_f1_3
    ;;
  f1.4)
    run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4
    ;;
  f1.5)
    run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_f1_5
    ;;
  f1|phase1)
    run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_f1_5; run_core
    ;;
  f2.1)
    run_invariants; run_f2_1
    ;;
  f2.2)
    run_invariants; run_f2_1; run_f2_2
    ;;
  f2.3)
    run_invariants; run_f2_1; run_f2_2; run_f2_3
    ;;
  f2.4)
    run_invariants; run_f2_1; run_f2_2; run_f2_3; run_f2_4
    ;;
  f2|phase2)
    run_invariants; run_f2_1; run_f2_2; run_f2_3; run_f2_4; run_core
    ;;
  f3.1)
    run_invariants; run_f2_1; run_f2_2; run_f2_3; run_f2_4; run_f3_1
    ;;
  f3|phase3)
    run_invariants; run_f2_1; run_f2_2; run_f2_3; run_f2_4; run_f3_1; run_core
    ;;
  t0)
    run_t0
    ;;
  t1.1)
    run_t1_1
    ;;
  t1.3)
    run_t1_3
    ;;
  t1.2)
    run_t1_2
    ;;
  t1.4)
    run_t1_4
    ;;
  t2.1)
    run_t2_1
    ;;
  t2.2)
    run_t2_2
    ;;
  t2.3)
    run_t2_3
    ;;
  t2.4)
    run_t2_4
    ;;
  t2)
    run_t2
    ;;
  t1)
    run_t1
    ;;
  release|release.pdfium)
    run_release_pdfium
    ;;
  core)
    run_core
    ;;
  all|*)
    run_invariants; run_f0_1; run_f0_2; run_f0_3; run_f0_4; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_f1_5; run_f2_1; run_f2_2; run_f2_3; run_f2_4; run_f3_1; run_t0; run_t1; run_t2; run_release_pdfium; run_core
    ;;
esac

# ══════════════════════════════════════════════════════════════════
# SUMMARY
# ══════════════════════════════════════════════════════════════════
echo ""
echo -e "══════════════════════════════════════════════════════"
echo -e "  ${GREEN}${BOLD}Passed  : $PASSED${RESET}"
[ "$FAILURES" -gt 0 ] && echo -e "  ${RED}${BOLD}Failed  : $FAILURES${RESET}" || echo -e "  ${DIM}Failed  : $FAILURES${RESET}"
[ "$WARNINGS" -gt 0 ] && echo -e "  ${YELLOW}${BOLD}Warnings: $WARNINGS${RESET}" || echo -e "  ${DIM}Warnings: $WARNINGS${RESET}"
echo -e "  ${DIM}Skipped : $SKIPPED${RESET}"
echo ""

if   [ "$FAILURES" -eq 0 ] && [ "$WARNINGS" -eq 0 ]; then
  echo -e "${GREEN}${BOLD}  ✔  All checks passed.${RESET}"
elif [ "$FAILURES" -eq 0 ]; then
  echo -e "${YELLOW}${BOLD}  ⚠  Passed with warnings — review before marking complete.${RESET}"
else
  echo -e "${RED}${BOLD}  ✗  $FAILURES check(s) failed.${RESET}"
fi
echo ""
exit "$FAILURES"
