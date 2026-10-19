import { invoke } from '@tauri-apps/api/core';
import { screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { onAppEvent, type AppEvent } from '../lib/events';
import { emitAppEvent, installMockIPC, renderApp } from './mockTauri';

describe('mockTauri helpers (controller)', () => {
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
    const event: AppEvent = { type: 'DisplayAssigned', payload: { display_id: 'display-7' } };
    await emitAppEvent(event);
    expect(a).toEqual([event]);
    expect(b).toEqual([event]);
  });

  it('stops delivering after unlisten', async () => {
    installMockIPC();
    const seen: AppEvent[] = [];
    const unlisten = await onAppEvent((e) => seen.push(e));
    unlisten();
    await emitAppEvent({ type: 'DeckEnd', payload: null });
    expect(seen).toEqual([]);
  });

  it('renders the real App against mocked IPC and reacts to events', async () => {
    const { calls, unhandled } = installMockIPC({
      get_build_info: () => ({ version: '0.0.0', tauri_version: '2', os: 'test', arch: 'x', profile: 'debug' }),
      slide_get_current: () => null,
      deck_get_all_slides: () => [],
      scene_get_active_source: () => 'SlideDeck',
      ndi_list_sources: () => ({ discovered: [], added: [], unavailable: null }),
      preview_get_snapshot: () => ({ frames: [], stats: [] }),
      preview_get_layout: () => '2x2',
    });
    renderApp();
    await waitFor(() => expect(screen.getByText('No slides yet')).toBeInTheDocument());
    expect(calls.map((c) => c.cmd).sort()).toEqual([
      'deck_get_all_slides',
      'get_build_info',
      'ndi_list_sources',
      'preview_get_layout',
      'preview_get_snapshot',
      'scene_get_active_source',
      'slide_get_current',
    ]);
    expect(unhandled).toEqual([]);
    const before = calls.length;
    await emitAppEvent({ type: 'PresentationVisibility', payload: { visible: true } });
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Stop Presenting/ })).toBeInTheDocument(),
    );
    expect(calls).toHaveLength(before);
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
