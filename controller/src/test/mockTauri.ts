import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { act, render } from '@testing-library/react';
import { createElement } from 'react';
import App from '../App';
import type { AppEvent } from '../lib/events';

export type IpcHandler = (args: Record<string, unknown>) => unknown;
export interface IpcCall {
  cmd: string;
  args: Record<string, unknown>;
}

// Unhandled commands reject so a missing stub fails loudly instead of returning undefined.
export function installMockIPC(handlers: Record<string, IpcHandler> = {}): { calls: IpcCall[]; unhandled: string[] } {
  const calls: IpcCall[] = [];
  const unhandled: string[] = [];
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      const handler = handlers[cmd];
      if (!handler) {
        unhandled.push(cmd);
        throw new Error(`unhandled IPC command in test: ${cmd}`);
      }
      return handler((args ?? {}) as Record<string, unknown>);
    },
    { shouldMockEvents: true },
  );
  return { calls, unhandled };
}

// Delivers through the real `listen('app-event')` path, so every mounted onAppEvent hears it.
export async function emitAppEvent(event: AppEvent): Promise<void> {
  await act(async () => {
    await emit('app-event', event);
  });
}

export function renderApp() {
  return render(createElement(App));
}

export { clearMocks };
