import { waitFor } from '@testing-library/react';
import { expect } from 'vitest';
import type { DisplayInfo, SlidePayload } from '../lib/ipc';
import { useControllerSlideStore } from '../store/slideStore';
import { useDisplayStore } from '../store/displayStore';
import { useSceneStore } from '../store/sceneStore';
import { usePipelineStore } from '../store/pipelineStore';
import type { IpcCall, IpcHandler } from './mockTauri';

export function resetStores() {
  useControllerSlideStore.setState({
    current: null, slides: [], atDeckStart: false, atDeckEnd: false, elapsedSecs: null,
    isCollapsed: false, importProgress: null, importError: null,
  });
  useDisplayStore.setState({ displays: [], assignedDisplayId: null, lostWarning: false });
  useSceneStore.setState({ activeSource: 'SlideDeck', currentMedia: null });
  usePipelineStore.setState({ state: 'Null', lastError: null });
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
    ...over,
  };
}

export async function expectCalled(ipc: { calls: IpcCall[] }, cmd: string, argsList: Array<Record<string, unknown>>) {
  await waitFor(() =>
    expect(ipc.calls.filter((c) => c.cmd === cmd)).toEqual(argsList.map((args) => ({ cmd, args }))),
  );
}
