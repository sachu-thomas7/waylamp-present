# Release bundling plan (Windows + Linux)

Status: plan only, nothing implemented. macOS is out of scope until an Apple Developer account exists.
Facts below come from Tauri, NDI, GStreamer and OBS public docs/repos (checked 2026-10-19); items marked **verify** are untested.

## What OBS and similar tools do (and what we copy)

| Practice | OBS | Our equivalent |
|---|---|---|
| Pinned, hashed third-party binaries | `buildspec.json` lists each prebuilt dependency (obs-deps, Qt, CEF) with a version and per-platform SHA-256, checked before use | `release/deps.json`: PDFium, NDI runtime, GStreamer, gst-plugin-ndi — version + SHA-256, verified in CI before bundling |
| One scripted, CI-driven build | CMake presets + GitHub Actions; the same script runs locally and in CI | `scripts/build-release.sh` called by both a developer and `release.yml` |
| Signed installer built in CI, keys never on runners | Signing keys in a cloud KMS, reached by workload identity | Same pattern when we sign (see Signing) |
| Primary channel is an installer; package managers point at it and verify by hash | EXE installer; winget/Chocolatey/Scoop reference it | NSIS installer + published SHA-256 file |
| Dependency licenses shipped with the product | Third-party license notices in the install | `THIRD_PARTY_NOTICES.txt` generated into the bundle (also required by NDI) |

## Runtime shipping

### PDFium (first, smallest)
- Source: bblanchon/pdfium-binaries, pinned in `deps.json`.
- Windows: `pdfium.dll` via `bundle.resources`, placed next to the exe (the loader already looks there).
- Linux: `libpdfium.so`; add an `$ORIGIN`-relative search path in `load_pdfium` for AppImage/deb layouts.
- Check: a release-mode test imports `dev-docs/test-assets` PDF through the real loader (today the PDF fixture test is `#[ignore]`d).

### GStreamer
- Windows: no official "relocatable runtime" exists for Tauri/NSIS (**verify** against GStreamer's Windows deployment docs). Ship the MSVC runtime, then trim by measurement, not by guess: GStreamer's own docs warn that hand-picked DLL sets break because plugins load based on media.
  - Our plugin set is small and known: app, videoconvertscale, videoscale, videotestsrc, queue/coreelements, webrtc, nice, dtls, srtp, rtpmanager, sdp, openh264 (or x264), vpx, plus gst-plugin-ndi and whip/webrtchttp.
  - At startup set `GST_PLUGIN_PATH`, `GST_PLUGIN_SYSTEM_PATH` and `PATH` to the bundled folder and ignore any system GStreamer so behavior doesn't depend on what the user installed.
  - The trimming step is automated: run the F2 real-pipeline tests against the trimmed folder and fail if any plugin is missing.
- Linux: two outputs.
  - AppImage: Tauri's documented `bundleMediaFramework` flag (Ubuntu build host only), plus our extra plugins copied in.
  - deb: `depends` on the distro GStreamer packages (smaller, relies on the system). **verify** exact package names on Ubuntu 22.04/24.04.
  - Flatpak (GStreamer comes from the runtime) is a later option, not planned now.

### NDI
- Licensing (from docs.ndi.video, **re-read the EULA in the SDK folder before shipping**):
  - Runtime binaries (`BIN`) may ship inside the app if our EULA covers the NDI SDK EULA terms.
  - Alternative: bundle the redistributable installer (`/verysilent` allowed only if our app separately covers the NDI license) and keep it current.
  - Ship the SDK's licenses/notice file with the binaries.
  - Show a link to ndi.video next to every place NDI appears in the UI (source list, settings) and on our site.
  - We must not redistribute the NDI Tools.
  - Runtime lookup: honour `NDI_RUNTIME_DIR_V5` first, then the bundled copy.
- `gst-plugin-ndi` is not in the official GStreamer installers: build it in CI (gst-plugins-rs) per OS against the same GStreamer version as the bundle, and pin the commit/tag in `deps.json`.
- Needs the NDI SDK installed locally before any of this can be tested.

## Release workflow (`.github/workflows/release.yml`)
- Trigger: tag `v*` only. Never on `pull_request` (metered minutes).
- Matrix: `windows-2022` and `ubuntu-24.04` (note OBS hit a runner change that dropped preinstalled NSIS; install NSIS explicitly rather than relying on the image).
- Steps: checkout, toolchain, fetch deps and verify SHA-256, build gst-plugin-ndi, trim GStreamer, `verify.sh core`, `cargo tauri build --features ndi,whip`, bundle assertions, smoke test, checksums, upload to a draft GitHub Release.
- Existing `ci.yml` stays as is for PRs (default features, no media SDKs). Consider adding `cargo fmt --check`, which CI lacks today.

## How we validate that the bundle is good
1. **Bundle contents assertion** (script, fails the build): installer/AppImage contains pdfium, the exact GStreamer plugin list, the NDI runtime and notices; no stray debug symbols or test fixtures; size within a recorded budget so growth is noticed.
2. **Clean-machine smoke test** on a fresh runner (no Rust, Node, GStreamer or NDI installed): install silently, launch, assert the window appears, a pipeline starts with `videotestsrc`, PDF import works, then uninstall and assert no leftover files/processes. This is what catches "works on my dev box" failures; our local gate cannot, since this machine has GStreamer on PATH.
3. **Self-check at startup** (also useful for users): enumerate required plugins/DLLs, show a clear message naming whatever is missing. Same check feeds the smoke test.
4. **Performance gate** (release build, not debug): the existing ignored CPU test for 8 simulated sources, plus time-to-first-frame for WHIP (the roadmap says 3 s; debug measured about 4 s). Record numbers per release and flag regressions beyond a threshold.
5. **Supply chain**: `cargo deny` (already in CI), hashed binary deps, `npm ci`, and a generated license inventory.
6. **Release hygiene**: published SHA-256 file, version embedded and shown in the app, changelog, and a dated record of which GStreamer/NDI/PDFium versions shipped.
7. **Crash/diagnostics** (later): log file location and a "copy diagnostics" button; crash reporting only if you want it, since it sends data off-machine.

## Signing (deferred; unsigned builds are fine for early testers)
- Windows options, researched for 2026:
  - Azure Artifact Signing (about $10/month): individuals limited to US/Canada, organizations in a few regions.
  - SignPath Foundation (free): needs an OSI open-source license and public source, and it can't sign bundled third-party binaries; releases need manual approval. The repo is currently private, so it doesn't qualify yet.
  - OV certificate ($150–300/year): works anywhere; EV no longer gives instant SmartScreen trust.
  - No option removes SmartScreen warnings immediately; reputation builds with downloads.
- Until then: ship unsigned with the SHA-256 file, and expect AV false positives (we already see this on the dev binary).
- Linux: no OS signing; publish checksums, optionally a GPG signature.
- Tauri updater: needs its own signing key; add only when auto-update is wanted.

## Build order
1. PDFium bundling (Windows, then Linux search path) + the bundle-contents assertion.
2. `deps.json` with hashes and a verify script.
3. Local Windows release build with trimmed GStreamer; measure size; startup self-check.
4. `release.yml` Windows-only with clean-machine smoke test.
5. gst-plugin-ndi CI build + NDI runtime bundling (needs the SDK installed here first).
6. Linux AppImage + deb, then add to the matrix.
7. Signing when you decide on a certificate route.

## Decisions needed from the user
- Will the repo become public? (decides SignPath eligibility and the license choice)
- Where are you located, and do you want a paid certificate? (decides Azure vs OV)
- Linux: AppImage only, or deb as well?
