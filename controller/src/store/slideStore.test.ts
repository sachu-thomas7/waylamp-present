import { beforeEach, describe, expect, it } from 'vitest';
import type { SlidePayload } from '../lib/ipc';
import { useControllerSlideStore } from './slideStore';

const samplePayload: SlidePayload = {
  index: 0,
  total: 3,
  title: 'Intro',
  body: 'Welcome',
  notes: '',
  background_path: null,
  layout_unsupported: false,
};

function resetStore() {
  useControllerSlideStore.setState({
    current: null,
    slides: [],
    atDeckStart: false,
    atDeckEnd: false,
    elapsedSecs: null,
    isCollapsed: false,
    importProgress: null,
    importError: null,
  });
}

describe('useControllerSlideStore', () => {
  beforeEach(resetStore);

  it('setSlide clears both deck-boundary flags', () => {
    // DeckEnd/DeckStart are emitted instead of SlideUpdate at boundaries (see CLAUDE.md
    // hard-won lessons) — a real SlideUpdate always means we've left the boundary.
    useControllerSlideStore.getState().setAtDeckEnd(true);
    useControllerSlideStore.getState().setAtDeckStart(true);

    useControllerSlideStore.getState().setSlide(samplePayload);

    const state = useControllerSlideStore.getState();
    expect(state.current).toEqual(samplePayload);
    expect(state.atDeckEnd).toBe(false);
    expect(state.atDeckStart).toBe(false);
  });

  it('resetTimer zeroes elapsedSecs regardless of prior value', () => {
    useControllerSlideStore.getState().setElapsedSecs(42);
    useControllerSlideStore.getState().resetTimer();
    expect(useControllerSlideStore.getState().elapsedSecs).toBe(0);
  });

  it('setImportProgress and setImportError are independent', () => {
    useControllerSlideStore.getState().setImportProgress({ slide: 2, total: 10 });
    useControllerSlideStore.getState().setImportError({ slide: 5, reason: 'bad layout' });

    const state = useControllerSlideStore.getState();
    expect(state.importProgress).toEqual({ slide: 2, total: 10 });
    expect(state.importError).toEqual({ slide: 5, reason: 'bad layout' });
  });

  it('toggleCollapse flips isCollapsed', () => {
    expect(useControllerSlideStore.getState().isCollapsed).toBe(false);
    useControllerSlideStore.getState().toggleCollapse();
    expect(useControllerSlideStore.getState().isCollapsed).toBe(true);
    useControllerSlideStore.getState().toggleCollapse();
    expect(useControllerSlideStore.getState().isCollapsed).toBe(false);
  });
});
