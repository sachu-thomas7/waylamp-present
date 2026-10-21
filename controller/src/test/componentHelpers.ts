import { waitFor } from '@testing-library/react';
import { expect } from 'vitest';
import type { DisplayInfo, SlidePayload, WhipInfo } from '../lib/ipc';
import { useWhipStore } from '../store/whipStore';
import { useControllerSlideStore } from '../store/slideStore';
import { useDisplayStore } from '../store/displayStore';
import { useSceneStore } from '../store/sceneStore';
import { usePipelineStore } from '../store/pipelineStore';
import { useNdiStore } from '../store/ndiStore';
import { usePreviewStore } from '../store/previewStore';
import { IDLE_COMPOSITOR, useCompositorStore } from '../store/compositorStore';
import type { IpcCall, IpcHandler } from './mockTauri';

export function resetStores() {
  useControllerSlideStore.setState({
    current: null, slides: [], atDeckStart: false, atDeckEnd: false, elapsedSecs: null,
    isCollapsed: false, importProgress: null, importError: null,
  });
  useDisplayStore.setState({ displays: [], assignedDisplayId: null, lostWarning: false });
  useSceneStore.setState({ activeSource: 'SlideDeck', currentMedia: null });
  usePipelineStore.setState({ state: 'Null', lastError: null });
  useNdiStore.setState({ discovered: [], added: [], unavailable: null, lastError: null });
  usePreviewStore.setState({ frames: {}, stats: {}, layout: '2x2' });
  useWhipStore.setState({ info: null, qr: {}, lastError: null });
  useCompositorStore.setState({ status: IDLE_COMPOSITOR, scenes: [], lastError: null });
}

export function whipInfo(over: Partial<WhipInfo> = {}): WhipInfo {
  const port = over.port ?? 8088;
  return {
    enabled: false, running: false, port, host: '192.168.1.20', token_set: false, unavailable: null, last_error: null,
    slots: [1, 2, 3, 4].map((n) => ({ slot: `camera${n}`, url: `http://192.168.1.20:${port}/whip/camera${n}`, session: null })),
    ...over,
  };
}

export function makeSlide(index: number, total: number, over: Partial<SlidePayload> = {}): SlidePayload {
  return {
    index, total, title: `Slide ${index + 1} title`, body: `Slide ${index + 1} body`,
    notes: '', background_path: null, layout_unsupported: false, ...over,
  };
}

export function makeDeck(total: number, over: Record<number, Partial<SlidePayload>> = {}): SlidePayload[] {
  return Array.from({ length: total }, (_, i) => makeSlide(i, total, over[i]));
}

export function makeDisplay(id: string, over: Partial<DisplayInfo> = {}): DisplayInfo {
  return { id, name: `Monitor ${id}`, width: 1920, height: 1080, x: 0, y: 0, is_primary: false, ...over };
}

// Baseline handlers for every command mounted panels call on startup.
export function baseHandlers(over: Record<string, IpcHandler> = {}): Record<string, IpcHandler> {
  return {
    get_build_info: () => ({ build_timestamp: '2026-10-18T00:00:00Z', version: '1.2.3' }),
    slide_get_current: () => null,
    deck_get_all_slides: () => [],
    scene_get_active_source: () => 'SlideDeck',
    display_list: () => [],
    pipeline_get_state: () => 'Null',
    ndi_list_sources: () => ({ discovered: [], added: [], unavailable: null }),
    preview_get_snapshot: () => ({ frames: [], stats: [] }),
    preview_get_layout: () => '2x2',
    whip_get_info: () => whipInfo(),
    compositor_get_state: () => IDLE_COMPOSITOR,
    compositor_list_scenes: () => [],
    ...over,
  };
}

export async function expectCalled(ipc: { calls: IpcCall[] }, cmd: string, argsList: Array<Record<string, unknown>>) {
  await waitFor(() =>
    expect(ipc.calls.filter((c) => c.cmd === cmd)).toEqual(argsList.map((args) => ({ cmd, args }))),
  );
}
