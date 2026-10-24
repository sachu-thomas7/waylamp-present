// @sync-with: src-tauri/src/events.rs AppEvent

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SlidePayload } from './ipc';
import type { SceneMediaPayload } from './ipc';
import type { DisplayInfo } from './ipc';

export type AppEvent =
  | { type: 'BusLag'; payload: { receiver_id: string; dropped: number } }
  | { type: 'Ping'; payload: { message: string } }
  | { type: 'ConfigReloaded'; payload: null }
  | { type: 'SlideUpdate'; payload: SlidePayload }
  | { type: 'PresentationVisibility'; payload: { visible: boolean } }
  | { type: 'SlideAdvance'; payload: null }
  | { type: 'SlideRewind'; payload: null }
  | { type: 'DeckEnd'; payload: null }
  | { type: 'DeckStart'; payload: null }
  | { type: 'TimerTick'; payload: { elapsed_secs: number } }
  | { type: 'TimerReset'; payload: null }
  | { type: 'DeckImportProgress'; payload: { slide: number; total: number } }
  | { type: 'DeckImportComplete'; payload: { deck_id: string } }
  | { type: 'DeckImportFailed';   payload: { slide: number; reason: string } }
  | { type: 'SceneMediaUpdate';   payload: SceneMediaPayload }
  | { type: 'SceneSourceChanged'; payload: { source: 'SlideDeck' | 'SceneMedia' } }
  | { type: 'DisplaysChanged';    payload: DisplayInfo[] }
  | { type: 'DisplayAssigned';    payload: { display_id: string } }
  | { type: 'DisplayLost';        payload: null };

export function onAppEvent(handler: (event: AppEvent) => void): Promise<UnlistenFn> {
  return listen<AppEvent>('app-event', (e) => handler(e.payload));
}
