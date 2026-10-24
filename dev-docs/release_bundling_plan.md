# Release bundling notes

The release procedure itself (build script, CI workflow, signing routes, AV checklist) is in [`release_process.md`](release_process.md). This file keeps only the bundling design notes that are not repeated there.

GStreamer, NDI and phone-camera runtimes are not bundled on `main`; their packaging is planned on `feature/live-production`. macOS is out of scope until an Apple Developer account exists.

## Practices borrowed from large open-source desktop apps

| Practice | Our equivalent |
|---|---|
| Pinned, hashed third-party binaries | `release/deps.json`: PDFium version and per-platform SHA-256, verified before bundling (`scripts/fetch-deps.sh`) |
| One scripted build used locally and in CI | `scripts/build-release.sh`, also called by `release.yml` |
| Signing keys never on CI runners unless a cloud signing service is used | see Signing in `release_process.md` |
| Installer is the primary channel; package managers can reference it by hash | NSIS installer + published `SHA256SUMS.txt` |
| Dependency licences shipped with the product | `THIRD_PARTY_NOTICES.txt` generated into the bundle |

## PDFium

- Source: bblanchon/pdfium-binaries, pinned in `release/deps.json`.
- Windows: `pdfium.dll` via `bundle.resources` (overlay `release/bundle.pdfium.conf.json`, passed only to builds), placed next to the exe, where the loader already looks.
- Linux: `libpdfium.so`; `load_pdfium` also searches `../lib/Waylamp Present` for deb/AppImage layouts. Untested on Linux (unverified item #8).
- A release-mode import of the `dev-docs/test-assets` PDF through the real loader is the check; the fixture test prints SKIPPED when PDFium is not found.

## Bundle validation

1. `scripts/check-bundle.sh` asserts the installer contains the required files, no debug symbols or test fixtures, and stays within the size budget in `release/bundle-budget.json`.
2. A clean-machine smoke test (install silently, launch, assert the window appears, uninstall, assert no leftovers) runs in `release.yml`. A local gate cannot catch "works on my dev box" failures.
3. Supply chain: `cargo deny` and `osv-scanner` in CI, hashed binary dependencies, `npm ci`, generated licence inventory.
4. Release hygiene: published SHA-256 file, version shown in the app, and a dated record of the PDFium version shipped.

## Open decisions

- Windows signing route (see `release_process.md`; the repo being public makes SignPath Foundation worth checking).
- Linux: AppImage only, or deb as well?
