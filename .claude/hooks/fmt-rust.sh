#!/usr/bin/env bash
# PostToolUse hook: best-effort auto-format after editing a Rust file. Never blocks
# or fails the edit — errors are swallowed on purpose.
cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
for candidate in "$(cygpath -u "${USERPROFILE:-}" 2>/dev/null)/.cargo/bin" "$HOME/.cargo/bin" "${USERPROFILE:-}/.cargo/bin"; do
  [ -d "$candidate" ] && export PATH="$candidate:$PATH" && break
done
cargo fmt --manifest-path src-tauri/Cargo.toml >/dev/null 2>&1
exit 0
