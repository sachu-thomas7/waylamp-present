# StreamDeck independent architecture, security and release review

Date: 2026-10-23. Reviewer role: Chief Architect + Principal Security/Release Engineer. Scope: the repo at `main` (9844897) plus uncommitted docs and `tools/`.

**Evidence labels:** **[code]** read in the repo; **[tool]** produced by a scanner or command I ran; **[web]** from a cited URL; **[inf]** my inference or opinion. I did not run the app, a full build or `verify.sh`. Web results came from search snippets and a few page fetches; where a source was thin I say so.

---

## 1. Verdict

1. **The direction is sound.** Tauri v2 + Rust + a GStreamer media engine, with the projector fed from a state bus, is a defensible 2026 choice for one developer. The invariants (bus-only state, capability minimalism, no `unwrap` in commands, secrets in the keychain, config as TOML) are real and are enforced by parsing tests (`tests/invariants.rs`); that is better than most solo projects.
2. **The biggest risk is blast radius, not a single bug.** The projector window, PDF/PPTX parsers, pdfium native code, GStreamer, the NDI plugin and the WHIP server all live in one process. One native fault blacks out the projector mid-service. There is no supervisor, no restart-into-last-state and no crash UX [code]. Move the media engine (and importers) out of process before adding RTMP egress.
3. **Probable functional bug, high impact:** imported slide backgrounds and scene media are passed to `<img src>` / `<video src>` as raw filesystem paths, with no `convertFileSrc`, no asset protocol in `tauri.conf.json`, and a `default-src 'self'` CSP [code]. This most likely renders nothing in a built app. Not run; verify first (F-01).
4. **Release engineering is the least developed area:** no release workflow, no signing, no updater, no GStreamer/NDI bundling decision, no rollback story, no diagnostics bundle. For "a volunteer upgrades the night before a service" this will hurt users most.
5. **Persisted-state resilience is thin:** a corrupt or half-written `streamdeck.toml` makes startup fail outright (non-atomic writes, fatal parse error in `.setup()`), and lossy broadcast events are never resynced (`BusLag` is declared in TypeScript but handled nowhere) [code].
6. **Security posture is decent for a LAN desktop app, with specific holes:** WHIP is plain HTTP on `0.0.0.0` with an optional token that is unset by default and `Access-Control-Allow-Origin: *`; importers have no decompression or page limits; `core:event:default` includes `emit`, so the "listen-only" presentation window is not listen-only; several granted permissions are unused.
7. **Supply chain is healthy on the surface:** `cargo deny check` passes, `npm audit` 0 and gitleaks 0 across 78 commits [tool]. OSV flags `glib 0.18.5` and `serde_with 3.20.0`, which local cargo-deny did not [tool].
8. **Plan fit:** the structured service-order document, Design/Show, Preview/Program with Take and the control API need one authoritative, versioned, revisioned application state in Rust. Today state is scattered (slide machine, scene handle, TOML, deck.json, a `static PRESENTING`) with no schema versions. Build that spine first (section 4.7).
9. **Over-scoped for one person:** F1.7 + F1.7a + F1.8 + F1.9 + F1.6 + U1-U3 + F3.3b + F3.7 + Drive import + control API as listed. Cut to a thin vertical slice: one service, projector only, then stream.
10. **The five that matter most:** (a) verify/fix media path loading; (b) out-of-process media engine plus a crash-recovery contract; (c) atomic, versioned, recoverable persistence; (d) hardened importers (limits, fuzzing, separate process); (e) a signed, updatable, rollbackable installer pipeline.

---

## 2. Findings table (most severe first)

Severity / Likelihood / Effort (S <= 1 day, M <= 1 week, L > 1 week). Everyone listed as hurt is the end user mid-service unless stated.

| ID | Area | Sev | Likelihood | Effort | Finding | Evidence | Recommendation |
|---|---|---|---|---|---|---|---|
| F-01 | Correctness | High | High | S-M | Slide backgrounds (`background_path`) and scene media (`path`) are raw absolute paths used as `<img src>`/`<video src>` in the presentation webview. No `convertFileSrc`, no `assetProtocol` scope, CSP `default-src 'self'`. A `tauri://` origin does not load `C:\...`. Imported PDF/PPTX backgrounds and pushed media probably do not display in a built app. [code], not run | `presentation/src/components/SlideCanvas.tsx:61`, `SceneMediaView.tsx:17-26`, `src-tauri/tauri.conf.json`, `commands/scene.rs` | Run a real import in a release build first. Fix with a custom URI scheme (`register_asynchronous_uri_scheme_protocol`) rooted at the app data dir with canonicalisation, plus `img-src`/`media-src` for it. Add a real-app E2E asserting `naturalWidth > 0`. |
| F-02 | Resilience | High | Medium | L | One process holds projector UI, parsers, pdfium, GStreamer, NDI plugin, WHIP. A native fault in any kills slides. An actor-thread panic leaves `gone()` forever: no supervisor, no `catch_unwind`. | `app.rs` single `run()`; `pipeline.rs:80-83`; no restart code | Run the media engine as a sidecar process (own crash domain, authenticated local channel). Slides stay in the main process. On engine exit: restart with backoff, replay desired state, show a banner. Start with pdfium import and egress. |
| F-03 | Resilience | High | Medium | S | `streamdeck.toml` is written with `fs::write` (truncate-then-write) and read-modify-written by several commands with no lock. A parse error is fatal in `.setup()`: the app never opens. | `config_loader.rs:36-53`, `app.rs` setup (`map_err(...)?`), `whip.rs:921-935` | Temp file + fsync + rename; keep `.bak`; on parse failure move the file to `*.broken-<ts>`, start with defaults, show a recovery banner; one config writer. Same for `deck.json`, `slide_state.json`. |
| F-04 | Security | High | Medium | M | PPTX/PDF import has no limits: zip entries are `read_to_string`/`read_to_end` without a decompressed-size cap (zip-bomb OOM), no slide/page cap, every PDF page rendered at 1920x1080 to PNG, untrusted PDF parsed by native pdfium in-process. Import replaces the live deck and switches the scene source at once. | `deck_importer.rs:75-110,382-390,461-480`; `commands/deck.rs:19-52` | Cap uncompressed size per entry and total, entry count, ratio, pages; import to a staging dir and require "Use this deck" (also needed for Design/Show); run in the sidecar; cargo-fuzz the slide/rels parsers. |
| F-05 | Security | High if enabled | Medium | M | WHIP: plain HTTP, default bind `0.0.0.0:8088`, bearer token optional and unset by default (no token = fully open), CORS `*` including preflight, no rate or connection limits, no Origin/Host check. Token crosses the LAN in cleartext. Off by default (good). | `config.rs:47-60`, `whip.rs:609-622`, `whip_server.rs:46-70` | Generate a random token on first enable (never open); bind the chosen LAN IP; reject browser `Origin`; connection/rate caps; auto-disable after idle; document venue guest Wi-Fi exposure. A drive-by web page on the operator's laptop can POST an SDP offer today when no token is set [inf from code]. |
| F-06 | Release | High | High | L | No release workflow, signing, updater, install-mode policy, GStreamer/NDI bundling, third-party notices or rollback. | `.github/workflows/` has only `ci.yml`; `tauri.conf.json` has no `plugins.updater` or `bundle.windows`; `release/` holds only PDFium | Section 4.5. Distributor and end user. |
| F-07 | Resilience | Medium | Medium | M | Lossy state sync. The bus is a 256-slot `broadcast`; on lag the bridge emits `BusLag`, but neither frontend handles it, and the presentation window cannot invoke to resync. `SlideUpdate`/`PresentationVisibility` carry state, so one drop leaves the projector stale until the next event. | `bus.rs:6-12`, `app.rs` bridge; `grep BusLag` finds only type declarations | Monotonic `rev` on state events; `StateSnapshot` heartbeat every 1-2 s and after any `BusLag`; presentation applies the highest `rev`. Keep `SourceStats`/`Qos`/`Latency`/`TimerTick` off the state channel. |
| F-08 | Security | Medium | Low | S | Capability hygiene. `core:event:default` = listen, unlisten, **emit, emit_to** [web], so the presentation window can forge `app-event` into the controller although docs and tests call it listen-only. `remote.urls` sits in the shipped capability. Controller grants `shell:allow-open`, `global-shortcut:allow-*`, `core:webview:allow-create-webview-window` yet its JS uses none of them. `devtools` Cargo feature is always on. CSP has `script-src 'unsafe-inline'`. | `capabilities/*.json`; `Cargo.toml` `features=["devtools"]`; grep of `controller/src` for shell/shortcut/WebviewWindow: no hits; https://v2.tauri.app/reference/acl/core-permissions/ | Presentation: `core:event:allow-listen` + `allow-unlisten` only. Dev-only capability for `remote.urls`. Drop unused grants and `tauri-plugin-shell`. Gate `devtools`. Remove `unsafe-inline` from `script-src`. Make `invariants.rs` assert the exact permission list. |
| F-09 | Architecture | Medium | Medium | M | `PipelineService` actor: unbounded command channel, serial blocking backend calls (state changes, `recv_timeout(1s)`), no per-call deadline, no heartbeat; `PipelineError` from the bus watcher does not change `state`. A hung element wedges every command. | `pipeline.rs:80-83,174-330`, `compositor.rs:431-438` | Per-command deadline and watchdog; `Errored` state with a restart policy; bounded channel. |
| F-10 | Correctness | Medium | High | S | Timer persists a wall-clock start across restarts: explains `214:51:51` (unverified #5). | `slide_state_machine.rs:224-238` | Persist accumulated elapsed + running flag, or reset on launch. |
| F-11 | UX | Medium | Medium | S | Escape is registered as an OS-wide global shortcut while presenting, stealing Escape from every other app on the laptop; `PRESENTING` is a process `static`. | `commands/presentation.rs:13-17,86-110` | Window-level key handler plus an on-screen control; keep a global key only as opt-in with a modifier. |
| F-12 | Supply chain | Medium | Medium | S | OSV on `Cargo.lock`: `glib 0.18.5` RUSTSEC-2024-0429 (fix 0.20.0; Linux GTK3 path via Tauri) and `serde_with 3.20.0` GHSA-7gcf-g7xr-8hxj (fix 3.21.0); local `cargo deny check` reported neither. Five `ignore` entries in `deny.toml` are stale. | [tool] osv-scanner Docker run; `cargo deny check` | Bump `serde_with`; document the glib ignore as Linux-only; refresh the advisory DB in CI; delete stale ignores; add osv-scanner to CI. |
| F-13 | Licensing | Medium | Medium | S | `tools/service-analyzer` uses PyMuPDF (AGPL-3.0 or Artifex commercial) [web]. Fine as an internal Docker tool; a trap if its logic migrates into the product ("the analyzer already does this" in F1.7a). x264 (GPL) and OpenH264 distribution need a decision before any bundle. | `tools/service-analyzer/Dockerfile`; roadmap addendum F1.7a; https://pymupdftest.readthedocs.io/ (snippet) | Re-implement classification on pdfium text objects (BSD) via `pdfium-render`. Ship LGPL/MPL GStreamer plugins plus OS/GPU hardware encoders; lawyer review for H.264/HEVC patents. |
| F-14 | Fidelity | Medium | High | M | PPTX speaker notes are never extracted (`let notes = String::new()`); PDF import keeps only the first line as title and an empty `body`, so PDF text is unusable for the service-order document. | `deck_importer.rs:123,289,494-500` | A real extraction layer (OOXML styles; pdfium text objects with font flags). Until then, do not claim notes. |
| F-15 | Tooling | Medium | Medium | M | `verify.sh`: 1,310 lines, ~257 `grep -q` presence checks, fixed `/tmp` log path, Git-Bash-specific, no real-app step. Green proves "files contain strings", not behaviour. | `scripts/verify.sh:44-80` | Keep real cargo/vitest steps; delete greps a compiler or test already enforces; move to `just`/`cargo xtask`; add nextest. |
| F-16 | Docs | Low | High | S | `rust-version = "1.78"` and CLAUDE.md "Rust 1.78" vs CI/`rust-toolchain.toml` 1.96.0; current deps (zip 9, toml 1.1 in tree) will not build on 1.78. MSRV is untested. | `Cargo.toml:5`, `rust-toolchain.toml` | Set `rust-version` to what builds (`cargo msrv`) and fix docs. |
| F-17 | Maintainability | Low | Medium | M | `ts-rs` is a dependency but one type uses it; `AppEvent` is hand-copied into two TS projects behind parity tests. | `grep ts_rs`: 1 hit | Generate `AppEvent` TS from Rust into both projects; keep parity tests as a net. Keeps the two-project choice without hand-copying. |
| F-18 | Dependency | Low | Medium | S | `pdfium-render` 0.8.37 locked while its README lists 0.9.4 as latest, targeting `pdfium_7881`; you ship pdfium 157.0.8100.0. No automated bump of the binary pin; Chrome ships PDFium security fixes continuously. | `Cargo.lock`, `release/deps.json`; https://github.com/ajrcarey/pdfium-render | Test 0.9.x against the pinned binary; monthly CI job proposing a new tag + SHA. |
| F-19 | Release | Low | Medium | S | `bundle.targets: "all"`, no `webviewInstallMode`, no `nsis.installMode`; Defender/Kaspersky trouble already seen. | `tauri.conf.json` | Decide per section 4.5. |
| F-20 | Scripts | Low | Medium | S | `pre-push-verify.sh` greps the whole stdin JSON for the text `git push` (any command mentioning it triggers a ~5 min run); fails open if `cd` fails. Analyzer image unpinned (`python:3.12-slim`, `pymupdf==1.24.*`). `fetch-deps.sh` is good (SHA-256 verified before extraction) but pins the archive only. | `.claude/hooks/pre-push-verify.sh`, `tools/service-analyzer/Dockerfile` | Parse `tool_input.command`; pin image digest and `--require-hashes`. |
| F-21 | CI | Low | Medium | S | Actions pinned by tag, not SHA; no SBOM or provenance; no osv/npm-audit job; `--locked` not used. cargo-deny job exists (good). | `.github/workflows/ci.yml` | Pin SHAs (Dependabot already configured), `cargo cyclonedx`, `actions/attest-build-provenance` in the release job. |

---

## 3. Scanner results (what actually ran)

| Tool | Result | Notes |
|---|---|---|
| `cargo deny check` 0.20.2 (local, `src-tauri`) | advisories ok, bans ok, licenses ok, sources ok | 5 stale ignores reported "advisory-not-detected" (RUSTSEC-2025-0075/0080/0081/0098/0100). Did not flag the two OSV items below. |
| `npm audit` (controller, presentation) | 0 vulnerabilities each | |
| gitleaks (Docker, `git` mode, 78 commits, redacted) | no leaks | Working tree not scanned separately; `tools/` is untracked. |
| osv-scanner (Docker, 3 lockfiles: 539 Rust, 170 + 169 npm packages) | `glib 0.18.5` RUSTSEC-2024-0429 (CVSS 6.9, fix 0.20.0); `proc-macro-error 1.0.4` RUSTSEC-2024-0370 (unmaintained, no fix); `serde_with 3.20.0` GHSA-7gcf-g7xr-8hxj (CVSS 5.1, fix 3.21.0) | Advisory text not fetched; severities are OSV's. |
| `cargo audit` | **Not run** (not installed; cargo-deny reads the same RustSec DB) | |
| trivy fs, Semgrep, cargo-geiger | **Not run**. `rg "unsafe "` over `src-tauri/src` found no `unsafe` | Add `#![forbid(unsafe_code)]`. |

---

## 4. Per-scope analysis

### 4.1 Security and trust boundaries

**Good**
- No `unsafe`; no `.unwrap()` in `commands/` (test-enforced); `AppError` everywhere; size limits validated in Rust before copy (10 MB image, 500 MB video) [code].
- Presentation window has no invoke permissions; window labels are checked against capability files by `invariants.rs` [code].
- WHIP: off by default, 64 KiB body limit, content-type checks, constant-time token compare, keychain failure keeps the endpoint closed, sane 401/409/503 [code].
- NDI source names are escaped before entering `gst::parse::launch` strings and the frontend never supplies pipeline strings [code: `ndi.rs:49-53`, `compositor.rs:254`].
- Secrets rule enforced by test; `keyring` per-OS features fixed after the mock-fallback bug.
- Tauri 2.12.4 / wry 0.57 are past the 2026 advisories I found: IPC origin confusion CVE-2026-42184 (fixed 2.10.3 per trackers), updater downgrade flag CVE-2026-95624 (plugin < 2.12.0, not used), CSP nonce bypass CVE-2026-95626 (needs `data:`/`blob:` in `script-src`; yours has neither) [web: https://cveawg.mitre.org/api/cve/CVE-2026-42184, https://www.sentinelone.com/vulnerability-database/cve-2026-95624/, https://www.sentinelone.com/vulnerability-database/cve-2026-95626/; third-party trackers, ranges conflict, confirm on GitHub advisories].

**Risky**
- F-04, F-05, F-08.
- **IPC takes arbitrary paths.** `deck_import(path)` and `scene_push_*` accept any path with an allowed extension. Only the controller can call them, so today a controller XSS means read any pptx/pdf and copy any png. Acceptable now; it grows with Drive import or any remote text in the controller (never use `dangerouslySetInnerHTML`). Prefer "dialog result held in Rust, import by token".
- **`scene_push_video` stores the original path** (no copy): moving the file after push breaks the projector at the worst moment. Copy or hard-link into app data like images.
- **Config hot reload** publishes `ConfigReloaded` and nothing re-reads it [code: `config_loader.rs`]. Safe but misleading. Implement validated reload or drop the watcher. `bind_address` comes from TOML, so any local process able to write it can rebind WHIP to `0.0.0.0` once enabled.
- **Auto-update:** none exists. The Tauri updater's signature check is mandatory and cannot be disabled; keep the private key offline [web: https://v2.tauri.app/plugin/updater/].
- **Planned Google OAuth:** loopback + PKCE + `drive.file` is the right pattern. Do it in the main process, bind `127.0.0.1` on an ephemeral port, verify `state`, time out, store only the refresh token in the keychain. Apps in OAuth "testing" status reportedly get 7-day refresh tokens [inf; verify], which would break a weekly workflow: plan for production verification.
- **Planned control API:** loopback only by default, random per-install token, WebSocket `Origin` check, no CORS `*`, rate limits, the same command allow-list as the UI (not raw bus access), off by default.
- **AV false positives / `VHO:Trojan-PSW`:** unsigned binary + credential APIs (`keyring`) + a fresh hash + a loopback server is the likely trigger [inf]. Mitigate with signing, submitting each release to Microsoft and Kaspersky, and no runtime-downloaded executables. Never tell users to add exclusions.
- **Signing/SmartScreen:** EV no longer bypasses SmartScreen; Azure Artifact Signing is "same as OV" and individual accounts are limited to USA/Canada; reputation builds per file hash [web: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options]. Expect warnings on early releases; sign every build; do not rebuild and re-sign unnecessarily. SignPath OSS needs a public repo (open decision #15).

**Change:** F-04/05/08, `#![forbid(unsafe_code)]`, cargo-fuzz targets (Unix + nightly only: use WSL/Docker [web: https://docs.rs/cargo-fuzz]), osv-scanner + fresh-DB cargo-deny in CI.

### 4.2 Third-party dependencies

| Dependency | Locked | Call | Notes |
|---|---|---|---|
| tauri / wry / tao | 2.12.4 / 0.57 / 0.37 | Keep | Remove unused `tauri-plugin-shell`. Keep `tauri-plugin-dialog`. Global-shortcut only if Escape stays global (F-11). |
| tokio 1.52, axum 0.8.9 / hyper 1.10 | | Keep | Tight feature set. No TLS (F-05). |
| zip 9.0.5, quick-xml 0.42 | | Keep, harden | Hand-rolled PPTX parser works; OOXML needs styles/rels/layouts for F1.7a, and `.docx` is the same toolchain. Add limits (F-04). |
| pdfium-render 0.8.37 + pdfium 157.0.8100 | | Keep with bump process | Best open choice. Memory-unsafe native code: isolate (F-02). Try 0.9.x [web]. BSD/Apache binary; licence file staged. |
| keyring 3.6.3 | | Keep | Persistence across restart still unverified (#2). Linux needs a Secret Service; minimal desktops have none, so you need a clear message or an encrypted-file fallback. |
| notify 6.1.1 | | Remove or replace | Feeds a notification nobody consumes; two majors old. Poll mtime every 2 s (display already polls) if reload is wanted. |
| gstreamer-rs 0.25.4 | | Keep, optional feature | GStreamer 1.28.x is a current line [web: https://gstreamer.freedesktop.org/releases/1.28/]. Core/base/good are LGPL; keep GPL-linked plugins (x264) out of a closed bundle [web: https://discourse.gstreamer.org/t/any-part-of-gstreamer-source-has-licenses-other-than-lgpl/597]. Pin a plugin allow-list. |
| gst-plugin-ndi + NDI runtime | | Spike, then decide | Redistribution allowed with conditions (current redistributable, licence terms in your EULA, link to ndi.video, vendor ID for commercial use) [web: https://docs.ndi.video/docs/sdk/software-distribution]. Safest solo policy: do not bundle; detect the runtime and link to NDI's download; SRT is the always-works path. |
| sysinfo, uuid, base64, qrcode | | Keep | |
| png 0.17 + 0.18, toml 0.8 + 1.1 | | Tidy | Duplicate majors in tree (cargo-deny only warns). |
| ts-rs 12 | | Use or drop | F-17. |
| glib 0.18.5, proc-macro-error | | Wait for Tauri | Linux-only; documented ignores are right. |
| serde_with 3.20.0 | | Bump | F-12. |
| React 19.3, Vite 8, TS 7, Vitest 5, Zustand 5, Biome 2 | | Keep | Modern and consistent. |
| PyMuPDF (tools only) | 1.24.* | Keep out of product | F-13. |

Notes: CLAUDE.md mentions `rust-embed`, which is not a direct dependency (docs drift). Licensing summary [inf, not legal advice]: Rust crates are MIT/Apache/MPL and the deny allow-list (incl. MPL-2.0) is fine. Items needing a lawyer or written policy: NDI SDK EULA; H.264/HEVC patents if software encoders are bundled (prefer NVENC/QSV/AMF/MediaFoundation/VideoToolbox); OpenH264 (Cisco's patent grant applies to Cisco-built binaries distributed on their terms); GStreamer LGPL relinking/notice duties; PyMuPDF if ever shipped.

Version skew: Rust 1.78 vs 1.96 (F-16); roadmap GStreamer "1.26" vs dev box 1.28.3 (open decision): choose 1.28.x for Windows/macOS bundles. Linux distros lag (Ubuntu 24.04 ships older GStreamer [inf]), so Linux must bundle (Flatpak/AppImage) or be "advanced".

### 4.3 Internal architecture

**Good**
- Clear modules, thin commands; the `PipelineBackend` trait (Simulated / Unavailable / Gst) makes CI and tests possible without the SDK; compositor selection is pure and tested; `PreviewHub` is rightly separate from the state bus (up to 64 frames/s would flood 256 slots) [code].
- Edge failures are considered: pdfium missing returns an error, keychain failure keeps WHIP closed, lost NDI sources are torn down and re-attached.
- Test depth is real: mock-runtime journeys, invariants parsed from repo files, TS/Rust event parity, fixture tests that caught the `Event::Empty` bug.

**Risky and what I would change**

1. **One broadcast bus for everything** (F-07). Broadcast suits notifications, not state. Split into (a) a Rust state store (the "spine": one struct, versioned, revisioned, persisted) publishing `StateChanged{rev}` with `snapshot()`; (b) telemetry; (c) frames. Frontends reconcile on `rev`. It also gives undo/revisions and the control API one read model. "No direct cross-module calls for state change" stays: the spine is the sanctioned path.
2. **Actor on `spawn_blocking`** is a reasonable single-owner pattern for GStreamer. It lacks deadlines, health and restart (F-09). If it moves to a sidecar, the actor goes with it and the main process holds a reconnecting client.
3. **Crash isolation contract (F-02):** engine heartbeat 1 s; `Lost` after 3 s; restart with backoff; replay the desired state from the spine; resume streaming only if the operator enabled auto-resume, else a one-click "Resume stream". Slides and projector never depend on the engine.
4. **Persistence:** four independent files (`streamdeck.toml`, `decks/<id>/deck.json`, `active_deck_id.txt`, `slide_state.json`), all `fs::write`, only `CONFIG_VERSION=1` versioned with no migration. Move to one atomic-write helper, a `schema_version` per artefact and a migration table, and SQLite (WAL) for the service-order document and its revisions; TOML stays for human-edited settings.
5. **Shutdown:** closing the controller calls `app.exit(0)` immediately, even while presenting or streaming (`app.rs` `Destroyed`). Add confirm-while-live and a state flush.
6. **`static PRESENTING`** and similar globals bypass the bus rule; fold into the spine.
7. **Two Vite projects with duplicated small files:** sound for real isolation (the presentation truly cannot invoke). Drift is covered by parity tests; improve with generated types (F-17), not a shared package. Keep.
8. **Zustand:** fine; with a revisioned snapshot the stores become thin caches.
9. **TOML + hot reload:** keep TOML for settings; implement validated reload or remove the watcher.
10. **What `verify.sh` proves and does not:** it proves compile, unit/mock-runtime journeys, string presence, event parity, repo invariants. It does not prove a built app launches, media renders (F-01), real-webview IPC works, installers install, GStreamer plugins exist on a target, or behaviour under packet loss, sleep, hotplug. CLAUDE.md already records five manual-launch regressions the gate missed; F-01 looks like a sixth.

**Most likely live-service failure modes** [inf from code]

| # | Failure | Today | Mitigation |
|---|---|---|---|
| 1 | Projector stale/blank (media path bug, dropped event) | likely (F-01, F-07) | fix F-01, rev + heartbeat, a Clear-to-focus-screen that needs no media |
| 2 | Native fault kills the app | possible | sidecar (F-02) |
| 3 | Venue Wi-Fi: WHIP flaps, NDI HX stutters | likely | wired Ethernet guidance, SRT fallback, one health chip |
| 4 | OS sleep, updates, notifications interrupt | likely | keep-awake while live, pre-service check, Focus Assist guidance |
| 5 | Operator closes window or presses Esc in another app | possible | confirm on close, F-11 |
| 6 | Bad config/deck after crash or power loss | possible | atomic writes, recovery (F-03) |
| 7 | Facebook RTMPS key rejected on the day | likely first time | a test-connection step before Go Live |

### 4.4 Is it modern and still the best option? (keep / change / spike)

| Bet | Call | Reasoning, cost of change | Source |
|---|---|---|---|
| Tauri v2 vs Electron / egui / Slint / iced / Flutter | **Keep** | Small binary, Rust where media lives, React for a forms-heavy operator UI and a rich-text editor. Electron gives one Chromium everywhere but costs ~100 MB+ and a Node main; Rust-native UIs lose editor tooling. The real Tauri risk is **Linux** (WebKitGTK/Wayland; NVIDIA/Wayland blank-window reports with a `WEBKIT_DISABLE_DMABUF_RENDERER=1` workaround): treat Linux as best-effort until proven on hardware. Switch cost later: L. | https://bugs.webkit.org/show_bug.cgi?id=280210 (snippet; Tauri CEF status not confirmed) |
| WebView2 install mode | **Change (config)** | Default `downloadBootstrapper` needs internet at install; use `embedBootstrapper` (+1.8 MB) or `offlineInstaller` (+127 MB) for churches with poor networks. | https://v2.tauri.app/distribute/windows-installer/ |
| GStreamer vs FFmpeg / libobs / MediaMTX / direct MediaFoundation+WGC | **Keep; spike a trimmed bundle** | One graph for NDI, WHIP, RTMP, SRT, compositor with an LGPL core. libobs is GPLv2 (reference only per your rule). An FFmpeg sidecar is simpler for RTMP/SRT + recording but weak at dynamic compositing. Danger is bundle size and plugin sprawl. Spike: minimal allow-list (coreelements, videoconvertscale, d3d12/d3d11, compositor, rtmp2, srt, mpegts, opus/aac, isomp4, ndi, webrtc); pass = installer < 150 MB, all plugins LGPL/MPL, cold start < 3 s. | GStreamer 1.28 notes; licensing thread above |
| d3d12compositor vs a wgpu Rust mixer | **Keep both, do not build wgpu** | Your measurements: d3d12 ~49.5 ms vs CPU ~33 ms for 3 inputs (unverified #6). A wgpu mixer is a multi-month project for no church-visible gain. Redefine the criterion as end-to-end < 66 ms, not < 33 ms. | CLAUDE.md F3.1 |
| Slides as injected frames vs screen capture | **Keep injection** | Capture adds permissions, DRM black frames and Wayland portal friction. Spike the render path: offscreen webview vs native raster; pass = 1080p, < 100 ms Take-to-composite, pixel-identical to the projector. | roadmap proposal |
| NDI|HX vs SRT/RTMP/WHIP for cameras | **Spike, SRT default** | NDI is Mevo Start/Core only, proprietary runtime, Wi-Fi sensitive. Hardware tests #1/#17 decide; do them before more NDI code. | `unverified_items.md` |
| WHIP/WebRTC for Facebook/YouTube egress | **Change: RTMPS first** | Facebook's Live API requires RTMPS; I found no evidence Facebook accepts WHIP. WHIP is good for phone ingest and for CDN/RTC vendors. Your order (RTMP/SRT before WHIP) is right. | https://developers.facebook.com/documentation/live-video-api/overview ; https://www.forasoft.com/blog/article/whip-whep-replace-rtmp-live-streaming-2026 (vendor blog, secondary) |
| AV1/HEVC egress | **Defer** | Platform RTMP acceptance unverified; H.264 CBR 1080p30 via hardware encoder is the safe default. | [not verified] |
| GPU/driver handling | **Change (policy)** | Auto-pick NVENC > QSV > AMF > MediaFoundation, software only if licensing allows; persist and show the choice in the pre-service check. | [inf] |
| Linux Wayland/PipeWire | **Spike, defer the promise** | Frame injection removes capture; display enumeration and fullscreen on Wayland remain risks. | [inf] |
| Auto-update | **Add Tauri updater** | Mandatory signature, TLS enforced, `passive` on Windows; anti-downgrade by default so rollback = ship a higher version containing old code. Never expose `allowDowngrades` to the webview (CVE-2026-95624). | https://v2.tauri.app/plugin/updater/ |

### 4.5 Deployment and distribution

**Today** [code]: PDFium staging with SHA-256 verification (good); `check-bundle.sh` asserts required files, no `.pdb`/fixtures, size budget (Windows 10 MB); one NSIS build verified at 6.8 MB; Linux never run; no macOS; no GStreamer or NDI in the bundle; `bundle.targets: "all"`.

**Recommended design (one developer)**
- **Windows only for the first year.** NSIS per-user (`currentUser`, no admin; updater `passive`). Skip MSI unless an organisation asks; winget/MSIX later. `webviewInstallMode: embedBootstrapper`.
- **GStreamer:** ship a trimmed, relocatable runtime in an `engine/` folder owned by the sidecar (also satisfies LGPL "replaceable DLL" expectations). Do not make users install the MSVC runtime. Budget 80-120 MB [inf; measure in the spike].
- **NDI runtime:** detect `NDI_RUNTIME_DIR_V*`, link to the official download, never redistribute NDI tools.
- **macOS:** needs an Apple Developer account, hardened runtime, notarisation, camera/mic/local-network entitlements. Postpone; claim nothing until a Mac has run it.
- **Linux:** a `.deb` cannot carry a GStreamer 1.28 requirement. Candidate: Flatpak (runtime + PipeWire portals) or AppImage with bundled GStreamer. Spike on Ubuntu 24.04 and one Wayland machine (unverified #8).
- **Updates and rollback:** `stable`/`beta` channels; promote only after a smoke test on a clean VM. Keep the previous installer and a pre-migration backup of the data dir; migrations are forward-only with backup; "Restore previous version" reinstalls the stored installer. Never auto-install while presenting or live; offer "install after the service" and a "freeze updates until <date>" toggle.
- **Diagnostics:** local-only by default. On abnormal exit write `diagnostics.zip` (versions, last 500 log lines, GStreamer plugin list, displays, config with secrets removed) that the user can email. No network telemetry without explicit opt-in. `tracing` with rolling files; panic hook. (`crash-handler`/`minidumper` crates are options; not evaluated.)
- **First run "Doctor":** WebView2, engine loads, PDFium loads, NDI runtime, Windows Firewall rule for WHIP (the first bind prompts and often needs admin), displays; each with a fix action.
- **Offline:** local-first today; keep Drive/OAuth optional and cache the last exported deck.
- **CI/CD:** `release.yml` on tag: build on `windows-2022`, `tauri build --config release/bundle.*.conf.json`, `check-bundle.sh`, sign, SBOM (`cargo cyclonedx`, `npm sbom`), provenance attestation, draft release, `latest.json` for the updater. Pin actions to SHAs; add `--locked`.
- **Key custody:** updater private key offline (hardware token or password manager + paper copy); losing it strands every installed copy [web: updater docs]; the public key can be set at runtime for rotation.
- **Notices:** generate `THIRD-PARTY-NOTICES.txt` in CI (`cargo about`, npm licence checker); add to the bundle and `check-bundle.sh` REQUIRED.
- **Upgrade the night before:** (1) auto-download only; (2) release notes and "now / after the service" on next launch; (3) pre-update data snapshot; (4) pre-service check re-runs and blocks Go Live until reviewed; (5) visible "restore previous version". A product feature, not a script.

### 4.6 Scripts and tooling

| Item | Assessment | Change |
|---|---|---|
| `scripts/verify.sh` | Real cargo/clippy/fmt/typecheck/vitest work plus ~257 grep presence checks. Git-Bash-bound (`/tmp`, `cygpath`, `sha256sum`, `stat -c`). `set -euo pipefail` on; one fixed log path so concurrent runs clobber. | Thin wrapper over `just`/`xtask`; drop redundant greps; add nextest, `cargo deny`, osv-scanner, `--locked`. |
| `check-bundle.sh` | Good design (extract, never execute; forbidden names; budget). GNU-only `stat -c`/`du -sb`. | Extend REQUIRED with GStreamer + notices; make portable for macOS. |
| `fetch-deps.sh` | Strong: pinned SHA-256, hard fail, idempotent, extract-only. | Scheduled PR to propose new tags + hashes. |
| `dev-both.cjs`, `merge-presentation-dist.cjs` | Small and purposeful. | Verify child processes die on Windows exit; one test for the merge. |
| `uia-dump.ps1` | Valuable stopgap for real-app smoke. Windows only. | Replace by Playwright-over-CDP (T4.x); first assertion: F-01. |
| `.claude` hooks | Pre-push gate is a good idea. Matches stdin text; fails open on `cd`. | Parse JSON `command`; keep fail-closed deny. |
| `tools/service-analyzer` | Useful offline research tool. Unpinned image/deps; handles the church's real documents. | Pin digest + hashes; README/.gitignore saying inputs are never committed; do not port PyMuPDF code. |
| `ci.yml` | Sensible: concurrency cancel, dependabot-aware matrix, cargo-deny job, fmt, clippy `-D warnings`. | `--locked`, SHA pins, osv-scanner, nextest, fresh advisory DB. macOS off is right. |

**Add:** cargo-nextest; cargo-fuzz targets for `parse_pptx_slide`, rels parsing, `SlideDeck::load_from_json`, WHIP `select_codec` (SDP); `proptest` for `validate_layout`/`pad_properties`; golden-image slide tests; Playwright-over-CDP import-render-present E2E; `cargo-mutants` on the pure state machines as a periodic job.

### 4.7 Product-architecture fit

The new plan needs foundations the current code does not have:

| Need | Gap today | Do first |
|---|---|---|
| Typed block document with variants, audience flags, cues | `SlideDeck`/`SlideEntry` is a flat render model (title/body/rich_body); no source/render split | `ServiceDocument` (blocks, variants, choices, cues) + pure `render(document, profile) -> Vec<Slide>`; the current deck becomes one output. |
| Revisions, undo, edit-vs-live concurrency | No revision concept; `deck_import` replaces the live deck immediately | Immutable revisions in SQLite; **Program** holds a revision pointer; edits create a new revision; Take swaps the pointer (the same mechanism as Preview/Program for scenes). |
| Design/Show modes, multi-window surfaces | One controller window; capabilities per window label | A second labelled window with its own capability, both attached to the same Rust spine; the Show lock is enforced in Rust (commands refused), not only in the UI. |
| Schema migration | `CONFIG_VERSION=1`, no migrations; `deck.json` unversioned | Version every persisted artefact now, before real user data exists. |
| Scene Take | `set_layout` exists, no staged vs committed distinction | `desired` (preview) vs `applied` (program) layouts in the spine; the engine applies on Take only. |
| Control API | Tauri-only commands | Transport-neutral application service behind Tauri IPC, local WebSocket and Companion. |
| Drive import | Import takes a path | Importers return `ImportResult{document, warnings, loss_report}`; Drive only supplies bytes. |
| Privacy flags | n/a | Enforce at render time per output (projector/stream/print) with a visible "hidden from stream" marker in Design. |

**Over-scoped for one developer (cut or defer)**
1. F1.9 learned readability profile and room calculator: ship a fixed minimum-font-size warning first.
2. Drive import: after `.docx`/PDF/PPTX importers and after the OAuth verification question is answered.
3. Control API, Companion and stage display: after the first complete service run.
4. Free-form scene editor (F3.3b): start with three fixed layouts (full, side panel, picture-in-picture).
5. Linux and macOS: promise nothing until Windows has run a real service.
6. U3 (usability prototype) is cheap and high value: do it in Figma or static HTML before building Show.

---

## 5. State-of-the-art comparison (key bets, with sources)

| Bet | 2026 status | Verdict | Sources |
|---|---|---|---|
| Tauri v2.12 | Active; advisories fixed in versions you use | Keep | https://v2.tauri.app/ ; CVE trackers in 4.1 |
| WebView2 install | Offline/embedded options exist | Embed or offline | https://v2.tauri.app/distribute/windows-installer/ |
| Tauri updater | Signature mandatory, TLS enforced | Adopt | https://v2.tauri.app/plugin/updater/ |
| Windows signing | EV not instant; Artifact Signing individuals US/CA only | OV or SignPath; expect early warnings | https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options |
| GStreamer 1.28.x | Current series; snippets disagree on latest patch | Baseline 1.28 | https://gstreamer.freedesktop.org/releases/1.28/ |
| GStreamer licensing | Core/base/good LGPL; x264 pulls GPL | Avoid x264 in a closed bundle | https://discourse.gstreamer.org/t/any-part-of-gstreamer-source-has-licenses-other-than-lgpl/597 |
| NDI distribution | Allowed with conditions; vendor ID for commercial use | Detect runtime, do not bundle | https://docs.ndi.video/docs/sdk/software-distribution |
| Facebook Live ingest | RTMPS required; no WHIP evidence found | RTMPS first | https://developers.facebook.com/documentation/live-video-api/overview |
| WHIP | Standardised; offered by CDN/RTC vendors per a vendor blog | Keep as phone ingest | https://www.forasoft.com/blog/article/whip-whep-replace-rtmp-live-streaming-2026 |
| pdfium-render | 0.9.4 available; targets pdfium 7881 | Test upgrade | https://github.com/ajrcarey/pdfium-render |
| PDFium security | Several 2026 Chrome-reported PDFium use-after-free/overflow CVEs (snippets, unconfirmed) | Isolate; bump monthly | SentinelOne CVE-2026-11304..11307 pages; confirm on NVD |
| WebKitGTK/Wayland | NVIDIA/Wayland blank-window reports with env workaround | Linux best-effort | https://bugs.webkit.org/show_bug.cgi?id=280210 |
| cargo-fuzz | Unix + nightly only | Run in Docker/WSL | https://docs.rs/cargo-fuzz |

Could not verify: AV1/HEVC acceptance on Facebook/YouTube RTMP in 2026; the current latest GStreamer 1.28 patch; Tauri CEF runtime status; whether `VHO:Trojan-PSW` is linked to `keyring` (inference).

---

## 6. Prioritised plan (one developer, ordered to avoid rework)

### Next 2 weeks (stop the bleeding, no new features)
1. **Verify F-01** with a release build (import a PDF and a PPTX, push an image and a video, confirm they render on the presentation window). Fix with a custom URI scheme; add the E2E assertion. (S-M)
2. Capability cleanup (F-08) and extend `invariants.rs`. (S)
3. Atomic write helper and recovery for `streamdeck.toml`/`deck.json`/`slide_state.json` (F-03); fix the timer (F-10); confirm-on-close while presenting/live. (S-M)
4. Importer limits and stage-then-confirm (F-04); one cargo-fuzz target in Docker. (M)
5. WHIP defaults (F-05): random token on first enable, bind the selected IP, reject browser `Origin`. (S-M)
6. Dependency hygiene: bump `serde_with`, prune stale deny ignores, add osv-scanner, `--locked`, nextest; set `rust-version`. (S)
7. Run the cheap P0 checks: keychain persistence (#2), installed app on a clean user/VM (#4), Mevo NDI test (#1/#17) to settle NDI vs SRT.

### Next 2 months (foundation, then a thin vertical slice)
1. **The spine:** Rust application state with `rev`, snapshot, heartbeat, versioned persistence, SQLite document store with revisions; presentation reconciles on `rev`; handle `BusLag` (F-07).
2. **Sidecar media engine** with heartbeat/restart/replay; move pdfium import into it (F-02, F-09). Keep the `PipelineBackend` trait; the sidecar is a transport behind it.
3. `release.yml`: NSIS per-user, signing, SBOM, provenance, updater `latest.json`, notices, `check-bundle.sh` additions; first signed internal release; submit to Microsoft and Kaspersky (F-06, F-19).
4. Service-order document MVP: typed blocks, variants, projector output first, `.docx` and PDF import via pdfium text objects, focus screen and Clear (F1.7/F1.8); usability prototype (U3) before Show mode.
5. RTMPS egress to Facebook plus local recording, a test-connection step and the pre-service check (U2).
6. Playwright-over-CDP smoke (local/nightly) and golden-image slide tests.

### Later
- Scene editor beyond fixed layouts, secondary PDF panel, stage display, audio (F3.5).
- Control API + Companion; Drive import after OAuth verification.
- macOS (Apple account, notarisation) and Linux (Flatpak spike) once Windows has run real services.
- winget/MSIX; opt-in diagnostics only.
- `pdfium-render` 0.9 and monthly pdfium bump automation if not done earlier.

---

## 7. Keep / change / spike summary

| Choice | Call |
|---|---|
| Tauri v2, React/TS, two Vite projects, Zustand | Keep (generate event types) |
| Event bus as the only cross-subsystem channel | **Change**: bus for notifications; spine + rev/snapshot for state |
| In-process pipeline actor | **Change**: sidecar process |
| TOML for settings | Keep, with atomic writes, recovery, versions |
| JSON files for deck/state | **Change**: versioned SQLite for documents and revisions |
| GStreamer | Keep; **spike** a trimmed Windows bundle and licence set |
| d3d12compositor / CPU compositor | Keep both; no wgpu mixer; redefine latency criterion |
| Frame injection for slides | Keep; spike the render path |
| NDI | **Spike** (hardware test); optional runtime; SRT default |
| WHIP server | Keep as optional phone ingest; harden |
| RTMPS to Facebook | Do first |
| PyMuPDF analyzer | Tool only; port logic to pdfium |
| verify.sh greps | Trim; rely on real tests |
| Updater / signing / release pipeline | **Add** |

---

## 8. Open questions for the owner, and what I could not verify

**Open questions**
1. Repo public or private? (SignPath OSS needs public; Artifact Signing for individuals is limited to USA/Canada.) Where are you based?
2. Is Windows-only for the first release acceptable? What OS do the volunteers actually use?
3. Is a full-app crash with recovery under 10 s acceptable, or must the projector survive engine failure? (Decides sidecar priority.)
4. Will the church tolerate an update prompt at all, or is manual update with a notice safer?
5. Who authors the order of service and in which tool (Word assumed)? Decides `.docx` vs PDF importer priority.
6. Is Facebook streaming through a personal page, a Page or a group, and do you have Live API access or only a stream key?
7. Do intercession names or bank details ever sit in the same document as projector text (privacy-flag defaults)?
8. Do you accept NDI as "detect runtime, link to download" rather than bundled?

**Could not verify / not done**
- Whether F-01 actually fails in a built app (I did not run it). Something I did not read may handle the paths.
- Full text of the `serde_with` and `glib` advisories, and why local cargo-deny missed them (stale DB, ignore list or target filtering); I did not run `cargo deny fetch`.
- Tauri 2026 CVE details (third-party trackers disagree on ranges).
- `cargo audit`, trivy, Semgrep, cargo-geiger not run.
- Read in detail: `whip_server.rs`, auth and lifecycle parts of `whip.rs`, `deck_importer.rs`, `app.rs`, `state.rs`, the `pipeline.rs` actor, `config*.rs`, `bus.rs`, `events.rs`, `commands/{deck,scene,presentation,ndi,whip}.rs`, capabilities, CI, scripts and the roadmap addendum. Not read in detail: most of `whip.rs` (1,717 lines), `compositor.rs`, `preview.rs`, `ndi.rs`, `display.rs`, frontend components and most Rust integration tests.
- GStreamer bundle size, start-up time and plugin licences were not measured.
- Linux and macOS behaviour: nothing has run there.
- AV1/HEVC egress acceptance, root cause of Windows AV flags, and Google OAuth testing-mode token expiry (inference).
