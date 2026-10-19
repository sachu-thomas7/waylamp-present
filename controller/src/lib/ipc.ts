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
  layout_unsupported: boolean;
}

export interface SceneMediaPayload {
  media_type: 'Image' | 'Video';
  path: string;
  display_name: string;
}

export type SceneSource = 'SlideDeck' | 'SceneMedia';

// @sync-with: src-tauri/src/display.rs DisplayInfo
export interface DisplayInfo {
  id: string;
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  is_primary: boolean;
}

export type PipelineState = 'Null' | 'Paused' | 'Playing';

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

export async function deckClear(): Promise<void> {
  return invoke<void>('deck_clear');
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

export async function displayList(): Promise<DisplayInfo[]> {
  return invoke<DisplayInfo[]>('display_list');
}

export async function displayAssignPresentation(id: string): Promise<void> {
  return invoke<void>('display_assign_presentation', { id });
}

export async function pipelineStartTest(): Promise<void> {
  return invoke<void>('pipeline_start_test');
}

export async function pipelinePause(): Promise<void> {
  return invoke<void>('pipeline_pause');
}

export async function pipelineResume(): Promise<void> {
  return invoke<void>('pipeline_resume');
}

export async function pipelineStop(): Promise<void> {
  return invoke<void>('pipeline_stop');
}

export async function pipelineGetState(): Promise<PipelineState> {
  return invoke<PipelineState>('pipeline_get_state');
}

export interface NdiSource {
  name: string;
  url: string | null;
  advertises_new_format: boolean;
}

export type StreamFormat = 'NewFormat' | 'Standard';

export interface AddedSource {
  name: string;
  format: StreamFormat;
  lost: boolean;
}

export interface NdiSnapshot {
  discovered: NdiSource[];
  added: AddedSource[];
  unavailable: string | null;
}

export async function ndiListSources(): Promise<NdiSnapshot> {
  return invoke<NdiSnapshot>('ndi_list_sources');
}

export async function ndiAddSource(name: string): Promise<void> {
  return invoke<void>('ndi_add_source', { name });
}

export async function ndiRemoveSource(name: string): Promise<void> {
  return invoke<void>('ndi_remove_source', { name });
}
