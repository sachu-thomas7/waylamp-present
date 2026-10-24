# Layer templates (adapt, don't paste blindly)

Skeletons only. Each was written from the Tauri / Playwright docs or from existing repo
tests; items marked **(verify)** have not yet been compiled in this repo — confirm against
current docs and fix this file when you do.

## L2 JS — component test with mockIPC + event registry

Canonical working example: `controller/src/test/eventWiring.integration.test.tsx`
(mocks `../lib/events` with an in-memory handler registry, resets Zustand stores,
`emitAppEvent` wraps handlers in `act`). Skeleton:

```tsx
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

const { handlers } = vi.hoisted(() => ({ handlers: [] as Array<(e: AppEvent) => void> }));
vi.mock('../lib/events', () => ({
  onAppEvent: (h: (e: AppEvent) => void) => {
    handlers.push(h);
    return Promise.resolve(() => handlers.splice(handlers.indexOf(h), 1));
  },
}));

const calls: Array<{ cmd: string; args: unknown }> = [];
beforeEach(() => {
  handlers.length = 0; calls.length = 0;
  mockIPC((cmd, args) => { calls.push({ cmd, args }); /* return per-cmd fixtures */ });
});
afterEach(() => { cleanup(); clearMocks(); });

it('Clear Deck invokes deck_clear', async () => {
  render(<App />);
  await userEvent.click(screen.getByRole('button', { name: /settings/i }));
  await userEvent.click(await screen.findByRole('button', { name: /clear deck/i }));
  expect(calls.some((c) => c.cmd === 'deck_clear')).toBe(true);
});
```
Tauri's `mockIPC(..., { shouldMockEvents: true })` (2.7+) can replace the registry for simple
cases; `emitTo`/`emit_filter` are unsupported. jsdom lacks WebCrypto — polyfill in setup.

## L2 JS — presentation is listen-only

```tsx
const invokeSpy = vi.fn();
mockIPC((cmd) => { invokeSpy(cmd); });
render(<App />);
emitAppEvent({ type: 'SlideUpdate', payload: slide(0, 'Hello', 'World') });
expect(await screen.findByText('Hello')).toBeInTheDocument();
expect(invokeSpy).not.toHaveBeenCalled();
```

## L2 Rust — command → bus → payload (verify)

Prefer extracting `fn build_state(data_dir: &Path) -> AppState` from `app.rs` so tests and
the app share wiring. Sketch using `tauri::test`:

```rust
use tauri::test::{mock_builder, mock_context, noop_assets};

#[tokio::test]
async fn deck_clear_publishes_empty_payload() {
    let dir = tempfile::tempdir().unwrap();
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![crate::commands::deck::deck_clear /* … */])
        .build(mock_context(noop_assets()))
        .unwrap();
    // manage() the same state app.rs manages, pointed at `dir`
    let mut rx = bus.subscribe();                 // subscribe BEFORE acting
    // invoke via tauri::test::get_ipc_response(&webview, InvokeRequest{ cmd: "deck_clear", .. })
    let events = drain(&mut rx, Duration::from_millis(200)).await; // deadline, not sleep-and-hope
    assert!(matches!(events[0], AppEvent::SlideUpdate(ref p) if p.total == 0));
}
```
Rules: temp dir for app-data; assert the exact variant **and order**; assert "instead of"
relationships (`DeckEnd` without `SlideUpdate`).

## L3 — golden snapshot of an imported deck

```rust
let deck = import_fixture("with_entities.pptx");
insta::assert_json_snapshot!(deck);   // snapshot reviewed in PR; diff is readable
```
Fixture matrix is listed in roadmap T3.1. A parser bump that changes output must produce a
reviewed snapshot diff, never an auto-accepted one.

## L3 — capability assertion

```rust
let cap: serde_json::Value = serde_json::from_str(include_str!("../capabilities/presentation.json"))?;
let perms: Vec<&str> = cap["permissions"].as_array().unwrap().iter().filter_map(|p| p.as_str()).collect();
assert_eq!(perms, ["core:event:default"]);   // listen-only (invariant 5)
```

## L4 — Playwright over WebView2 CDP (verify during T4.0)

Based on Playwright's WebView2 guide (env var + `connectOverCDP`). Fixture per worker:

```ts
// e2e/fixtures/app.ts
import { test as base, chromium, type Page } from '@playwright/test';
import { spawn, execSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export const test = base.extend<{ app: { controller: Page; presentation: Page; dataDir: string } }>({
  app: async ({}, use, testInfo) => {
    const port = 10000 + testInfo.workerIndex;
    const dataDir = mkdtempSync(path.join(tmpdir(), 'waylamp-present-e2e-'));
    const proc = spawn(process.env.WAYLAMP_BIN!, [], {
      env: {
        ...process.env,
        WAYLAMP_DATA_DIR: dataDir,                       // T4.1 seam
        WEBVIEW2_USER_DATA_FOLDER: path.join(dataDir, 'wv2'),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
      },
    });
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); // retry until up
    const pages = browser.contexts()[0].pages();
    const byTitle = (t: string) => pages.find((p) => p.url().includes(t))!;     // identify windows
    await use({ controller: /* … */, presentation: /* … */, dataDir });
    await browser.close();
    execSync(`taskkill /pid ${proc.pid} /T /F`);            // Windows teardown
    rmSync(dataDir, { recursive: true, force: true });
  },
});
```
Open questions T4.0 must settle: do both windows appear in `pages()`; how to tell them apart
(URL vs title); whether `window.__TAURI_INTERNALS__.invoke` is callable via `page.evaluate`
in the controller (lets tests import a fixture without the native dialog).

Journey skeleton:

```ts
test('J3 navigation tracks on the projector', async ({ app }) => {
  const { controller, presentation } = app;
  await controller.evaluate((p) => (window as any).__TAURI_INTERNALS__.invoke('deck_import', { path: p }), FIXTURE);
  await expect(controller.getByText(/\b3 slides\b/)).toBeVisible();
  await controller.getByRole('button', { name: 'Present' }).click();
  await expect(presentation.getByRole('heading')).toHaveText('Slide 1 title');
  await controller.getByRole('button', { name: /next/i }).click();
  await expect(presentation.getByRole('heading')).toHaveText('Slide 2 title');
});
```
