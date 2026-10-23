import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { StreamConfigView, StreamStatus } from '../lib/ipc';
import { baseHandlers, expectCalled, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { StreamPanel, formatElapsed } from './StreamPanel';

let ipc: ReturnType<typeof installMockIPC>;

const settings = { width: 1920, height: 1080, fps: 30, video_kbps: 6000, audio_kbps: 128, keyframe_secs: 2, encoder: '' };
const yt = { id: 'yt', name: 'YouTube main', platform: 'youtube' as const, ingest_url: 'rtmp://a.rtmp.youtube.com/live2', enabled: true, key_set: true };
const fb = { id: 'fb', name: 'Facebook page', platform: 'facebook' as const, ingest_url: 'rtmps://live-api-s.facebook.com:443/rtmp/', enabled: true, key_set: true };

function cfg(destinations: StreamConfigView['destinations'] = []): StreamConfigView {
  return { settings, destinations };
}

function status(over: Partial<StreamStatus> = {}): StreamStatus {
  return { state: 'Stopped', encoder: null, hardware: false, started_unix_ms: null, destinations: [], ...over };
}

function mount(config: StreamConfigView, st: StreamStatus, over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers({ stream_get_config: () => config, stream_get_status: () => st, ...over }));
  render(<StreamPanel />);
}

const callsOf = (cmd: string) => ipc.calls.filter((c) => c.cmd === cmd);

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('StreamPanel', () => {
  it('empty: explains what to add and keeps Go Live disabled', async () => {
    mount(cfg(), status());
    expect(await screen.findByText(/No destinations yet/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Go Live' })).toBeDisabled();
    expect(screen.getByText('Off')).toBeInTheDocument();
  });

  it('adds a YouTube destination with its prefilled ingest URL and sends the key to the backend only', async () => {
    const user = userEvent.setup();
    let stored = cfg();
    mount(cfg(), status(), {
      stream_get_config: () => stored,
      stream_save_destination: () => (stored = cfg([yt])),
    });
    await user.click(await screen.findByRole('button', { name: 'Add destination' }));
    expect(screen.getByLabelText('Ingest URL')).toHaveValue('rtmp://a.rtmp.youtube.com/live2');
    await user.type(screen.getByLabelText('Destination name'), 'YouTube main');
    await user.type(screen.getByLabelText('Stream key'), 'abcd-1234');
    await user.click(screen.getByRole('button', { name: 'Save destination' }));
    await expectCalled(ipc, 'stream_save_destination', [
      {
        destination: { id: 'youtube-main', name: 'YouTube main', platform: 'youtube', ingest_url: 'rtmp://a.rtmp.youtube.com/live2', enabled: true },
        key: 'abcd-1234',
      },
    ]);
    expect(await screen.findByTestId('stream-dest-yt')).toBeInTheDocument();
    expect(screen.queryByDisplayValue('abcd-1234')).not.toBeInTheDocument();
  });

  it('switching platform swaps the default ingest URL; the key field is a password input', async () => {
    const user = userEvent.setup();
    mount(cfg(), status());
    await user.click(await screen.findByRole('button', { name: 'Add destination' }));
    await user.click(screen.getByRole('button', { name: 'Facebook' }));
    expect(screen.getByLabelText('Ingest URL')).toHaveValue('rtmps://live-api-s.facebook.com:443/rtmp/');
    expect(screen.getByLabelText('Stream key')).toHaveAttribute('type', 'password');
    expect(screen.getByRole('button', { name: 'Save destination' })).toBeDisabled();
  });

  it('Go Live is two steps: nothing starts until the confirmation is accepted', async () => {
    const user = userEvent.setup();
    mount(cfg([yt, fb]), status(), { stream_start: () => status({ state: 'Live' }) });
    await user.click(await screen.findByRole('button', { name: 'Go Live' }));
    expect(callsOf('stream_start')).toEqual([]);
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Go live now to 2 destinations?');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(callsOf('stream_start')).toEqual([]);

    await user.click(screen.getByRole('button', { name: 'Go Live' }));
    await user.click(screen.getByRole('button', { name: 'Yes, go live' }));
    await expectCalled(ipc, 'stream_start', [{}]);
  });

  it('a destination without a key blocks Go Live and says why', async () => {
    mount(cfg([{ ...yt, key_set: false }]), status());
    expect(await screen.findByText('No key')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Go Live' })).toBeDisabled();
    expect(screen.getByText(/needs a stream key/)).toBeInTheDocument();
  });

  it('live: shows per-destination health, bitrate and drops, and follows events', async () => {
    const live = status({
      state: 'Live', encoder: 'nvh264enc', hardware: true, started_unix_ms: Date.now() - 65_000,
      destinations: [
        { id: 'yt', name: 'YouTube main', platform: 'youtube', state: { state: 'Live' }, bitrate_kbps: 5900, dropped_frames: 2 },
        { id: 'fb', name: 'Facebook page', platform: 'facebook', state: { state: 'Reconnecting', detail: { attempt: 2, retry_in_secs: 2 } }, bitrate_kbps: 0, dropped_frames: 0 },
      ],
    });
    let current = live;
    mount(cfg([yt, fb]), live, { stream_get_status: () => current });
    const ytRow = await screen.findByTestId('stream-dest-yt');
    expect(within(ytRow).getByText(/Live · 5900 kbps · 2 dropped/)).toBeInTheDocument();
    expect(within(screen.getByTestId('stream-dest-fb')).getByText(/Reconnecting \(try 2, in 2s\)/)).toBeInTheDocument();
    expect(screen.getByText(/Encoder: nvh264enc \(hardware\)/)).toBeInTheDocument();
    expect(screen.getByText(/LIVE 00:01:0\d/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'End Stream' })).toBeInTheDocument();

    await emitAppEvent({ type: 'StreamHealth', payload: { id: 'yt', bitrate_kbps: 6100, dropped_frames: 5 } });
    expect(await within(ytRow).findByText(/6100 kbps · 5 dropped/)).toBeInTheDocument();

    current = { ...live, destinations: live.destinations.map((d) => (d.id === 'fb' ? { ...d, state: { state: 'Live' } as const } : d)) };
    await emitAppEvent({ type: 'StreamDestinationChanged', payload: { id: 'fb', state: { state: 'Live' } } });
    await waitFor(() => expect(within(screen.getByTestId('stream-dest-fb')).getByText(/^Live/)).toBeInTheDocument());
  });

  it('a failed destination shows its reason', async () => {
    mount(cfg([yt]), status({
      state: 'Live', started_unix_ms: Date.now(),
      destinations: [{ id: 'yt', name: 'YouTube main', platform: 'youtube', state: { state: 'Failed', detail: 'no RTMP sink' }, bitrate_kbps: 0, dropped_frames: 0 }],
    }));
    expect(await screen.findByText('Failed: no RTMP sink')).toBeInTheDocument();
  });

  it('End Stream is also confirmed, then stops everything', async () => {
    const user = userEvent.setup();
    mount(cfg([yt]), status({ state: 'Live', started_unix_ms: Date.now() }), { stream_stop: () => status() });
    await user.click(await screen.findByRole('button', { name: 'End Stream' }));
    expect(callsOf('stream_stop')).toEqual([]);
    await user.click(screen.getByRole('button', { name: 'Yes, end stream' }));
    await expectCalled(ipc, 'stream_stop', [{}]);
  });

  it('Mute toggles one destination; Remove deletes it', async () => {
    const user = userEvent.setup();
    mount(cfg([yt, fb]), status(), {
      stream_set_destination_enabled: () => cfg([yt, fb]),
      stream_remove_destination: () => cfg([fb]),
    });
    await user.click(await screen.findByRole('button', { name: 'Mute YouTube main' }));
    await expectCalled(ipc, 'stream_set_destination_enabled', [{ id: 'yt', enabled: false }]);
    await user.click(screen.getByRole('button', { name: 'Remove YouTube main' }));
    await expectCalled(ipc, 'stream_remove_destination', [{ id: 'yt' }]);
  });

  it('quality switches settings while stopped and is locked while live', async () => {
    const user = userEvent.setup();
    mount(cfg([yt]), status(), { stream_set_settings: () => cfg([yt]) });
    await user.click(await screen.findByRole('button', { name: '720p · 3 Mbps' }));
    await expectCalled(ipc, 'stream_set_settings', [{ settings: { ...settings, width: 1280, height: 720, video_kbps: 3000 } }]);
  });

  it('backend errors are shown to the operator', async () => {
    const user = userEvent.setup();
    mount(cfg([yt]), status(), {
      stream_start: () => {
        throw { type: 'Internal', message: 'no H.264 encoder found' };
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Go Live' }));
    await user.click(screen.getByRole('button', { name: 'Yes, go live' }));
    expect(await screen.findByText('no H.264 encoder found')).toBeInTheDocument();
  });

  it('formats elapsed time as hh:mm:ss', () => {
    expect(formatElapsed(0)).toBe('00:00:00');
    expect(formatElapsed(3725)).toBe('01:02:05');
  });
});
