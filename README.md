# StreamDeck

Cross-platform desktop application for live presentation control, multi-camera
production, and live-streaming egress. Built on Tauri v2 + Rust/Tokio + React/TypeScript.

For architecture, invariants, and day-to-day working conventions in this repo, see
[`CLAUDE.md`](./CLAUDE.md). For the full feature/phase roadmap, see
[`dev-docs/streamdeck_product_roadmap_2026.md`](./dev-docs/streamdeck_product_roadmap_2026.md).

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | pinned via [`rust-toolchain.toml`](./rust-toolchain.toml) | Install via [rustup](https://rustup.rs) — the pinned toolchain auto-installs the first time you run `cargo`/`rustc` inside this repo. |
| Node.js | 20.x | Managed via [fnm](https://github.com/Schniz/fnm) is recommended (matches CI's `actions/setup-node`). |
| Tauri CLI | v2 | `cargo install tauri-cli --version "^2" --locked` |
| `cargo-deny` | latest | `cargo install cargo-deny --locked` — only needed to run the dependency/license/security check locally; CI runs it regardless. |

**Platform-specific system packages** (only needed to build/run natively, not to edit code):
- **Linux**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf` (see `.github/workflows/ci.yml` for the exact `apt-get` invocation)
- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows**: [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11) + the MSVC build tools (Visual Studio Build Tools, "Desktop development with C++" workload)

### Windows / Git Bash PATH note

On Windows, `cargo`/`rustup` live under `/c/Users/<you>/.cargo/bin` and Node is managed
via `fnm` — **neither is on a non-interactive Git Bash shell's default PATH**, and
`$HOME` in MSYS2/Git Bash resolves to the MSYS home (`/c/msys64/home/<you>`), not your
Windows user profile, so `$HOME/.cargo/bin` silently doesn't exist either. If a plain
`cargo --version` or `node --version` comes back `command not found` in a fresh shell,
run:

```bash
export PATH="/c/Users/<you>/.cargo/bin:$PATH"
eval "$(fnm env --shell bash)"
```

This repo's own tooling (`.claude/hooks/*.sh`, any automation invoking bash
non-interactively) needs this too. The durable fix is a `$BASH_ENV`-pointed file (bash
only auto-sources `.bashrc` for *interactive* shells — non-interactive ones, including
one-shot script/tool invocations, only read whatever `$BASH_ENV` points at) containing
just the export lines, with `BASH_ENV` set as a persistent Windows user environment
variable pointing at it.

## Setup

```bash
git clone <repo-url>
cd streamdeck
npm run install:all        # npm install --prefix controller && --prefix presentation
```

## Running the app

```bash
npm run dev                # == cargo tauri dev — launches both windows
```

Tauri's config only has one `devUrl` (controller's); the presentation window's dev
server is started separately by `scripts/dev-both.cjs` (wired via `beforeDevCommand`).
See CLAUDE.md's "Multi-window dev/build wiring" section for why this matters and what
has broken here before — **a manual `cargo tauri dev` launch is required before
considering any multi-window or IPC change done**, since `verify.sh` cannot exercise
this at all.

```bash
npm run build               # == cargo tauri build — production bundle
```

## Testing

```bash
# Rust — always independent of frontend state
cd src-tauri && cargo test

# Frontend unit/integration tests (Vitest + @tauri-apps/api/mocks)
npm test --prefix controller
npm test --prefix presentation
npm run test:watch --prefix controller   # watch mode while iterating
```

Frontend tests mock the Tauri IPC/event bridge (`mockIPC` from `@tauri-apps/api/mocks`,
plus a small hand-rolled `onAppEvent` fake for the event side) rather than needing a
real webview — see `controller/src/test/eventWiring.integration.test.tsx` and
`presentation/src/test/appEventWiring.integration.test.tsx` for the pattern used to test
several components reacting to the same emitted event together.

## Linting

```bash
cd src-tauri && cargo clippy -- -D warnings
npm run lint --prefix controller      # Biome
npm run lint --prefix presentation    # Biome
npm run lint:fix --prefix controller  # auto-fix what Biome can
```

## Security / dependency hygiene

```bash
cd src-tauri && cargo deny check      # advisories, license compliance, banned crates, sources
```

- **Dependabot** (`.github/dependabot.yml`) opens weekly PRs for cargo, both npm
  projects, and GitHub Actions dependencies.
- **`cargo-deny`** (`src-tauri/deny.toml`) runs in CI on every push/PR and checks the
  Rust dependency tree for known vulnerabilities (RustSec advisory DB), license
  compliance, banned/duplicate crates, and untrusted sources. Findings that are real and
  fixable should be fixed (bump the dependency), not silenced — `ignore` entries in
  `deny.toml` are reserved for genuinely unfixable transitive issues (unmaintained
  crates with no upstream replacement yet), each with a `reason` explaining why.
- Secrets (WHIP bearer tokens, etc.) belong in the OS keychain via the `keyring` crate —
  **never** in `streamdeck.toml` or any committed file.

## The full Definition-of-Done gate

None of the individual commands above is sufficient on its own to call a change done —
`scripts/verify.sh` is:

```bash
bash scripts/verify.sh              # full check, current phase
bash scripts/verify.sh f1.4         # one feature's checks (+ prerequisites)
bash scripts/verify.sh core         # build + test + lint + cargo-deny, no structural checks
bash scripts/verify.sh invariants   # architecture invariants only
```

It bootstraps npm installs and frontend builds (hash-cached), then runs Rust
build/test/clippy, TypeScript typecheck, Biome lint, Vitest, and `cargo deny check`
(non-blocking locally if `cargo-deny` isn't installed — CI always runs it). See
CLAUDE.md's "Verification" section for the full breakdown and known limits (it cannot
catch multi-window dev/build wiring bugs — that needs a manual launch, see above).

## CI

`.github/workflows/ci.yml` runs on every push/PR to `main`:

- **`rust`** — build + test + clippy, matrixed over Ubuntu/macOS/Windows
- **`cargo-deny`** — advisories/licenses/bans/sources
- **`frontend`** — TypeScript typecheck + Biome lint + Vitest, both windows

A `PreToolUse` git hook (`.claude/hooks/pre-push-verify.sh`) also runs
`verify.sh core` before any `git push` from within a Claude Code session and blocks the
push on failure.
