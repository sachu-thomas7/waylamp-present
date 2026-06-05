---
name: streamdeck-ai-context-handoff
version: 1
description: >
  AI context engineering skill for the StreamDeck project.
  Defines the 7-block handoff structure, 6 context principles, anti-patterns,
  phase-completion checklist, master + mini templates, phase-gate snapshots,
  tech stack reference, and architecture invariants.
  Load this skill at the start of every new AI coding session to convey project
  state efficiently and prevent redundant code generation.
---

# StreamDeck — AI Context Handoff System

Expert framework for conveying project state to Claude or any AI coding assistant — efficiently, precisely, and without redundant regeneration across phase boundaries.

> **The core problem:** Every new AI thread starts cold. Without a structured handoff, you waste the first 20% of your context window re-explaining architecture, re-generating boilerplate, and correcting the model's wrong assumptions. This skill eliminates that waste.

---

## The 6 Context Engineering Principles

### Principle 01 — State Over History
Never give the AI the full conversation history. Give it the *current state*: what exists, what works, what's broken, what's next. History wastes tokens; state gives direction.

### Principle 02 — Boundary Before Code
Always declare architecture invariants before asking for code. This prevents the model from generating patterns that violate your event-bus-only channel rule or process-separation constraint.

### Principle 03 — Scope Locking
Explicitly name what is OUT of scope for the current task. Claude will otherwise try to be helpful by touching adjacent systems — which breaks stable code you didn't want changed.

### Principle 04 — Acceptance Criteria First
Paste the exact acceptance criteria from the roadmap. The model generates toward measurable tests, not toward vague descriptions. This alone halves back-and-forth iterations.

### Principle 05 — Minimal Viable Context
Only include the subsystems the AI will *touch* in this session. Don't paste the full roadmap — paste only the current feature block plus its direct dependencies. Smaller context = sharper output.

### Principle 06 — Output Contract
Tell Claude exactly what files to produce and what to leave untouched. "Generate only X, modify only Y, do not touch Z" prevents the model from refactoring stable code mid-session.

---

## The 7-Block Context Anatomy

Every handoff prompt to Claude must follow this exact structure. Each block serves a distinct cognitive purpose for the model.

| # | Block | Purpose | Token Budget |
|---|---|---|---|
| 1 | 🔖 Project Identity | Project name, stack summary, current phase, one-sentence mission. Sets the model's entire reasoning frame. | ~100 |
| 2 | 🏗️ Architecture Invariants | The 5 never-break rules. Pasted verbatim every time. Guards against AI anti-patterns. | ~200 |
| 3 | ✅ Completed State | Feature IDs (F0.1, F1.2…) + one-line status each. No code — just ground truth. | ~150 |
| 4 | 🎯 Current Task | Single feature being built: description, technologies, verbatim acceptance criteria. The AI's primary directive. | ~300 |
| 5 | 🔌 Interface Contract | Types, event variants, Tauri command signatures the current feature integrates with. Interfaces only — not implementation. Prevents mismatched types. | ~200 |
| 6 | 🚫 Scope Boundary | Explicit "DO NOT touch / generate / modify" list. Short but critical. | ~50 |
| 7 | 📦 Output Contract | Exact file paths to generate or modify. Expected response format (code only / code + tests). | ~50 |

---

## Master Handoff Template

Copy, fill in the brackets, and paste to start any session. This is the canonical format for StreamDeck AI handoffs.

```
# ═══════════════════════════════════════════════════
# STREAMDECK AI HANDOFF — [PHASE X · FEATURE Y]
# ═══════════════════════════════════════════════════

## 1. PROJECT IDENTITY
Project: StreamDeck — cross-platform live streaming + presentation app
Stack:   Tauri v2.11 · Rust 1.78 / Tokio 1.38 · React 18 / TypeScript 5 · GStreamer 1.26 · NDI SDK 6.3
Phase:   [CURRENT PHASE NUMBER AND NAME]
Task:    [FEATURE ID e.g. F2.2 — NDI Source Discovery & Ingest]

## 2. ARCHITECTURE INVARIANTS (never violate these)
1. GStreamer pipeline runs in its own Tokio task — Tauri UI events NEVER block the media thread pool
2. Event bus is the ONLY channel — all state transitions go through tokio::sync::broadcast AppEvent enum
3. Config is TOML only — no hard-coded pipeline topology, scenes, or WHIP endpoints
4. Coexistence first — new subsystems are additive; do NOT break Mevo Studio/ProPresenter integration
5. Observability from day one — all GStreamer bus errors forward to AppEvent within 100ms

## 3. COMPLETED & STABLE (treat as ground truth, do not regenerate)
# List each completed feature ID + one-line status:
- F0.1 ✓  Tauri v2 workspace, two windows (controller 1280×800, presentation 1920×1080)
- F0.2 ✓  IPC command layer — typed invoke<T>() with AppError discriminated union
- F0.3 ✓  tokio::sync::broadcast AppEvent bus, Tauri emit_all() bridge
- F0.4 ✓  TOML config loader with notify v6 hot-reload, keyring for secrets
- [ADD COMPLETED FEATURES HERE]

## 4. CURRENT TASK
# Paste directly from roadmap:
Feature:   [FEATURE ID AND NAME]
Goal:      [ONE SENTENCE GOAL]
Technologies:
  - [technology + version]
  - [technology + version]
Acceptance Criteria:
  - [EXACT criterion from roadmap — verbatim]
  - [EXACT criterion from roadmap — verbatim]
  - [EXACT criterion from roadmap — verbatim]

## 5. INTERFACE CONTRACTS (what this feature must integrate with)
# Types and events the new code will read from or write to:
// AppEvent variants relevant to this feature:
AppEvent::SlideUpdate(SlidePayload)   // emitted by slide state machine
AppEvent::PipelineError { element, message }  // GStreamer errors

// Tauri commands this feature exposes or calls:
invoke('slide_advance') -> Result<(), AppError>
invoke('slide_rewind')  -> Result<(), AppError>

// Rust structs/traits to conform to:
struct SlidePayload { index: u32, title: String, body: String, notes: String, background_path: Option<PathBuf> }

## 6. SCOPE BOUNDARY (do NOT touch)
- Do NOT modify the AppEvent enum (it is non_exhaustive — add variants only if required by this feature)
- Do NOT change tauri.conf.json
- Do NOT refactor existing Tauri command handlers
- Do NOT generate UI for features outside this task

## 7. OUTPUT CONTRACT
Generate:
  - src-tauri/src/[module].rs  (Rust implementation)
  - controller/src/components/[Component].tsx  (React UI if applicable)
Format:  Code blocks only, one file per block, filename as the code block language tag
Tests:   Include unit tests in the same Rust file under #[cfg(test)]
Do NOT:  Provide prose explanation unless I ask — code only
```

---

## Mini-Handoff Template (Sub-Feature / Bug Fix)

For single-file fixes or small sub-features — same 7 blocks, just condensed.

> **Rule of thumb:** If you can describe the task's acceptance criteria in under 5 bullet points, use the mini handoff. If the task touches 2+ subsystems or introduces a new module, use the full master template.

```
# STREAMDECK MINI HANDOFF — [sub-feature / bug]
Stack:        Tauri v2.11 · Rust 1.78 · GStreamer 1.26 · React 18 / TS 5
Invariants:   Event bus only · GStreamer in own Tokio task · TOML config · no breaking external tool changes
Stable:       F0.1–F[LAST COMPLETED] all passing CI
Fix/Task:     [ONE SENTENCE — what exactly needs to change]
File(s):      src-tauri/src/[module].rs
Must NOT change: [list of files/modules to leave untouched]
AC:
  - [test condition 1]
  - [test condition 2]
Output: Code diff / replacement block only, no prose
```

---

## Context Anti-Patterns to Avoid

### ❌ Pasting the Full Roadmap
The 40KB roadmap eats your entire context window. The AI reads all of Phase 5 even when you're working on Phase 0. Paste only the current feature's section + the invariants header.

### ❌ "Continue from where we left off"
New threads have no memory. This prompt forces the AI to hallucinate the prior state. Always provide an explicit completed-features list instead — it takes 60 seconds and saves 10 minutes of corrections.

### ❌ No Scope Boundary Declaration
Without an explicit "do not touch" list, Claude will refactor your working IPC layer while implementing NDI discovery. It's being helpful — but it breaks stable code. Always lock scope explicitly.

### ❌ Vague Task Description ("implement NDI")
The AI will invent requirements. Paste the exact acceptance criteria from the roadmap — they're testable, specific, and already match your architecture. Verbatim beats paraphrased every time.

### ❌ Sharing Interface Details Inside Code Comments
If your interface contract is buried in code comments or inline, the model deprioritises it. Put types and event signatures in a dedicated block before any code. Named blocks = higher attention weight.

---

## Phase Completion Checklist

Run through this at the end of every phase or sub-feature before starting the next AI session.

- [ ] **All acceptance criteria tested** — every bullet in the roadmap's AC section has a passing unit/integration test
- [ ] **AppEvent enum reviewed** — new variants are additive, `#[non_exhaustive]` is maintained, no variants removed
- [ ] **TOML schema updated** — any new config fields have default values and migration handled
- [ ] **Feature flag verified** — feature compiles independently under its Cargo feature flag (`phase2`, `phase3`…)
- [ ] **External tools still operational** — Mevo Studio / ProPresenter still work alongside the new feature
- [ ] **CI passes** — `cargo test`, `cargo clippy -D warnings`, `npm run typecheck` all green
- [ ] **Observability wired** — new subsystem errors surface as `AppEvent` within 100ms
- [ ] **Update the completed-features list** — add the new feature ID to your handoff template's Block 3

---

## Phase-Gate Handoff Snapshots

Pre-filled Block 3 for each phase boundary. Copy the relevant snapshot directly into the **Completed & Stable** block of the master template.

### Phase 0 Complete → Starting Phase 1

```
## PHASE 0 COMPLETE — use this as Block 3 when starting Phase 1
- F0.1 ✓  Tauri v2 workspace · controller (1280×800) + presentation (1920×1080) windows · cargo tauri dev OK
- F0.2 ✓  IPC layer · invoke<T>() wrappers · AppError discriminated union · TS types in sync · <2ms dispatch
- F0.3 ✓  tokio::sync::broadcast(256) AppEvent bus · emit_all() bridge · BusLag backpressure · 1000-event test OK
- F0.4 ✓  TOML config loader · notify v6 hot-reload (<500ms) · keyring for WHIP tokens · config v0→v1 migration
## Phase 0 gate: CI green · no GStreamer dep yet · Mevo + ProPresenter untouched
```

### Phase 1 Complete → Starting Phase 2

```
## PHASE 1 COMPLETE — use this as Block 3 when starting Phase 2
- F0.1–F0.4 ✓  (see Phase 0 — stable, do not regenerate)
- F1.1 ✓  Presentation Window · fullscreen · decorations:false · CSS Grid slide zones · listen('slide:update')
- F1.2 ✓  SlideStateMachine · RwLock · advance/rewind/jump · DeckEnd/DeckStart events · state serialised to disk
- F1.3 ✓  Controller Slide Panel · SlideDeck / SlideThumb / SlidePreview / SpeakerNotes / Timer components
- F1.4 ✓  Slide import (.pptx / .pdf) · 50-slide import <10s · SlideDeck JSON v1 schema · error dialogs
- F1.5 ✓  Multi-display detection · operator assigns Presentation Window per display · persisted in TOML
## Phase 1 gate: ProPresenter still active in parallel · no GStreamer yet
```

### Phase 2 Complete → Starting Phase 3

```
## PHASE 2 COMPLETE — use this as Block 3 when starting Phase 3
- F0.1–F1.5 ✓  (Phases 0–1 stable)
- F2.1 ✓  GStreamer 1.26 init · PipelineService actor · tokio::task::spawn_blocking · start/stop 100× no leak
- F2.2 ✓  ndisrc element · NDI 6.3 New Format Stream · discovery polling 2s · Mevo Start/Core/Go detected <5s
- F2.3 ✓  Camera preview grid · appsink→base64 PNG @8fps · up to 8 streams · <40% CPU on 8-core
- F2.4 ✓  WHIP server axum 0.8 · RFC 9725 · Larix Broadcaster connect <3s · QR code in controller
## Phase 2 gate: Mevo Studio still handles streaming · GStreamer preview only, no egress yet
```

### Phase 3 Complete → Starting Phase 4

```
## PHASE 3 COMPLETE — use this as Block 3 when starting Phase 4
- F0.1–F2.4 ✓  (Phases 0–2 stable)
- F3.1 ✓  compositor element · d3d12/glvideomixer GPU path · 1920×1080@30fps · <33ms source-to-composite
- F3.2 ✓  Presentation slide capture · AVFoundation/D3D12/PipeWire platform impls · <2 frame lag
- F3.3 ✓  SceneManager · TOML scene defs · switch <2 frames · 4 transition types · controller thumbnail grid
- F3.4 ✓  AdaptiveOverlayEngine · SlideClassifier (Title/Visual/Content/Transition) · WCAG AA contrast verified
## Phase 3 gate: Mevo Studio still handles streaming egress · compositor is local-only
```

### Phase 4 Complete → Starting Phase 5

```
## PHASE 4 COMPLETE — use this as Block 3 when starting Phase 5
- F0.1–F3.4 ✓  (Phases 0–3 stable)
- F4.1 ✓  whipclientsink · vtenc/mfh264/vaapi HW encoder · FB Live stream live <8s · HW CPU <10%
- F4.2 ✓  GStreamer tee fan-out · 3 WHIP destinations · 1.15× bandwidth · auto-reconnect 30s backoff
- F4.3 ✓  StreamPanel React · Go Live 2-step · bitrate sparkline 60s · dropped-frame counter · EOS on end
- F4.4 ✓  SRT ingest relay · srtsrc listener · 10% packet loss OK · SRT+NDI coexist
## Phase 4 gate: Mevo Studio STREAMING RETIRED · Mevo cameras still used via NDI · ProPresenter optional
```

### Phase 5 Complete → Starting Phase 6

```
## PHASE 5 COMPLETE — use this as Block 3 when starting Phase 6
- F0.1–F4.4 ✓  (Phases 0–4 stable)
- F5.1 ✓  SlideCanvas React · CSS absolute layers · 6 SlideTransition types @60fps · 4K support
- F5.2 ✓  SlideEditor contentEditable · slide_update_text invoke · delta layer JSON · undo/redo
- F5.3 ✓  TemplatePicker · 8 built-in templates · variables system · .streamdeck ZIP format
- F5.4 ✓  ProPresenterBridge · TCP port 50001 · slide events <200ms · auto-reconnect polling
## Phase 5 gate: ProPresenter fully optional · all external tools optional except Mevo cameras (NDI)
```

---

## Tech Stack Quick Reference

Include relevant rows from this table in Block 5 (Interface Contracts) when needed.

| Layer | Technology | Version | First Phase |
|---|---|---|---|
| App shell | Tauri | v2.11.x | P0 |
| Backend lang | Rust + Tokio | 1.78 / 1.38 | P0 |
| Frontend | TypeScript + React | 5.x / 18.x | P0 |
| Event bus | tokio::sync::broadcast | built-in | P0 |
| Config | TOML + serde | latest | P0 |
| Secrets | keyring | 3.x | P0 |
| Frontend state | Zustand | 5.x | P1 |
| Media pipeline | GStreamer | 1.26.x | P2 |
| NDI | NDI SDK + gst-plugin-ndi | 6.3 / 0.14.x | P2 |
| WebRTC ingest | whipserversrc | 0.14.x | P2 |
| WHIP HTTP server | axum | 0.8 | P2 |
| WebRTC egress | whipclientsink | 0.14.x | P4 |
| Charts | Chart.js | 4.x | P4 |

---

## Architecture Invariants (Copy-Paste Ready)

Always include Block 2 verbatim — these 5 rules prevent the most common AI-generated anti-patterns in a Tauri + GStreamer architecture.

```
## ARCHITECTURE INVARIANTS — include in every AI session

1. PROCESS SEPARATION: GStreamer pipeline runs in a dedicated Tokio task.
   Tauri UI events NEVER block the media pipeline thread pool.
   No GStreamer calls from command handlers — use PipelineService actor channel.

2. EVENT BUS ONLY: ALL state transitions (slide change, scene cut, stream start/stop)
   flow through tokio::sync::broadcast AppEvent enum.
   No direct function calls across subsystem boundaries.

3. CONFIG AS CODE: Pipeline topology, scene definitions, overlay zones, WHIP endpoints
   are TOML files. No hard-coded behaviour anywhere.
   WHIP bearer tokens go in OS keychain (keyring crate), never in TOML plaintext.

4. COEXISTENCE FIRST: New subsystems are additive.
   Mevo Studio and ProPresenter continue working until the phase that explicitly retires them.
   No breaking changes to external tool integration until the replacement passes all AC.

5. OBSERVABILITY FROM DAY ONE: All GStreamer bus messages (Error, Warning, Latency, QoS)
   translate to AppEvent variants visible in the controller UI.
   New subsystems must wire their errors to AppEvent within 100ms of occurrence.
```
