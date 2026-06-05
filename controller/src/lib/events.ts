// @sync-with: src-tauri/src/events.rs AppEvent

import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type AppEvent =
  | { type: 'BusLag'; payload: { receiver_id: string; dropped: number } }
  | { type: 'Ping';   payload: { message: string } };

export type AppEventHandler = (event: AppEvent) => void;

export async function onAppEvent(handler: AppEventHandler): Promise<UnlistenFn> {
  return listen<AppEvent>('app-event', (e) => handler(e.payload));
}