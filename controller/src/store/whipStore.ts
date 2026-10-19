import { create } from 'zustand';
import { whipGetInfo, type QrMatrix, type WhipInfo } from '../lib/ipc';
import { useNdiStore } from './ndiStore';

interface WhipStore {
  info: WhipInfo | null;
  qr: Record<string, QrMatrix>;
  lastError: string | null;
  setInfo: (info: WhipInfo) => void;
  setQr: (slot: string, qr: QrMatrix) => void;
  setLastError: (message: string | null) => void;
}

export const useWhipStore = create<WhipStore>((set) => ({
  info: null,
  qr: {},
  lastError: null,
  setInfo: (info) => set({ info }),
  setQr: (slot, qr) => set((s) => ({ qr: { ...s.qr, [slot]: qr } })),
  setLastError: (lastError) => set({ lastError }),
}));

export function whipSlotsInUse(info: WhipInfo | null): Set<string> {
  return new Set((info?.slots ?? []).filter((s) => s.session).map((s) => s.slot));
}

// Phone slots live in the same source list as NDI sources, so a refresh also seeds that store.
export async function refreshWhip(): Promise<void> {
  const info = await whipGetInfo();
  useWhipStore.getState().setInfo(info);
  const ndi = useNdiStore.getState();
  for (const { slot, session } of info.slots) {
    if (!session) continue;
    ndi.markAdded(slot, 'Standard');
    ndi.setLost(slot, session.state === 'Lost');
  }
}
