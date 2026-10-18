import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { expectCalled, baseHandlers, makeDisplay, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { DisplayPanel } from './DisplayPanel';

let ipc: ReturnType<typeof installMockIPC>;

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<DisplayPanel />);
}

const twoDisplays = () => [makeDisplay('d1', { name: 'Laptop', is_primary: true }), makeDisplay('d2', { name: 'Projector', width: 1280, height: 720 })];
const LOST = /Assigned display disconnected/;

beforeEach(resetStores);
afterEach(() => {
  expect(ipc.unhandled).toEqual([]);
  vi.restoreAllMocks();
});

describe('DisplayPanel', () => {
  it('shows the empty message when no displays are detected', async () => {
    mount();
    expect(await screen.findByText('No displays detected')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Assign' })).not.toBeInTheDocument();
    expect(screen.queryByText(LOST)).not.toBeInTheDocument();
  });

  it('lists each display with resolution and primary marker, each with an Assign button', async () => {
    mount({ display_list: twoDisplays });
    expect(await screen.findByText('Laptop')).toBeInTheDocument();
    expect(screen.getByText('1920×1080 · Primary')).toBeInTheDocument();
    expect(screen.getByText('Projector')).toBeInTheDocument();
    expect(screen.getByText('1280×720')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Assign' })).toHaveLength(2);
    expect(screen.queryByText('Assigned')).not.toBeInTheDocument();
  });

  it('Assign invokes display_assign_presentation with the id and marks it Assigned', async () => {
    const user = userEvent.setup();
    mount({ display_list: twoDisplays, display_assign_presentation: () => null });
    await screen.findByText('Projector');
    const buttons = screen.getAllByRole('button', { name: 'Assign' });
    await user.click(buttons[1]);
    await expectCalled(ipc, 'display_assign_presentation', [{ id: 'd2' }]);
    expect(await screen.findByText('Assigned')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Assign' })).toHaveLength(1);
  });

  it('does not mark a display Assigned when the command fails', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    mount({
      display_list: twoDisplays,
      display_assign_presentation: () => {
        throw new Error('save failed');
      },
    });
    await screen.findByText('Projector');
    expect(screen.getAllByRole('button', { name: 'Assign' })).toHaveLength(2);
    await user.click(screen.getAllByRole('button', { name: 'Assign' })[1]);
    await expectCalled(ipc, 'display_assign_presentation', [{ id: 'd2' }]);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    await act(async () => {});
    expect(screen.queryByText('Assigned')).not.toBeInTheDocument();
  });

  it('Assign clears the lost warning locally', async () => {
    const user = userEvent.setup();
    mount({ display_list: twoDisplays, display_assign_presentation: () => null });
    await screen.findByText('Laptop');
    await emitAppEvent({ type: 'DisplayLost', payload: null });
    await screen.findByText(LOST);
    await user.click(screen.getAllByRole('button', { name: 'Assign' })[0]);
    await waitFor(() => expect(screen.queryByText(LOST)).not.toBeInTheDocument());
    expect(screen.getByText('Assigned')).toBeInTheDocument();
  });

  it('DisplaysChanged replaces the list', async () => {
    mount({ display_list: twoDisplays });
    await screen.findByText('Laptop');
    await emitAppEvent({ type: 'DisplaysChanged', payload: [makeDisplay('d3', { name: 'TV' })] });
    expect(await screen.findByText('TV')).toBeInTheDocument();
    expect(screen.queryByText('Laptop')).not.toBeInTheDocument();
  });

  it('DisplayAssigned marks that display Assigned', async () => {
    mount({ display_list: twoDisplays });
    await screen.findByText('Laptop');
    await emitAppEvent({ type: 'DisplayAssigned', payload: { display_id: 'd1' } });
    expect(await screen.findByText('Assigned')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Assign' })).toHaveLength(1);
  });

  it('DisplayLost shows the fallback warning and a later DisplayAssigned clears it', async () => {
    mount({ display_list: twoDisplays });
    await screen.findByText('Laptop');
    await emitAppEvent({ type: 'DisplayLost', payload: null });
    expect(await screen.findByText(LOST)).toBeInTheDocument();
    await emitAppEvent({ type: 'DisplayAssigned', payload: { display_id: 'd1' } });
    await waitFor(() => expect(screen.queryByText(LOST)).not.toBeInTheDocument());
  });
});
