// @sync-with: src-tauri/src/events.rs AppEvent

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SlidePayload } from './ipc';
import type { SceneMediaPayload } from './ipc';
import type { DisplayInfo, NdiSource, PipelineState, PreviewFrame, StreamFormat } from './ipc';

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
  | { type: 'DisplayLost';        payload: null }
  | { type: 'PipelineStateChanged'; payload: { state: PipelineState } }
  | { type: 'PipelineError';   payload: { element: string; message: string } }
  | { type: 'PipelineWarning'; payload: { element: string; message: string } }
  | { type: 'PipelineLatency'; payload: { element: string } }
  | { type: 'PipelineQos';     payload: { element: string; processed: number; dropped: number } }
  | { type: 'SourceDiscovered'; payload: { source: NdiSource } }
  | { type: 'SourceLost';       payload: { name: string } }
  | { type: 'SourceListChanged'; payload: { sources: NdiSource[] } }
  | { type: 'SourceAdded';      payload: { name: string; format: StreamFormat } }
  | { type: 'SourceRemoved';    payload: { name: string } }
  | { type: 'SourceStats';      payload: { name: string; width: number; height: number; fps: number; latency_ms: number } };

export function onAppEvent(handler: (event: AppEvent) => void): Promise<UnlistenFn> {
  return listen<AppEvent>('app-event', (e) => handler(e.payload));
}

// High-rate thumbnails use their own channel (not the AppEvent bus) so they never flood the other window or the bus.
export function onPreviewFrame(handler: (frame: PreviewFrame) => void): Promise<UnlistenFn> {
  return listen<PreviewFrame>('preview-frame', (e) => handler(e.payload));
}
