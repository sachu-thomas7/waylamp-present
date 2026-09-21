import { beforeEach, describe, expect, it } from 'vitest';
import type { DisplayInfo } from '../lib/ipc';
import { useDisplayStore } from './displayStore';

const primary: DisplayInfo = {
  id: 'display-1',
  name: 'Primary',
  width: 1920,
  height: 1080,
  x: 0,
  y: 0,
  is_primary: true,
};

describe('useDisplayStore', () => {
  beforeEach(() => {
    useDisplayStore.setState({ displays: [], assignedDisplayId: null, lostWarning: false });
  });

  it('DisplayAssigned-style update clears any lost warning', () => {
    useDisplayStore.getState().setLostWarning(true);
    useDisplayStore.getState().setAssignedDisplayId(primary.id);
    useDisplayStore.getState().setLostWarning(false);

    const state = useDisplayStore.getState();
    expect(state.assignedDisplayId).toBe(primary.id);
    expect(state.lostWarning).toBe(false);
  });

  it('setDisplays replaces the full list', () => {
    useDisplayStore.getState().setDisplays([primary]);
    expect(useDisplayStore.getState().displays).toEqual([primary]);
  });
});
