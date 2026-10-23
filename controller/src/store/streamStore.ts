import { create } from 'zustand';
import { streamGetConfig, streamGetStatus, type StreamConfigView, type StreamStatus } from '../lib/ipc';

interface StreamStore {
  config: StreamConfigView | null;
  status: StreamStatus | null;
  lastError: string | null;
  setConfig: (config: StreamConfigView) => void;
  setStatus: (status: StreamStatus) => void;
  patchHealth: (id: string, bitrateKbps: number, droppedFrames: number) => void;
  setLastError: (message: string | null) => void;
}

export const useStreamStore = create<StreamStore>((set) => ({
  config: null,
  status: null,
  lastError: null,
  setConfig: (config) => set({ config }),
  setStatus: (status) => set({ status }),
  patchHealth: (id, bitrate_kbps, dropped_frames) =>
    set((s) =>
      s.status
        ? {
            status: {
              ...s.status,
              destinations: s.status.destinations.map((d) => (d.id === id ? { ...d, bitrate_kbps, dropped_frames } : d)),
            },
          }
        : {},
    ),
  setLastError: (lastError) => set({ lastError }),
}));

export async function refreshStream(): Promise<void> {
  const [config, status] = await Promise.all([streamGetConfig(), streamGetStatus()]);
  useStreamStore.getState().setConfig(config);
  useStreamStore.getState().setStatus(status);
}
