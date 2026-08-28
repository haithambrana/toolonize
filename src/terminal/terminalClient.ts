import { invoke } from "@tauri-apps/api/core";
import type { TerminalProfile, SessionInfo, OutputChunk } from "./terminalTypes";

// Semantic mapping: terminal::* -> terminal_* invoke ids (Tauri 2 behaviour)

export async function terminalProfiles(): Promise<TerminalProfile[]> {
  return invoke<TerminalProfile[]>("terminal_profiles");
}

export async function terminalStart(
  profileId: string,
  rows: number,
  cols: number
): Promise<SessionInfo> {
  const r = await invoke<{ session: SessionInfo }>("terminal_start", {
    request: { profileId, rows, cols },
  });
  return r.session;
}

export async function terminalList(): Promise<SessionInfo[]> {
  const r = await invoke<{ sessions: SessionInfo[] }>("terminal_list");
  return r.sessions;
}

export async function terminalAttach(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_attach", {
    request: { sessionId },
  });
}

export async function terminalDetach(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_detach", {
    request: { sessionId },
  });
}

export async function terminalHide(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_hide", {
    request: { sessionId },
  });
}

export async function terminalShow(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_show", {
    request: { sessionId },
  });
}

export async function terminalWrite(sessionId: string, data: Uint8Array | number[]): Promise<void> {
  const bytes = data instanceof Uint8Array ? Array.from(data) : data;
  if (bytes.length > 16 * 1024) {
    throw new Error("write payload too large");
  }
  await invoke("terminal_write", {
    request: { sessionId, data: bytes },
  });
}

export async function terminalResize(sessionId: string, rows: number, cols: number): Promise<void> {
  if (rows === 0 || cols === 0 || rows > 500 || cols > 1000) {
    throw new Error("invalid dimensions");
  }
  await invoke("terminal_resize", {
    request: { sessionId, rows, cols },
  });
}

export async function terminalAck(sessionId: string, sequence: number): Promise<void> {
  await invoke("terminal_ack", {
    request: { sessionId, sequence },
  });
}

export async function terminalClose(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_close", {
    request: { sessionId },
  });
}

export async function terminalRestart(sessionId: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("terminal_restart", {
    request: { sessionId },
  });
}

export async function terminalPoll(
  sessionId: string,
  maxChunks = 16
): Promise<{ chunks: OutputChunk[]; replayTruncated: boolean }> {
  const r = await invoke<{
    chunks: OutputChunk[];
    replayTruncated: boolean;
  }>("terminal_poll", {
    request: { sessionId, maxChunks },
  });
  // Tauri serde renames to camelCase -> check both
  // Backend returns `replay_truncated` as `replayTruncated` due to serde rename?
  // Our Rust struct uses `replay_truncated` with default serde (snake_case) but
  // we serialize with Rust's default (snake). Need to handle both.
  const replayTruncated =
    (r as unknown as { replay_truncated?: boolean }).replay_truncated ?? r.replayTruncated ?? false;
  return { chunks: r.chunks, replayTruncated };
}

export async function terminalReplay(
  sessionId: string
): Promise<{ bytes: number[]; truncated: boolean }> {
  const r = await invoke<{ bytes: number[]; truncated: boolean }>("terminal_replay", {
    request: { sessionId },
  });
  return r;
}
