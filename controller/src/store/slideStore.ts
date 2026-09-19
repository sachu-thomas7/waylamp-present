import { create } from 'zustand';
import type { SlidePayload } from '../lib/ipc';

interface ControllerSlideStore {
  current: SlidePayload | null;
  slides: SlidePayload[];
  atDeckStart: boolean;
  atDeckEnd: boolean;
  elapsedSecs: number | null;
  isCollapsed: boolean;
  setSlide: (payload: SlidePayload) => void;
  setSlides: (slides: SlidePayload[]) => void;
  setAtDeckEnd: (val: boolean) => void;
  setAtDeckStart: (val: boolean) => void;
  setElapsedSecs: (secs: number) => void;
  resetTimer: () => void;
  toggleCollapse: () => void;
  importProgress: { slide: number; total: number } | null;
  importError: { slide: number; reason: string } | null;
  setImportProgress: (progress: { slide: number; total: number } | null) => void;
  setImportError: (error: { slide: number; reason: string } | null) => void;
}

export const useControllerSlideStore = create<ControllerSlideStore>((set) => ({
  current: null,
  slides: [],
  atDeckStart: false,
  atDeckEnd: false,
  elapsedSecs: null,
  isCollapsed: false,
  importProgress: null,
  importError: null,
  setSlide: (payload) => set({ current: payload, atDeckEnd: false, atDeckStart: false }),
  setSlides: (slides) => set({ slides }),
  setAtDeckEnd: (val) => set({ atDeckEnd: val }),
  setAtDeckStart: (val) => set({ atDeckStart: val }),
  setElapsedSecs: (secs) => set({ elapsedSecs: secs }),
  resetTimer: () => set({ elapsedSecs: 0 }),
  toggleCollapse: () => set((s) => ({ isCollapsed: !s.isCollapsed })),
  setImportProgress: (progress) => set({ importProgress: progress }),
  setImportError: (error) => set({ importError: error }),
}));