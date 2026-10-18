import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { expectCalled, baseHandlers, makeSlide, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { useControllerSlideStore } from '../store/slideStore';
import { PresentButton } from './PresentButton';

let ipc: ReturnType<typeof installMockIPC>;

async function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<PresentButton />);
  await screen.findByRole('button');
}

// Panels feed this store in the app; seed it directly for the isolated button.
function loadDeck(total: number) {
  act(() => useControllerSlideStore.getState().setSlide(makeSlide(0, total)));
}

const callsOf = (cmd: string) => ipc.calls.filter((c) => c.cmd === cmd);

beforeEach(resetStores);
afterEach(() => {
  expect(ipc.unhandled).toEqual([]);
  vi.restoreAllMocks();
});

describe('PresentButton', () => {
  it('is disabled with an explanatory title when no deck is loaded', async () => {
    await mount();
    const btn = screen.getByRole('button', { name: 'Present' });
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute('title', 'Import a deck to enable presenting');
  });

  it('does not invoke when clicked while disabled', async () => {
    const user = userEvent.setup();
    await mount({ presentation_show: () => null });
    await user.click(screen.getByRole('button', { name: 'Present' }));
    expect(callsOf('presentation_show')).toEqual([]);
  });

  it('enables once a deck with slides is current and invokes presentation_show', async () => {
    const user = userEvent.setup();
    await mount({ presentation_show: () => null });
    loadDeck(3);
    const btn = await screen.findByRole('button', { name: 'Present' });
    await waitFor(() => expect(btn).toBeEnabled());
    await user.click(btn);
    await expectCalled(ipc, 'presentation_show', [{}]);
  });

  it('stays in Present state after a click until the visibility event confirms', async () => {
    const user = userEvent.setup();
    await mount({ presentation_show: () => null });
    loadDeck(2);
    await user.click(await screen.findByRole('button', { name: 'Present' }));
    await expectCalled(ipc, 'presentation_show', [{}]);
    expect(screen.getByRole('button', { name: 'Present' })).toBeInTheDocument();
  });

  it('shows Stop Presenting on visibility true and hides via presentation_hide', async () => {
    const user = userEvent.setup();
    await mount({ presentation_hide: () => null });
    await emitAppEvent({ type: 'PresentationVisibility', payload: { visible: true } });
    const stop = await screen.findByRole('button', { name: 'Stop Presenting' });
    expect(stop).toBeEnabled();
    expect(stop).not.toHaveAttribute('title');
    await user.click(stop);
    await expectCalled(ipc, 'presentation_hide', [{}]);
  });

  it('returns to Present when visibility turns false', async () => {
    const user = userEvent.setup();
    await mount({ presentation_show: () => null });
    loadDeck(2);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Present' })).toBeEnabled());
    await emitAppEvent({ type: 'PresentationVisibility', payload: { visible: true } });
    await screen.findByRole('button', { name: 'Stop Presenting' });
    await emitAppEvent({ type: 'PresentationVisibility', payload: { visible: false } });
    const present = await screen.findByRole('button', { name: 'Present' });
    expect(present).toBeEnabled();
    await user.click(present);
    await expectCalled(ipc, 'presentation_show', [{}]);
  });

  it('logs a failed presentation_show and keeps the Present label', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    await mount({
      presentation_show: () => {
        throw new Error('no window');
      },
    });
    loadDeck(2);
    await user.click(await screen.findByRole('button', { name: 'Present' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: 'Present' })).toBeInTheDocument();
  });
});
