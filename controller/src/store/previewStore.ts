import { create } from 'zustand';
import type { PreviewFrame, PreviewLayout, PreviewSnapshot, SourceStatsSample } from '../lib/ipc';

interface PreviewStore {
  frames: Record<string, PreviewFrame>;
  stats: Record<string, SourceStatsSample>;
  layout: PreviewLayout;
  setFrame: (frame: PreviewFrame) => void;
  setStats: (stats: SourceStatsSample) => void;
  setSnapshot: (snapshot: PreviewSnapshot) => void;
  forget: (name: string) => void;
  setLayout: (layout: PreviewLayout) => void;
}

const byName = <T,>(items: T[], key: (t: T) => string): Record<string, T> =>
  Object.fromEntries(items.map((i) => [key(i), i]));

export const usePreviewStore = create<PreviewStore>((set) => ({
  frames: {},
  stats: {},
  layout: '2x2',
  setFrame: (frame) => set((s) => ({ frames: { ...s.frames, [frame.source]: frame } })),
  setStats: (stats) => set((s) => ({ stats: { ...s.stats, [stats.name]: stats } })),
  // Snapshot is the backend's latest, so it wins; it is how a woken UI catches up.
  setSnapshot: ({ frames, stats }) =>
    set((s) => ({
      frames: { ...s.frames, ...byName(frames, (f) => f.source) },
      stats: { ...s.stats, ...byName(stats, (t) => t.name) },
    })),
  forget: (name) =>
    set((s) => {
      const { [name]: _f, ...frames } = s.frames;
      const { [name]: _s, ...stats } = s.stats;
      return { frames, stats };
    }),
  setLayout: (layout) => set({ layout }),
}));
