import { create } from 'zustand';
import type { PipelineState } from '../lib/ipc';

interface PipelineStore {
  state: PipelineState;
  lastError: string | null;
  setState: (state: PipelineState) => void;
  setLastError: (message: string | null) => void;
}

export const usePipelineStore = create<PipelineStore>((set) => ({
  state: 'Null',
  lastError: null,
  setState: (state) => set({ state }),
  setLastError: (lastError) => set({ lastError }),
}));
