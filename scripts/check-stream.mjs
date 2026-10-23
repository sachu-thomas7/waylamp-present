// Real-app smoke for streaming (local only): drives a running `cargo tauri dev --features gstreamer` over WebView2 CDP,
// adds a custom RTMP destination pointing at the local MediaMTX from scripts/mediamtx-e2e.yml, goes live, and checks the
// server sees H.264 + AAC. It removes the destination (and its keychain entry) afterwards.
//   docker run -d --rm --name sd-mtx -p 1935:1935 -p 9997:9997 -v <repo>/scripts/mediamtx-e2e.yml:/mediamtx.yml:ro bluenviron/mediamtx:1
//   node scripts/check-stream.mjs
const port = process.env.CDP_PORT ?? '9222';
const api = process.env.MTX_API ?? 'http://127.0.0.1:9997';
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const pages = targets.filter((t) => t.type === 'page');
const controllerTarget = pages.find((t) => !(t.url.includes(':5174') || t.url.includes('/presentation/')));

const ws = new WebSocket(controllerTarget.webSocketDebuggerUrl);
const pending = new Map();
let id = 0;
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  pending.get(d.id)?.(d);
};
await new Promise((r) => (ws.onopen = r));
const evaluate = async (expression) => {
  const i = ++id;
  ws.send(JSON.stringify({ id: i, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
  const r = await new Promise((res) => pending.set(i, res));
  if (r.result.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description);
  return r.result.result.value;
};
const invoke = (cmd, args) =>
  evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args ?? {})}).then((v) => ({ ok: v }), (e) => ({ err: e }))`,
  );
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const paths = async () => (await (await fetch(`${api}/v3/paths/list`)).json()).items ?? [];

let failed = 0;
const check = (name, ok, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  (${detail})`}`);
  if (!ok) failed++;
};

const dest = { id: 'e2e-local', name: 'Local test server', platform: 'custom', ingest_url: 'rtmp://127.0.0.1:1935/live', enabled: true };
const KEY = 'app-smoke';
try {
  const saved = await invoke('stream_save_destination', { destination: dest, key: KEY });
  check('destination saved, key reported only as key_set', saved.ok?.destinations?.some((d) => d.id === dest.id && d.key_set) && !JSON.stringify(saved).includes(KEY), JSON.stringify(saved));

  const started = await invoke('stream_start');
  check('stream_start succeeds', started.ok?.state === 'Live', JSON.stringify(started));
  console.log(`      encoder: ${started.ok?.encoder} (hardware: ${started.ok?.hardware})`);

  let live = false;
  for (let i = 0; i < 30 && !live; i++) {
    await sleep(500);
    live = (await paths()).some((p) => p.name === `live/${KEY}` && p.ready);
  }
  check('server sees the published stream', live);
  const p = (await paths()).find((x) => x.name === `live/${KEY}`);
  const tracks = JSON.stringify(p?.tracks ?? []);
  check('H.264 video + AAC audio tracks', tracks.includes('H264') && (tracks.includes('MPEG-4') || tracks.includes('AAC')), tracks);

  await sleep(3000);
  const status = await invoke('stream_get_status');
  const d = status.ok?.destinations?.find((x) => x.id === dest.id);
  check('destination reports Live with a bitrate', d?.state?.state === 'Live' && d.bitrate_kbps > 0, JSON.stringify(d));
} finally {
  const stopped = await invoke('stream_stop');
  check('stream_stop succeeds', stopped.ok?.state === 'Stopped', JSON.stringify(stopped));
  await sleep(1500);
  check('server no longer sees the stream', !(await paths()).some((p) => p.name === `live/${KEY}` && p.ready));
  const cleaned = await invoke('stream_remove_destination', { id: dest.id });
  check('test destination removed', !cleaned.ok?.destinations?.some((d) => d.id === dest.id), JSON.stringify(cleaned));
}
process.exit(failed ? 1 : 0);
