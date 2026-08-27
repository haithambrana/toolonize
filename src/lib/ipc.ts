import { invoke } from "@tauri-apps/api/core";

export interface PingResponse {
  app_name: string;
  app_version: string;
  target_os: string;
  target_arch: string;
  status: string;
}

/**
 * Typed wrapper for the single M1 IPC command.
 *
 * Tauri command identifier is `ping` (Rust function name). The semantic
 * identity per PRD/IMPLEMENTATION_PLAN is `app::ping`; the literal invoke
 * key follows Tauri 2 conventions (function name) and is documented here.
 */
export async function ping(): Promise<PingResponse> {
  return invoke<PingResponse>("ping");
}
