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
