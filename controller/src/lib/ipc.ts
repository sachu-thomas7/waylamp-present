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

// @sync-with: src-tauri/src/preview.rs PreviewFrame
export interface PreviewFrame {
  source: string;
  width: number;
  height: number;
  png_base64: string;
}

// @sync-with: src-tauri/src/preview.rs SourceStatsSample
export interface SourceStatsSample {
  name: string;
  width: number;
  height: number;
  fps: number;
  latency_ms: number;
}

export interface PreviewSnapshot {
  frames: PreviewFrame[];
  stats: SourceStatsSample[];
}

export type PreviewLayout = '2x2' | '2x3' | '2x4';

export async function previewGetSnapshot(): Promise<PreviewSnapshot> {
  return invoke<PreviewSnapshot>('preview_get_snapshot');
}

export async function previewGetLayout(): Promise<PreviewLayout> {
  return invoke<PreviewLayout>('preview_get_layout');
}

export async function previewSetLayout(layout: PreviewLayout): Promise<void> {
  return invoke<void>('preview_set_layout', { layout });
}

export type WhipLinkState = 'Connected' | 'Disconnected' | 'Lost';
export type WhipCodec = 'H264Baseline' | 'Vp8';

// @sync-with: src-tauri/src/whip.rs SessionInfo
export interface WhipSession {
  slot: string;
  state: WhipLinkState;
  codec: WhipCodec;
}

// @sync-with: src-tauri/src/whip.rs WhipInfo
export interface WhipInfo {
  enabled: boolean;
  running: boolean;
  port: number;
  host: string | null;
  token_set: boolean;
  unavailable: string | null;
  last_error: string | null;
  slots: Array<{ slot: string; url: string | null; session: WhipSession | null }>;
}

// @sync-with: src-tauri/src/whip.rs QrMatrix
export interface QrMatrix {
  size: number;
  modules: boolean[];
}

export async function whipGetInfo(): Promise<WhipInfo> {
  return invoke<WhipInfo>('whip_get_info');
}

export async function whipSetEnabled(enabled: boolean): Promise<void> {
  return invoke<void>('whip_set_enabled', { enabled });
}

export async function whipSetToken(token: string | null): Promise<void> {
  return invoke<void>('whip_set_token', { token });
}

export async function whipGetQr(slot: string): Promise<QrMatrix> {
  return invoke<QrMatrix>('whip_get_qr', { slot });
}

export type CompositorSourceKind = 'test' | 'ndi';

// @sync-with: src-tauri/src/compositor.rs SourceGeometry
export interface SourceGeometry {
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
  kind: CompositorSourceKind;
}

// @sync-with: src-tauri/src/compositor.rs CompositorStatus
export interface CompositorStatus {
  running: boolean;
  element: string | null;
  hardware: boolean;
  width: number;
  height: number;
  fps: number;
  sources: SourceGeometry[];
}

export async function compositorListScenes(): Promise<string[]> {
  return invoke<string[]>('compositor_list_scenes');
}

export async function compositorStart(sceneId: string | null): Promise<void> {
  return invoke<void>('compositor_start', { sceneId });
}

export async function compositorStop(): Promise<void> {
  return invoke<void>('compositor_stop');
}

export async function compositorSetLayout(sources: SourceGeometry[]): Promise<void> {
  return invoke<void>('compositor_set_layout', { sources });
}

export async function compositorGetState(): Promise<CompositorStatus> {
  return invoke<CompositorStatus>('compositor_get_state');
}
