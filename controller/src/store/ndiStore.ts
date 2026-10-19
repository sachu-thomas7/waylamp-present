import { create } from 'zustand';
import type { AddedSource, NdiSnapshot, NdiSource, StreamFormat } from '../lib/ipc';

interface NdiStore {
  discovered: NdiSource[];
  added: AddedSource[];
  unavailable: string | null;
  lastError: string | null;
  setSnapshot: (snapshot: NdiSnapshot) => void;
  setDiscovered: (sources: NdiSource[]) => void;
  markAdded: (name: string, format: StreamFormat) => void;
  markRemoved: (name: string) => void;
  setLost: (name: string, lost: boolean) => void;
  setLastError: (message: string | null) => void;
}

export const useNdiStore = create<NdiStore>((set) => ({
  discovered: [],
  added: [],
  unavailable: null,
  lastError: null,
  setSnapshot: ({ discovered, added, unavailable }) => set({ discovered, added, unavailable }),
  setDiscovered: (discovered) => set({ discovered }),
  markAdded: (name, format) =>
    set((s) => ({
      added: s.added.some((a) => a.name === name) ? s.added : [...s.added, { name, format, lost: false }],
    })),
  markRemoved: (name) => set((s) => ({ added: s.added.filter((a) => a.name !== name) })),
  setLost: (name, lost) =>
    set((s) => ({ added: s.added.map((a) => (a.name === name ? { ...a, lost } : a)) })),
  setLastError: (lastError) => set({ lastError }),
}));
