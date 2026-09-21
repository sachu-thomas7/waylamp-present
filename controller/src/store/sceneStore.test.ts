import { beforeEach, describe, expect, it } from 'vitest';
import type { SceneMediaPayload } from '../lib/ipc';
import { useSceneStore } from './sceneStore';

const sampleMedia: SceneMediaPayload = {
  media_type: 'Image',
  path: '/tmp/example.png',
  display_name: 'example.png',
};

describe('useSceneStore', () => {
  beforeEach(() => {
    useSceneStore.setState({ activeSource: 'SlideDeck', currentMedia: null });
  });

  it('defaults to SlideDeck with no staged media', () => {
    const state = useSceneStore.getState();
    expect(state.activeSource).toBe('SlideDeck');
    expect(state.currentMedia).toBeNull();
  });

  it('setActiveSource and setCurrentMedia update independently', () => {
    useSceneStore.getState().setCurrentMedia(sampleMedia);
    useSceneStore.getState().setActiveSource('SceneMedia');

    const state = useSceneStore.getState();
    expect(state.activeSource).toBe('SceneMedia');
    expect(state.currentMedia).toEqual(sampleMedia);
  });
});
