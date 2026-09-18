import { invoke } from '@tauri-apps/api/core';

// @sync-with: src-tauri/src/error.rs AppError
export type AppError =
  | { type: 'Internal';     message: string }
  | { type: 'NotFound';     message: string }
  | { type: 'InvalidInput'; message: string };

// @sync-with: src-tauri/src/commands/system.rs PingResponse
export interface PingResponse {
  ok: boolean;
  version: string;
}

export interface SceneMediaPayload {
  media_type: 'Image' | 'Video';
  path: string;
  display_name: string;
}


/**
 * Validates the IPC layer end-to-end.
 * Never call invoke() directly from components — use this wrapper.
 */
export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>('ping');
}

export async function presentationShow(): Promise<void> {
  return invoke<void>('presentation_show');
}

export async function presentationHide(): Promise<void> {
  return invoke<void>('presentation_hide');
}