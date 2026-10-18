import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { expectCalled, baseHandlers, makeDeck, makeDisplay, makeSlide, resetStores } from './test/componentHelpers';
import { emitAppEvent, installMockIPC, renderApp, type IpcHandler } from './test/mockTauri';

let ipc: ReturnType<typeof installMockIPC>;

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  renderApp();
}

function loadedHandlers(total = 3): Record<string, IpcHandler> {
  const deck = makeDeck(total);
  return { slide_get_current: () => deck[0], deck_get_all_slides: () => deck };
}

beforeEach(resetStores);
afterEach(() => {
  expect(ipc.unhandled).toEqual([]);
  vi.restoreAllMocks();
});

describe('App first run (empty deck)', () => {
  it('shows the empty deck card, Present disabled, and the Scene Media panel', async () => {
    mount();
    expect(await screen.findByText('No slides yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import a Deck' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Present' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Push Image' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
  });
});

describe('App loaded deck', () => {
  it('shows the deck, enables Present and invokes presentation_show', async () => {
    const user = userEvent.setup();
    mount({ ...loadedHandlers(), presentation_show: () => null });
    expect(await screen.findByText('1 / 3')).toBeInTheDocument();
    const present = screen.getByRole('button', { name: 'Present' });
    await waitFor(() => expect(present).toBeEnabled());
    await user.click(present);
    await expectCalled(ipc, 'presentation_show', [{}]);
  });

  it('Present flips to Stop Presenting on the visibility event', async () => {
    mount(loadedHandlers());
    await screen.findByText('1 / 3');
    await emitAppEvent({ type: 'PresentationVisibility', payload: { visible: true } });
    expect(await screen.findByRole('button', { name: 'Stop Presenting' })).toBeEnabled();
  });
});

describe('App popovers', () => {
  it('keeps Settings content hidden until opened, then shows build info', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByText('No slides yet');
    expect(screen.queryByText('Build Info')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Settings/ }));
    expect(await screen.findByText('Version v1.2.3')).toBeInTheDocument();
    expect(screen.getByText('Timestamp 2026-10-18T00:00:00Z')).toBeInTheDocument();
    expect(screen.getByText('Displays')).toBeInTheDocument();
  });

  it('shows Loading… until build info resolves', async () => {
    const user = userEvent.setup();
    mount({ get_build_info: () => new Promise(() => {}) });
    await user.click(await screen.findByRole('button', { name: /Settings/ }));
    expect(await screen.findByText('Loading…')).toBeInTheDocument();
  });

  it('shows an IPC error when build info fails', async () => {
    const user = userEvent.setup();
    mount({
      get_build_info: () => {
        throw new Error('boom');
      },
    });
    await user.click(await screen.findByRole('button', { name: /Settings/ }));
    expect(await screen.findByText(/IPC error:.*boom/)).toBeInTheDocument();
  });

  it('lists displays inside Settings', async () => {
    const user = userEvent.setup();
    mount({ display_list: () => [makeDisplay('d1', { name: 'Laptop', is_primary: true })] });
    await user.click(await screen.findByRole('button', { name: /Settings/ }));
    expect(await screen.findByText('Laptop')).toBeInTheDocument();
  });

  it('lists the keyboard shortcuts in Help', async () => {
    const user = userEvent.setup();
    mount();
    await user.click(await screen.findByRole('button', { name: 'Keyboard shortcuts' }));
    expect(await screen.findByText('Advance slide')).toBeInTheDocument();
    expect(screen.getByText('Rewind slide')).toBeInTheDocument();
    expect(screen.getByText('Exit presentation')).toBeInTheDocument();
  });
});

describe('App Clear Deck', () => {
  it('invokes deck_clear and returns to the empty state when the backend publishes the empty slide', async () => {
    const user = userEvent.setup();
    mount({ ...loadedHandlers(), deck_clear: () => null });
    await screen.findByText('1 / 3');
    await user.click(screen.getByRole('button', { name: /Settings/ }));
    await user.click(await screen.findByRole('button', { name: 'Clear Deck' }));
    await expectCalled(ipc, 'deck_clear', [{}]);
    await emitAppEvent({ type: 'SlideUpdate', payload: makeSlide(0, 0, { title: '', body: '' }) });
    expect(await screen.findByText('No slides yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Present' })).toBeDisabled();
  });

  it('logs a failed deck_clear and keeps the deck', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    mount({
      ...loadedHandlers(),
      deck_clear: () => {
        throw new Error('nope');
      },
    });
    await screen.findByText('1 / 3');
    await user.click(screen.getByRole('button', { name: /Settings/ }));
    await user.click(await screen.findByRole('button', { name: 'Clear Deck' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });
});
