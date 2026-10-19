#!/usr/bin/env bash
# Fetches pinned third-party binaries from release/deps.json, verifies SHA-256, stages them under src-tauri/resources/.
# Usage: bash scripts/fetch-deps.sh [windows-x64|linux-x64]   (default: host platform). Idempotent.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPS="$ROOT/release/deps.json"
STAGE="$ROOT/src-tauri/resources/pdfium"
STAGE_MARK="$ROOT/src-tauri/resources/pdfium.pinned"

PLATFORM="${1:-}"
if [ -z "$PLATFORM" ]; then
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) PLATFORM=windows-x64 ;;
    Linux*) PLATFORM=linux-x64 ;;
    *) echo "unsupported host $(uname -s); pass a platform explicitly" >&2; exit 2 ;;
  esac
fi

# Field lookup via node (already required by the frontends).
field() {
  node -e 'const d=require(process.argv[1]).pdfium;const p=d.platforms[process.argv[2]];if(!p){process.exit(3)}const k=process.argv[3];console.log(k in p?p[k]:d[k])' "$(cygpath -m "$DEPS" 2>/dev/null || echo "$DEPS")" "$PLATFORM" "$1"
}

ASSET="$(field asset)"; WANT_SHA="$(field sha256)"; LIB="$(field library)"; BASE="$(field base_url)"; VERSION="$(field version)"
LIBNAME="$(basename "$LIB")"

if [ -f "$STAGE/$LIBNAME" ] && [ "$(cat "$STAGE_MARK" 2>/dev/null)" = "$WANT_SHA" ]; then
  echo "pdfium $VERSION ($PLATFORM) already staged"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/dl" "$WORK/x"

curl -fsSL --retry 3 -o "$WORK/dl/$ASSET" "$BASE/$ASSET"
GOT_SHA="$(sha256sum "$WORK/dl/$ASSET" | cut -d' ' -f1)"
if [ "$GOT_SHA" != "$WANT_SHA" ]; then
  echo "SHA-256 mismatch for $ASSET: expected $WANT_SHA, got $GOT_SHA" >&2
  exit 1
fi

# Treat the archive as data: extract only, never execute anything from it.
tar -xzf "$WORK/dl/$ASSET" -C "$WORK/x"
[ -f "$WORK/x/$LIB" ] || { echo "$LIB missing from $ASSET" >&2; exit 1; }

rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$WORK/x/$LIB" "$STAGE/$LIBNAME"
cp "$WORK/x/LICENSE" "$STAGE/PDFIUM-LICENSE.txt"
echo "$WANT_SHA" > "$STAGE_MARK"
echo "staged pdfium $VERSION ($PLATFORM) -> $STAGE"
