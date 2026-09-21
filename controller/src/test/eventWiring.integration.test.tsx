import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DisplayPanel } from '../components/DisplayPanel';
import { PresentButton } from '../components/PresentButton';
import { SlideDeckPanel } from '../components/SlideDeckPanel';
import { useControllerSlideStore } from '../store/slideStore';
import { useDisplayStore } from '../store/displayStore';
import type { AppEvent } from '../lib/events';

// Every controller panel subscribes to the same `app-event` stream independently
// (SlideDeckPanel, PresentButton, DisplayPanel, SceneMediaPanel all call onAppEvent).
// Replace it with an in-memory registry so one emitted event fans out to every
// mounted listener, the same way the real Tauri event bus does.
const { handlers } = vi.hoisted(() => ({ handlers: [] as Array<(e: AppEvent) => void> }));

vi.mock('../lib/events', () => ({
  onAppEvent: (handler: (e: AppEvent) => void) => {
    handlers.push(handler);
    return Promise.resolve(() => {
      const i = handlers.indexOf(handler);
      if (i >= 0) handlers.splice(i, 1);
    });
  },
}));

function emitAppEvent(event: AppEvent) {
  act(() => {
    for (const handler of handlers) handler(event);
  });
}

function resetStores() {
  useControllerSlideStore.setState({
    current: null,
    slides: [],
    atDeckStart: false,
    atDeckEnd: false,
    elapsedSecs: null,
    isCollapsed: false,
    importProgress: null,
    importError: null,
  });
  useDisplayStore.setState({ displays: [], assignedDisplayId: null, lostWarning: false });
}

describe('cross-component event wiring (controller)', () => {
  beforeEach(() => {
    handlers.length = 0;
    resetStores();
    mockIPC((cmd) => {
      switch (cmd) {
        case 'slide_get_current':
          return null;
        case 'deck_get_all_slides':
          return [];
        case 'presentation_show':
        case 'presentation_hide':
          return null;
        case 'display_list':
          return [];
        default:
          throw new Error(`unhandled IPC command in test: ${cmd}`);
      }
    });
  });

  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it('a single SlideUpdate event updates the deck panel and unlocks Present together', async () => {
    render(
      <>
        <SlideDeckPanel />
        <PresentButton />
      </>,
    );

    await waitFor(() => expect(screen.getByText('No slides yet')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /Present/ })).toBeDisabled();

    emitAppEvent({
      type: 'SlideUpdate',
      payload: {
        index: 0,
        total: 3,
        title: 'Intro',
        body: '',
        notes: '',
        background_path: null,
        layout_unsupported: false,
      },
    });

    await waitFor(() => expect(screen.getByText('1 / 3')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /Present/ })).not.toBeDisabled();
  });

  it('DeckEnd shows the boundary banner without clobbering the current slide', async () => {
    render(<SlideDeckPanel />);
    await waitFor(() => expect(screen.getByText('No slides yet')).toBeInTheDocument());

    emitAppEvent({
      type: 'SlideUpdate',
      payload: {
        index: 2,
        total: 3,
        title: 'Last slide',
        body: '',
        notes: '',
        background_path: null,
        layout_unsupported: false,
      },
    });
    await waitFor(() => expect(screen.getByText('3 / 3')).toBeInTheDocument());

    emitAppEvent({ type: 'DeckEnd', payload: null });

    await waitFor(() => expect(screen.getByText('End of deck ▶')).toBeInTheDocument());
    expect(screen.getByText('3 / 3')).toBeInTheDocument();
  });

  it('PresentationVisibility flips the Present button into its live state', async () => {
    render(<PresentButton />);
    expect(screen.getByRole('button', { name: /Present/ })).toBeInTheDocument();

    emitAppEvent({ type: 'PresentationVisibility', payload: { visible: true } });

    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Stop Presenting/ })).toBeInTheDocument(),
    );
  });

  it('DisplayLost sets the warning, DisplayAssigned clears it', async () => {
    render(<DisplayPanel />);

    emitAppEvent({ type: 'DisplayLost', payload: null });
    await waitFor(() =>
      expect(screen.getByText(/Assigned display disconnected/)).toBeInTheDocument(),
    );

    emitAppEvent({ type: 'DisplayAssigned', payload: { display_id: 'display-1' } });
    await waitFor(() =>
      expect(screen.queryByText(/Assigned display disconnected/)).not.toBeInTheDocument(),
    );
  });
});
