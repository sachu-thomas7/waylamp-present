# Next session: NDI

Read first: `CLAUDE.md`, `dev-docs/owner_decisions_2026-10-23.md`, `dev-docs/unverified_items.md` (items 1, 17, 26-29).

## Goal
Prove NDI ingest end to end **without the Mevo camera**, using the free NDI Tools test patterns, then add NDI audio.
Windows only. The church network is closed, so everything we ship must install offline.

## Steps
1. **Runtime.** Install the NDI runtime (Tools gives you Studio Monitor and Test Patterns). Confirm `ndisrc`/`ndisink` are in the
   GStreamer install (they were on 2026-10-23: `gst-inspect-1.0 ndisrc`) and that `NDI_RUNTIME_DIR_V*` is set.
2. **Discovery and ingest.** `cargo tauri dev --features ndi,gstreamer` (needs GStreamer `bin` on PATH and `PKG_CONFIG_PATH`; see
   CLAUDE.md). Start NDI Tools Test Patterns, check the source appears in Settings > NDI sources, add it, see the preview tile.
   Known gaps to fix or confirm: `advertises_new_format` is always false, `format` does not change `ndisrc`, source lost/return.
3. **Quality numbers** on this machine: preview fps, per-source latency, CPU. Compare 1 and 4 sources.
4. **NDI into the compositor and the stream.** NDI source -> compositor layout -> `program` channel -> a local MediaMTX stream
   (`scripts/stream-e2e.sh` pattern). Record what works.
5. **NDI audio.** Add audio from `ndisrc` to the stream engine (`AudioSource::Ndi`), keep `Silence` as the default, leave OS audio
   devices and the mixer for later. A/V sync offset belongs here.
6. **Installer plan (design only):** bundle the NDI redistributable as an optional installer step with its licence shown, link to
   ndi.video next to every NDI picker, trademark notice in About, never bundle NDI Tools. Read the EULA shipped with the SDK.
7. Update `dev-docs/unverified_items.md` with what was proven and what still needs the camera.

## Needs the owner
Installing the NDI Tools and runtime; deciding the open-source licence (owner said "similar to Linux"; GPL-3.0-or-later is the
recommendation because Rust crates under Apache-2.0 are compatible with GPLv3 but not GPLv2). Adding a `LICENSE` file waits for that.

## Camera checks (later, when the Mevo arrives)
NDI keeps running after the iPad app closes? iPad control and StreamDeck video at once? Ethernet vs Wi-Fi quality, NDI|HX latency,
`ndisrc` HX decode, SRT setting present in the app?
