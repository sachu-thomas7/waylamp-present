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

  # Hash all .ts/.tsx/.css files in src/
  local current_hash
  current_hash=$(find "$src_dir" -type f \( -name "*.ts" -o -name "*.tsx" -o -name "*.css" \) \
    -exec md5sum {} \; 2>/dev/null | sort | md5sum | cut -d' ' -f1)
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
                 --exclude='ipc.ts' 2>/dev/null | grep -q ."
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
  file_contains "core:webview:allow-create-webview-window" \
    "src-tauri/capabilities/default.json" "core:webview:allow-create-webview-window"

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
  file_contains "ping registered"            "src-tauri/src/app.rs" "system::ping"
  file_contains "get_build_info registered"  "src-tauri/src/app.rs" "info::get_build_info"

  subsection "ipc.ts — both windows"
  for win in controller presentation; do
    local f="$win/src/lib/ipc.ts"
    if [ -f "$f" ]; then
      file_contains "$win: AppError type"          "$f" "AppError"
      file_contains "$win: ping() exported"        "$f" "export async function ping"
      file_contains "$win: PingResponse interface" "$f" "PingResponse"
      file_contains "$win: @tauri-apps/api import" "$f" "@tauri-apps/api"
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

# ══════════════════════════════════════════════════════════════════
# CORE — Build + Lint (requires frontend dist to exist)
# ══════════════════════════════════════════════════════════════════
run_core() {
  section "Core Build & Lint"

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
  f1|phase1)
    run_invariants; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_core
    ;;
  core)
    run_core
    ;;
  all|*)
    run_invariants; run_f0_1; run_f0_2; run_f0_3; run_f0_4; run_f1_1; run_f1_2; run_f1_3; run_f1_4; run_core
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
