# Handover: ship the presentation product first (branch split, verify.sh rebuild, Windows release)

Written 2026-10-24 from a read-only investigation (no code changed). Audience: the next agent or developer who executes this.
Facts are labelled: **[repo]** read from this checkout, **[docs]** from Microsoft/Tauri documentation fetched 2026-10-24, **[inference]** my
reasoning, **[verify]** must be checked before relying on it.

## 1. Decision and goal

Ship a **presentation-only Windows release** (import PPTX/PDF, present on the projector, controller with notes/timer/scene media, multi-display)
as the first customer-facing product. Everything for live production (cameras, NDI, WHIP, compositor, streaming) moves to a long-lived branch and
returns to `main` when it is finished and proven. Users must not see half-built panels.

Owner constraints already decided (`dev-docs/owner_decisions_2026-10-23.md`): Windows only; manual updates (no auto-updater); the church network is
closed so the installer must work **offline**; the repo is private now and may go public later under a GPL-style licence (not yet chosen).

## 2. Where we are (2026-10-24) [repo]

- `main` = `origin/main` = `71275fe` (fast-forwarded this session; it was 13 commits behind locally). Working tree clean.
- Phases 0 and 1 (F0.1-F0.4, F1.1-F1.5) are implemented and gated. Hardening fixes F-01..F-12 are in. Live-production code (F2.1-F2.4, F3.1, F4.1) is also in `main`.
- The roadmap addenda F1.6-F1.9 (stage display, service-order document, focus/blank screen, readability) are **not implemented** and the proposal is unapproved.
- No `release.yml`, no `LICENSE`, no signing, no WebView2 offline setting, no user guide. The only installer ever built (NSIS, 6.8 MB) predates the hardening work and was never run on a clean machine.
- `scripts/verify.sh` is 1,380 lines, organised per roadmap feature (see section 5).

## 3. Is the presentation system done? Done and gated, not yet proven installed

| Area | Status |
|---|---|
| F1.1 projector window, F1.2 state machine, F1.3 controller panel, F1.4 PPTX/PDF import, F1.5 multi-display | Implemented, covered by Rust journeys + Vitest component tests |
| Hardening (media scheme, listen-only projector, atomic saves + recovery banner, import limits, BusLag, confirm-on-close) | Implemented; verified on dev and un-bundled release exe |
| Owed checks for a first release (from `unverified_items.md`) | #4/#20 installed app on a clean machine; #5 timer after real relaunch; #16/#19 display hotplug and ClickShare mirroring; #21 recovery banner + import-confirm dialog in the real window; #22 Quit-while-presenting; #12 real-deck fidelity; #25 delete test data from the dev data dir |
| Product gaps worth a thin slice before v0.1 | Blank/black screen control (F1.8 minimum; I found no dedicated control); About box (version, licences, PDFium notice); first-run empty state; short user guide; log file + "copy diagnostics" |
| Explicitly later | F1.6-F1.9 service-order document, Drive import, stage display |

## 4. The branch split

### 4.1 Strategy

1. Create `feature/live-production` at `71275fe` (nothing lost; NDI work continues there).
2. On `main`, remove the live code in **one removal commit** (plus follow-up commits for docs/verify). Keep it as a single revert-able unit.
3. Later, to bring live work back: `git revert <removal-commit>` on `main` first, then merge `feature/live-production`. Fixes made on `main` to shared files in the meantime
   (`events.rs`, `state.rs`, `app.rs`, `config.rs`, controller `App.tsx`) will conflict; keep those edits small and mirror them to the live branch if they matter.
4. Alternative considered: a default-off cargo feature `live` + Vite flag on one trunk. Same editing effort, no re-merge problem, but live code stays in `main`'s tree and CI forever.
   The owner asked for a branch; the cleaner product boundary favours the branch. Revisit if the live branch is expected to live longer than a few months.

Note the history is interleaved (live commits F2.1..F4.1 are mixed with hardening commits), so reverting individual commits is not an option; remove forward.

### 4.2 Inventory: remove from `main`

**Rust modules (about 8,000 lines + 7 test files) [repo]**
`src-tauri/src/`: `pipeline.rs`, `compositor.rs`, `ndi.rs`, `preview.rs`, `whip.rs`, `whip_server.rs`, `stream.rs`, `stream_service.rs`, `stream_gst.rs`,
`commands/{pipeline,compositor,ndi,preview,whip,stream}.rs`.
Tests: `src-tauri/tests/{compositor,ndi,pipeline,preview,stream,whip}_journeys.rs`, `stream_gst_e2e.rs`, `whip_gst_e2e.rs`.
Wiring to edit: `lib.rs` (mod list), `commands/mod.rs`, `state.rs` (22 live entries in `generate_handler!`, `manage_pipeline/ndi/preview/whip/stream`, `production_*_backend`),
`app.rs` (abort-handle structs, preview bridge, ndi/stream/whip tasks in `.setup()`, exit hooks, `run_preview_bridge`), `config.rs` (`WhipConfig`, `StreamConfig`, `[scene.*]`/`SceneConfig` from `compositor.rs`, `UiConfig.preview_grid`),
`permissions/controller-commands.toml` and the `autogenerated` set (regenerate with `scripts/gen-command-permissions.cjs`; `invariants.rs` asserts the set equals the handler list).

**`AppEvent` variants to delete [repo]** (`events.rs`): `Pipeline{StateChanged,Error,Warning,Latency,Qos}`, `Source{Discovered,Lost,ListChanged,Added,Removed,Stats}`,
`Whip{ServerChanged,SessionChanged}`, `Compositor{StateChanged,LayoutChanged}`, `Stream{StateChanged,DestinationChanged,Health}`.
Keep: Bus/Ping/Config/Slide*/Presentation*/Timer*/DeckImport*/SceneMediaUpdate/SceneSourceChanged/Displays*.
Both TypeScript unions and both `events.exhaustive.test.ts` files must change in the same commit (they parse `events.rs`).

**Cargo dependencies only the live code uses [repo grep]** - remove from `Cargo.toml`: `keyring` (all three target blocks; used only by `stream_service.rs` and `whip.rs`),
`axum`, `qrcode`, `sysinfo` (only `compositor.rs`), `gstreamer`, `gstreamer-app`; `png` and `base64` only if `preview.rs` is their sole user (check after removal).
Remove the `gstreamer`/`ndi`/`whip` features. Also drop `gen-command-permissions` entries and any `[features]` mentions in CI/verify.

**Controller frontend [repo]** - delete components `CameraPreview(Grid)`, `CompositorPanel`, `NdiSourcesPanel`, `PipelinePanel`, `StreamPanel`, `WhipPanel`, `QrCode`
(+ their tests), stores `compositor|ndi|pipeline|preview|stream|whip` (+ tests), the matching `ipc.ts`/`events.ts`/`resync.ts` entries, `onPreviewFrame`, and the Settings-popover sections and
`<CameraPreviewGrid/>`/`<StreamPanel/>` mounts in `App.tsx`. Presentation frontend: only `events.ts` and `events.exhaustive.test.ts` (the `NOT_MODELLED` list).
`test/eventWiring.integration.test.tsx`, `componentHelpers.ts`, `mockTauri.ts` need a pass for live references.

**Scripts/CI/docs** - `scripts/stream-e2e.sh`, `check-stream.mjs`, `mediamtx-e2e.yml`; `verify.sh` blocks `run_f2_1..run_f4_1`; CLAUDE.md sections for F2.x/F3.1/F4.1 (keep a one-paragraph pointer to the branch);
roadmap/status docs; README description ("multi-camera production, live-streaming" -> presentation control).

### 4.3 Keep on `main`

Presentation, slides, state machine, deck import (PPTX/PDF, PDFium bundling), `scene.rs` + `SceneMediaPanel` (projector media), displays, `persist.rs` + recovery banner,
`media_protocol.rs` (sdmedia), capabilities/app manifest, `invariants.rs` (adjusted), all hardening, fuzz target, `release/` + `fetch-deps.sh` + `check-bundle.sh`, deny/osv config.

### 4.4 Execution steps (acceptance in brackets)

1. `git branch feature/live-production 71275fe && git push origin feature/live-production` [branch exists on origin].
2. Branch `chore/split-presentation-only` from `main`; do the removal in one commit; fix compile, tests, typechecks, `permissions` regeneration [`cargo nextest`, clippy, both frontends green; `invariants` green].
3. Delete the dead root scaffold if still unused: root `vite.config.ts`, `index.html`, `src/assets/` (CLAUDE.md says it was removed but these exist; **verify** nothing references them first).
4. Rewrite the CLAUDE.md status block and README for the presentation product; note the live branch and how to merge back [no live claims remain on `main`].
5. Manual launch required (CLAUDE.md rule): `cargo tauri dev`, then the real-app scripts (`check-media-render.mjs`) [UI shows no camera/stream/NDI/WHIP surface; Settings = Displays + build info].
6. Confirm the default exe has no listening socket and no keychain use (grep the dependency tree for `axum`/`keyring`; check with `netstat` while running) [none].
7. Merge via PR; tag nothing yet.

## 5. `verify.sh` redesign (test-architect findings) [repo]

Problems: 1,380 lines keyed to roadmap features and cumulative (`f2.4` runs F2.1..F2.4); `events.exhaustive` appears 21 times, `npm run typecheck` 19, `--test invariants` 6, clippy 15;
`run_core` runs the whole Rust suite twice (`cargo test` and nextest); about 350 `file_contains`-style grep checks prove a word exists, not behaviour, and the real bugs we hit passed them;
hard-coded counts (`ok. 6 passed`, `-ge 28`); one shared log file and `/tmp` paths (no parallelism); the pre-push hook runs `core` including Docker osv-scanner. CI took 11 min on the last run. (I did not time `verify.sh` itself.)

Target tiers (call the same commands from CI so local and CI cannot drift):

| Tier | Runs | Contents |
|---|---|---|
| `quick` | every edit | `fmt`, `cargo check`, nextest for the touched area, typecheck + Vitest for the touched frontend |
| `push` | pre-push hook and CI | fmt, clippy, **one** nextest run (includes `invariants`), both frontends typecheck + Biome + Vitest once |
| `release` | before tagging | release build + installer, `check-bundle.sh`, install/uninstall smoke on a clean VM, real-app CDP/UIA scripts, short manual checklist |
| `deps` | CI + weekly | `cargo deny`, `osv-scanner`, fuzz smoke |
| `live` (live branch only) | there | `--features gstreamer/ndi/whip`, ignored e2e tests, hardware checks |

Rules: features become test filters/tags (nextest filtersets, Vitest projects), not scripts. Keep roughly ten regression greps that protect known past bugs (fullscreen, `prevent_close`, capability rules),
preferably moved into `invariants.rs`; delete the rest. Replace hard-coded counts with "nothing unexpectedly skipped/ignored" (`--no-tests=fail`; `check-vitest-json.cjs` already does this for Vitest).
Per-step logs in a temp dir, Rust and frontend jobs in parallel. Keep bash under about 150 lines or use a small Node script (no Python on this box). Do this **after** the split (it deletes a third of the script).
Measure first: time `core` and `all` on the pre-split tree, and again after, and record both in this file.

## 6. Windows release: research and recommendations

### 6.1 What to ship

| Option | Verdict |
|---|---|
| **NSIS per-user `-setup.exe`** (Tauri default `installMode: currentUser`, installs under `%LOCALAPPDATA%`, no admin) [docs] | **Primary.** Matches owner decisions (no admin, manual updates). winget supports Nullsoft installers (`/S` silent) [docs] |
| MSI (WiX v3, built on Windows only) [docs] | Optional second artifact if a church/IT asks for Group Policy deployment. Some third-party reports say MSI is flagged less often than NSIS; not confirmed |
| MSIX | Not recommended now: needs signing anyway, Store path is the only way to avoid SmartScreen entirely [docs], and Tauri's own docs cover NSIS/MSI only [verify MSIX support before ever considering it] |
| Portable zip (exe + `pdfium.dll` + resources, "copy and run") | Works only if WebView2 is already on the machine; useful as a fallback for locked-down PCs and for USB handover. Not auto-updating, no Start-menu entry, no uninstall. Offer second, not first |

**Install vs copy:** both can work. The installer is the supported path (WebView2, shortcuts, uninstall, upgrade-in-place keeping user data). The zip is a documented fallback.

### 6.2 Offline install (closed church network)

- WebView2 Evergreen is preinstalled on Windows 11; most but not all Windows 10 machines have it [docs]. Tauri's `webviewInstallMode` options [docs]: `downloadBootstrapper` (default, **needs internet**), `embedBootstrapper` (+1.8 MB, needs internet), **`offlineInstaller` (+about 127 MB, no internet, recommended for offline)**, `fixedRuntime` (+about 180 MB), `skip` (not recommended).
- Recommendation: `"webviewInstallMode": {"type": "offlineInstaller"}` so the installer works on a PC that never touches the internet; expect an installer of roughly 135 MB. **[verify]** that it only runs the embedded runtime installer when WebView2 is missing, and measure the real installer size.
- Also set `minimumWebview2Version` (needed for custom URI schemes such as our `sdmedia` scheme; pick the version we actually tested against) [docs].
- No auto-updater (decided). Upgrades = run the newer installer; test upgrade-in-place keeps `%APPDATA%` data and settings.

### 6.3 Code signing and SmartScreen (Microsoft docs, updated 2026-05/08)

- Two signals: publisher reputation (signed? known certificate?) and file-hash reputation. Unsigned files start at zero for **every** version; signed files can inherit reputation if the **same publisher identity** keeps signing [docs].
- **EV no longer bypasses SmartScreen** (since 2024); OV and Azure Artifact Signing behave the same. First downloads of signed apps still warn until reputation builds ("several weeks and hundreds of clean installs") [docs]. There is no manual SmartScreen submission for consumer endpoints [docs].
- **Windows 11 Smart App Control** can block **unsigned** executables unless they have positive reputation, for every executable, not only downloaded ones [docs]. An unsigned release will fail on any machine with SAC enforcing. This makes signing close to mandatory even for a small audience.
- Options [docs]: **Azure Artifact Signing** (about $9.99/month, no hardware token, CI-friendly; organisations in USA/Canada/EU/UK, **individuals only USA/Canada**; identity validation takes a few business days; one Microsoft Q&A thread in March 2026 reported fewer countries than the docs list, so confirm with the live sign-up form), **OV certificate** ($150-300/year, private key must be on an HSM/token since June 2023, worldwide), EV ($400+, no SmartScreen benefit), self-signed (blocks), **SignPath Foundation** (free for qualifying open-source projects: public repo, OSI licence, no commercial dual licence; the repo is private today).
- Tauri wiring [docs]: `bundle.windows.certificateThumbprint` + `digestAlgorithm` + `timestampUrl` for an installed cert; or `bundle.windows.signCommand` (any tool, `%1` = file) which is required for CI/cross-builds; Azure example: `cargo install artifact-signing-cli`, env `AZURE_CLIENT_ID/TENANT_ID/CLIENT_SECRET`, `signCommand: artifact-signing-cli -e <endpoint> -a <account> -c <profile> -d <description> %1`. Confirm in the built artifacts that **both the installer and the inner `streamdeck.exe` (and bundled `pdfium.dll` if we re-sign it) are signed**; the doc does not say. Always timestamp.
- **Decision needed from the owner:** country/individual vs organisation (decides Azure vs OV) and whether to wait for the repo to go public (SignPath). Until signed, only hand builds to people you can brief and test with SAC off.
- Never modify a file after signing; sign every release with the same identity; never sign anything that behaves like a PUA [docs].

### 6.4 Antivirus hygiene (what we can control)

Evidence: Defender `!ml` detections (Wacatac) are machine-learning heuristics; reported triggers include registry/system modification and unusual packing; signing inner binaries too is a commonly suggested fix [third-party forums, low confidence]. Our own history: `VHO:Trojan-PSW` on the dev binary attributed to `keyring` + unsigned [repo CLAUDE.md]; Kaspersky/Norton deleting fresh exes [repo].

Checklist:
1. **Presentation-only build has no `keyring`, no `axum` listener, no GStreamer** - fewer credential-store and socket behaviours is the biggest AV win and falls out of the split for free [inference].
2. Sign everything (section 6.3); consistent publisher name; fill exe version-info (company, product, description, copyright: currently unset in `tauri.conf.json` [repo]).
3. Never pack/compress the exe (no UPX); build in CI on a clean `windows-2022` runner, not on the dev box; keep builds reproducible (`SOURCE_DATE_EPOCH` support already in `build.rs`).
4. No runtime downloads of executables; PDFium is bundled and SHA-256-pinned (`release/deps.json`) [repo]. Third-party DLLs stay unmodified.
5. Per-user install, no admin, no self-modifying behaviour, no auto-start, no firewall prompt (no listening socket).
6. Before publishing each release: scan on VirusTotal (informational), and submit the installer to the Microsoft Security Intelligence portal (https://www.microsoft.com/wdsi/filesubmission) for analysis; check status when signed in with the same account [docs]. Do the same for Kaspersky/Norton since the owner and church use them. Never tell users to add exclusions.
7. Release profile: the root `Cargo.toml` only tunes dev/test profiles [repo]. Add a `[profile.release]` (for example `strip = true`, `lto = "thin"`, `codegen-units = 1`) and measure size and startup; do not change `panic` without testing the actor threads.

### 6.5 Delivery to users (closed network)

Channels: GitHub Release (draft first) with installer + `SHA256SUMS.txt`; a USB stick for the church. **[verify]** Windows marks files with Mark-of-the-Web only when they arrive via a browser/download; files copied from a USB stick or share normally carry none, which avoids the download prompt but **not** Smart App Control, which checks all executables [docs]. Plan the signature, not the delivery path, as the fix. Later: winget manifest (needs public URL, hash, licence) once the repo or a public download exists; Microsoft Store (MSIX) is the only way to skip SmartScreen entirely [docs] but is a separate project.

### 6.6 Release pipeline (`release.yml`)

Trigger on tag `v*` only (metered minutes). `windows-2022`. Steps: checkout, pinned toolchain, `npm ci`, `scripts/fetch-deps.sh windows-x64` (verifies hashes), `verify.sh push`,
`cargo tauri build --config release/bundle.pdfium.conf.json` (plus the signing config/secret via OIDC, never plain secrets in repo), `scripts/check-bundle.sh` (extend `REQUIRED`: WebView2 offline installer, notices, licence; keep the size budget in `release/bundle-budget.json`),
silent install/uninstall smoke on the runner (install, launch, import the fixture PDF/PPTX through the real loader, quit, uninstall, assert no leftovers), compute checksums, upload to a **draft** release.
Add: `THIRD_PARTY_NOTICES.txt` (PDFium BSD/Apache, Rust crates, npm packages) generated in CI; SBOM; artifact attestation/provenance **[verify availability for private repos]**.

### 6.7 Legal/product items to settle before v0.1

- **Product name:** "StreamDeck" collides with Elgato's Stream Deck product [inference; not researched] - decide a name before the installer, publisher name and winget id exist; the bundle `identifier` (`app.streamdeck.desktop`) also fixes the app-data folder, so change it **before** any real user has data.
- Licence (owner leaning GPL-3.0-or-later; not added), `LICENSE` file, EULA text for the installer, privacy statement (the app sends nothing off-machine in this build - confirm after the split).
- Clean the dev data dir (`unverified_items.md` #25) before using this machine for the clean-install test.

## 7. Work breakdown (suggested order)

| # | Work | Size | Depends on | Done when |
|---|---|---|---|---|
| 1 | Time current `verify.sh core` and `all` | S | none | numbers recorded here (after split: `core` 95 s, full 158 s with 307 checks; verify.sh 1380 -> 1046 lines): `core` 4m15s (incl. frontend bootstrap on a fresh worktree, warm target dir), full `--no-frontend` 9m18s (471 checks passed, 1 warning, 1 skipped), pre-split tree 71275fe |
| 2 | Create `feature/live-production` + push | S | none | branch on origin |
| 3 | Removal commit on a PR branch (4.2-4.4) | L | 2 | `push`-equivalent checks green, no live surface in UI, no listener/keychain |
| 4 | Rebuild `verify.sh` as tiers; wire CI + hook to it | M | 1, 3 | `quick`/`push` documented; timing compared to #1 |
| 5 | Product polish slice: blank screen, About + notices, empty states, log file, name decision | M | 3, name | works in real app |
| 6 | Release config: offline WebView2, per-user NSIS, version-info, release profile, `minimumWebview2Version` | M | 3 | installer builds; size recorded |
| 7 | Signing decision + setup (Azure identity validation takes days; start early) | M | owner | signed installer + inner exe |
| 8 | `release.yml` + `check-bundle.sh` extension + clean-VM smoke | L | 4, 6, 7 | draft release from a tag |
| 9 | Owed manual checks: #4, #5, #16, #19, #21, #22 on the installed build | M | 8 | `unverified_items.md` updated |
| 10 | Notices/licence/EULA, README + short user guide | M | name, licence | in the installer |
| 11 | Pilot with one real service; submit to WDSI/Kaspersky | M | 8, 9 | no AV block; feedback logged |

Start #7's identity validation and the name decision first; they have waiting time.

## 8. Risks

- Smart App Control blocks an unsigned build on clean Windows 11 PCs. Mitigation: sign before any external user.
- Offline WebView2 inflates the installer (about 127 MB). Accept for a closed network; offer a small online installer later if wanted.
- Removal commit conflicts on re-merge. Mitigation: single commit, minimal shared-file edits afterwards.
- `verify.sh` rewrite regresses coverage. Mitigation: do not delete behavioural tests; convert needed greps into `invariants.rs`; compare test counts before/after.
- Reputation takes weeks. Mitigation: consistent publisher identity, brief the early users.
- AV deletes build outputs on the dev box (known). Mitigation: build and test in CI/VM.

## 9. Questions for the owner

1. Where are you based and are you an individual or a registered business (Azure Artifact Signing vs OV)? Will the repo go public soon (SignPath)?
2. Product name (avoid the Elgato trademark), publisher name, support contact.
3. Which Windows versions do the church machines run (Windows 10 without WebView2? Smart App Control on?).
4. Which antivirus products are installed there (Kaspersky, Norton, Defender)?
5. Licence choice and whether a GPL release is compatible with the commercial intent.
6. Is a 130 MB offline installer acceptable, and is USB handover the norm?

## 10. Sources

- Tauri v2 Windows installer: https://v2.tauri.app/distribute/windows-installer/
- Tauri v2 Windows signing: https://v2.tauri.app/distribute/sign/windows/
- Code signing options (Microsoft Learn, 2026-08-29): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- SmartScreen reputation (Microsoft Learn, 2026-08-17): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
- Defender false positives and submission: https://learn.microsoft.com/en-us/defender-endpoint/defender-endpoint-false-positives-negatives and https://www.microsoft.com/wdsi/filesubmission
- winget manifest: https://learn.microsoft.com/en-us/windows/package-manager/package/manifest
- WebView2 distribution: https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution
- Azure Artifact Signing quickstart: https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart
- Earlier internal docs: `dev-docs/release_bundling_plan.md`, `dev-docs/owner_decisions_2026-10-23.md`, `dev-docs/unverified_items.md`, `dev-docs/review/architecture_review_2026-10-23.md`

## Status log

- 2026-10-24 step 1 done: baseline on 71275fe: `verify.sh core` 255 s (20 checks), `verify.sh --no-frontend` (all) 558 s (471 checks, 0 failed, 1 warning, 1 skipped). Same box, warm `target`.
- 2026-10-24 step 2 done: `feature/live-production` created at 71275fe and pushed to origin; untouched since.
- 2026-10-24 steps 3-4 done on branch claude/split-presentation-only-e3a679: removal in ONE commit (101 files, -13.8k lines), docs/root-scaffold in a second. Rust 175 tests, controller vitest 168 -> 107, presentation 52 -> 52. `cargo tree`: no keyring/axum/gstreamer/qrcode/sysinfo/png/base64.
- 2026-10-24 step 5 done: verify core 95 s, full 158 s (307 checks, 0 failed). Manual `cargo tauri dev`: Settings = Displays, Deck, Build Info only; no listening socket; check-media-render.mjs all PASS except video not tested (no fixture video passed). Not done: UIA check of Quit-while-presenting etc. (unchanged code).
- Follow-ups left: verify.sh tier redesign (still per-feature, 1046 lines); `AppError::Keychain` variant kept (harmless); handler-count floor in invariants.rs lowered 40 -> 20; `.claude/agents/feature-builder.md` line 51 mentions GStreamer/NDI.
- 2026-10-24 merge-back rehearsal (temp worktree, since removed): `git revert <removal commit>` then merge `feature/live-production` on top of this branch. Only conflict: `controller/src/App.tsx` (resolve: keep the `.app-main` grid/`<aside>` markup, add `<CameraPreviewGrid/>` to the primary column and `<StreamPanel/>` to the aside). Full `verify.sh` on the result: 473 checks passed, 0 failed, 1 warning, 1 skipped, 640 s. The live branch had no newer commits, so the merge itself was a no-op.
- 2026-10-24 controller layout: CSS grid + container query + `minWidth/minHeight` window size; live panels, when re-added, should be placed in `.app-main-primary` / `.app-main-aside`.
- 2026-10-24 release packaging (steps 6, 8, 10; branch claude/release-packaging-f35671): per-user NSIS + `offlineInstaller` + `minimumWebview2Version` 122.0.2365.46, `[profile.release]` (strip, thin LTO, cgu 1; in root Cargo.toml, profiles only apply there), GPL-3.0-or-later, `LICENSE`, `scripts/gen-notices.cjs`, `scripts/build-release.sh`, `release.yml` (tag-only, draft release), `dev-docs/release_process.md`.
- Measured: installer **223,347,445 B** (not ~135 MB: Microsoft's offline WebView2 installer is now 214.8 MB); app exe 13.41 MB -> 12.07 MB with the release profile; window-handle cold start median ~91 ms vs ~106 ms (noise, no gain or loss). Installed/uninstalled silently as a normal user on this box. Not proven: signing, clean VM, SmartScreen/SAC, AV verdicts, the GitHub run. See `unverified_items.md` #29.
