#!/usr/bin/env bash
# Builds the Windows release installer end to end. No arguments; exit 0 only if every step passed.
#   fetch pinned deps -> notices -> frontends -> cargo tauri build (NSIS) -> [sign] -> check-bundle -> SHA256SUMS.txt
# Signing is optional: set STREAMDECK_SIGN_COMMAND to a command template where %1 is the file, e.g.
#   artifact-signing-cli -e <endpoint> -a <account> -c <profile> -d StreamDeck %1
#   signtool sign /sha1 <thumbprint> /fd sha256 /tr http://timestamp.digicert.com /td sha256 %1
# The command MUST timestamp. Secrets come from the environment only, never from the repo.
# Env: CARGO_TARGET_DIR (default <repo>/target), SKIP_NPM_CI=1 to reuse node_modules.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[ -d "/c/Users/$USERNAME/.cargo/bin" ] && export PATH="/c/Users/$USERNAME/.cargo/bin:$PATH"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
export CARGO_TARGET_DIR="$TARGET_DIR"
WIN() { cygpath -m "$1" 2>/dev/null || echo "$1"; }
step() { echo; echo "==> $*"; }

case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) ;; *) echo "build-release.sh builds the Windows installer; run it on Windows" >&2; exit 2 ;; esac

step "fetch pinned dependencies (SHA-256 verified)"
bash scripts/fetch-deps.sh windows-x64

step "frontend dependencies"
if [ "${SKIP_NPM_CI:-0}" != "1" ] || [ ! -d controller/node_modules ] || [ ! -d presentation/node_modules ]; then
  npm ci --prefix controller
  npm ci --prefix presentation
fi

step "third-party notices"
node scripts/gen-notices.cjs

CONFIGS=(--config release/bundle.pdfium.conf.json --config release/bundle.release.conf.json)
SIGNED=0
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
if [ -n "${STREAMDECK_SIGN_COMMAND:-}" ]; then
  SIGNED=1
  # Tauri signs the app exe and the installer with this command (%1 = file).
  node -e 'require("fs").writeFileSync(process.argv[1], JSON.stringify({bundle:{windows:{signCommand:process.env.STREAMDECK_SIGN_COMMAND}}}))' "$(WIN "$WORK/sign.conf.json")"
  CONFIGS+=(--config "$(WIN "$WORK/sign.conf.json")")
else
  cat >&2 <<'EOF'

  ############################################################################
  #  WARNING: building UNSIGNED. Windows 11 Smart App Control can block this  #
  #  installer and streamdeck.exe outright, and SmartScreen will warn.        #
  #  Set STREAMDECK_SIGN_COMMAND before handing this build to anyone.         #
  #  See dev-docs/release_process.md.                                         #
  ############################################################################

EOF
fi

step "cargo tauri build (NSIS, per-user, offline WebView2)"
cargo tauri build --ci --bundles nsis "${CONFIGS[@]}"

NSIS_DIR="$TARGET_DIR/release/bundle/nsis"
shopt -s nullglob
INSTALLERS=("$NSIS_DIR"/*-setup.exe)
[ "${#INSTALLERS[@]}" -eq 1 ] || { echo "expected exactly one installer in $NSIS_DIR, found ${#INSTALLERS[@]}" >&2; exit 1; }
INSTALLER="${INSTALLERS[0]}"

if [ "$SIGNED" = "1" ]; then
  step "verify signatures (installer and inner streamdeck.exe, with timestamp)"
  X="$WORK/x"; mkdir -p "$X"
  SEVENZ="$(command -v 7z 2>/dev/null || echo "/c/Program Files/7-Zip/7z.exe")"
  "$SEVENZ" e -y -o"$(WIN "$X")" "$(WIN "$INSTALLER")" streamdeck.exe >/dev/null
  for f in "$INSTALLER" "$X/streamdeck.exe"; do
    powershell.exe -NoProfile -Command "\$s=Get-AuthenticodeSignature -LiteralPath '$(WIN "$f")'; if(\$s.Status -ne 'Valid' -or -not \$s.TimeStamperCertificate){Write-Error ('not validly signed+timestamped: ' + \$s.Status); exit 1}; Write-Host ('signed: ' + \$s.SignerCertificate.Subject)" \
      || { echo "signature check failed for $f" >&2; exit 1; }
  done
fi

step "check bundle"
bash scripts/check-bundle.sh "$INSTALLER"

step "checksums"
( cd "$NSIS_DIR" && sha256sum "$(basename "$INSTALLER")" > SHA256SUMS.txt && cat SHA256SUMS.txt )
echo
echo "installer: $INSTALLER"
echo "size:      $(stat -c %s "$INSTALLER") bytes"
[ "$SIGNED" = "1" ] && echo "signed:    yes" || echo "signed:    NO (unsigned build)"
