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

/**
 * Validates the IPC layer end-to-end.
 * Never call invoke() directly from components — use this wrapper.
 */
export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>('ping');
}