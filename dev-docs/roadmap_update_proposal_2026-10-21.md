# Roadmap update proposal (2026-10-21, revised 2026-10-23) — for review, not yet applied

> **Two 2026-10-23 addenda at the end of this file supersede parts of the sections below** (audience, ProPresenter bridge, Phase 4 order, F5.x deferrals; the second reorders everything after the architecture review). Read both first.

Source: `dev-docs/research/competitor-analysis-2026-10-21.md` (OBS, FreeShow, ProPresenter, Mevo, plus
source reads of CasparCG, voctomix, MediaMTX, Companion, OpenLP, gst-plugins-rs, Sunshine, Cap,
DistroAV; raw notes in `dev-docs/research/notes/`). The spec of record stays
`streamdeck_product_roadmap_2026.md` until you approve edits below.

## Decisions needed from you

1. **Audience.** Talks/events only, or worship services too? Songs/lyrics/scripture is the largest
   ProPresenter gap and the roadmap names no audience. If out of scope, say "no ProPresenter
   dependency" means decks, not songs.
2. **Independence wording.** Pick one phase for "zero dependency" (spec says Phase 5 in the summary,
   Phase 4/5/6 in the gate table) and define it as a test: a full service run, streamed and
   recorded, with Mevo Studio and ProPresenter uninstalled. Write down the permanent exceptions
   (NDI runtime, GStreamer plugin set, cameras, OS keychain/signing).
3. **GStreamer baseline.** Roadmap targets 1.26; this dev machine runs 1.28.3 (adds webrtcsink
   renegotiation, VA encoders, WHEP). Choose one.
4. **F3.1 latency.** Accept "about one output frame" (CPU compositor 33.4 ms p50 for the oldest
   input; d3d12compositor ~49.5 ms) in place of "< 33 ms", or keep the target open until real
   sources exist.

## Proposed sequencing

1. Hygiene already done: CI runs `cargo fmt`; F3.1 shipped (`1a1cdf8`, `226d5db`).
2. **F3.2 spike (1 week, two arms):** frame injection of rasterised slides via `appsrc`, and a
   dedicated program-render window at stream resolution. OS capture (WGC / ScreenCaptureKit /
   PipeWire) stays a fallback behind a `CaptureTarget` enum. Windows WGC is verified available
   locally (`d3d12screencapturesrc window-handle`).
3. **F3.3 spike first:** voctomix-style timestamped control-source transitions with inactive pads
   parked at alpha 0 / zorder -1, tested on `d3d12compositor`. Cut, fade, fade-to-black (its own
   transition), slide/push (rename "side-wipe"). Measure fade smoothness at 30 fps before building
   scene UI. Fallback if frame pacing fails: revisit a wgpu mixer.
4. **F3.4:** static PNG alpha fade, then Rust-rendered RGBA lower thirds. Transparent webview / CEF
   only as a time-boxed spike. Defer SlideClassifier auto-overlay.
5. **Phase 4 re-sequenced:** encoder probe (test-encode, isolated, vendor-aware, TOML override) →
   RTMP and SRT egress with per-destination leaky queues, reconnect and health → local recording →
   WHIP fan-out as an extra transport. Pull send-queue congestion + dropped-frame % + basic bitrate
   reduction from F6.2 into this phase.

## New or moved items

| Item | Change |
|---|---|
| F4.4 SRT ingest | Move from optional/Phase 4 to Phase 2; verify with Mevo Start/Core/Plus (SRT is the only output on every model; NDI only on Start/Core) |
| New F3.5 Audio | Capture (NDI, phones, local device), mix, meters, A/V sync, Opus for WebRTC, AAC for RTMP |
| New F3.6 Control API | Rust-owned WebSocket + HTTP bridge on the event bus, scoped bearer tokens in keychain, state on connect, first-party Companion module (booleans: presenting, blackout, stream live, source lost) |
| New F1.6 Stage display + countdown | Bus-fed; we already hold the data |
| New F4.5 Local recording | Crash-safe fragmented MP4 off the encoded stream |
| New phase: Distribution | GStreamer bundling + size budget, NDI runtime policy, macOS notarisation + CI, signing, AV exclusions |
| F1.4 / F5.1 PPTX | Replace "indistinguishable from PowerPoint" with a documented fidelity matrix (PDFium raster + structured text); drop `.key` or route via PDF export |
| F5.2 / F5.3 / F6.1 | Defer until the above ship |
| Maybe later | NDI program output with key/fill (`ndisink` exists), PTZ, PJLink, Planning Center import |

## Corrections to our own docs

- CLAUDE.md lists Rust 1.78; verify against `rustc --version` and the lockfile (only blocks us if we
  build gst-plugins-rs 1.92 or vendor Cap 1.88).
- `scap-direct3d` (MIT) depends on `cap-d3d-adapter`, which is AGPL by Cap's root text: reimplement
  the ~60-line DXGI adapter pick or use the `windows` crate directly.
- `advertises_new_format` in `ndi.rs` can compare width/height/fps/FourCC (what gst `ndisrc` and
  DistroAV do); expose NDI `extra_ips` and groups in config (mDNS does not cross subnets).
- Research caveat: much is code-verified from shallow clones, but nothing was benchmarked, Mevo and
  ProPresenter rest on docs/reports, and NDI runtime redistribution terms are unread.

## Licensing rule of thumb

Reference only (copyleft): OBS, FreeShow, CasparCG, OpenLP, Sunshine, DistroAV, Cap outside its MIT
crates. Usable with notices: voctomix, MediaMTX, Companion core (MIT), gst-plugins-rs (MPL-2.0,
file-level). Implement importers and protocols from public specs, never from GPL importer code.

---

# Addendum 2026-10-23: plan rebuilt around the real service workflow

Sources: the user's actual order of service (41-page A4 PDF) and the slide deck derived from it (56 slides), analysed with `tools/service-analyzer` (Docker; fonts and styles from the PDF, tables, layouts and notes from the PPTX, word-level alignment); competitor workflow and human-factors research in `dev-docs/research/notes/ux_flows_and_operator_design.md`; Mevo Start and Google Drive research summarised below. Nothing in this addendum is implemented.

## Context that changes the plan

- Audience is **church services** (Anglican Eucharist: liturgy, responses, hymns, readings, notices), live-streamed to Facebook Live. Hymn and liturgy text is in scope. **ProPresenter is not used**, so the F5.4 bridge is dropped. Decision 1 above is answered. Decision 2's test becomes: the Mevo Studio iPad and the Google Slides/ClickShare laptop are no longer needed for a full service.
- Today: an iPad runs Mevo Studio (camera, scenes, PDF in the centre or a portrait PDF at the left, Facebook Live); a laptop runs Google Slides in Edge to the projector through a ClickShare button. The main person writes the order of service (fonts indicate Microsoft Word; to confirm), exports a PDF, and the tech person hand-builds the slides.
- Measured on the sample: slide text overlaps the PDF by 86%. Liturgy pages are 97-100% present; the Merbecke variants, the interleaved English/Hungarian Lord's Prayer and the notices pages (27-41) are cut or condensed. The tech person's edits are deliberate readability choices: optional "may" becomes definite, abbreviated dates are expanded, "All:" cues are added, priest-only instructions are dropped, one setting is chosen, UK spelling is used. The blank slide after "The Sermon" is a deliberate focus cue. These habits are product requirements, not noise.

## New and changed items

| Id | Item | Acceptance (summary) |
|---|---|---|
| **F1.7** | **Service-order document**: one structured source rendered to projector slides, stream side panel and print/PDF. Typed blocks (heading, instruction, leader, all, hymn verse, reading, notice, poster image). **Variants** ("Or in the Merbecke setting") are chosen before the service and the choice is recorded. Per-block **audience flags** (projector / stream / print). Per-block **cues** (focus screen, scene switch). A fixed-text library (Creed, Lord's Prayer, Gloria) plus weekly fill-in (readings, hymns, intercessions, notices). Revisions with undo | A full service is built without a PPTX; changing one block updates all three outputs; flags keep intercession names and bank details off stream/projector when set; the variant choice is visible |
| **F1.7a** | **Importers into the document**: `.docx` with named styles (preferred), PDF (fonts, italic and bold map to block roles; the analyzer already does this), PPTX (text, including tables), **Google Drive** (below) | The sample PDF imports with headings, instructions and responses classified; accuracy reported on the sample; no silent text loss |
| **F1.8** | **Focus / default screens**: black, branded default, dimmed seasonal colour or sermon title; one-key Clear to focus screen; also a block cue so no blank slide is needed | One key clears and restores the exact previous slide; configurable default; the stream scene can switch with it |
| **F1.9** | **Readability profile**: auto-pagination at natural boundaries (stanza, paragraph, response) to a chunk and size profile learned from existing decks (theirs: median 88 words, 9 lines, 25 pt; practitioner guides say 20-40 words, 4 lines, 24 pt or more); a **room profile** (screen height, farthest row) that computes minimum text size from viewing-distance guidance; a back-of-room preview | Profile is editable; each slide shows words, lines and size; the preview warns below the room's minimum. AVIXA 8/6/4 and "text height x 150" are guides, not rules |
| **F1.6** | Stage / confidence display (from the earlier proposal, now first-class): current and next, notes, clock | Bus-fed; no invoke permission for presentation windows |
| **U1** | **Design / Show split** with the operator-screen principles below: exactly two modes, a persistent high-contrast mode label, structural edits locked in Show with a narrow in-place text fix (revision logged); Design usable in its own window or tab on a second monitor | See "Operator screen principles" |
| **U2** | **Pre-service check**: cameras present, audio live, projector assigned, both outputs previewed, room profile applied; a gate, not a third mode | Go Live confirmation requires the check reviewed or explicitly skipped |
| **U3** | **Usability test**: a clickable prototype of the Show screen rehearsed with the actual operator on a real service before it is built | Time-to-find and error counts recorded |
| **F2.2b** | **Mevo Start via NDI\|HX** (NDI works on Start and Core only; enabling it is in the Mevo Studio app) with **SRT** as the fallback for every Mevo (moves up from F4.4). The iPad stays the camera operator's tool (framing, exposure, NDI switch): the camera has no desktop control and its camera-to-app transport is proprietary and not reverse engineered | **Hardware test first** (below) before building on it |
| **F3.3b** | **Scene editor and source model**: sources are camera (NDI/SRT/phone), slide, image, video clip, text, colour; canvas editor (drag, resize, crop, layer order, lock, hide); full-screen versus overlay scene types as in Mevo; **Preview / Program with an explicit Take**; writes the existing `[[scene.<id>.sources]]` TOML | Program is the single source for stream, recording and projector mirror; nothing edited in Design reaches Program without a Take |
| **F3.7** | **Secondary document source** (the portrait PDF / "words to follow" panel): PDF pages or service-order block text rendered as a stream scene layer, paged independently or **following the projected section automatically**; avoid showing identical text in two stream layers at once (redundancy) | The panel moves to the current section within one slide change; poster pages (notices) can be shown as images |
| F3.5, F3.6 | Audio and control API stay as in the earlier proposal | unchanged |
| F4.x | Phase 4 order stays RTMP/SRT first (Facebook Live accepts RTMP; Mevo itself pushes RTMP), then recording, then WHIP | unchanged |
| F1.4, F5.x | PPTX/PDF import becomes a migration path, no longer the main workflow; F5.3 templates merge into F1.7's fixed-text library; F5.4 ProPresenter bridge **dropped**; the F5.1 fidelity claim stays replaced by a fidelity matrix | |

### Google Drive import (part of F1.7a; spike first)

- OAuth for desktop apps through the system browser, loopback redirect, PKCE.
- Google Picker on the narrow `drive.file` scope, so only the chosen file is accessible.
- Export Google Slides to PPTX (PDF is also listed) with the Drive export call, **capped at 10 MB of exported content**; larger decks need a fallback or a warning.
- Refresh token in the OS keychain, never TOML, matching the secrets rule; a "re-sync from Drive" action before a service.
- **No source found confirms** that Picker plus export works on `drive.file` alone, so the first task is a small spike (sign in, pick, export, import) before promising it. It needs a Google Cloud project and OAuth client.
- Google Docs (the order of service itself) could use the same path.

## Operator screen principles

Sources and evidence tiers are in the UX research note; most are secondary summaries.

1. **Perception first** (Endsley): the Show screen answers three questions at a glance: what the audience sees now, what comes next, is anything wrong.
2. **Neutral by default, colour means attention** (ISA-101): extends our rule that red, amber and green belong to stream health; violet stays for "presenting".
3. **Overview first, detail on demand** (ISA-101 display hierarchy): Show is the overview: Program, Next, a one-column service rundown with the current item highlighted, big Prev/Next, Clear to focus screen, a few scene buttons, one health chip. Cameras, audio and network are one click down, never permanent.
4. **Few modes, always visible** (Sarter and Woods): two modes only; the label and a colour band are always on screen; the lock is a mode, not a permission, and live corrections go through the narrow text-fix path.
5. **Program is the truth** (OBS Studio Mode leak reports from forums): edits never affect Program without a Take; nested content is not exempt.
6. **Escape hatches**: one-key Clear and a Lock Output (FreeShow has both), and undo for the last Take.
7. **Roles**: the main person authors in Design; the tech person runs Show; a phone or Companion surface (F3.6) can serve a second operator.
8. **Validate with the real operator (U3).** Treat the research as direction, not proof.

## Revised MVP path to retire the iPad's streaming job

1. Hardware test of the Mevo Start (below).
2. F1.7, F1.8, F1.9 on the sample service (projector only, ClickShare unchanged).
3. F3.2 slide frame injection, F3.3b scene editor, F3.7 PDF panel.
4. F3.5 audio.
5. RTMP egress to Facebook Live, plus local recording.
6. U1, U2, U3 are woven in from step 2, not added at the end.

The Drive spike can run in parallel with step 2.

## Hardware test owed on the user's Mevo Start (also added to `unverified_items.md`)

- Does NDI\|HX reach the laptop while the iPad is connected, or does one receiver take over?
- Must the iPad app stay open for NDI to keep running?
- NDI\|HX latency over Wi-Fi versus Ethernet (Mevo recommends Ethernet through its power adapter).
- Does `ndisrc` decode HX cleanly with the installed NDI runtime?
- Can ClickShare share only the presentation window instead of the whole screen? This decides what the operator sees on the laptop while presenting.

## Still open

- Decisions 2-4 above (independence wording, GStreamer baseline, F3.1 latency wording).
- What the main person authors in (Word assumed).
- Whether the sidebar PDF is the only stream layout, or the centre slide is used too.
- Privacy defaults for intercession names and bank details (proposal: off the stream unless a block is flagged on).

## Evidence quality

Mevo facts come from Mevo help pages (NDI only on Start/Core, NDI|HX, no desktop control, PDF-only presentations); behaviour with two receivers is untested. Church legibility numbers are practitioner guidance, not controlled studies. Human-factors points are third-party summaries of Endsley, ISA-101, Sarter and Woods and Mayer; no primary papers were read. ProPresenter's user guide was unreachable, so Clear-to-Logo and Clear Groups behaviour is unverified.

---

# Addendum 2 (2026-10-23): effect of the architecture review on the plan

Source: `dev-docs/review/architecture_review_2026-10-23.md` (independent review; findings F-01..F-19, plan in its section 6). Nothing here is implemented. Where this addendum and the first addendum disagree on ORDER, this one wins; the feature list and acceptance criteria of the first addendum stand.

## What changes

1. **Foundation before features.** The review found crash blast radius, fragile persistence and missing release engineering to be the real risks, not the stack. A "stop the bleeding" block (no new features) comes first: verify and fix slide/media paths in a built app (F-01), capability cleanup (F-08), atomic writes and recovery for `streamdeck.toml`, `deck.json` and `slide_state.json` plus the timer fix (F-03, F-10), importer limits with stage-then-confirm and one fuzz target (F-04), WHIP defaults (token on first enable, bind the chosen IP, reject browser origins; F-05), dependency hygiene, and the cheap P0 checks in `unverified_items.md`.
2. **A state spine replaces "event bus carries state".** The bus stays for notifications; application state lives in Rust with a revision counter and snapshot, and the presentation reconciles on revision (this also fixes `BusLag` leaving the projector stale, F-07). This is also the base the service-order document, revisions and Take need, so it precedes F1.7.
3. **Versioned SQLite document store** for decks and service-order documents (revisions, undo); TOML stays for settings with atomic writes.
4. **Media engine as a separate process** (heartbeat, restart, replay of desired state) behind the existing `PipelineBackend` trait; pdfium import moves into it. Priority depends on open question 3 below.
5. **Release pipeline is a feature.** `release.yml` for a Windows-first, per-user NSIS installer with signing, SBOM, provenance, signed updater and rollback, notices, and a first signed internal release. macOS and Linux follow once Windows has run real services.
6. **Scope cuts.** Scene editor beyond fixed layouts, the secondary PDF panel, stage display, audio, control API and Drive import move to "later" (after the foundation and a thin service-order slice); WHIP is an optional hardened phone ingest; RTMPS to Facebook plus local recording comes first; NDI is "detect the runtime, link to download" with SRT as the default until the hardware test says otherwise.
7. **Order now:** stop-the-bleeding (2 weeks) -> spine, SQLite, sidecar, release pipeline (about 2 months) with a thin service-order slice (typed blocks, variants, projector output, `.docx`/PDF import, focus screen, Clear) and a usability prototype (U3) -> RTMPS, recording and the pre-service check (U2) -> scene editor, PDF panel, audio, control API, Drive.

## Open questions (all still open; they decide priorities, not direction)

From the review: repo public or private, and where the owner is based (signing options); is Windows-only acceptable for the first release; must the projector survive an engine crash, or is full-app recovery under 10 s enough (decides sidecar priority); will the church accept an update prompt or prefer manual updates; who authors the order of service and in which tool (Word assumed); Facebook target (personal page, Page or group, stream key versus Live API); do intercession names or bank details share a document with projector text; accept NDI as detect-and-link. Plus the earlier decisions: independence wording, GStreamer baseline (1.26 versus 1.28), F3.1 latency wording.

## Not verified by the review

F-01 was inferred from code, not run in a built app. Several large files were not read in detail (`whip.rs`, `compositor.rs`, `preview.rs`, `ndi.rs`, `display.rs`, most frontend components). GStreamer bundle size and plugin licences were not measured. The first task of the stop-the-bleeding work is to verify F-01 before fixing anything.
