# StreamDeck — AI Handoff Document
## Feature F0.2 · IPC Command Layer

**Document Type:** AI Session Handoff  
**Phase:** Phase 0 — Project Skeleton & Toolchain  
**Feature:** F0.2 — IPC Command Layer  
**Prerequisite:** F0.1 must be completed and passing CI before starting this session  
**Date:** May 2026  

---

## ═══════════════════════════════════════════
## BLOCK 1 — PROJECT IDENTITY
## ═══════════════════════════════════════════

```
Project : StreamDeck — cross-platform live streaming + presentation desktop app
Mission : Unify local presentation control, multi-camera production, and live-stream egress
          into a single Tauri app. Zero dependency on Mevo Studio / ProPresenter by Phase 5.

Stack   : Tauri v2.11.x  ·  Rust 1.78 / Tokio 1.38  ·  React 18.x / TypeScript 5.x  ·  Vite 5.x
Phase   : Phase 0 — Project Skeleton & Toolchain  (no media pipeline yet)
Task    : F0.2 — IPC Command Layer
```

---

## ═══════════════════════════════════════════
## BLOCK 2 — ARCHITECTURE INVARIANTS (never violate)
## ═══════════════════════════════════════════

```
1. PROCESS SEPARATION
   GStreamer pipeline (added in Phase 2) runs in a dedicated Tokio task.
   Tauri UI events NEVER block the media pipeline thread pool.
   No GStreamer calls from command handlers — use PipelineService actor channel.
   [Not relevant in Phase 0, but command handler patterns established here must
    remain compatible with this constraint in all future phases.]

2. EVENT BUS ONLY
   ALL state transitions (slide change, scene cut, stream start/stop) flow through
   a tokio::sync::broadcast AppEvent enum.
   No direct function calls across subsystem boundaries.
   [F0.3 wires the bus; F0.2 establishes the command layer that feeds into it.]

3. CONFIG AS CODE
   Pipeline topology, scene definitions, overlay zones, and WHIP endpoints are TOML files.
   No hard-coded behaviour.
   WHIP bearer tokens stored in OS keychain (keyring crate), never in TOML plaintext.

4. COEXISTENCE FIRST
   New subsystems are additive. Mevo Studio and ProPresenter continue operating until
   the phase that explicitly retires them. No breaking changes to external tool integration.

5. OBSERVABILITY FROM DAY ONE
   All errors surface as typed AppEvent variants within 100 ms.
   Command handlers return Result<T, AppError> — never unwrap() or panic in handlers.
```

---

## ═══════════════════════════════════════════
## BLOCK 3 — COMPLETED & STABLE STATE
## ═══════════════════════════════════════════

> Treat everything below as ground truth. Do NOT regenerate, refactor, or modify these.

```
F0.1 ✓  Tauri v2 workspace bootstrapped
        - Rust backend at src-tauri/
        - Two frontend entry points: controller/ and presentation/
        - Vite 5.x + React 18.x + TypeScript 5.x wired for both frontends
        - tauri.conf.json defines two windows:
            controller   → default,  1280×800
            presentation → secondary, 1920×1080
        - `cargo tauri dev` launches both windows simultaneously ✓
        - Both windows render their React apps and show build timestamp ✓
        - `cargo tauri build` produces signed installer on macOS + Windows ✓
        - core:webview:allow-create-webview-window declared in
          src-tauri/capabilities/default.json ✓
        - CI (GitHub Actions) runs cargo test, cargo clippy -- -D warnings,
          npm run typecheck on every PR ✓
```

**External tools at this stage:** Mevo Studio handles all streaming. ProPresenter handles all
presentation. StreamDeck has no media functionality yet.

---

## ═══════════════════════════════════════════
## BLOCK 4 — CURRENT TASK
## ═══════════════════════════════════════════

### Feature ID: F0.2 — IPC Command Layer

**Description:**  
Define the Tauri command interface that all subsequent features will extend.
Establish typed request/response envelopes so the frontend never calls raw Rust functions.
This layer becomes the stable contract that every future phase (NDI ingest, compositing,
streaming egress) builds on — the patterns established here must scale to 30+ commands.

### Technologies

| Technology | Version | Role |
|---|---|---|
| `#[tauri::command]` macro | Tauri v2.11.x | Expose Rust functions to frontend |
| `serde` + `serde_json` | latest | Serialise/deserialise command payloads |
| TypeScript `invoke<T>()` | `@tauri-apps/api` v2.x | Typed frontend call wrapper |
| `ts-rs` crate (or manual) | latest | Generate TS types from Rust structs |
| `AppError` enum | custom | Discriminated union surfaced to TypeScript |

### Acceptance Criteria (verbatim from roadmap)

- [ ] `ping` command returns `{ ok: true, version: string }` from Rust to both windows
- [ ] All commands return `Result<T, AppError>` — no `unwrap()` in command handlers
- [ ] TypeScript types are generated or maintained in sync with Rust structs (enforced by CI typecheck)
- [ ] Command dispatch latency < 2 ms measured in unit test

### Definition of Done

All four acceptance criteria above must be passing in CI before this feature is marked complete
and before work on F0.3 (Event Bus Foundation) begins.

---

## ═══════════════════════════════════════════
## BLOCK 5 — INTERFACE CONTRACTS
## ═══════════════════════════════════════════

### Rust Types to Define (new in this feature)

```rust
// src-tauri/src/error.rs
// AppError is the single error type for ALL Tauri commands across all phases.
// Use #[non_exhaustive] so future phases can add variants without breaking existing match arms.

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "type", content = "message")]
#[non_exhaustive]
pub enum AppError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // Future phases will extend this — do NOT add phase-specific variants here yet.
    // Phase 2 will add: PipelineError, SourceNotFound
    // Phase 4 will add: StreamError, EncoderError
}

// Required: impl From<AppError> for tauri error serialisation bridge
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(e: AppError) -> Self {
        tauri::ipc::InvokeError::from_anyhow(e.into())
    }
}
```

```rust
// src-tauri/src/commands/system.rs
// The ping command — validates the IPC layer end-to-end.
// Must be registered in src-tauri/src/lib.rs under .invoke_handler(tauri::generate_handler![...])

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct PingResponse {
    pub ok: bool,
    pub version: String,
}

#[tauri::command]
pub async fn ping() -> Result<PingResponse, AppError> {
    Ok(PingResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
```

### TypeScript Interface Contract

```typescript
// controller/src/lib/ipc.ts  (and mirrored in presentation/src/lib/ipc.ts)
// Typed wrapper around Tauri's invoke — every command goes through this module.
// Never call invoke() directly from components.

import { invoke } from '@tauri-apps/api/core';

// Generated by ts-rs or maintained manually in sync with Rust AppError
export type AppError =
  | { type: 'Internal';      message: string }
  | { type: 'NotFound';      message: string }
  | { type: 'InvalidInput';  message: string };

export interface PingResponse {
  ok: boolean;
  version: string;
}

// Typed invoke wrapper — add one function per command. Never use raw invoke() in UI code.
export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>('ping');
}
```

### File Structure This Feature Produces

```
src-tauri/
├── src/
│   ├── error.rs                  ← AppError enum (NEW)
│   ├── commands/
│   │   ├── mod.rs                ← pub mod system; (NEW)
│   │   └── system.rs             ← ping command (NEW)
│   └── lib.rs                    ← register commands in invoke_handler (MODIFY)
├── bindings/                     ← ts-rs generated TypeScript types (NEW)
│   └── PingResponse.ts
controller/
└── src/
    └── lib/
        └── ipc.ts                ← typed invoke wrapper (NEW)
presentation/
└── src/
    └── lib/
        └── ipc.ts                ← same typed wrapper (NEW)
```

### Existing Files to Read (for context only — do not modify)

```
src-tauri/tauri.conf.json         — window definitions (READ ONLY)
src-tauri/Cargo.toml              — workspace deps (READ: add thiserror, ts-rs if absent)
src-tauri/capabilities/default.json — IPC capability grants (READ ONLY)
```

---

## ═══════════════════════════════════════════
## BLOCK 6 — SCOPE BOUNDARY
## ═══════════════════════════════════════════

```
DO NOT implement or scaffold:
  ✗  F0.3 Event Bus (tokio::sync::broadcast) — that is the next feature
  ✗  F0.4 TOML config loader — separate feature
  ✗  Any GStreamer imports, pipeline code, or NDI references
  ✗  Any slide, scene, or streaming commands — those belong to Phase 1–4
  ✗  UI components beyond the minimal ping test button (if generating UI at all)
  ✗  Window management or display logic

DO NOT modify:
  ✗  tauri.conf.json
  ✗  src-tauri/capabilities/default.json
  ✗  Existing CI workflow files (.github/workflows/)
  ✗  controller/src/App.tsx or presentation/src/App.tsx (unless adding a ping test call)

The AppError enum must use #[non_exhaustive].
Do NOT add phase-specific error variants (PipelineError, StreamError) — those come later.
```

---

## ═══════════════════════════════════════════
## BLOCK 7 — OUTPUT CONTRACT
## ═══════════════════════════════════════════

### Files to Generate

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/error.rs` | **CREATE** | `AppError` enum with `thiserror` + `serde` + `#[non_exhaustive]` |
| `src-tauri/src/commands/mod.rs` | **CREATE** | `pub mod system;` |
| `src-tauri/src/commands/system.rs` | **CREATE** | `ping` command + `PingResponse` struct with `ts_rs::TS` derive |
| `src-tauri/src/lib.rs` | **MODIFY** | Register `commands::system::ping` in `invoke_handler![]` |
| `src-tauri/Cargo.toml` | **MODIFY** | Add `thiserror`, `ts-rs` to `[dependencies]` if not present |
| `controller/src/lib/ipc.ts` | **CREATE** | Typed `invoke` wrappers + `AppError` TS type |
| `presentation/src/lib/ipc.ts` | **CREATE** | Same as controller ipc.ts |

### Files to Generate: Tests

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/commands/system.rs` | Include `#[cfg(test)]` block | Test: `ping()` resolves to `ok: true`, version non-empty |
| `src-tauri/src/commands/system.rs` | Include latency test | Measure `ping` dispatch < 2 ms via `std::time::Instant` |

### Output Format

```
- One fenced code block per file
- Use the file path as the language identifier comment at the top of each block:
    ```rust
    // src-tauri/src/error.rs
    ...
    ```
- Include #[cfg(test)] unit tests inside the same Rust files (not separate test files)
- Do NOT provide prose explanation unless I ask for it
- Do NOT show partial diffs — show the complete file content for every file
```

### Verification Commands (run after generation)

```bash
# These must all pass before F0.2 is considered complete:
cargo test                          # unit tests including ping latency test
cargo clippy -- -D warnings         # zero warnings
npm run typecheck                   # TypeScript types in sync with Rust
cargo tauri dev                     # both windows launch, ping responds in DevTools
```

---

## CONTEXT NOTES FOR THE AI

> Read this section before generating any code.

1. **`AppError` is permanent infrastructure.** It will be extended by every subsequent phase.
   Design it for extensibility: `#[non_exhaustive]`, `serde` tag + content discriminated union,
   `thiserror` for display. Do not make it minimal to the point of being unextendable.

2. **The `ipc.ts` wrapper pattern is mandatory.** No component in either frontend ever calls
   `invoke()` directly. All commands are exposed as named async functions in `ipc.ts`.
   This is enforced by convention now and will be enforced by ESLint rule in Phase 1.

3. **Type sync strategy:** Use `ts-rs` for generating TypeScript from Rust structs if the
   crate is available. If you choose manual types instead, add a comment marking each TS
   interface as `// @sync-with: src-tauri/src/commands/system.rs PingResponse` so CI
   typecheck can flag drift.

4. **Both windows get the same `ipc.ts`.** The controller and presentation windows have
   separate React apps but share the same IPC contract. The `ipc.ts` file is identical
   in both — consider whether a shared `packages/ipc/` workspace package would be
   cleaner, but only introduce that complexity if the project structure already has a
   monorepo packages layout.

5. **`lib.rs` vs `main.rs`:** Tauri v2 uses `lib.rs` as the entry point for the backend
   (not `main.rs`). The `invoke_handler![]` macro registration goes in the `Builder`
   chain inside `lib.rs`.

