import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { baseHandlers, resetStores } from '../test/componentHelpers';
import { installMockIPC, renderApp } from '../test/mockTauri';

let ipc: ReturnType<typeof installMockIPC>;

beforeEach(resetStores);
afterEach(() => expect(ipc.unhandled).toEqual([]));

describe('StartupNotices', () => {
  it('shows nothing when startup was clean', async () => {
    ipc = installMockIPC(baseHandlers());
    renderApp();
    await screen.findByText('No slides yet');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows each recovery notice in a banner that can be dismissed', async () => {
    const notice = 'Settings (streamdeck.toml) was damaged and no usable backup existed; defaults are in use.';
    ipc = installMockIPC(baseHandlers({ get_startup_notices: () => [notice] }));
    renderApp();
    expect(await screen.findByRole('alert')).toHaveTextContent(notice);
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
