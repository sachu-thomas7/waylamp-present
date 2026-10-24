#!/usr/bin/env bash
# PreToolUse hook: blocks `git push` until scripts/verify.sh push passes.
# scripts/verify.sh is this repo's real Definition-of-Done gate (see CLAUDE.md) —
# this hook enforces it at the one point that matters most: before code reaches
# the shared GitHub remote.
set -u

# Only act on `git push`; the settings.json `if:` filter isn't honoured for compound commands.
grep -q 'git push' <<<"$(cat)" || exit 0

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

# $HOME resolves to the MSYS home (e.g. /c/msys64/home/<you>), NOT the Windows user profile,
# in this Git Bash environment — cargo/fnm live under the Windows profile instead.
for candidate in "$(cygpath -u "${USERPROFILE:-}" 2>/dev/null)/.cargo/bin" "$HOME/.cargo/bin" "${USERPROFILE:-}/.cargo/bin"; do
  [ -d "$candidate" ] && export PATH="$candidate:$PATH" && break
done
if command -v fnm >/dev/null 2>&1; then
  eval "$(fnm env --shell bash)" 2>/dev/null
fi

LOG="$(mktemp)"
if bash scripts/verify.sh push > "$LOG" 2>&1; then
  rm -f "$LOG"
  exit 0
fi

{
  echo "scripts/verify.sh push failed — last 40 lines:"
  tail -n 40 "$LOG"
} >&2
rm -f "$LOG"

cat <<'JSON'
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"scripts/verify.sh push failed. Fix the failures (see stderr) before pushing to the shared remote."}}
JSON
exit 0
