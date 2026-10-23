#!/usr/bin/env bash
# Real-engine streaming test: starts a local MediaMTX RTMP server in Docker, streams to it with GStreamer, tears down.
# Needs Docker and the GStreamer SDK (see CLAUDE.md for PATH/PKG_CONFIG_PATH). Local only; not part of CI.
set -euo pipefail
cd "$(dirname "$0")/.."

NAME=streamdeck-e2e-mediamtx
CONF="$(pwd -W 2>/dev/null || pwd)/scripts/mediamtx-e2e.yml"
docker rm -f "$NAME" >/dev/null 2>&1 || true
MSYS_NO_PATHCONV=1 docker run -d --rm --name "$NAME" -p 1935:1935 -p 9997:9997 -v "$CONF:/mediamtx.yml:ro" bluenviron/mediamtx:1 >/dev/null
trap 'rc=$?; [ $rc -ne 0 ] && docker logs --tail 40 "$NAME" 2>&1 | sed "s/^/[mediamtx] /"; docker stop "$NAME" >/dev/null 2>&1 || true' EXIT

for _ in $(seq 1 30); do
  curl -fsS http://127.0.0.1:9997/v3/paths/list >/dev/null 2>&1 && break
  sleep 1
done

cd src-tauri
STREAMDECK_RTMP_URL=rtmp://127.0.0.1:1935/live STREAMDECK_MEDIAMTX_API=127.0.0.1:9997 \
  cargo test --features gstreamer --test stream_gst_e2e -- --ignored --nocapture --test-threads=1
