import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { expectCalled, baseHandlers, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { SceneMediaPanel } from './SceneMediaPanel';

let ipc: ReturnType<typeof installMockIPC>;

async function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<SceneMediaPanel />);
  await waitFor(() => expect(ipc.calls.some((c) => c.cmd === 'scene_get_active_source')).toBe(true));
}

const callsOf = (cmd: string) => ipc.calls.filter((c) => c.cmd === cmd);
const onScreen = () => screen.getByText(/On screen now:/);

beforeEach(resetStores);
afterEach(() => {
  expect(ipc.unhandled).toEqual([]);
  vi.restoreAllMocks();
});

describe('SceneMediaPanel', () => {
  it('defaults to Slide Deck on screen with nothing staged', async () => {
    await mount();
    expect(onScreen()).toHaveTextContent('On screen now: Slide Deck');
    expect(screen.getByText('Staged: none')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Push Image' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Push Video' })).toBeEnabled();
  });

  it('reflects the backend active source on startup', async () => {
    await mount({ scene_get_active_source: () => 'SceneMedia' });
    await waitFor(() => expect(onScreen()).toHaveTextContent('On screen now: Scene Media'));
  });

  it('choosing Scene Media invokes scene_set_source and the label only follows the event', async () => {
    const user = userEvent.setup();
    await mount({ scene_set_source: () => null });
    await user.click(screen.getByRole('button', { name: 'Scene Media' }));
    await expectCalled(ipc, 'scene_set_source', [{ source: 'SceneMedia' }]);
    expect(onScreen()).toHaveTextContent('On screen now: Slide Deck');
    await emitAppEvent({ type: 'SceneSourceChanged', payload: { source: 'SceneMedia' } });
    await waitFor(() => expect(onScreen()).toHaveTextContent('On screen now: Scene Media'));
  });

  it('choosing Slide Deck invokes scene_set_source with SlideDeck', async () => {
    const user = userEvent.setup();
    await mount({ scene_get_active_source: () => 'SceneMedia', scene_set_source: () => null });
    await user.click(screen.getByRole('button', { name: 'Slide Deck' }));
    await expectCalled(ipc, 'scene_set_source', [{ source: 'SlideDeck' }]);
  });

  it('Push Image sends the picked path to scene_push_image', async () => {
    const user = userEvent.setup();
    await mount({ 'plugin:dialog|open': () => '/media/logo.png', scene_push_image: () => null });
    await user.click(screen.getByRole('button', { name: 'Push Image' }));
    await expectCalled(ipc, 'scene_push_image', [{ path: '/media/logo.png' }]);
  });

  it('Push Video sends the picked path to scene_push_video', async () => {
    const user = userEvent.setup();
    await mount({ 'plugin:dialog|open': () => '/media/loop.mp4', scene_push_video: () => null });
    await user.click(screen.getByRole('button', { name: 'Push Video' }));
    await expectCalled(ipc, 'scene_push_video', [{ path: '/media/loop.mp4' }]);
  });

  it('Push Image and Push Video open the dialog with their own extension filters', async () => {
    const user = userEvent.setup();
    await mount({ 'plugin:dialog|open': () => null });
    await user.click(screen.getByRole('button', { name: 'Push Image' }));
    await user.click(screen.getByRole('button', { name: 'Push Video' }));
    await expectCalled(ipc, 'plugin:dialog|open', [
      { options: { multiple: false, filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] } },
      { options: { multiple: false, filters: [{ name: 'Video', extensions: ['mp4', 'webm'] }] } },
    ]);
  });

  it('cancelling the dialog pushes nothing', async () => {
    const user = userEvent.setup();
    await mount({ 'plugin:dialog|open': () => null });
    await user.click(screen.getByRole('button', { name: 'Push Image' }));
    await waitFor(() => expect(callsOf('plugin:dialog|open')).toHaveLength(1));
    expect(callsOf('scene_push_image')).toEqual([]);
  });

  it('a staged image shows its name and a preview', async () => {
    await mount();
    await emitAppEvent({
      type: 'SceneMediaUpdate',
      payload: { media_type: 'Image', path: 'asset://logo.png', display_name: 'logo.png' },
    });
    expect(await screen.findByText('Staged: logo.png')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'logo.png' })).toHaveAttribute('src', 'asset://logo.png');
  });

  it('a staged video shows its name and no image preview', async () => {
    await mount();
    await emitAppEvent({
      type: 'SceneMediaUpdate',
      payload: { media_type: 'Video', path: 'asset://loop.mp4', display_name: 'loop.mp4' },
    });
    expect(await screen.findByText('Staged: loop.mp4')).toBeInTheDocument();
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
  });
});
