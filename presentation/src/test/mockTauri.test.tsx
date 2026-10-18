import { invoke } from '@tauri-apps/api/core';
import { screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { onAppEvent, type AppEvent } from '../lib/events';
import { emitAppEvent, installMockIPC, renderApp } from './mockTauri';

describe('mockTauri helpers (presentation)', () => {
  it('intercepts invoke, passes args, and returns the handler result', async () => {
    const { calls } = installMockIPC({ greet: (args) => `hi ${args.name}` });
    await expect(invoke('greet', { name: 'ada' })).resolves.toBe('hi ada');
    expect(calls).toEqual([{ cmd: 'greet', args: { name: 'ada' } }]);
  });

  it('rejects commands without a handler', async () => {
    installMockIPC({});
    await expect(invoke('nope')).rejects.toThrow(/unhandled IPC command in test: nope/);
  });

  it('delivers emitted app events to every onAppEvent listener', async () => {
    installMockIPC();
    const a: AppEvent[] = [];
    const b: AppEvent[] = [];
    await onAppEvent((e) => a.push(e));
    await onAppEvent((e) => b.push(e));
    const event: AppEvent = { type: 'Ping', payload: { message: 'hi' } };
    await emitAppEvent(event);
    expect(a).toEqual([event]);
    expect(b).toEqual([event]);
  });

  it('stops delivering after unlisten', async () => {
    installMockIPC();
    const seen: AppEvent[] = [];
    const unlisten = await onAppEvent((e) => seen.push(e));
    unlisten();
    await emitAppEvent({ type: 'Ping', payload: { message: 'late' } });
    expect(seen).toEqual([]);
  });

  it('renders the real App from events alone and never invokes a command', async () => {
    const { calls, unhandled } = installMockIPC();
    renderApp();
    await emitAppEvent({
      type: 'SlideUpdate',
      payload: {
        index: 0,
        total: 1,
        title: 'Welcome',
        body: 'Body text',
        notes: '',
        background_path: null,
        layout_unsupported: false,
      },
    });
    await waitFor(() => expect(screen.getByText('Welcome')).toBeInTheDocument());
    expect(calls).toEqual([]);
    expect(unhandled).toEqual([]);
  });
});

describe('mock isolation between tests (ordered pair)', () => {
  it('A: installs IPC and a live listener', async () => {
    installMockIPC({ ping: () => 'pong' });
    await onAppEvent(() => {});
    await expect(invoke('ping')).resolves.toBe('pong');
  });

  it('B: the previous test mocks are gone', async () => {
    expect(
      (window as unknown as { __TAURI_INTERNALS__?: { invoke?: unknown } }).__TAURI_INTERNALS__?.invoke,
    ).toBeUndefined();
    await expect(invoke('ping')).rejects.toThrow();
  });

  it('jsdom provides WebCrypto getRandomValues for mockIPC', () => {
    expect(typeof window.crypto.getRandomValues).toBe('function');
  });
});
