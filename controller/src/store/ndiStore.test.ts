import { beforeEach, describe, expect, it } from 'vitest';
import { useNdiStore } from './ndiStore';

const src = (name: string) => ({ name, url: null, advertises_new_format: false });

beforeEach(() => useNdiStore.setState({ discovered: [], added: [], unavailable: null, lastError: null }));

describe('ndiStore', () => {
  it('markAdded is idempotent and setLost only touches the named source', () => {
    const s = useNdiStore.getState();
    s.markAdded('A', 'Standard');
    s.markAdded('A', 'Standard');
    s.markAdded('B', 'NewFormat');
    s.setLost('A', true);
    expect(useNdiStore.getState().added).toEqual([
      { name: 'A', format: 'Standard', lost: true },
      { name: 'B', format: 'NewFormat', lost: false },
    ]);
  });

  it('markRemoved drops the source and setSnapshot replaces everything', () => {
    const s = useNdiStore.getState();
    s.setSnapshot({ discovered: [src('A')], added: [{ name: 'A', format: 'Standard', lost: false }], unavailable: 'x' });
    expect(useNdiStore.getState().unavailable).toBe('x');
    s.markRemoved('A');
    expect(useNdiStore.getState().added).toEqual([]);
    expect(useNdiStore.getState().discovered).toEqual([src('A')]);
  });
});
