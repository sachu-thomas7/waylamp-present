import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, expectCalled, resetStores, whipInfo } from '../test/componentHelpers';
import { emitAppEvent, emitPreviewFrame, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { usePreviewStore } from '../store/previewStore';
import { CameraPreviewGrid } from './CameraPreviewGrid';

let ipc: ReturnType<typeof installMockIPC>;

const added = (name: string, lost = false) => ({ name, format: 'Standard', lost });
const frame = (source: string, png = 'AAAA') => ({ source, width: 320, height: 180, png_base64: png });

function mount(sources: string[] = [], over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(
    baseHandlers({
      ndi_list_sources: () => ({ discovered: [], added: sources.map((n) => added(n)), unavailable: null }),
      ...over,
    }),
  );
  return render(<CameraPreviewGrid />);
}

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('CameraPreviewGrid', () => {
  it('empty: points the operator at Settings', async () => {
    mount();
    expect(await screen.findByText(/No cameras added/)).toBeInTheDocument();
    expect(screen.queryByTestId('camera-grid')).not.toBeInTheDocument();
  });

  it('shows one tile per added source, waiting until the first frame, then the image', async () => {
    mount(['Cam A', 'Cam B']);
    const tile = await screen.findByTestId('camera-preview-Cam A');
    expect(screen.getByTestId('camera-preview-Cam B')).toBeInTheDocument();
    expect(within(tile).getByText('Waiting for video…')).toBeInTheDocument();

    await emitPreviewFrame(frame('Cam A', 'QUJD'));
    const img = await screen.findByTestId('preview-image-Cam A');
    expect(img).toHaveAttribute('src', 'data:image/png;base64,QUJD');
    expect(screen.queryByTestId('preview-image-Cam B')).not.toBeInTheDocument();

    await emitPreviewFrame(frame('Cam A', 'REVG'));
    expect(screen.getByTestId('preview-image-Cam A')).toHaveAttribute('src', 'data:image/png;base64,REVG');
  });

  it('overlays resolution, fps and latency from SourceStats', async () => {
    mount(['Cam A']);
    expect(await screen.findByTestId('preview-meta-Cam A')).toHaveTextContent('—');
    await emitAppEvent({
      type: 'SourceStats',
      payload: { name: 'Cam A', width: 1920, height: 1080, fps: 29.97, latency_ms: 17 },
    });
    expect(await screen.findByText('1920×1080 · 30.0 fps · 17 ms')).toBeInTheDocument();
  });

  it('SourceLost swaps the image for the Source Lost placeholder and a return restores it', async () => {
    mount(['Cam A']);
    await screen.findByTestId('camera-preview-Cam A');
    await emitPreviewFrame(frame('Cam A'));
    expect(await screen.findByTestId('preview-image-Cam A')).toBeInTheDocument();

    await emitAppEvent({ type: 'SourceLost', payload: { name: 'Cam A' } });
    expect(await screen.findByTestId('preview-lost-Cam A')).toHaveTextContent('Source Lost');
    expect(screen.queryByTestId('preview-image-Cam A')).not.toBeInTheDocument();

    await emitAppEvent({
      type: 'SourceDiscovered',
      payload: { source: { name: 'Cam A', url: null, advertises_new_format: false } },
    });
    expect(await screen.findByTestId('preview-image-Cam A')).toBeInTheDocument();
  });

  it('SourceAdded creates a tile and SourceRemoved drops it and its frame', async () => {
    mount();
    await screen.findByText(/No cameras added/);
    await emitAppEvent({ type: 'SourceAdded', payload: { name: 'Cam A', format: 'Standard' } });
    expect(await screen.findByTestId('camera-preview-Cam A')).toBeInTheDocument();
    await emitPreviewFrame(frame('Cam A'));
    await emitAppEvent({ type: 'SourceRemoved', payload: { name: 'Cam A' } });
    expect(screen.queryByTestId('camera-preview-Cam A')).not.toBeInTheDocument();
    expect(usePreviewStore.getState().frames).toEqual({});
  });

  it('loads the persisted layout and persists a change, applying it to the grid', async () => {
    const user = userEvent.setup();
    mount(['A'], { preview_get_layout: () => '2x3', preview_set_layout: () => null });
    const grid = await screen.findByTestId('camera-grid');
    expect(await screen.findByRole('button', { name: '2×3', pressed: true })).toBeInTheDocument();
    expect(grid.style.gridTemplateColumns).toBe('repeat(3, 1fr)');

    await user.click(screen.getByRole('button', { name: '2×4' }));
    await expectCalled(ipc, 'preview_set_layout', [{ layout: '2x4' }]);
    expect(screen.getByTestId('camera-grid').style.gridTemplateColumns).toBe('repeat(4, 1fr)');
  });

  it('reverts the layout when saving fails', async () => {
    const user = userEvent.setup();
    mount(['A'], {
      preview_set_layout: () => {
        throw { type: 'Config', message: 'disk full' };
      },
    });
    await screen.findByRole('button', { name: '2×2', pressed: true });
    await user.click(screen.getByRole('button', { name: '2×4' }));
    await expectCalled(ipc, 'preview_set_layout', [{ layout: '2x4' }]);
    expect(await screen.findByRole('button', { name: '2×2', pressed: true })).toBeInTheDocument();
  });

  it('caps tiles at the grid size and says how many are hidden', async () => {
    mount(['A', 'B', 'C', 'D', 'E']);
    await screen.findByTestId('camera-preview-A');
    expect(screen.queryByTestId('camera-preview-E')).not.toBeInTheDocument();
    expect(screen.getByText('1 more source(s) not shown — pick a larger grid')).toBeInTheDocument();
    await act(async () => usePreviewStore.getState().setLayout('2x3'));
    expect(await screen.findByTestId('camera-preview-E')).toBeInTheDocument();
  });

  it('catches up after sleep/wake: re-fetches the snapshot and keeps receiving live frames', async () => {
    let snapshotFrames: ReturnType<typeof frame>[] = [];
    mount(['Cam A'], { preview_get_snapshot: () => ({ frames: snapshotFrames, stats: [] }) });
    await screen.findByTestId('camera-preview-Cam A');
    await expectCalled(ipc, 'preview_get_snapshot', [{}]);

    snapshotFrames = [frame('Cam A', 'U05BUA==')];
    await act(async () => {
      document.dispatchEvent(new Event('visibilitychange'));
    });
    expect(await screen.findByTestId('preview-image-Cam A')).toHaveAttribute('src', 'data:image/png;base64,U05BUA==');
    expect(ipc.calls.filter((c) => c.cmd === 'preview_get_snapshot')).toHaveLength(2);

    await emitPreviewFrame(frame('Cam A', 'TElWRQ=='));
    expect(screen.getByTestId('preview-image-Cam A')).toHaveAttribute('src', 'data:image/png;base64,TElWRQ==');
  });

  it('stops listening when unmounted', async () => {
    const { unmount } = mount(['Cam A']);
    await screen.findByTestId('camera-preview-Cam A');
    unmount();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    await emitPreviewFrame(frame('Cam A'));
    expect(usePreviewStore.getState().frames).toEqual({});
  });
});

describe('CameraPreviewGrid phone sources', () => {
  it('a live WHIP session shows as a tile like an NDI source, and Source Lost follows its state', async () => {
    const info = whipInfo({ running: true, enabled: true });
    info.slots[0].session = { slot: 'camera1', state: 'Connected', codec: 'H264Baseline' };
    let live = info;
    mount([], { whip_get_info: () => live });
    expect(await screen.findByTestId('camera-preview-camera1')).toBeInTheDocument();
    await emitPreviewFrame(frame('camera1', 'QUJD'));
    expect(await screen.findByTestId('preview-image-camera1')).toBeInTheDocument();

    const lost = whipInfo({ running: true, enabled: true });
    lost.slots[0].session = { slot: 'camera1', state: 'Lost', codec: 'H264Baseline' };
    live = lost;
    await emitAppEvent({ type: 'WhipSessionChanged', payload: { slot: 'camera1', state: 'Lost' } });
    expect(await screen.findByTestId('preview-lost-camera1')).toBeInTheDocument();
  });
});
