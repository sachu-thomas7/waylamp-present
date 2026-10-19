import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, expectCalled, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { PipelinePanel } from './PipelinePanel';

let ipc: ReturnType<typeof installMockIPC>;

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<PipelinePanel />);
}

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('PipelinePanel', () => {
  it('idle: shows Stopped with only a Start button', async () => {
    mount();
    expect(await screen.findByText('Stopped')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Stop' })).not.toBeInTheDocument();
  });

  it('Start invokes pipeline_start_test and PipelineStateChanged flips the controls', async () => {
    const user = userEvent.setup();
    mount({ pipeline_start_test: () => null });
    await user.click(await screen.findByRole('button', { name: 'Start' }));
    await expectCalled(ipc, 'pipeline_start_test', [{}]);
    await emitAppEvent({ type: 'PipelineStateChanged', payload: { state: 'Playing' } });
    expect(await screen.findByRole('button', { name: 'Pause' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Stop' })).toBeInTheDocument();
    await emitAppEvent({ type: 'PipelineStateChanged', payload: { state: 'Paused' } });
    expect(await screen.findByRole('button', { name: 'Resume' })).toBeInTheDocument();
  });

  it('shows PipelineError from the backend', async () => {
    mount();
    await screen.findByText('Stopped');
    await emitAppEvent({ type: 'PipelineError', payload: { element: 'filesrc0', message: 'not found' } });
    expect(await screen.findByText('filesrc0: not found')).toBeInTheDocument();
  });

  it('shows a rejected command instead of failing silently', async () => {
    const user = userEvent.setup();
    mount({
      pipeline_start_test: () => {
        throw { type: 'Internal', message: 'GStreamer support not compiled in' };
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Start' }));
    expect(await screen.findByText(/not compiled in/)).toBeInTheDocument();
  });
});
