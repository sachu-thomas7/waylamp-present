import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { WhipInfo } from '../lib/ipc';
import { baseHandlers, expectCalled, resetStores, whipInfo } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { WhipPanel } from './WhipPanel';

let ipc: ReturnType<typeof installMockIPC>;

function mount(info: WhipInfo, over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers({ whip_get_info: () => info, ...over }));
  render(<WhipPanel />);
}

const session = (state: 'Connected' | 'Disconnected' | 'Lost') => ({ slot: 'camera1', state, codec: 'H264Baseline' as const });

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('WhipPanel', () => {
  it('off: offers Enable and shows no URLs', async () => {
    mount(whipInfo());
    expect(await screen.findByRole('button', { name: 'Enable phone cameras' })).toBeInTheDocument();
    expect(screen.getByText('Off')).toBeInTheDocument();
    expect(screen.queryByTestId('whip-slot-camera1')).not.toBeInTheDocument();
  });

  it('Enable invokes whip_set_enabled(true) and then re-reads the info', async () => {
    const user = userEvent.setup();
    mount(whipInfo(), { whip_set_enabled: () => null });
    await user.click(await screen.findByRole('button', { name: 'Enable phone cameras' }));
    await expectCalled(ipc, 'whip_set_enabled', [{ enabled: true }]);
    expect(ipc.calls.filter((c) => c.cmd === 'whip_get_info').length).toBeGreaterThanOrEqual(2);
  });

  it('running: lists the four phone URLs, a no-token warning and Disable', async () => {
    mount(whipInfo({ running: true, enabled: true }));
    expect(await screen.findByText('http://192.168.1.20:8088/whip/camera1')).toBeInTheDocument();
    expect(screen.getByText('http://192.168.1.20:8088/whip/camera4')).toBeInTheDocument();
    expect(screen.getByText(/No access token set/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Disable phone cameras' })).toBeInTheDocument();
    expect(screen.getByText('Listening :8088')).toBeInTheDocument();
  });

  it('shows session state and follows WhipSessionChanged', async () => {
    const info = whipInfo({ running: true, enabled: true, token_set: true });
    info.slots[0].session = session('Connected');
    let live = info;
    mount(info, { whip_get_info: () => live });
    const slot = await screen.findByTestId('whip-slot-camera1');
    expect(within(slot).getByText('Connected')).toBeInTheDocument();
    expect(screen.queryByText(/No access token set/)).not.toBeInTheDocument();

    const next = whipInfo({ running: true, enabled: true, token_set: true });
    next.slots[0].session = session('Disconnected');
    live = next;
    await emitAppEvent({ type: 'WhipSessionChanged', payload: { slot: 'camera1', state: 'Disconnected' } });
    expect(await within(screen.getByTestId('whip-slot-camera1')).findByText('Reconnecting…')).toBeInTheDocument();
  });

  it('QR button fetches the matrix and renders an SVG labelled with the URL', async () => {
    const user = userEvent.setup();
    mount(whipInfo({ running: true, enabled: true }), {
      whip_get_qr: () => ({ size: 21, modules: Array.from({ length: 441 }, (_, i) => i % 2 === 0) }),
    });
    await user.click(await screen.findByRole('button', { name: 'QR code for camera1' }));
    await expectCalled(ipc, 'whip_get_qr', [{ slot: 'camera1' }]);
    const svg = await screen.findByRole('img', { name: 'QR code for http://192.168.1.20:8088/whip/camera1' });
    expect(svg.querySelector('path')?.getAttribute('d')).toMatch(/^M4 4h1v1h-1z/);
    await user.click(screen.getByRole('button', { name: 'QR code for camera1' }));
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
  });

  it('saves a token through whip_set_token and clears the field', async () => {
    const user = userEvent.setup();
    mount(whipInfo({ running: true, enabled: true }), { whip_set_token: () => null });
    const field = await screen.findByLabelText('Access token');
    await user.type(field, 'tok-123');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await expectCalled(ipc, 'whip_set_token', [{ token: 'tok-123' }]);
    expect(field).toHaveValue('');
  });

  it('Clear removes an existing token', async () => {
    const user = userEvent.setup();
    mount(whipInfo({ running: true, enabled: true, token_set: true }), { whip_set_token: () => null });
    await user.click(await screen.findByRole('button', { name: 'Clear' }));
    await expectCalled(ipc, 'whip_set_token', [{ token: null }]);
  });

  it('shows why phone ingest is unavailable and command errors', async () => {
    const user = userEvent.setup();
    mount(whipInfo({ unavailable: 'WHIP support not compiled in' }), {
      whip_set_enabled: () => {
        throw { type: 'Internal', message: 'cannot listen on 0.0.0.0:8088' };
      },
    });
    expect(await screen.findByText('WHIP support not compiled in')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Enable phone cameras' }));
    expect(await screen.findByText(/cannot listen on 0.0.0.0:8088/)).toBeInTheDocument();
  });
});
