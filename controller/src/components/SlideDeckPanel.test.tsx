import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SlidePayload } from '../lib/ipc';
import { expectCalled, baseHandlers, makeDeck, makeSlide, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { SlideDeckPanel } from './SlideDeckPanel';

let ipc: ReturnType<typeof installMockIPC>;

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  return render(<SlideDeckPanel />);
}

async function mountLoaded(
  total = 3,
  over: Record<number, Partial<SlidePayload>> = {},
  extra: Record<string, IpcHandler> = {},
) {
  const deck = makeDeck(total, over);
  mount({ slide_get_current: () => deck[0], deck_get_all_slides: () => deck, ...extra });
  await screen.findByRole('button', { name: 'Slide 1' });
  return deck;
}

const callsOf = (cmd: string) => ipc.calls.filter((c) => c.cmd === cmd);

beforeEach(resetStores);
afterEach(() => {
  expect(ipc.unhandled).toEqual([]);
  vi.restoreAllMocks();
});

describe('SlideDeckPanel empty state', () => {
  it('shows the empty message, 0 slides badge and Import CTA, with no navigation controls', async () => {
    mount();
    expect(await screen.findByText('No slides yet')).toBeInTheDocument();
    expect(screen.getByText('0 slides')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import a Deck' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: /Prev/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Next/ })).not.toBeInTheDocument();
  });

  it('Import a Deck sends the picked path to deck_import', async () => {
    const user = userEvent.setup();
    mount({ 'plugin:dialog|open': () => '/decks/talk.pptx', deck_import: () => 'deck-1' });
    await user.click(await screen.findByRole('button', { name: 'Import a Deck' }));
    await expectCalled(ipc, 'deck_import', [{ path: '/decks/talk.pptx' }]);
  });

  it('opens the dialog filtered to pptx and pdf', async () => {
    const user = userEvent.setup();
    mount({ 'plugin:dialog|open': () => null });
    await user.click(await screen.findByRole('button', { name: 'Import a Deck' }));
    await expectCalled(ipc, 'plugin:dialog|open', [
      { options: { multiple: false, filters: [{ name: 'Presentation', extensions: ['pptx', 'pdf'] }] } },
    ]);
  });

  it('a failed deck_import is logged, shown to the operator, and the empty state remains', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    mount({
      'plugin:dialog|open': () => '/decks/bad.pptx',
      deck_import: () => {
        throw new Error('unsupported');
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Import a Deck' }));
    await expectCalled(ipc, 'deck_import', [{ path: '/decks/bad.pptx' }]);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(await screen.findByText(/unsupported/)).toBeInTheDocument();
    expect(screen.queryByText(/Slide 0/)).not.toBeInTheDocument();
    expect(screen.getByText('No slides yet')).toBeInTheDocument();
  });

  it('cancelling the file dialog does not import', async () => {
    const user = userEvent.setup();
    mount({ 'plugin:dialog|open': () => null });
    await user.click(await screen.findByRole('button', { name: 'Import a Deck' }));
    await waitFor(() => expect(callsOf('plugin:dialog|open')).toHaveLength(1));
    expect(callsOf('deck_import')).toEqual([]);
  });

  it('shows import progress and failure reason', async () => {
    mount();
    await screen.findByText('No slides yet');
    await emitAppEvent({ type: 'DeckImportProgress', payload: { slide: 2, total: 5 } });
    expect(await screen.findByText('Importing… slide 2 / 5')).toBeInTheDocument();
    await emitAppEvent({ type: 'DeckImportFailed', payload: { slide: 3, reason: 'corrupt xml' } });
    expect(await screen.findByText('⚠ Slide 3: corrupt xml')).toBeInTheDocument();
  });

  it('import complete clears progress and refetches the deck', async () => {
    mount();
    await screen.findByText('No slides yet');
    await emitAppEvent({ type: 'DeckImportProgress', payload: { slide: 1, total: 2 } });
    await screen.findByText('Importing… slide 1 / 2');
    const before = callsOf('deck_get_all_slides').length;
    await emitAppEvent({ type: 'DeckImportComplete', payload: { deck_id: 'd1' } });
    await waitFor(() => expect(screen.queryByText(/Importing…/)).not.toBeInTheDocument());
    await waitFor(() => expect(callsOf('deck_get_all_slides')).toHaveLength(before + 1));
  });

  it('a SlideUpdate with slides replaces the empty state', async () => {
    mount();
    await screen.findByText('No slides yet');
    await emitAppEvent({ type: 'SlideUpdate', payload: makeSlide(0, 2) });
    expect(await screen.findByText('1 / 2')).toBeInTheDocument();
    expect(screen.queryByText('No slides yet')).not.toBeInTheDocument();
  });
});

describe('SlideDeckPanel loaded state', () => {
  it('renders the count badge, one thumbnail per slide, notes and an unset timer', async () => {
    await mountLoaded(3, { 0: { notes: 'Say hello' } });
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /^Slide [0-9]+$/ })).toHaveLength(3);
    expect(within(screen.getByRole('button', { name: 'Slide 3' })).getByText('Slide 3 title')).toBeInTheDocument();
    expect(screen.getByText(/Notes — Say hello/)).toBeInTheDocument();
    expect(screen.getByText('--:--')).toBeInTheDocument();
  });

  it('previews the current slide and the next slide', async () => {
    await mountLoaded(3);
    expect(within(screen.getByTestId('preview-current')).getByText('Slide 1 title')).toBeInTheDocument();
    const next = within(screen.getByTestId('preview-next'));
    expect(next.getByText('Slide 2 title')).toBeInTheDocument();
    expect(next.queryByText('Slide 3 title')).not.toBeInTheDocument();
  });

  it('shows the layout_unsupported badge only on flagged slides', async () => {
    await mountLoaded(3, { 1: { layout_unsupported: true } });
    const badge = 'Unsupported layout — imported with blank body';
    expect(within(screen.getByRole('button', { name: 'Slide 2' })).getByRole('img', { name: badge })).toBeInTheDocument();
    expect(within(screen.getByRole('button', { name: 'Slide 1' })).queryByRole('img', { name: badge })).not.toBeInTheDocument();
    expect(within(screen.getByRole('button', { name: 'Slide 3' })).queryByRole('img', { name: badge })).not.toBeInTheDocument();
  });

  it('shows no badge when no slide is flagged', async () => {
    await mountLoaded(2);
    expect(screen.queryByTitle(/Unsupported layout/)).not.toBeInTheDocument();
  });

  it('a SlideUpdate event moves the counter and the next preview', async () => {
    await mountLoaded(3);
    await emitAppEvent({ type: 'SlideUpdate', payload: makeSlide(1, 3) });
    expect(await screen.findByText('2 / 3')).toBeInTheDocument();
    expect(within(screen.getByTestId('preview-current')).getByText('Slide 2 title')).toBeInTheDocument();
    expect(within(screen.getByTestId('preview-next')).getByText('Slide 3 title')).toBeInTheDocument();
  });

  describe('filter', () => {
    const titled = {
      0: { title: 'Intro', body: 'welcome' },
      1: { title: 'Agenda', body: 'secret numbers' },
      2: { title: 'Budget', body: 'q3' },
    };
    const thumbs = () => screen.queryAllByRole('button', { name: /^Slide [0-9]+$/ });

    it('matches titles case-insensitively', async () => {
      const user = userEvent.setup();
      await mountLoaded(3, titled);
      await user.type(screen.getByPlaceholderText('Filter slides…'), 'AGEN');
      expect(thumbs().map((t) => t.getAttribute('aria-label'))).toEqual(['Slide 2']);
    });

    it('does not match body text', async () => {
      const user = userEvent.setup();
      await mountLoaded(3, titled);
      await user.type(screen.getByPlaceholderText('Filter slides…'), 'secret');
      expect(thumbs()).toHaveLength(0);
    });

    it('shows no thumbnails for a non-matching query and all again once cleared', async () => {
      const user = userEvent.setup();
      await mountLoaded(3, titled);
      const input = screen.getByPlaceholderText('Filter slides…');
      await user.type(input, 'zzz');
      expect(thumbs()).toHaveLength(0);
      await user.clear(input);
      expect(thumbs()).toHaveLength(3);
    });
  });

  it('Import Deck in the loaded state sends the picked path', async () => {
    const user = userEvent.setup();
    await mountLoaded(2, {}, { 'plugin:dialog|open': () => '/decks/b.pdf', deck_import: () => 'd' });
    await user.click(screen.getByRole('button', { name: 'Import Deck' }));
    await expectCalled(ipc, 'deck_import', [{ path: '/decks/b.pdf' }]);
  });

  it('shows import progress and failure above the strip', async () => {
    await mountLoaded(2);
    await emitAppEvent({ type: 'DeckImportProgress', payload: { slide: 4, total: 9 } });
    expect(await screen.findByText('Importing… slide 4 / 9')).toBeInTheDocument();
    await emitAppEvent({ type: 'DeckImportFailed', payload: { slide: 5, reason: 'bad layout' } });
    expect(await screen.findByText('⚠ Slide 5: bad layout')).toBeInTheDocument();
  });
});

describe('SlideDeckPanel navigation commands', () => {
  it('Prev and Next invoke slide_rewind and slide_advance with no args', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_rewind: () => null, slide_advance: () => null });
    await user.click(screen.getByRole('button', { name: 'Next' }));
    await user.click(screen.getByRole('button', { name: 'Prev' }));
    await expectCalled(ipc, 'slide_advance', [{}]);
    await expectCalled(ipc, 'slide_rewind', [{}]);
  });

  it('Jump to + Go invokes slide_jump_to with the numeric index', async () => {
    const user = userEvent.setup();
    await mountLoaded(5, {}, { slide_jump_to: () => null });
    await user.type(screen.getByLabelText('Jump to slide index'), '3');
    await user.click(screen.getByRole('button', { name: 'Go' }));
    await expectCalled(ipc, 'slide_jump_to', [{ index: 3 }]);
  });

  it('Go with an empty index does not invoke', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_jump_to: () => null });
    await user.click(screen.getByRole('button', { name: 'Go' }));
    expect(callsOf('slide_jump_to')).toEqual([]);
  });

  it('clicking a thumbnail jumps to that slide', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_jump_to: () => null });
    await user.click(screen.getByRole('button', { name: 'Slide 3' }));
    await expectCalled(ipc, 'slide_jump_to', [{ index: 2 }]);
  });

  it('marks only the current thumbnail with aria-current', async () => {
    await mountLoaded(3);
    expect(screen.getByRole('button', { name: 'Slide 1' })).toHaveAttribute('aria-current', 'true');
    expect(screen.getByRole('button', { name: 'Slide 2' })).not.toHaveAttribute('aria-current');
  });

  it('Space on a focused thumbnail selects it and does not also advance', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_jump_to: () => null });
    screen.getByRole('button', { name: 'Slide 3' }).focus();
    await user.keyboard(' ');
    await expectCalled(ipc, 'slide_jump_to', [{ index: 2 }]);
    expect(callsOf('slide_advance')).toEqual([]);
  });

  it('Space and ArrowRight advance, ArrowLeft rewinds', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_rewind: () => null, slide_advance: () => null });
    await user.keyboard(' ');
    await user.keyboard('{ArrowRight}');
    await user.keyboard('{ArrowLeft}');
    await expectCalled(ipc, 'slide_advance', [{}, {}]);
    await expectCalled(ipc, 'slide_rewind', [{}]);
  });

  it('typing a space in the filter box does not advance the slide', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_advance: () => null });
    const input = screen.getByPlaceholderText('Filter slides…');
    await user.type(input, 'a b');
    expect((input as HTMLInputElement).value.startsWith('a')).toBe(true);
    expect(input).toHaveValue('a b');
    expect(callsOf('slide_advance')).toEqual([]);
  });

  // Same root cause: arrow keys in the filter box should move the caret, not change slides.
  it('arrow keys in the filter box do not change slides', async () => {
    const user = userEvent.setup();
    await mountLoaded(3, {}, { slide_advance: () => null, slide_rewind: () => null });
    const input = screen.getByPlaceholderText('Filter slides…');
    await user.type(input, 'ab{ArrowLeft}{ArrowRight}');
    expect(input).toHaveValue('ab');
    expect(callsOf('slide_advance')).toEqual([]);
    expect(callsOf('slide_rewind')).toEqual([]);
  });

  it('out-of-range and negative jumps reach the backend, whose rejection is logged', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    await mountLoaded(3, {}, {
      slide_jump_to: () => {
        throw new Error('out of range');
      },
    });
    const input = screen.getByLabelText('Jump to slide index');
    await user.type(input, '99');
    await user.click(screen.getByRole('button', { name: 'Go' }));
    await user.clear(input);
    await user.type(input, '-1');
    await user.click(screen.getByRole('button', { name: 'Go' }));
    await expectCalled(ipc, 'slide_jump_to', [{ index: 99 }, { index: -1 }]);
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });

  it('a rejected command is logged and the panel stays usable', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    await mountLoaded(3, {}, {
      slide_advance: () => {
        throw new Error('backend down');
      },
    });
    await user.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });
});

describe('SlideDeckPanel boundaries and timer', () => {
  it('DeckEnd shows the end banner and the next SlideUpdate clears it', async () => {
    await mountLoaded(2);
    await emitAppEvent({ type: 'DeckEnd', payload: null });
    expect(await screen.findByText('End of deck ▶')).toBeInTheDocument();
    await emitAppEvent({ type: 'SlideUpdate', payload: makeSlide(0, 2) });
    await waitFor(() => expect(screen.queryByText('End of deck ▶')).not.toBeInTheDocument());
  });

  it('DeckStart shows the start banner and the next SlideUpdate clears it', async () => {
    await mountLoaded(2);
    await emitAppEvent({ type: 'DeckStart', payload: null });
    expect(await screen.findByText('◀ Start of deck')).toBeInTheDocument();
    expect(screen.queryByText('End of deck ▶')).not.toBeInTheDocument();
    await emitAppEvent({ type: 'SlideUpdate', payload: makeSlide(1, 2) });
    await waitFor(() => expect(screen.queryByText('◀ Start of deck')).not.toBeInTheDocument());
  });

  it('TimerTick formats mm:ss and hh:mm:ss', async () => {
    await mountLoaded(2);
    await emitAppEvent({ type: 'TimerTick', payload: { elapsed_secs: 65 } });
    expect(await screen.findByText('01:05')).toBeInTheDocument();
    await emitAppEvent({ type: 'TimerTick', payload: { elapsed_secs: 3661 } });
    expect(await screen.findByText('01:01:01')).toBeInTheDocument();
  });

  it('TimerReset zeroes the display and the reset button invokes slide_timer_reset', async () => {
    const user = userEvent.setup();
    await mountLoaded(2, {}, { slide_timer_reset: () => null });
    await emitAppEvent({ type: 'TimerTick', payload: { elapsed_secs: 90 } });
    await screen.findByText('01:30');
    await emitAppEvent({ type: 'TimerReset', payload: null });
    expect(await screen.findByText('00:00')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Reset timer' }));
    await expectCalled(ipc, 'slide_timer_reset', [{}]);
  });
});
