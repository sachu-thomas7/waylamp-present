import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { baseHandlers, makeDeck, makeDisplay, resetStores } from '../test/componentHelpers';
import { installMockIPC } from '../test/mockTauri';
import { useDisplayStore } from '../store/displayStore';
import { useControllerSlideStore } from '../store/slideStore';
import { useSceneStore } from '../store/sceneStore';
import { resyncFromBackend } from './resync';

beforeEach(resetStores);
afterEach(() => vi.restoreAllMocks());

describe('resyncFromBackend', () => {
  it('re-reads slide, deck, scene and display state from the backend', async () => {
    const deck = makeDeck(4);
    installMockIPC(
      baseHandlers({
        slide_get_current: () => deck[2],
        deck_get_all_slides: () => deck,
        scene_get_active_source: () => 'SceneMedia',
        display_list: () => [makeDisplay('d1')],
      }),
    );

    await resyncFromBackend();

    expect(useControllerSlideStore.getState().current?.index).toBe(2);
    expect(useControllerSlideStore.getState().slides).toHaveLength(4);
    expect(useSceneStore.getState().activeSource).toBe('SceneMedia');
    expect(useDisplayStore.getState().displays.map((d) => d.id)).toEqual(['d1']);
  });

  it('one failing command does not stop the others', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const deck = makeDeck(2);
    installMockIPC(
      baseHandlers({
        deck_get_all_slides: () => deck,
        display_list: () => {
          throw new Error('boom');
        },
      }),
    );

    await resyncFromBackend();

    expect(useControllerSlideStore.getState().slides).toHaveLength(2);
    expect(spy).toHaveBeenCalled();
  });
});
