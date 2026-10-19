import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, expectCalled, resetStores } from '../test/componentHelpers';
import { emitAppEvent, installMockIPC, type IpcHandler } from '../test/mockTauri';
import { NdiSourcesPanel } from './NdiSourcesPanel';

let ipc: ReturnType<typeof installMockIPC>;

const src = (name: string) => ({ name, url: null, advertises_new_format: false });

function mount(over: Record<string, IpcHandler> = {}) {
  ipc = installMockIPC(baseHandlers(over));
  render(<NdiSourcesPanel />);
}

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('NdiSourcesPanel', () => {
  it('empty: says no sources were found', async () => {
    mount();
    expect(await screen.findByText('No NDI sources found on the network')).toBeInTheDocument();
  });

  it('shows why discovery is unavailable instead of the empty message', async () => {
    mount({ ndi_list_sources: () => ({ discovered: [], added: [], unavailable: 'NDI support not compiled in' }) });
    expect(await screen.findByText('NDI support not compiled in')).toBeInTheDocument();
    expect(screen.queryByText('No NDI sources found on the network')).not.toBeInTheDocument();
  });

  it('a discovered source appears via SourceListChanged and Add invokes ndi_add_source', async () => {
    const user = userEvent.setup();
    mount({ ndi_add_source: () => null });
    await screen.findByText('No NDI sources found on the network');
    await emitAppEvent({ type: 'SourceListChanged', payload: { sources: [src('Cam A')] } });
    await user.click(await screen.findByRole('button', { name: 'Add Cam A' }));
    await expectCalled(ipc, 'ndi_add_source', [{ name: 'Cam A' }]);
    await emitAppEvent({ type: 'SourceAdded', payload: { name: 'Cam A', format: 'NewFormat' } });
    expect(await screen.findByText('New Format')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add Cam A' })).not.toBeInTheDocument();
  });

  it('SourceLost shows the Source Lost placeholder and SourceDiscovered clears it', async () => {
    mount({
      ndi_list_sources: () => ({
        discovered: [src('Cam A')],
        added: [{ name: 'Cam A', format: 'Standard', lost: false }],
        unavailable: null,
      }),
    });
    expect(await screen.findByText('Standard')).toBeInTheDocument();
    await emitAppEvent({ type: 'SourceLost', payload: { name: 'Cam A' } });
    expect(await screen.findByText('Source Lost')).toBeInTheDocument();
    await emitAppEvent({ type: 'SourceDiscovered', payload: { source: src('Cam A') } });
    expect(await screen.findByText('Standard')).toBeInTheDocument();
    expect(screen.queryByText('Source Lost')).not.toBeInTheDocument();
  });

  it('Remove invokes ndi_remove_source and SourceRemoved drops the row', async () => {
    const user = userEvent.setup();
    mount({
      ndi_list_sources: () => ({ discovered: [src('Cam A')], added: [{ name: 'Cam A', format: 'Standard', lost: false }], unavailable: null }),
      ndi_remove_source: () => null,
    });
    await user.click(await screen.findByRole('button', { name: 'Remove Cam A' }));
    await expectCalled(ipc, 'ndi_remove_source', [{ name: 'Cam A' }]);
    await emitAppEvent({ type: 'SourceRemoved', payload: { name: 'Cam A' } });
    expect(await screen.findByRole('button', { name: 'Add Cam A' })).toBeInTheDocument();
  });

  it('shows a rejected add instead of failing silently', async () => {
    const user = userEvent.setup();
    mount({
      ndi_list_sources: () => ({ discovered: [src('Cam A')], added: [], unavailable: null }),
      ndi_add_source: () => {
        throw { type: 'Internal', message: 'no ndisrc' };
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Add Cam A' }));
    expect(await screen.findByText(/no ndisrc/)).toBeInTheDocument();
  });
});
