import { create } from 'zustand';
import type { DisplayInfo } from '../lib/ipc';

interface DisplayStore {
  displays: DisplayInfo[];
  assignedDisplayId: string | null;
  lostWarning: boolean;
  setDisplays: (displays: DisplayInfo[]) => void;
  setAssignedDisplayId: (id: string | null) => void;
  setLostWarning: (lost: boolean) => void;
}

export const useDisplayStore = create<DisplayStore>((set) => ({
  displays: [],
  assignedDisplayId: null,
  lostWarning: false,
  setDisplays: (displays) => set({ displays }),
  setAssignedDisplayId: (assignedDisplayId) => set({ assignedDisplayId }),
  setLostWarning: (lostWarning) => set({ lostWarning }),
}));
