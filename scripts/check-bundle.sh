#!/usr/bin/env bash
# Asserts a built bundle contains the required runtime files, leaks nothing it shouldn't, and fits the size budget.
# Usage: bash scripts/check-bundle.sh <installer.exe | .deb | dir> [windows-x64|linux-x64]
# Installers are extracted (never executed) into a temp dir: .exe needs 7z, .deb needs dpkg-deb.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:?usage: check-bundle.sh <installer|dir> [platform]}"
[ -e "$TARGET" ] || { echo "not found: $TARGET" >&2; exit 2; }

PLATFORM="${2:-}"
if [ -z "$PLATFORM" ]; then
  case "$TARGET" in
    *.exe|*.msi) PLATFORM=windows-x64 ;;
    *.deb|*.AppImage) PLATFORM=linux-x64 ;;
    *) case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) PLATFORM=windows-x64 ;; *) PLATFORM=linux-x64 ;; esac ;;
  esac
fi

# Extend these lists as later release steps add runtime pieces (GStreamer, NDI, notices).
case "$PLATFORM" in
  windows-x64) REQUIRED=(pdfium.dll PDFIUM-LICENSE.txt streamdeck.exe) ;;
  linux-x64)   REQUIRED=(libpdfium.so PDFIUM-LICENSE.txt streamdeck) ;;
  *) echo "unknown platform $PLATFORM" >&2; exit 2 ;;
esac
FORBIDDEN_NAMES=('*.pdb' '*.dSYM' '*.pptx' '*.pdf' 'streamdeck-test-*' '*.rlib' '*.d')
FORBIDDEN_DIRS=('fixtures' 'test-assets' 'tests')

FAILS=0
bad() { echo "  FAIL  $1"; FAILS=$((FAILS+1)); }
ok()  { echo "  ok    $1"; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
TREE="$TARGET"
if [ -f "$TARGET" ]; then
  TREE="$WORK/x"; mkdir -p "$TREE"
  case "$TARGET" in
    *.exe|*.msi)
      SEVENZ="$(command -v 7z 2>/dev/null || true)"
      [ -n "$SEVENZ" ] || SEVENZ="/c/Program Files/7-Zip/7z.exe"
      [ -x "$SEVENZ" ] || { echo "7z not found; pass an extracted directory instead" >&2; exit 2; }
      "$SEVENZ" x -y -o"$(cygpath -m "$TREE" 2>/dev/null || echo "$TREE")" "$(cygpath -m "$TARGET" 2>/dev/null || echo "$TARGET")" >/dev/null ;;
    *.deb) dpkg-deb -x "$TARGET" "$TREE" ;;
    *) echo "cannot extract $TARGET; pass an extracted directory" >&2; exit 2 ;;
  esac
fi

echo "bundle check: $TARGET ($PLATFORM)"
for f in "${REQUIRED[@]}"; do
  if [ -n "$(find "$TREE" -type f -name "$f" -print -quit)" ]; then ok "has $f"; else bad "missing $f"; fi
done
# A real pdfium is megabytes; a placeholder or empty file must not pass.
LIBFILE="$(find "$TREE" -type f \( -name pdfium.dll -o -name libpdfium.so \) -print -quit)"
if [ -n "$LIBFILE" ]; then
  [ "$(stat -c %s "$LIBFILE")" -gt 1000000 ] && ok "pdfium library size plausible" || bad "pdfium library under 1 MB"
fi
for n in "${FORBIDDEN_NAMES[@]}"; do
  hit="$(find "$TREE" -type f -name "$n" -print -quit)"
  [ -z "$hit" ] && ok "no $n" || bad "leaked $n: $hit"
done
for d in "${FORBIDDEN_DIRS[@]}"; do
  hit="$(find "$TREE" -type d -name "$d" -print -quit)"
  [ -z "$hit" ] && ok "no $d/ dir" || bad "leaked dir $d/: $hit"
done

if [ -f "$TARGET" ]; then SIZE="$(stat -c %s "$TARGET")"; else SIZE="$(du -sb "$TARGET" | cut -f1)"; fi
BUDGET="$(node -e 'const b=require(process.argv[1])[process.argv[2]];console.log(b?b.max_bytes:0)' "$(cygpath -m "$ROOT/release/bundle-budget.json" 2>/dev/null || echo "$ROOT/release/bundle-budget.json")" "$PLATFORM")"
if [ "$BUDGET" -gt 0 ]; then
  [ "$SIZE" -le "$BUDGET" ] && ok "size $SIZE <= budget $BUDGET bytes" || bad "size $SIZE exceeds budget $BUDGET bytes"
else
  echo "  info  size $SIZE bytes (no budget recorded for $PLATFORM)"
fi

[ "$FAILS" -eq 0 ] && echo "bundle OK" || { echo "$FAILS check(s) failed" >&2; exit 1; }
