# FreeShow (ChurchApps/FreeShow) profile, as of 2026-10-21

Method note: all facts come from WebFetch/WebSearch of GitHub pages, raw source files and freeshow.app docs on 2026-10-21. Several fetches were summarized by a small model; directory listings were truncated ("View all files"). Statements about what a file does based only on its name are in Inferences. I read only two source files (Output.ts header, CaptureHelper.ts).

## Feature inventory (slides/shows, scripture, timers, media/audio, stage, remote, outputs, NDI/BMD/streaming, overlays, transitions, import, templates, projects, cloud, automation)

### Takeaway
FreeShow is a broad ProPresenter/EasyWorship-class church presentation tool. It is GPL-3.0 and ships fast. The 2026 releases added Blackmagic SDI, Open Media Transport (OMT), Scenes, audio routing and sidechain, recording, RTMP, and AI/voice scripture. Breadth is its strength; polish in text transitions and fitting are weak points.

### Cited Findings
- Project is GPL-3.0, funded by donations (ChurchApps / GitHub Sponsors); repo showed about 1.3k stars, 346 forks, 597 commits, 7 open issues at fetch time. Platforms documented for dev: Windows and Linux; macOS builds exist per release notes. — [GitHub README](https://github.com/ChurchApps/FreeShow)
- Website feature claims: text styling (outline/shadow), live text editing, autoplay timers, countdowns, MIDI control to advance slides or trigger lighting, simultaneous outputs with different looks, web output for mobile, NDI output, mobile remote control, templates/slide styles, scripture lookup, song-title lyric search, import from PowerPoint/ProPresenter/EasyWorship/OpenSong, multiple export formats, cloud sync for multi-computer teams. — [freeshow.app](https://freeshow.app)
- Release 1.6.7 stable (21 Oct 2026): transitions keep items unchanged between slides and only fade text for persistent text boxes; fixes for cloud sync, transitions, auto size, overlay refresh. — [Releases](https://github.com/ChurchApps/FreeShow/releases)
- 1.6.7 betas: per-video start/end times, stop for action timeouts, live stream stats in fullscreen preview, Icecast status widget, more audio visualizers, zoom control for output preview grid, output-specific effects, automatic location of missing PDFs. — [Releases](https://github.com/ChurchApps/FreeShow/releases)
- 1.6.6 stable (9 Oct 2026): opt-in automatic scripture from voice input, Open Media Transport support, audio sidechaining (renamed from ducking), Scenes, a "Smart LLM" that generates slide items from a prompt, RTMP bitrate mode selection, custom output cropping; recording of any output with audio, multi-channel mic input, API password option (beta.4); QR Code item, OnStage content provider, candle/confetti effects, media inputs via the Camera item, overlay export/import (beta.2/3); custom fonts manager, collapsed project mode, textbox cutout shape, GPU-native output capture (beta.1). — [Releases](https://github.com/ChurchApps/FreeShow/releases)
- 1.6.5 (26 Aug 2026): audio routing, mesh gradient effects, custom calendars with URL sync, item snapping, chords in TXT export; "Layout" renamed "Arrangement"; Icecast toggle. — [Releases](https://github.com/ChurchApps/FreeShow/releases)
- Blackmagic SDI/DeckLink output shipped (PR #3011, issue #124 closed 10 Mar 2026, in 1.6.0-beta.1 per the issue). Users wanted it chiefly for key+fill with ATEM switchers. Early testing: 720p60 8-bit YUV worked; other modes failed until restart, 2160p59.94 failed, a failed resolution change stalled output until restart, transitions ran about 3 fps at 1080p60. Maintainer had no device until Mar 2026 (bought a Thunderbolt Mac + UltraStudio Monitor). — [Issue #124](https://github.com/ChurchApps/FreeShow/issues/124)
- Docs: FreeShow can control OBS via obs-websocket; for transparent (alpha) output to OBS, use NDI with the DistroAV plugin. — [Livestreaming docs](https://freeshow.app/docs/livestreaming)
- Remote/API: Companion module needs the "Bitfocus Companion" option enabled, port 5505 (WebSocket); HTTP GET/POST on 5506 with action/data params. API action groups: project, show, presentation, stage, clear, media, overlays, audio; also send_midi, start_trigger, run_action, toggle_action, send_rest_command. — [Companion docs](https://freeshow.app/docs/companion), [API docs](https://freeshow.app/api)
- Built-in servers (Express + Socket.IO, Bonjour-advertised, default 10-client cap): REMOTE 5510, STAGE 5511, CONTROLLER 5512, OUTPUT_STREAM 5513; each also serves `/nowplayingcover`. — [servers.ts](https://raw.githubusercontent.com/ChurchApps/FreeShow/master/src/electron/servers.ts)
- Import formats: ProPresenter 4-7+, EasyWorship 6-7, VideoPsalm, OpenLP, OpenSong, MediaShout, Quelea, SoftProjector, Songbeamer, Easyslides, VerseVIEW, text, CSV, ChordPro, Word, PowerPoint, PDF, Lessons.church, projects, .ics calendars. Keynote not listed. — [Import docs](https://freeshow.app/docs/importing)
- Frontend component dirs suggest feature areas: slide, show, edit, draw, output, stage, actions, timeline, media, drawer, quicksearch, export, settings. — [components tree](https://github.com/ChurchApps/FreeShow/tree/master/src/frontend/components)

### Inferences
- The "Scenes", Camera-item inputs and OnStage provider show it moving from pure lyrics display toward live-production features, i.e. toward our space.
- Release cadence (about weekly betas in Aug-Oct 2026) means features appear fast but regress often (transition/cloud/overlay fixes in nearly every release).

### Gaps
- OSC support unconfirmed (no FreeShow OSC docs found). MIDI is documented on the website and via send_midi.
- Full list of templates/projects/cloud details (cloud provider, conflict handling) not read; Firebase/pusher-js dependencies suggest the cloud sync stack but I did not read the code.
- Keynote import: not listed in docs.

## Architecture (stack, outputs/windows, state sync, NDI/BMD/capture, rendering, data formats, performance, packaging)

### Takeaway
Electron 37 + Svelte 3 + TypeScript. Each output is an Electron BrowserWindow, optionally rendered offscreen (OSR); frames are captured and fanned out by a rate-aware capture layer to NDI/OMT/Blackmagic/WebRTC/RTMP/recorder/stage/web consumers, with sender workers.

### Cited Findings
- package.json v1.6.7: electron ^37.10.3, electron-builder ^26, electron-updater, svelte ^3.59, vite ^4.5, typescript ^4.9, express + socket.io, better-sqlite3, pdfjs-dist, sherpa-onnx-node (optional, local speech), @sentry/electron, firebase, pusher-js, Playwright + Vitest tests; Node >=22.12; native modules rebuilt (`rebuild-opus`, `rebuild-osr`); `osr-capture` is a pinned GitHub fork. No NDI npm package appears in package.json. — [package.json](https://raw.githubusercontent.com/ChurchApps/FreeShow/master/package.json)
- Main-process source tree `src/electron/`: IPC, ai, audio, blackmagic, capture, cloud, contentProviders, data, ndi, omt, output, streaming, timecode, utils, plus index.ts, preload.ts, servers.ts. — [src/electron](https://github.com/ChurchApps/FreeShow/tree/master/src/electron)
- NDI dir: NdiSender.ts, NdiReceiver.ts, ndiWorker.ts, talk.ts, vingester-util.ts (sender and receiver; worker). — [src/electron/ndi](https://github.com/ChurchApps/FreeShow/tree/master/src/electron/ndi)
- `Output` class holds a BrowserWindow with flags `osr` ("captured via offscreen paint events instead of the capturePage poll"), `invisible`, `boundsLocked`, `transparent`, `screen`, `intendedBounds`, plus per-output `webrtcData`, `rtmpData`, `captureOptions`. — [Output.ts](https://raw.githubusercontent.com/ChurchApps/FreeShow/master/src/electron/output/Output.ts)
- CaptureHelper per-consumer default frame rates: stage 20, server 10, webrtc 30, rtmp 30, recorder 30, idle 1, NDI/OMT connected 30. Max active rate drives OSR render rate via `webContents.setFrameRate`, idle at about 1 fps. Source comment: several 4K OSR surfaces at 60 fps saturate the GPU and slow readbacks; per-consumer rates enforced by admission-time decimation and a send pacer in a worker. Linux uses a begin-frame drive. — [CaptureHelper.ts](https://raw.githubusercontent.com/ChurchApps/FreeShow/master/src/electron/capture/CaptureHelper.ts)
- Capture dir: CaptureHelper, CaptureOptions, SenderCapture, StreamReceiverHost, senderWorker, streamFrames, streamReceiverProcess. — [capture tree](https://github.com/ChurchApps/FreeShow/tree/master/src/electron/capture)
- Output dir has `ppt/` with pptToPdf.ts, pptToShow.ts, presentation.ts. — [ppt tree](https://github.com/ChurchApps/FreeShow/tree/master/src/electron/output/ppt)
- Project import docs: PPTX import brings text only by default; to keep styling import as PDF (FreeShow can export PPTX to PDF if PowerPoint is installed, else an online converter); PDF pages can be converted to images to allow editing and timers. — [Import docs](https://freeshow.app/docs/importing)
- 1.6.6 added "GPU-native output capture for better performance". — [Releases](https://github.com/ChurchApps/FreeShow/releases)

### Inferences
- Slides are rendered as HTML/CSS in the output webview, so what the operator previews equals what NDI/Blackmagic/stream get; cost is webview paint + readback + CPU copy to each sender. The Blackmagic test (about 3 fps transitions at 1080p60) is consistent with this readback bottleneck. Not verified in source.
- `ppt/pptToPdf.ts` plus docs suggest PPTX-to-PDF conversion relies on an installed PowerPoint (Windows/macOS) rather than own renderer; PPTX native import is text-only. Inference from docs + filenames.
- `better-sqlite3` and `data/` suggest a DB for some features; show/project file formats (JSON per the discussion about `projects.json`) not verified from source.
- Slide/state sync between controller and output windows is Electron IPC (preload.ts, IPC/main.ts); remote/stage clients use Socket.IO. Inference from file names plus servers.ts; I did not read the IPC code.

### Gaps
- Exact NDI SDK binding (ndi-related native lib), Blackmagic binding (macadam was suggested in #124; not confirmed as used), and show file format were not read.
- Performance numbers (CPU/GPU, latency) not found beyond anecdotes above.

## Known pain points, feature requests, roadmap direction

### Takeaway
Transitions/text fitting and output edge cases are the recurring pain; top requests are CCLI integration and a transitions rework. Direction is to add more production features (scenes, audio, streaming, AI).

### Cited Findings
- Top open issues by thumbs-up: CCLI song search and reporting script #883 (54), Rework Transitions #2169 (52), HID inputs while unfocused #877 (21), Auto Lyrics #3394 (15); stability issues #3904 (reaching max connections resets all connected stage displays) and #3905 (monthly repeat events on 29-31 drift). The open list is short (7). — [Issues](https://github.com/ChurchApps/FreeShow/issues)
- #2169: text fading or flashing to black between slides, including identical consecutive text; regression in 1.6.3 traced to line-limit output styles; fixes shipped in 1.6.6-beta.3 and 1.6.7; maintainer judged WebGL (PixiJS/Three.js) transitions overkill. — [Issue #2169](https://github.com/ChurchApps/FreeShow/issues/2169)
- Discussions: scripture text wrap and grow/shrink fit are the most commented problems; show search behaves unexpectedly; requests for MIDI project control, non-web remote control, API for current output slide text, creating shows outside a project, stem splitting, offline AI, and externally re-reading projects.json; Linux/Windows install portability and Chromebook questions. — [Discussions](https://github.com/orgs/ChurchApps/discussions/categories/freeshow)
- Blackmagic early output limitations (see Q1). — [Issue #124](https://github.com/ChurchApps/FreeShow/issues/124)

### Inferences
- Stage display scaling limit (10-client cap, reset on overflow) is a weakness for larger venues.
- Because open issues are few, high-friction items are likely tracked in discussions or fixed quickly; absence of performance/multi-monitor issues in the open list is not evidence they don't occur (many are probably closed). I did not review closed issues.

### Gaps
- Closed-issue analysis (multi-monitor, crash, performance) not done; Discord/Facebook community content not indexed/checked.
- No formal roadmap document found; direction inferred from releases.

## Strengths to copy, gaps to exploit (for a competing Tauri/Rust/GStreamer app)

### Takeaway
Copy its connectivity breadth and low-friction import/API surface; exploit its Electron webview-readback capture path, text-only PPTX import, and church-centric scope.

### Cited Findings
- Reference list of what users value: ProPresenter/EasyWorship import, Companion module with HTTP/WebSocket API, key+fill demand with ATEM, alpha via NDI to OBS, per-consumer frame-rate budgeting. — see sources above ([Import docs](https://freeshow.app/docs/importing), [Companion docs](https://freeshow.app/docs/companion), [Issue #124](https://github.com/ChurchApps/FreeShow/issues/124), [CaptureHelper.ts](https://raw.githubusercontent.com/ChurchApps/FreeShow/master/src/electron/capture/CaptureHelper.ts))

### Inferences
- Copy: Companion/HTTP/WebSocket action API with documented action catalog; per-consumer fps budgeting with idle=1 fps (our F2.3 `FrameThrottle` is similar); rich importers; stage/remote web servers advertised via Bonjour; fast release cadence; test builds shared with hardware owners.
- Exploit: (1) PPTX fidelity: they import text only or require PDF; our deck importer plus real rendering could win. (2) GPU/native compositing via GStreamer instead of webview readback, targeting stable 1080p60 transitions and key+fill. (3) Multi-camera production (NDI ingest, phone WHIP, switching) is nascent in FreeShow (Camera item, Scenes new in 1.6.6). (4) Memory/CPU of Electron; smaller footprint with Tauri. (5) Transition quality and scripture auto-fit are persistent complaints. (6) Stage display limit of 10 clients and reset-on-overflow behavior.
- Risks: FreeShow's feature velocity and zero cost make feature parity a moving target; GPL-3.0 means we can read it for ideas but must not copy code into a non-GPL product.

### Gaps
- No head-to-head performance benchmark found; unverified that FreeShow's Blackmagic output is stable today beyond the March 2026 thread.
