# Other open-source GitHub projects relevant to a Tauri + Rust + GStreamer live-production app

Scope note: research was capped at ~15 tool calls. Several GitHub pages rendered without release/version data; those items are listed under Gaps rather than asserted. Several areas (wgpu mixers, FreeShow, LibreOffice PPTX rendering, hlssink3/RIST, Larix) were NOT researched.

## 1. Per-repo profile (activity / license / what it does / adopt-or-beware)

### Takeaway
MediaMTX (MIT, very active, WHIP/SRT/MoQ) is the best drop-in relay and ingest reference; CasparCG (HTML producer on CEF, GPU mixer) is the best reference for graphics playout; Companion is the integration target for control; Cap is the closest Tauri+Rust analog but is AGPL; Sunshine, OpenLP, obs-websocket and DistroAV are GPL, so reuse of their code is risky and they should be treated as references or separate processes.

### Cited Findings
- CasparCG Server: latest listed is v2.5.1 Stable (16 Sep, year not shown on page; it removed the AVX2 requirement except v210 DeckLink output, updated FFmpeg to 8.1.2, added SFML 3 on Linux). v2.5.0 (10 Dec) added initial HDR, updated the HTML producer to CEF 142 with audio support, and reimplemented mixer transforms to handle routes correctly. v2.4.0 added an interlaced mixer pipeline for field accuracy and 4K DCI/8K frames. — [CasparCG releases](https://github.com/CasparCG/server/releases)
- CasparCG 2.5 headless Linux operation (no X server) requires 2.5.0 or later. — [CasparCG docs](https://casparcg.com/docs/guides/linux-headless)
- MediaMTX: MIT license, ~20.5k stars; supports SRT, WebRTC, RTSP, RTMP, LL-HLS, MPEG-TS, RTP, Media-over-QUIC; records to fMP4/MPEG-TS; runs external hook commands on connect/disconnect/read/publish; has a Control API and Prometheus-format metrics. — [MediaMTX repo](https://github.com/bluenviron/mediamtx)
- MediaMTX release cadence is roughly every 2-3 weeks: v1.21.2 on 08 Oct (2026 inferred from page footer), v1.18.2 on 15 May. v1.18.2 added WHIP ICE restarts and support for servers without trickle ICE; v1.19.0 introduced MoQ read/publish; OpenAPI definition largely auto-generated from v1.19.3. — [MediaMTX releases](https://github.com/bluenviron/mediamtx/releases)
- Bitfocus Companion: ~2.3k stars, ~10k commits, TypeScript/Node monorepo with a `satellite-protocol` folder and "700+" supported devices/software; repo last pushed 2026-08-07 per a mirror. Releases are served via Bitfocus's own servers, not GitHub Releases (Winget lists 4.1.4-4.3.2; a community-script page mentions v5.0.3 so a 5.x line exists, version/date not confirmed by a primary source). — [Companion repo](https://github.com/bitfocus/companion), [mirror](https://gittrend.io/repo/bitfocus/companion), [community script page](https://community-scripts.org/scripts/bitfocus-companion)
- obs-websocket: GPL-2.0, default port 4455, bundled with OBS Studio 28+; clients include Bitfocus Companion and a Rust client library `obws`. — [obs-websocket](https://github.com/obsproject/obs-websocket)
- DistroAV (formerly obs-ndi): GPL-2.0, ~4.6k stars, requires NDI Runtime 6.3+ and OBS 31.1.1+; provides NDI Source, Output and Filter. — [DistroAV](https://github.com/DistroAV/DistroAV)
- Cap (screen recorder): Tauri v2 desktop app with SolidStart UI and Rust crates for capture, camera, audio, encoding, rendering, muxing and export; ~23.5k stars; licensing is split: `cap-camera*` and `scap-*` crates MIT, everything else AGPLv3. — [Cap repo](https://github.com/CapSoftware/Cap)
- Sunshine: GPL-3.0, ~42.5k stars; encoders by platform: Windows AMF/Media Foundation/NVENC/QuickSync/Software, Linux NVENC/VAAPI/Vulkan Video/Software, macOS VideoToolbox/Software; capture via DXGI, WGC, KMS/DRM, PipeWire-style portals, ScreenCaptureKit. — [Sunshine](https://github.com/LizardByte/Sunshine)
- OpenLP: GPL-3.0-or-later, hosted on GitLab, 18k+ commits, 23 releases, project created 16 Sep 2019 on GitLab. — [OpenLP GitLab](https://gitlab.com/openlp/openlp)
- LiveKit server: Apache-2.0, ~21.7k stars, Go + Pion SFU, single-binary deploy, simulcast/SVC, JWT, webhooks; Ingress and Egress are separate companion projects. — [LiveKit](https://github.com/livekit/livekit)
- voctomix: MIT, ~636 stars, Python + GStreamer "Full-HD software live-video-mixer" split into `voctocore` and `voctogui`; repo has CI and `uv`/pytest/mypy tooling; 97 open issues. — [voctomix](https://github.com/voc/voctomix), [Debian package description](https://packages.debian.org/sid/voctomix)
- GStreamer 1.28.0 (27 Jan 2026) notes list webrtcsink renegotiation, VA hardware encoder support, and a WebRTC WHEP client and server signaller; 1.28.1 improved H.264 profile/level negotiation in webrtcsink and added custom signalling headers and request pads in webrtcsrc; 1.28.3 added i.MX8MP H.265 hardware encoder support. Latest point release conflicts across mirrors (1.28.4 June vs 1.28.6 5 Aug). — [GStreamer 1.28 notes](https://gstreamer.freedesktop.org/releases/1.28/), [1.28.0 announcement](https://discourse.gstreamer.org/t/gstreamer-1-28-0-new-major-feature-release/5700)

### Inferences
- MediaMTX's own Prometheus metrics, hooks and API make it a plausible optional sidecar (MIT) for relay/fan-out and for monitoring ingest, without linking code into the app.
- Cap's MIT-licensed `scap-*`/`cap-camera*` crates are the only reusable capture pieces; the rest is AGPL and should be reference-only.
- CasparCG's CEF-based HTML producer with audio (2.5.0) shows that HTML is the mainstream way to produce graphics for a broadcast mixer.
- Companion supports OBS via obs-websocket; a StreamDeck-app module or generic HTTP/satellite API would be the integration path (not verified from primary docs).

### Gaps
- Licenses for CasparCG Server (believed GPL-3 from memory) and Companion (believed MIT from memory) were not confirmed from fetched pages.
- Star counts/last release for CasparCG, OpenLP, DistroAV not obtained; GitHub page captures often omitted version info.
- Not researched: FreeShow, wgpu-based Rust compositors, Larix, Janus, Moonlight, Singular/overlay-controller projects, LibreOffice headless, SRT/RIST/hlssink3 specifics.
- gst-plugins-rs README and whipserversrc docs were blocked (Anubis access-denied page); whipserversrc status in 1.28 not confirmed.
- No FOSDEM/GStreamer Conference talk summaries were retrieved.

## 2. (a) Getting slides/graphics into the pipeline: HTML overlay vs native render vs capture

### Takeaway
Evidence gathered supports HTML (CEF) as the established approach for broadcast graphics (CasparCG), but no direct comparison data was found.

### Cited Findings
- CasparCG's HTML producer was moved to CEF 142 and gained audio support in 2.5.0. — [CasparCG releases](https://github.com/CasparCG/server/releases)
- Cap renders in Rust crates (render/encode/export) inside a Tauri v2 app, showing native Rust rendering alongside a webview UI; details of its renderer (wgpu etc.) were not on the page. — [Cap repo](https://github.com/CapSoftware/Cap)

### Inferences
- For StreamDeck, the existing presentation webview could be captured, or slides could be rasterised once (as the deck importer already does) and pushed as frames; CasparCG's design suggests HTML overlays for lower-thirds and native frames for slide images.

### Gaps
- No benchmarks or primary sources on webview capture latency, OBS browser-source internals, or offscreen CEF vs WebView2 capture.

## 3. (b) Scene/transition engines and frame-accurate transitions

### Takeaway
Thin evidence: CasparCG's mixer was reworked for route handling and interlaced field accuracy; voctomix is a GStreamer mixer with a core/GUI split.

### Cited Findings
- CasparCG 2.4.0 added an interlaced mixer pipeline for field accuracy; 2.5.0 reimplemented mixer transforms to handle routes correctly. — [CasparCG releases](https://github.com/CasparCG/server/releases)
- voctomix separates `voctocore` (mixing process) from `voctogui`. — [voctomix](https://github.com/voc/voctomix)
- GStreamer 1.28.1 added request-type pads in webrtcsrc for encoded downstream data. — [GStreamer 1.28 notes](https://gstreamer.freedesktop.org/releases/1.28/)

### Inferences
- voctomix's core/GUI separation matches StreamDeck's actor/bus separation; its composite/transition protocol was not verified.

### Gaps
- Voctomix composite/transition mechanism, Move transition plugin, and any wgpu transition engines not researched.

## 4. (c) Hardware encoder auto-selection and fallback

### Takeaway
GStreamer 1.28 webrtcsink gained VA encoder support; Sunshine documents a per-platform encoder matrix with Software as universal fallback, but its selection code was not inspected.

### Cited Findings
- webrtcsink in 1.28.0 supports VA hardware encoders; 1.28.3 added the i.MX8MP vpuenc_hevc encoder. — [GStreamer 1.28 notes](https://gstreamer.freedesktop.org/releases/1.28/)
- Sunshine's supported encoders per OS (see section 1); Software is the only encoder supported everywhere. — [Sunshine](https://github.com/LizardByte/Sunshine)

### Inferences
- A platform table (NVENC/AMF/QSV on Windows, VA on Linux, VideoToolbox on macOS, x264/openh264 fallback) mirrors what Sunshine documents and is a sensible baseline for the compositor encoder stage.

### Gaps
- Actual probe-and-fallback logic in Sunshine/OBS (GPL; read for ideas only) not examined.

## 5. (d) Phone-as-camera WHIP/WebRTC ingest

### Takeaway
MediaMTX keeps improving WHIP (ICE restarts in v1.18.2); GStreamer has WHEP signaller work in 1.28. Larix was not researched.

### Cited Findings
- MediaMTX added WHIP ICE restarts and non-trickle-ICE server support in v1.18.2. — [MediaMTX releases](https://github.com/bluenviron/mediamtx/releases)
- MediaMTX lists WebRTC clients using WHIP and WHEP playback in its support table; SRT cameras/servers supported. — [pkg.go.dev search result summary via MediaMTX docs](https://github.com/bluenviron/mediamtx) (the repo page itself did not name WHIP; WHIP support is confirmed by the release notes above)
- A secondary source says OBS has shipped native WHIP output since v30 and FFmpeg 8.0 gained a WHIP muxer. — [DEV Community tutorial](https://dev.to/masonwritescode/ship-a-go-live-button-browser-whip-publishing-with-mediamtx-no-sdks-4kfp) (secondary; verify)

### Inferences
- ICE-restart handling matters for phones switching Wi-Fi/cellular; the app's `LinkMachine` grace period addresses the same issue.

### Gaps
- Larix Broadcaster status/WHIP support, Janus, and whipserversrc maturity unverified.

## 6. (e) Remote control APIs and Companion integration

### Takeaway
Companion is the dominant control-surface ecosystem with a satellite protocol and 700+ integrations; OBS exposes a documented WebSocket API.

### Cited Findings
- Companion repo contains `satellite-protocol`, `module-local-dev`, `webui`, `launcher` folders and lists 700+ supported devices/software. — [Companion repo](https://github.com/bitfocus/companion)
- obs-websocket 5.x on port 4455 with PROTOCOL.md and clients in many languages, including Rust (`obws`) and Companion. — [obs-websocket](https://github.com/obsproject/obs-websocket)

### Inferences
- Exposing a small, documented WebSocket/HTTP API (event-bus mirror) plus a Companion module would let existing Stream Deck hardware drive the app; obs-websocket is a design template (request/response + events).

### Gaps
- Companion HTTP/OSC/TCP remote API and module-base SDK not confirmed from docs; ProPresenter API docs fetch returned 404.

## 7. (f) Multi-output and stream health monitoring

### Takeaway
MediaMTX exposes Prometheus metrics and an OpenAPI Control API; LiveKit Egress is a separate project for multi-destination.

### Cited Findings
- MediaMTX: Prometheus-compatible metrics, Control API, hooks, recording. — [MediaMTX repo](https://github.com/bluenviron/mediamtx)
- LiveKit lists Egress and Ingress as separate companion projects (details not on page). — [LiveKit](https://github.com/livekit/livekit)

### Inferences
- A GStreamer `tee` to multiple rtmp/srt sinks inside the app, or a local MediaMTX fan-out, are both viable; no source compared them.

### Gaps
- LiveKit Egress multi-RTMP behaviour, SRT/RIST stats, health-metric design in OBS not researched.

## 8. (g) PPTX/PDF import fidelity

### Takeaway
No findings; OpenLP's presentation handling was not retrievable.

### Cited Findings
- OpenLP supports presentations among its content types (page stated this at a high level only). — [OpenLP GitLab](https://gitlab.com/openlp/openlp)

### Inferences
- None supportable.

### Gaps
- OpenLP's PowerPoint/Impress/PDF integration, LibreOffice headless rendering, pptx libraries: not researched.

## 9. Licensing risks

### Takeaway
Treat GPL/AGPL projects as references or separate processes; prefer MIT/Apache components.

### Cited Findings
- MIT: MediaMTX, voctomix, Cap's `scap-*`/`cap-camera*` crates. — [MediaMTX](https://github.com/bluenviron/mediamtx), [voctomix](https://github.com/voc/voctomix), [Cap](https://github.com/CapSoftware/Cap)
- Apache-2.0: LiveKit server. — [LiveKit](https://github.com/livekit/livekit)
- GPL-2.0: obs-websocket, DistroAV. GPL-3.0: Sunshine, OpenLP (3.0 or later). AGPLv3: most of Cap. — [obs-websocket](https://github.com/obsproject/obs-websocket), [DistroAV](https://github.com/DistroAV/DistroAV), [Sunshine](https://github.com/LizardByte/Sunshine), [OpenLP](https://gitlab.com/openlp/openlp), [Cap](https://github.com/CapSoftware/Cap)

### Inferences
- Bundling GPL/AGPL code into a distributed binary would impose copyleft on StreamDeck; running MediaMTX (MIT) as a sidecar or talking to OBS/Companion over their network APIs avoids this.
- NDI SDK redistribution terms are separate from the GPL (DistroAV page states none); consistent with the repo's "real NDI needs the SDK" note.

### Gaps
- CasparCG and Companion licenses not confirmed from fetched pages; legal reading of "separate process" boundary not sourced.
