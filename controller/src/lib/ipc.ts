// controller/src/lib/ipc.ts
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

// @sync-with: src-tauri/src/error.rs AppError
export type AppError =
  | { type: 'Internal'; message: string }
  | { type: 'NotFound'; message: string }
  | { type: 'InvalidInput'; message: string }
  | { type: 'Config'; message: string }
  | { type: 'Keychain'; message: string };

// @sync-with: src-tauri/src/commands/system.rs PingResponse
export interface PingResponse {
  ok: boolean;
  version: string;
}

// @sync-with: src-tauri/src/commands/info.rs BuildInfo
export interface BuildInfo {
  build_timestamp: string;
  version: string;
}

// @sync-with: src-tauri/src/slide.rs SlidePayload
export interface SlidePayload {
  index: number;
  total: number;
  title: string;
  body: string;
  notes: string;
  background_path: string | null;
}

export interface SceneMediaPayload {
  media_type: 'Image' | 'Video';
  path: string;
  display_name: string;
}

export type SceneSource = 'SlideDeck' | 'SceneMedia';

export async function pickDeckFile(): Promise<string | null> {
  const result = await open({
    multiple: false,
    filters: [{ name: 'Presentation', extensions: ['pptx', 'pdf'] }],
  });
  return typeof result === 'string' ? result : null;
}

export async function deckImport(path: string): Promise<string> {
  return invoke<string>('deck_import', { path });
}

export async function deckGetAllSlides(): Promise<SlidePayload[]> {
  return invoke<SlidePayload[]>('deck_get_all_slides');
}

export async function scenePushImage(path: string): Promise<void> {
  return invoke<void>('scene_push_image', { path });
}

export async function scenePushVideo(path: string): Promise<void> {
  return invoke<void>('scene_push_video', { path });
}

export async function sceneSetSource(source: SceneSource): Promise<void> {
  return invoke<void>('scene_set_source', { source });
}

export async function sceneGetActiveSource(): Promise<SceneSource> {
  return invoke<SceneSource>('scene_get_active_source');
}

/**
 * Validates the IPC layer end-to-end.
 * Never call invoke() directly from components — use this wrapper.
 */
export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>('ping');
}

export async function getBuildInfo(): Promise<BuildInfo> {
  return invoke<BuildInfo>('get_build_info');
}

export async function presentationShow(): Promise<void> {
  return invoke<void>('presentation_show');
}

export async function presentationHide(): Promise<void> {
  return invoke<void>('presentation_hide');
}

export async function slideAdvance(): Promise<void> {
  return invoke<void>('slide_advance');
}

export async function slideRewind(): Promise<void> {
  return invoke<void>('slide_rewind');
}

export async function slideJumpTo(index: number): Promise<void> {
  return invoke<void>('slide_jump_to', { index });
}

export async function slideGetCurrent(): Promise<SlidePayload> {
  return invoke<SlidePayload>('slide_get_current');
}

export async function slideTimerReset(): Promise<void> {
  return invoke<void>('slide_timer_reset');
}