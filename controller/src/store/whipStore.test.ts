import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, resetStores, whipInfo } from '../test/componentHelpers';
import { clearMocks, installMockIPC } from '../test/mockTauri';
import { useNdiStore } from './ndiStore';
import { refreshWhip, useWhipStore, whipSlotsInUse } from './whipStore';

const session = (slot: string, state: 'Connected' | 'Disconnected' | 'Lost') => ({ slot, state, codec: 'H264Baseline' as const });

beforeEach(resetStores);
afterEach(clearMocks);

describe('whipStore', () => {
  it('whipSlotsInUse lists only slots that hold a session', () => {
    const info = whipInfo();
    info.slots[0].session = session('camera1', 'Connected');
    expect([...whipSlotsInUse(info)]).toEqual(['camera1']);
    expect(whipSlotsInUse(null).size).toBe(0);
  });

  it('refreshWhip stores the info and seeds phone slots into the shared source list', async () => {
    const info = whipInfo({ running: true });
    info.slots[0].session = session('camera1', 'Connected');
    info.slots[1].session = session('camera2', 'Lost');
    installMockIPC(baseHandlers({ whip_get_info: () => info }));
    await refreshWhip();
    expect(useWhipStore.getState().info).toEqual(info);
    expect(useNdiStore.getState().added).toEqual([
      { name: 'camera1', format: 'Standard', lost: false },
      { name: 'camera2', format: 'Standard', lost: true },
    ]);
  });

  it('refreshWhip clears a lost mark once the slot is connected again', async () => {
    useNdiStore.getState().markAdded('camera1', 'Standard');
    useNdiStore.getState().setLost('camera1', true);
    const info = whipInfo({ running: true });
    info.slots[0].session = session('camera1', 'Connected');
    installMockIPC(baseHandlers({ whip_get_info: () => info }));
    await refreshWhip();
    expect(useNdiStore.getState().added[0].lost).toBe(false);
  });
});
