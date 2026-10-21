# OBS Studio (github.com/obsproject/obs-studio) as a live-production reference

Research date: 2026-10-21. Coverage is partial: the research budget was spent mostly on release notes, backend-design docs and obs-websocket. Several key questions are mostly or wholly gaps (listed under each section). Nothing here was verified by reading libobs source files directly; "verified from docs" means read from the official docs or release pages via fetch.

## Feature inventory as of 2026 (latest release, scenes/sources/transitions/audio/outputs/remote control)

### Takeaway
Latest stable seen is 32.2.2 (14 Aug 2026), with 33.0.0 in release-candidate stage (RC3 on 20 Oct 2026). 2026 work focuses on UI overhaul (audio mixer, add-source dialog), plugin management, WebRTC simulcast, and platform maintenance. Most of the classic feature list (transitions, filters, studio mode, replay buffer, virtual camera) was NOT re-verified in this pass and is only inferred from the obs-websocket API surface.

### Cited Findings
- Release timeline: 32.2.0 (21 Jul), 32.2.1 (24 Jul), 32.2.2 (14 Aug, marked Latest); 33.0.0 Beta 4 (25 Sep) through RC3 (20 Oct). Year is inferred from asset timestamps, not printed in headers. — [GitHub releases](https://github.com/obsproject/obs-studio/releases)
- 32.0.0: basic plugin manager; OBS refuses to load plugins built for a newer release; Hybrid MP4/MOV out of beta and default container for new profiles (adds ProRes on macOS, HEVC/H.264 + PCM on all platforms); default bitrate raised 2500 -> 6000 kbps; experimental Metal renderer for Apple Silicon; opt-in crash-log upload on Windows/macOS; NVIDIA RTX Audio Effects voice activity detection; macOS screen capture above 60 fps; custom OBS widgets "in preparation for larger UI updates". — [32.0.0 release](https://github.com/obsproject/obs-studio/releases/tag/32.0.0)
- 32.1.0: WebRTC simulcast support; audio mixer overhaul (default vertical layout, layout toggle, pinning sources, audio monitoring toggle, sources shown in Studio Mode preview, hidden/out-of-scene sources displayable); partial Canvas support in obs-websocket; undo/redo for scene-item changes (scale filtering, blend mode, deinterlacing); improved security for browser sources using local files. 32.1.2 dated 21 Apr 2026. — [32.1.0 release](https://github.com/obsproject/obs-studio/releases/tag/32.1.0); [web search summary](https://obsproject.com/welcome)
- 32.2.0: overhauled "add source" flow with a new window showing preview thumbnails; plugins can set custom icons for new source types; dynamic bitrate for multitrack video; multitrack config limited to Custom service; NVIDIA SDK 13 (driver 570+); macOS 12 dropped (stay on 32.1.2); macOS Intel builds forced to Apple Silicon build on Apple Silicon Macs; VAAPI AV1 over WHIP fixed. — [releases page](https://github.com/obsproject/obs-studio/releases); [welcome page via search](https://obsproject.com/welcome)
- obs-websocket 5.x API surface (confirms existing features): scenes, preview scene (studio mode only), transitions incl. T-bar position and studio-mode transitions, inputs (volume/mute/monitor type/audio tracks/sync offset), filters, scene items (transform/blend mode), media inputs, virtual camera, replay buffer (start/stop/save/last replay), record (pause/resume/split/chapters), stream, projectors, hotkeys, canvases (5.7.0). — [obs-websocket protocol.md](https://raw.githubusercontent.com/obsproject/obs-websocket/master/docs/generated/protocol.md)
- Repo is GPL-2.0-or-later; top-level dirs include libobs, frontend, plugins, libobs-d3d11, libobs-metal, libobs-opengl, libobs-winrt; 828 issues and 340 PRs shown in tabs (open vs total not labelled). — [GitHub repo](https://github.com/obsproject/obs-studio)
- Composable themes (base + variants, "Appearance" tab) arrived in 30.2; hybrid MP4 beta and chapter markers via hotkey/API also 30.2. — [30.2 release notes](https://obsproject.com/blog/obs-studio-30-2-release-notes)

### Inferences
- Add-source thumbnails (32.2), audio mixer rework (32.1) and "custom widgets" (32.0) suggest a multi-release UI modernization aimed at reducing operator complexity.
- Studio Mode, replay buffer, virtual camera and transitions (incl. stinger/Move via plugin) exist, but this is from API surface and background knowledge, not freshly sourced.

### Gaps
- Not verified this pass: full source type list (browser source, media, text, NDI via DistroAV plugin), stinger/Move transition details, filter list, hotkey system, scripting (Lua/Python), plugin ecosystem size. 31.0/31.1 release notes (fetch 404) were not read.

## Architecture (libobs, graphics, threads, encoders, capture, plugin API, storage, UI)

### Takeaway
libobs spawns a graphics thread, a video thread and an audio thread; sources form a tree rendered to a final texture which is converted to YUV and fed to encoders, with an interleave queue ordering packets. Graphics backends are D3D11, OpenGL and (new, experimental) Metal.

### Cited Findings
- Three primary threads: graphics thread (`obs_graphics_thread`, libobs/obs-video.c) renders previews and the final mix; video thread (`video_thread`, media-io/video-io.c) handles video encoding/output; audio thread (`audio_thread`, media-io/audio-io.c) handles audio processing, encoding and output. — [Backend design (rst source)](https://raw.githubusercontent.com/obsproject/obs-studio/master/docs/sphinx/backend-design.rst)
- Video pipeline: graphics thread renders output-channel sources to a final texture, converts to backend (typically YUV) format, queues frames (up to MAX_CACHE_SIZE) with a semaphore to the video thread; if the queue is full the last frame is duplicated (source of skipped/duplicated frames when encoder lags); encoders emit packets to outputs; outputs with audio+video use an interleave queue for monotonic timestamps. — same source
- Audio pipeline: ticks every 1024 samples (about 21 ms at 48 kHz); sources push audio, filters run, resample/remix via swresample; the source tree is snapshotted each tick and audio propagates up the tree, with scenes/transitions mixing children (e.g. crossfade) before final mix to encoders. — same source
- Output channels: `obs_set_output_source()` assigns sources to channels (0..MAX_CHANNELS-1); scenes and transitions are themselves sources; the frontend uses one channel for the current scene and others for global audio. — same source
- Docs expose plugin module types: sources, inputs, filters, transitions, outputs, encoders, services; module entry points load/unload/post_load/locale; separate "Rendering Graphics" docs cover graphics context, effects (HLSL-like effect files with techniques/samplers). — [docs.obsproject.com](https://docs.obsproject.com/)
- Platform renderers in repo: libobs-d3d11 (Windows), libobs-opengl, libobs-metal (macOS; experimental Metal from 32.0), libobs-winrt (Windows Graphics Capture glue). — [GitHub repo](https://github.com/obsproject/obs-studio); [32.0.0 release](https://github.com/obsproject/obs-studio/releases/tag/32.0.0)
- 30.2: NVENC on Linux (incl. AV1); NVENC, QuickSync, VA-API support shared textures on Linux; PipeWire window and display capture merged into one "Screen Capture" source. — [30.2 release notes](https://obsproject.com/blog/obs-studio-30-2-release-notes)
- 33.0 (RC): first-party plugins moved to a "core" folder, third-party plugins get a new install location/structure (legacy locations load until 34.0), Safe Mode skips third-party plugins; CEF 127 -> 150; NVENC on Windows Arm64; Windows builds on Visual Studio 2026; Qt 6.4-6.9 and FFmpeg 6.x/7.x support removed; Ubuntu 24.04 dropped; Game Capture fullscreen detection improved; Window Capture methods renamed. — [GitHub releases](https://github.com/obsproject/obs-studio/releases)
- 32.2: game capture hook fix and process mitigation/DLL loading changes on Windows (32.2.1). — [GitHub releases](https://github.com/obsproject/obs-studio/releases)
- Upcoming code-signing certificate change flagged in 30.2 as possibly affecting game capture with some anti-cheat. — [30.2 release notes](https://obsproject.com/blog/obs-studio-30-2-release-notes)

### Inferences
- Frame duplication on queue overflow explains OBS's user-visible "skipped/missed frames" indicators; a competitor can surface encode-lag more clearly.
- The single-compositor-thread, single-canvas model (until recent Canvas work) is a constraint for multi-output with different layouts.
- UI stack is Qt (33.0 notes reference Qt version support), not otherwise sourced here.

### Gaps
- Texture sharing mechanics, D3D11 shared handles, texture encoders, per-vendor encoder details, game-hook internals, DXGI vs WGC limitations, ScreenCaptureKit and PipeWire portal limits, config/profile file layout: not found/fetched (KB capture-guide URLs 404'd).

## Output and streaming (protocols, Enhanced RTMP, multitrack, simulcast, health)

### Takeaway
OBS added Enhanced RTMP/FLV (HEVC/AV1, multitrack) and multitrack video ("Enhanced Broadcasting" on Twitch) in 30.2, and WebRTC/WHIP with simulcast by 32.1. Dynamic bitrate for multitrack landed in 32.2.

### Cited Findings
- 30.2 multitrack video: initially Windows only, NVIDIA GTX 900/10/RTX 20+ or AMD RX 6000+, stereo/mono audio only; OBS sends system/settings data (OBS version, CPU, GPU, RAM, OS) to the service at stream start. Enhanced RTMP/FLV multitrack replaces the metadata-based "VOD Track". — [30.2 release notes](https://obsproject.com/blog/obs-studio-30-2-release-notes)
- WebRTC: HEVC supported and WHIP outputs may be audio-only or video-only (30.2); simulcast added in 32.1.0. — [30.2 notes](https://obsproject.com/blog/obs-studio-30-2-release-notes); [32.1.0](https://github.com/obsproject/obs-studio/releases/tag/32.1.0)
- Enhanced RTMP (Veovera spec) enables HEVC/AV1 in FLV; 29.1 first enabled AV1/HEVC over Enhanced RTMP for YouTube (2023 info, may be outdated). Closed captions work with HEVC/AV1 (30.2). — [AlternativeTo](https://alternativeto.net/news/2024/7/obs-studio-30-2-launched-with-multitrack-streaming-enhanced-rtmp-flv-and-composable-themes); [Betanews](https://betanews.com/article/obs-studio-29-1)
- 32.2: multitrack config is limited to the Custom service, dynamic bitrate added. — [GitHub releases](https://github.com/obsproject/obs-studio/releases)

### Inferences
- Simulcast and multitrack push transcoding/ladder work to the encoder side; a competing app that wants multi-destination egress must plan for multiple encoder sessions (NVENC session limits) or a relay.

### Gaps
- SRT/RIST/HLS status, reconnect and bitrate-adaptation behavior, stream health indicators (dropped frames, congestion), and multi-RTMP plugin ecosystem: not sourced. Current service support list for multitrack unknown.

## Known pain points and user requests

### Takeaway
Evidence gathered is thin: recurring reports of capture black screens/crashes (Windows game/window capture) and Linux PipeWire crashes. No ranked list of requests was found.

### Cited Findings
- Linux PipeWire screen capture crashes/hangs reported with OBS 31.0.0 (Dec 2024), traced to an upstream allocation issue with fix PRs #11618/#11638. — [Arch forum](https://bbs.archlinux.org/viewtopic.php?id=301775)
- Older forum threads describe window capture going black and game-capture crashes with a fault in an NVIDIA driver DLL. — [OBS forum](https://obsproject.com/forum/threads/obs-black-screen-capture-window-and-crashes-for-game-capture.17054)
- obs-websocket: requests during a scene collection change are undefined and can crash OBS; hotkey requests unsupported in practice. — [protocol.md](https://raw.githubusercontent.com/obsproject/obs-studio/master/docs/generated/protocol.md)
- Plugin compatibility is a visible pain: 32.0 stops loading plugins built for newer OBS; 32.2 forces Apple Silicon builds so Intel-only plugins stop loading; 33.0 relocates plugin folders. — [32.0.0](https://github.com/obsproject/obs-studio/releases/tag/32.0.0); [releases](https://github.com/obsproject/obs-studio/releases)

### Inferences
- The UI-overhaul releases (add-source thumbnails, mixer rework, widgets) imply OBS itself regards setup complexity for non-technical operators as a main weakness. Inference only.

### Gaps
- No quantified issue statistics, no GitHub Discussions/roadmap/forum-wide "most requested" data, nothing on presentation-software integration gaps.

## Recent direction (2024-2026)

### Takeaway
2024: composable themes, multitrack/Enhanced RTMP, hybrid MP4 (30.2). 2025-26: hybrid MOV default, plugin manager, Metal renderer, mixer and add-source UI overhauls, WebRTC simulcast, canvases groundwork, plugin loading restructure and CEF/Qt/FFmpeg modernization in 33.0.

### Cited Findings
- See timelines above: [30.2](https://obsproject.com/blog/obs-studio-30-2-release-notes), [32.0.0](https://github.com/obsproject/obs-studio/releases/tag/32.0.0), [32.1.0](https://github.com/obsproject/obs-studio/releases/tag/32.1.0), [releases index](https://github.com/obsproject/obs-studio/releases).
- obs-websocket 5.7.0 added GetCanvasList and optional `canvasUuid`, consistent with the canvas work in 32.1. — [protocol.md](https://raw.githubusercontent.com/obsproject/obs-studio/master/docs/generated/protocol.md)

### Inferences
- Multiple canvases plus WebRTC simulcast suggest OBS is moving toward multi-output (e.g. vertical + horizontal) natively; confirm in 33.0 notes.

### Gaps
- 31.x (2024-25) release notes and the official roadmap were not read; 33.0 final notes not yet published as of this research.
