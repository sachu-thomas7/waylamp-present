# Release process (Windows installer)

Primary artifact: per-user NSIS `Waylamp Present_<version>_x64-setup.exe` (no admin, installs under `%LOCALAPPDATA%\Waylamp Present`), with the offline WebView2 runtime installer embedded. No auto-update: upgrading = run the newer installer.

## Name, identifier, publisher: one place to change

`src-tauri/tauri.conf.json`: `productName`, `identifier` (also fixes the app-data folder, so change it before real users have data), `bundle.publisher`, `bundle.copyright`, descriptions. The `Waylamp Present` name collides with Elgato's Stream Deck trademark; decide a new name before v0.1 goes to anyone outside. The release scripts take the installer name from the build output, so nothing else hard-codes it (the CI smoke step and `check-bundle.sh` match `*-setup.exe`).

## Cut a release

1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, both `package.json` files; commit on `main`.
2. Local dry run: `bash scripts/build-release.sh` (no arguments, exit 0 = good). Output: `target/release/bundle/nsis/*-setup.exe` and `SHA256SUMS.txt`. It fetches PDFium (SHA-256 pinned in `release/deps.json`), regenerates `THIRD_PARTY_NOTICES.txt`, builds both frontends, runs `cargo tauri build --bundles nsis` with `release/bundle.pdfium.conf.json` + `release/bundle.release.conf.json`, then `scripts/check-bundle.sh`.
3. `git tag v0.1.0 && git push origin v0.1.0` triggers `.github/workflows/release.yml` (tags only, `windows-2022`): builds, silent-installs, launches, uninstalls, checks leftovers, uploads installer + `SHA256SUMS.txt` to a **draft** GitHub release. Review, then publish by hand.
4. Run the AV checklist below on the published bytes.

The pdfium/release overlays are passed only to builds, never put in `tauri.conf.json`: tauri-build errors on a resource glob that matches nothing, which would break `cargo test`/clippy on a clean checkout.

## Signing (route not decided yet)

Signing is optional in the scripts so unsigned dev builds keep working. **Unsigned builds can be blocked outright by Windows 11 Smart App Control** (and always trigger SmartScreen). Sign before giving a build to anyone who is not briefed. Set `WAYLAMP_SIGN_COMMAND` to a command template where `%1` is the file; the build signs the inner `waylamp-present.exe` and the installer with it, then `build-release.sh` verifies both with `Get-AuthenticodeSignature` (status Valid and a timestamp present). The command must timestamp. Never sign after the build finishes by hand (the installer would then differ from `SHA256SUMS.txt`).

### Route A: Azure Artifact Signing (about $9.99/month, no hardware token, CI friendly)

Individuals: USA and Canada only (organisations: USA/Canada/EU/UK); identity validation takes a few business days, confirm eligibility on the live sign-up form. Not bought or registered.

```bash
cargo install artifact-signing-cli --locked
export AZURE_CLIENT_ID=... AZURE_TENANT_ID=... AZURE_CLIENT_SECRET=...   # service principal; env only
export WAYLAMP_SIGN_COMMAND='artifact-signing-cli -e <endpoint> -a <account> -c <certificate-profile> -d Waylamp Present %1'
bash scripts/build-release.sh
```

In GitHub: secrets `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET`; repository variables `AZURE_SIGN_ENDPOINT`, `AZURE_SIGN_ACCOUNT`, `AZURE_SIGN_PROFILE`. The workflow composes the command when `AZURE_CLIENT_ID` exists; with none of them it builds unsigned. Prefer OIDC federation for the service principal over a client secret when set up.

### Route B: OV code-signing certificate ($150-300/year, worldwide)

The private key must live on an HSM/USB token (since June 2023), so CI cannot use it unless the vendor offers cloud signing; sign on a local machine instead.

```bash
export WAYLAMP_SIGN_COMMAND='signtool sign /sha1 <certificate-thumbprint> /fd sha256 /tr http://timestamp.digicert.com /td sha256 %1'
bash scripts/build-release.sh
```

(Equivalent Tauri-native keys: `bundle.windows.certificateThumbprint`, `digestAlgorithm`, `timestampUrl`.) EV no longer bypasses SmartScreen, so it is not worth the premium. SignPath Foundation (free) needs a public OSI-licensed repo; the repo must be public.

Rules: same publisher identity for every release (reputation carries over); never modify a file after signing; no UPX/packers. Reputation still takes weeks after the first signed release; SmartScreen may warn until then. No secrets in the repo.

## Antivirus hygiene checklist

1. The presentation-only build has no `keyring`, no listening socket, no GStreamer (fewer AV triggers). Keep it that way.
2. Sign everything (above); keep publisher name consistent; exe version info comes from `tauri.conf.json` (publisher, copyright, description).
3. No packing; build on a clean `windows-2022` runner (the release workflow), not the dev box.
4. No runtime downloads of executables; PDFium is bundled and hash-pinned.
5. Per-user install, no admin, no auto-start, no firewall prompt.
6. Before publishing: scan on VirusTotal (informational). Submit the installer to Microsoft Security Intelligence (https://www.microsoft.com/wdsi/filesubmission) for analysis; also report to any other AV vendor that flags it. Never tell users to add exclusions.
7. Windows Defender detections named `!ml`/Wacatac are heuristics; signing and submission are the fix.

## Delivery

GitHub draft release (installer + `SHA256SUMS.txt`). Files copied without a download (for example from removable media) carry no Mark-of-the-Web, which avoids the download prompt but not Smart App Control: the signature is the fix. Verify on the target PC: `Get-FileHash .\Waylamp Present_*-setup.exe` against `SHA256SUMS.txt`.

## Licence

GPL-3.0-or-later (`LICENSE`, shown in the installer, installed as `LICENSE.txt`). `THIRD_PARTY_NOTICES.txt` (generated by `scripts/gen-notices.cjs`; Rust crates, npm production deps, PDFium, WebView2) is installed next to the exe. `cargo deny check` enforces the dependency-licence allow list.

## Size, speed, minimum runtime

Numbers are recorded in `release/bundle-budget.json`. `minimumWebview2Version` = `122.0.2365.46` (the first stable release with WebResourceRequested for iframes/workers, which wry needs for custom protocols such as `sdmedia`); the installer updates an older runtime from the embedded offline installer.
