import { create } from 'zustand';
import type { SceneMediaPayload, SceneSource } from '../lib/ipc';

interface SceneStore {
  activeSource: SceneSource;
  currentMedia: SceneMediaPayload | null;
  setActiveSource: (source: SceneSource) => void;
  setCurrentMedia: (media: SceneMediaPayload) => void;
}

export const useSceneStore = create<SceneStore>((set) => ({
  activeSource: 'SlideDeck',
  currentMedia: null,
  setActiveSource: (source) => set({ activeSource: source }),
  setCurrentMedia: (media) => set({ currentMedia: media }),
}));