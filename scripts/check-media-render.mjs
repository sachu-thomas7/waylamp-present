// Real-app smoke (local only): drives a running `cargo tauri dev` over WebView2 CDP and asserts decks/media actually render.
// Launch the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222, then:
//   node scripts/check-media-render.mjs <repo-root> <video.webm|mp4>
import path from 'node:path';

const [, , root = process.cwd(), video] = process.argv;
const port = process.env.CDP_PORT ?? '9222';
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = (port) => targets.find((t) => t.type === 'page' && t.url.includes(`:${port}`));

function connect(target) {
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  const pending = new Map();
  let id = 0;
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    pending.get(d.id)?.(d);
  };
  const open = new Promise((r) => (ws.onopen = r));
  return async (expression) => {
    await open;
    const i = ++id;
    ws.send(JSON.stringify({ id: i, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
    const r = await new Promise((res) => pending.set(i, res));
    if (r.result.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description);
    return r.result.result.value;
  };
}

const controller = connect(page('5173'));
const presentation = connect(page('5174'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const invoke = (cmd, args) =>
  controller(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)}).then(() => 'ok', (e) => 'ERR ' + JSON.stringify(e))`);
const probe = (sel, prop) => presentation(`(() => { const e = document.querySelector('[data-testid=${sel}]'); return e ? ${prop} : null })()`);

let failed = 0;
const check = (name, ok, detail) => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  (${detail})`}`);
  if (!ok) failed++;
};

const asset = (f) => path.join(root, 'dev-docs/test-assets', f);
await invoke('deck_import', { path: asset('streamdeck-test-deck.pdf') });
await sleep(4000);
await invoke('slide_jump_to', { index: 1 });
await sleep(800);
const bg = await probe('slide-background', 'e.naturalWidth');
check('PDF page background renders (naturalWidth > 0)', bg > 0, `naturalWidth=${bg}`);

check('PPTX import succeeds', (await invoke('deck_import', { path: asset('streamdeck-test-deck.pptx') })) === 'ok', 'import failed');
await sleep(3000);

await invoke('scene_push_image', { path: asset('streamdeck-test-scene-image.png') });
await invoke('scene_set_source', { source: 'SceneMedia' });
await sleep(800);
const img = await probe('scene-media-image', 'e.naturalWidth');
check('scene image renders (naturalWidth > 0)', img > 0, `naturalWidth=${img}`);

if (video) {
  await invoke('scene_push_video', { path: path.resolve(video) });
  await sleep(2000);
  const v = await probe('scene-media-video', '({ ready: e.readyState, err: e.error && e.error.code, w: e.videoWidth })');
  check('scene video loads (readyState >= 2, no error)', v && v.ready >= 2 && !v.err, JSON.stringify(v));
}
await invoke('scene_set_source', { source: 'SlideDeck' });
process.exit(failed ? 1 : 0);
