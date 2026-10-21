import { beforeEach, describe, expect, it } from 'vitest';
import { IDLE_COMPOSITOR, useCompositorStore } from './compositorStore';

beforeEach(() => useCompositorStore.setState({ status: IDLE_COMPOSITOR, scenes: [], lastError: null }));

describe('compositorStore', () => {
  it('starts stopped at 1920x1080@30 with no scenes or error', () => {
    const s = useCompositorStore.getState();
    expect(s.status).toMatchObject({ running: false, width: 1920, height: 1080, fps: 30, sources: [] });
    expect(s.scenes).toEqual([]);
    expect(s.lastError).toBeNull();
  });

  it('records status, scenes and error independently', () => {
    useCompositorStore.getState().setScenes(['keynote']);
    useCompositorStore.getState().setLastError('boom');
    useCompositorStore.getState().setStatus({ ...IDLE_COMPOSITOR, running: true, element: 'compositor' });
    expect(useCompositorStore.getState()).toMatchObject({
      scenes: ['keynote'], lastError: 'boom', status: { running: true, element: 'compositor' },
    });
  });
});
