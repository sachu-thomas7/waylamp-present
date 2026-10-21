import { create } from 'zustand';
import type { CompositorStatus } from '../lib/ipc';

export const IDLE_COMPOSITOR: CompositorStatus = {
  running: false, element: null, hardware: false, width: 1920, height: 1080, fps: 30, sources: [],
};

interface CompositorStore {
  status: CompositorStatus;
  scenes: string[];
  lastError: string | null;
  setStatus: (status: CompositorStatus) => void;
  setScenes: (scenes: string[]) => void;
  setLastError: (message: string | null) => void;
}

export const useCompositorStore = create<CompositorStore>((set) => ({
  status: IDLE_COMPOSITOR,
  scenes: [],
  lastError: null,
  setStatus: (status) => set({ status }),
  setScenes: (scenes) => set({ scenes }),
  setLastError: (lastError) => set({ lastError }),
}));
