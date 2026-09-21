// presentation/src/lib/events.ts
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { SceneMediaPayload } from './ipc';

export interface SlidePayload {
  index: number;
  total: number;
  title: string;
  body: string;
  notes: string;
  background_path: string | null;
  layout_unsupported: boolean;
}

export type AppEvent =
  | { type: 'BusLag'; payload: { receiver_id: string; dropped: number } }
  | { type: 'Ping'; payload: { message: string } }
  | { type: 'SlideUpdate'; payload: SlidePayload }
  | { type: 'DeckImportProgress'; payload: { slide: number; total: number } }
  | { type: 'DeckImportComplete'; payload: { deck_id: string } }
  | { type: 'DeckImportFailed';   payload: { slide: number; reason: string } }
  | { type: 'SceneMediaUpdate';   payload: SceneMediaPayload }
  | { type: 'SceneSourceChanged'; payload: { source: 'SlideDeck' | 'SceneMedia' } };

export type AppEventHandler = (event: AppEvent) => void;

export async function onAppEvent(handler: AppEventHandler): Promise<UnlistenFn> {
  return listen<AppEvent>('app-event', (e) => handler(e.payload));
}