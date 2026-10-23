import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import { mediaUrl } from '../lib/mediaUrl';
import { useSlideStore } from '../store/slideStore';
import type { AppEvent } from '../lib/events';

// App.tsx registers two separate onAppEvent listeners (scene switching, slide updates).
// Fan an emitted event out to both, mirroring the real `app-event` broadcast.
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

describe('presentation window scene switching', () => {
  beforeEach(() => {
    handlers.length = 0;
    useSlideStore.setState({ current: null });
  });

  afterEach(cleanup);

  it('renders the slide canvas by default and reacts to SlideUpdate', async () => {
    render(<App />);
    emitAppEvent({
      type: 'SlideUpdate',
      payload: {
        index: 0,
        total: 1,
        title: 'Welcome',
        body: 'Body text',
        notes: '',
        background_path: null,
        layout_unsupported: false,
      },
    });
    await waitFor(() => expect(screen.getByText('Welcome')).toBeInTheDocument());
  });

  it('switches to scene media only once both source and media events have arrived', async () => {
    const { container } = render(<App />);

    // Source flips before media is staged — should NOT switch away from slides yet
    // (App.tsx requires sceneSource === 'SceneMedia' && sceneMedia, both together).
    emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
    expect(container.querySelector('img')).not.toBeInTheDocument();

    emitAppEvent({
      type: 'SceneMediaUpdate',
      payload: { media_type: 'Image', path: '/tmp/photo.png', display_name: 'photo.png' },
    });

    await waitFor(() =>
      expect(container.querySelector('img')).toHaveAttribute('src', mediaUrl('/tmp/photo.png')),
    );
  });

  it('switches back to the slide canvas when source returns to SlideDeck', async () => {
    const { container } = render(<App />);
    emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
    emitAppEvent({
      type: 'SceneMediaUpdate',
      payload: { media_type: 'Image', path: '/tmp/photo.png', display_name: 'photo.png' },
    });
    await waitFor(() => expect(container.querySelector('img')).toBeInTheDocument());

    emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SlideDeck' } });
    await waitFor(() => expect(container.querySelector('img')).not.toBeInTheDocument());
  });
});
