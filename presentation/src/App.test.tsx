import { mediaUrl } from './lib/mediaUrl';
import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { AppEvent, SlidePayload } from './lib/events';
import type { SceneMediaPayload } from './lib/ipc';
import { useSlideStore } from './store/slideStore';
import { emitAppEvent, installMockIPC, renderApp } from './test/mockTauri';

let ipc: ReturnType<typeof installMockIPC>;

function slide(index: number, total: number, over: Partial<SlidePayload> = {}): AppEvent {
  return {
    type: 'SlideUpdate',
    payload: {
      index, total, title: `Title ${index + 1}`, body: `Body ${index + 1}`,
      notes: '', background_path: null, layout_unsupported: false, ...over,
    },
  };
}

const image: SceneMediaPayload = { media_type: 'Image', path: '/media/photo.png', display_name: 'photo.png' };
const video: SceneMediaPayload = { media_type: 'Video', path: '/media/clip.mp4', display_name: 'clip.mp4' };

const showMedia = async (media: SceneMediaPayload) => {
  await emitAppEvent({ type: 'SceneMediaUpdate', payload: media });
  await emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
};

beforeEach(() => {
  useSlideStore.setState({ current: null });
  ipc = installMockIPC();
  renderApp();
});

// Presentation is listen-only: no test may cause an invoke.
afterEach(() => {
  expect(ipc.calls).toEqual([]);
  expect(ipc.unhandled).toEqual([]);
});

describe('presentation: slide rendering', () => {
  it('shows an empty black canvas before any event', () => {
    expect(screen.getByTestId('slide-canvas')).toBeInTheDocument();
    expect(screen.queryByText(/\d+ \/ \d+/)).not.toBeInTheDocument();
    expect(screen.queryByTestId('slide-background')).not.toBeInTheDocument();
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
    expect(screen.queryByTestId('scene-media-video')).not.toBeInTheDocument();
  });

  it('renders title, body and position from SlideUpdate', async () => {
    await emitAppEvent(slide(1, 3));
    expect(await screen.findByText('Title 2')).toBeInTheDocument();
    expect(screen.getByText('Body 2')).toBeInTheDocument();
    expect(screen.getByText('2 / 3')).toBeInTheDocument();
    expect(screen.queryByTestId('slide-background')).not.toBeInTheDocument();
  });

  it('replaces the previous slide on the next SlideUpdate', async () => {
    await emitAppEvent(slide(0, 3));
    await screen.findByText('Title 1');
    await emitAppEvent(slide(1, 3));
    await screen.findByText('Title 2');
    expect(screen.queryByText('Title 1')).not.toBeInTheDocument();
    expect(screen.queryByText('Body 1')).not.toBeInTheDocument();
    expect(screen.queryByText('1 / 3')).not.toBeInTheDocument();
  });

  it('renders the background image only while the slide has one', async () => {
    await emitAppEvent(slide(0, 2, { background_path: '/bg/one.png' }));
    expect(await screen.findByTestId('slide-background')).toHaveAttribute('src', mediaUrl('/bg/one.png'));
    await emitAppEvent(slide(1, 2, { background_path: null }));
    await screen.findByText('Title 2');
    expect(screen.queryByTestId('slide-background')).not.toBeInTheDocument();
  });

  it('renders a layout_unsupported slide like any other (no warning on the projector)', async () => {
    await emitAppEvent(slide(0, 1, { layout_unsupported: true }));
    expect(await screen.findByText('Title 1')).toBeInTheDocument();
    expect(screen.queryByText(/unsupported/i)).not.toBeInTheDocument();
  });

  it('keeps the current slide on DeckEnd and DeckStart (boundary events carry no slide)', async () => {
    await emitAppEvent(slide(2, 3));
    await screen.findByText('Title 3');
    // Not in the presentation's AppEvent union, but the backend still broadcasts them.
    await emitAppEvent({ type: 'DeckEnd' } as unknown as AppEvent);
    await emitAppEvent({ type: 'DeckStart' } as unknown as AppEvent);
    expect(screen.getByText('Title 3')).toBeInTheDocument();
    expect(screen.getByText('3 / 3')).toBeInTheDocument();
  });
});

describe('presentation: scene switching', () => {
  it('stays on the slide when media is staged but the source is still SlideDeck', async () => {
    await emitAppEvent(slide(0, 1));
    await screen.findByText('Title 1');
    await emitAppEvent({ type: 'SceneMediaUpdate', payload: image });
    expect(screen.getByText('Title 1')).toBeInTheDocument();
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
  });

  it('stays on the slide when the source flips to SceneMedia with nothing staged', async () => {
    await emitAppEvent(slide(0, 1));
    await screen.findByText('Title 1');
    await emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
    expect(screen.getByText('Title 1')).toBeInTheDocument();
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
  });

  it('shows the staged image when the source flips to SceneMedia, replacing the slide', async () => {
    await emitAppEvent(slide(0, 1));
    await screen.findByText('Title 1');
    await showMedia(image);
    expect(await screen.findByTestId('scene-media-image')).toHaveAttribute('src', mediaUrl('/media/photo.png'));
    expect(screen.queryByText('Title 1')).not.toBeInTheDocument();
    expect(screen.queryByTestId('slide-canvas')).not.toBeInTheDocument();
  });

  it('plays a staged video looping and autoplaying', async () => {
    await showMedia(video);
    const el = (await screen.findByTestId('scene-media-video')) as HTMLVideoElement;
    expect(el).toHaveAttribute('src', mediaUrl('/media/clip.mp4'));
    expect(el.loop).toBe(true);
    expect(el.autoplay).toBe(true);
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
  });

  it('swaps image for video when new media is pushed while SceneMedia is active', async () => {
    await showMedia(image);
    await screen.findByTestId('scene-media-image');
    await emitAppEvent({ type: 'SceneMediaUpdate', payload: video });
    expect(await screen.findByTestId('scene-media-video')).toHaveAttribute('src', mediaUrl('/media/clip.mp4'));
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
  });

  it('returns to the latest slide when the source goes back to SlideDeck', async () => {
    await emitAppEvent(slide(0, 2));
    await showMedia(image);
    await screen.findByTestId('scene-media-image');
    // A slide advance while media is on screen must not be lost.
    await emitAppEvent(slide(1, 2));
    expect(screen.getByTestId('scene-media-image')).toBeInTheDocument();
    await emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SlideDeck' } });
    expect(await screen.findByText('Title 2')).toBeInTheDocument();
    expect(screen.queryByTestId('scene-media-image')).not.toBeInTheDocument();
  });
});

// Adding an AppEvent variant breaks typecheck here until it is rendered or ignored on purpose.
type AppEventType = AppEvent['type'];
type Known<T extends AppEventType> = T;
type RenderedType = Known<'SlideUpdate' | 'SceneMediaUpdate' | 'SceneSourceChanged'>;
type IgnoredType = Exclude<AppEventType, RenderedType>;
const ignoredEvents: { [K in IgnoredType]: Extract<AppEvent, { type: K }> } = {
  BusLag: { type: 'BusLag', payload: { receiver_id: 'r1', dropped: 3 } },
  Ping: { type: 'Ping', payload: { message: 'ping-message' } },
  DeckImportProgress: { type: 'DeckImportProgress', payload: { slide: 2, total: 5 } },
  DeckImportComplete: { type: 'DeckImportComplete', payload: { deck_id: 'deck-1' } },
  DeckImportFailed: { type: 'DeckImportFailed', payload: { slide: 1, reason: 'bad-file-reason' } },
};

describe('presentation: events intentionally ignored', () => {
  const cases = Object.values(ignoredEvents).flatMap((event) =>
    (['slide shown', 'media staged, source SlideDeck', 'media active'] as const).map((mode) => ({ event, mode })),
  );

  it.each(cases)('$event.type changes nothing while $mode', async ({ event, mode }) => {
    await emitAppEvent(slide(0, 2));
    await screen.findByText('Title 1');
    if (mode !== 'slide shown') await emitAppEvent({ type: 'SceneMediaUpdate', payload: image });
    if (mode === 'media active') {
      await emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
      await screen.findByTestId('scene-media-image');
    }
    const before = document.body.innerHTML;
    await emitAppEvent(event);
    expect(document.body.innerHTML).toBe(before);
  });
});
