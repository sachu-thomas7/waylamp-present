import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, expectCalled, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import type { SourceGeometry } from '../lib/ipc';
import { CompositorPanel, nextTestSource } from './CompositorPanel';

let ipc: ReturnType<typeof installMockIPC>;

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<CompositorPanel />);
}

const src = (name: string, over: Partial<SourceGeometry> = {}): SourceGeometry =>
  ({ name, x: 0, y: 0, w: 640, h: 360, z: 0, kind: 'test', ...over });

async function runWith(sources: SourceGeometry[]) {
  await emitAppEvent({ type: 'CompositorStateChanged', payload: { running: true, element: 'd3d12compositor', hardware: true } });
  await emitAppEvent({ type: 'CompositorLayoutChanged', payload: { sources } });
}

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('CompositorPanel', () => {
  it('idle: Stopped with a scene picker (demo first, then configured scenes) and Start only', async () => {
    mount({ compositor_list_scenes: () => ['keynote'] });
    expect(await screen.findByText('Stopped')).toBeInTheDocument();
    expect(await screen.findByRole('option', { name: 'keynote' })).toBeInTheDocument();
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual(['Demo (3 test sources)', 'keynote']);
    expect(screen.queryByRole('button', { name: 'Stop' })).not.toBeInTheDocument();
  });

  it('Start sends null for the demo scene and the id for a configured one', async () => {
    const user = userEvent.setup();
    mount({ compositor_list_scenes: () => ['keynote'], compositor_start: () => null });
    await user.click(await screen.findByRole('button', { name: 'Start' }));
    await expectCalled(ipc, 'compositor_start', [{ sceneId: null }]);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Scene' }), 'keynote');
    await user.click(screen.getByRole('button', { name: 'Start' }));
    await expectCalled(ipc, 'compositor_start', [{ sceneId: null }, { sceneId: 'keynote' }]);
  });

  it('events flip to running, show the element and list sources; Stop returns to idle', async () => {
    const user = userEvent.setup();
    mount({ compositor_stop: () => null });
    await screen.findByText('Stopped');
    await runWith([src('cam-main'), src('cam-2')]);
    expect(await screen.findByText('GPU · d3d12compositor')).toBeInTheDocument();
    expect(screen.getByTestId('compositor-source-cam-main')).toBeInTheDocument();
    expect(screen.getByTestId('compositor-source-cam-2')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Stop' }));
    await expectCalled(ipc, 'compositor_stop', [{}]);
    await emitAppEvent({ type: 'CompositorStateChanged', payload: { running: false, element: '', hardware: false } });
    expect(await screen.findByText('Stopped')).toBeInTheDocument();
    expect(screen.queryByTestId('compositor-source-cam-main')).not.toBeInTheDocument();
  });

  it('Remove sends the layout without that source; Add appends the next free test source', async () => {
    const user = userEvent.setup();
    mount({ compositor_set_layout: () => null });
    await screen.findByText('Stopped');
    await runWith([src('cam-main'), src('cam-2', { z: 1 })]);
    await user.click(await screen.findByRole('button', { name: 'Remove cam-2' }));
    await emitAppEvent({ type: 'CompositorLayoutChanged', payload: { sources: [src('cam-main')] } });
    await user.click(screen.getByRole('button', { name: 'Add test source' }));
    await expectCalled(ipc, 'compositor_set_layout', [
      { sources: [src('cam-main')] },
      { sources: [src('cam-main'), src('cam-2', { x: 1280, z: 1 })] },
    ]);
  });

  it('shows a rejected command and PipelineError instead of failing silently', async () => {
    const user = userEvent.setup();
    mount({
      compositor_start: () => {
        throw { type: 'Internal', message: 'GStreamer support not compiled in' };
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Start' }));
    expect(await screen.findByText(/not compiled in/)).toBeInTheDocument();
    await emitAppEvent({ type: 'PipelineError', payload: { element: 'mix', message: 'negotiation failed' } });
    expect(await screen.findByText('mix: negotiation failed')).toBeInTheDocument();
  });
});

describe('nextTestSource', () => {
  it('picks a free name and a z above every existing source', () => {
    expect(nextTestSource([src('cam-1', { z: 4 }), src('cam-2')])).toMatchObject({ name: 'cam-3', z: 5, w: 640, h: 360, kind: 'test' });
    expect(nextTestSource([src('cam-2')])).toMatchObject({ name: 'cam-3' });
    expect(nextTestSource([])).toMatchObject({ name: 'cam-1', z: 1 });
  });
});
