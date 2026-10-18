import { render, waitFor } from '@testing-library/react';
import { StrictMode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import App from './App';
import { installMockIPC } from './test/mockTauri';

const tracked = vi.hoisted(() => ({ registered: 0, released: 0 }));

// Wrap listen so every registration and its unlisten call is counted.
vi.mock('@tauri-apps/api/event', async (importOriginal) => {
  const real = await importOriginal<typeof import('@tauri-apps/api/event')>();
  return {
    ...real,
    listen: async (...args: Parameters<typeof real.listen>) => {
      const off = await real.listen(...args);
      tracked.registered++;
      return () => {
        tracked.released++;
        off();
      };
    },
  };
});

describe('presentation: listener lifecycle', () => {
  it('releases every event listener on unmount, including under StrictMode double-mount', async () => {
    installMockIPC();
    const { unmount } = render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await waitFor(() => expect(tracked.registered).toBeGreaterThan(0));
    unmount();
    await waitFor(() => expect(tracked.released).toBe(tracked.registered));
  });
});
