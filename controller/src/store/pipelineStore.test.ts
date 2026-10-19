import { beforeEach, describe, expect, it } from 'vitest';
import { usePipelineStore } from './pipelineStore';

beforeEach(() => usePipelineStore.setState({ state: 'Null', lastError: null }));

describe('pipelineStore', () => {
  it('starts idle with no error', () => {
    expect(usePipelineStore.getState().state).toBe('Null');
    expect(usePipelineStore.getState().lastError).toBeNull();
  });

  it('records state and error independently', () => {
    usePipelineStore.getState().setState('Playing');
    usePipelineStore.getState().setLastError('boom');
    expect(usePipelineStore.getState()).toMatchObject({ state: 'Playing', lastError: 'boom' });
  });
});
