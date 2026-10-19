import { beforeEach, describe, expect, it } from 'vitest';
import { usePreviewStore } from './previewStore';

const frame = (source: string, png = 'AAA') => ({ source, width: 320, height: 180, png_base64: png });
const stats = (name: string, fps = 8) => ({ name, width: 1920, height: 1080, fps, latency_ms: 10 });

beforeEach(() => usePreviewStore.setState({ frames: {}, stats: {}, layout: '2x2' }));

describe('previewStore', () => {
  it('keeps the latest frame and stats per source', () => {
    const s = usePreviewStore.getState();
    s.setFrame(frame('a', '1'));
    s.setFrame(frame('a', '2'));
    s.setFrame(frame('b'));
    s.setStats(stats('a', 7));
    expect(usePreviewStore.getState().frames.a.png_base64).toBe('2');
    expect(Object.keys(usePreviewStore.getState().frames)).toEqual(['a', 'b']);
    expect(usePreviewStore.getState().stats.a.fps).toBe(7);
  });

  it('forget drops one source only', () => {
    const s = usePreviewStore.getState();
    s.setFrame(frame('a'));
    s.setFrame(frame('b'));
    s.setStats(stats('a'));
    s.forget('a');
    expect(Object.keys(usePreviewStore.getState().frames)).toEqual(['b']);
    expect(usePreviewStore.getState().stats).toEqual({});
  });

  it('a snapshot replaces matching sources and keeps the others', () => {
    const s = usePreviewStore.getState();
    s.setFrame(frame('a', 'old'));
    s.setFrame(frame('b', 'keep'));
    s.setSnapshot({ frames: [frame('a', 'new')], stats: [stats('a', 5)] });
    const st = usePreviewStore.getState();
    expect(st.frames.a.png_base64).toBe('new');
    expect(st.frames.b.png_base64).toBe('keep');
    expect(st.stats.a.fps).toBe(5);
  });
});
