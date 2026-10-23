#!/usr/bin/env bash
# Scans the three lockfiles against OSV (broader than RustSec alone). Needs Docker, or osv-scanner on PATH.
# Reviewed exceptions live in osv-scanner.toml; anything new fails the run.
set -euo pipefail
cd "$(dirname "$0")/.."

IMAGE="ghcr.io/google/osv-scanner:v2.7.0"
ARGS=(scan source --config=osv-scanner.toml -L Cargo.lock -L controller/package-lock.json -L presentation/package-lock.json)

if command -v osv-scanner >/dev/null 2>&1; then
  osv-scanner "${ARGS[@]}"
else
  MSYS_NO_PATHCONV=1 docker run --rm -v "$(pwd -W 2>/dev/null || pwd):/src" -w /src "$IMAGE" "${ARGS[@]}"
fi
