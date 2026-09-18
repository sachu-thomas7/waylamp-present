import { create } from 'zustand';
import type { SlidePayload } from '../lib/events';

interface SlideStore {
  current: SlidePayload | null;
  setSlide: (payload: SlidePayload) => void;
}

export const useSlideStore = create<SlideStore>((set) => ({
  current: null,
  setSlide: (payload) => set({ current: payload }),
}));