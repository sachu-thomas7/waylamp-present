# StreamDeck

Free, open-source desktop application for controlling slide presentations. Import a
PowerPoint (`.pptx`) or PDF deck, show it fullscreen on a projector or second display,
and run it from a separate controller window with speaker notes, a timer, next-slide
preview and scene media (a background image or video).

Built with Tauri v2, Rust and React/TypeScript. Licensed under
[GPL-3.0-or-later](./LICENSE).

> **Status: pre-release (0.1.0).** The first Windows installer has not been published yet.
> The application works from source on Windows; Linux builds in CI but is untested on real
> hardware; macOS is not built or tested.

## What it does

- **Import** `.pptx` and `.pdf` decks (PDF import needs the bundled PDFium library, see below).
- **Present** fullscreen on the chosen display; the controller window stays on your own screen.
- **Control** from the controller: next/previous, jump to slide, notes, elapsed timer,
  keyboard shortcuts.
- **Scene media**: show a background image or a video behind or instead of slides.
- **Multi-display**: choose which monitor shows the presentation; falls back to the primary
  display if it is unplugged.
- **Offline and private**: no accounts, no telemetry, no network access. See
  [docs/privacy.md](./docs/privacy.md).

Live production features (cameras, NDI, phone ingest, compositor, streaming) are developed
separately on the `feature/live-production` branch and are not part of `main`.

## Build from source

Prerequisites:

| Tool | Notes |
|---|---|
| Rust | The toolchain is pinned in [`rust-toolchain.toml`](./rust-toolchain.toml); [rustup](https://rustup.rs) installs it automatically. |
| Node.js | 20 or newer (CI uses the version in the workflow). |
| Tauri CLI | `cargo install tauri-cli --version "^2" --locked` |

System packages (only to build and run natively):

- **Windows**: MSVC build tools ("Desktop development with C++") and the
  [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11).
- **Linux**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`
  (see [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) for the exact command).

```bash
git clone <repository-url>
cd <repository-directory>
npm ci --prefix controller
npm ci --prefix presentation
cargo tauri dev          # launches both windows
```

PDF import uses the PDFium library. For development, `bash scripts/fetch-deps.sh` downloads a
pinned, checksum-verified copy; release installers bundle it. Installer builds are described
in [dev-docs/release_process.md](./dev-docs/release_process.md).

On Windows with Git Bash, `cargo` and `fnm` are often not on the default PATH of
non-interactive shells. The repository's scripts look for them under `%USERPROFILE%`; if
you still see `command not found`, add `$USERPROFILE/.cargo/bin` to `PATH` and run
`eval "$(fnm env --shell bash)"`.

## Test and verify

```bash
bash scripts/verify.sh quick   # while editing: fmt, cargo check, fast tests
bash scripts/verify.sh push    # before pushing, same as CI: clippy, all tests, typecheck, lint
bash scripts/verify.sh deps    # dependency audit (cargo-deny, osv-scanner; fuzz smoke needs Docker)
```

`verify.sh` cannot launch the app. For any change to window wiring or IPC, also run
`cargo tauri dev` and check both windows by hand.

## Documentation

- [CONTRIBUTING.md](./CONTRIBUTING.md): how to contribute (DCO sign-off, PR checklist)
- [SECURITY.md](./SECURITY.md): reporting vulnerabilities
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)
- [docs/privacy.md](./docs/privacy.md) and [docs/code-signing-policy.md](./docs/code-signing-policy.md)
- [CHANGELOG.md](./CHANGELOG.md)
- [CLAUDE.md](./CLAUDE.md): architecture, invariants and working notes (also used by AI coding assistants)
- [dev-docs/](./dev-docs): roadmap, test plan, release process

## License

GPL-3.0-or-later. See [LICENSE](./LICENSE). Third-party notices are generated for release
builds by `scripts/gen-notices.cjs`.
