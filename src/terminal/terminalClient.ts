import { invoke } from "@tauri-apps/api/core";
import type {
  TerminalProfile,
  SessionInfo,
  OutputChunk,
  AttachResponse,
  ReplayInfo,
} from "./terminalTypes";

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

export async function terminalAttach(sessionId: string): Promise<AttachResponse> {
  const r = await invoke<AttachResponse>("terminal_attach", {
    request: { sessionId },
  });
  // Backward compat: if server still returns plain SessionInfo (old mock), wrap
  if ((r as unknown as SessionInfo).session_id) {
    const sess = r as unknown as SessionInfo;
    return {
      session: sess,
      attachment_epoch: 0,
      next_sequence: 0,
      acknowledged_up_to: null,
      replay_truncated: sess.replay_truncated,
      replay_discarded_bytes: 0,
    };
  }
  return r;
}

// Helper for callers that only need SessionInfo
export async function terminalAttachSession(sessionId: string): Promise<SessionInfo> {
  const r = await terminalAttach(sessionId);
  return r.session;
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
): Promise<{ chunks: OutputChunk[]; replayTruncated: boolean; nextSequence: number }> {
  const r = await invoke<{
    chunks: OutputChunk[];
    replayTruncated?: boolean;
    replay_truncated?: boolean;
    nextSequence?: number;
    next_sequence?: number;
    replayDiscardedBytes?: number;
  }>("terminal_poll", {
    request: { sessionId, maxChunks },
  });
  const replayTruncated =
    (r as unknown as { replay_truncated?: boolean }).replay_truncated ?? r.replayTruncated ?? false;
  const nextSequence =
    (r as unknown as { next_sequence?: number }).next_sequence ?? r.nextSequence ?? 0;
  return { chunks: r.chunks, replayTruncated, nextSequence };
}

export async function terminalReplay(sessionId: string): Promise<ReplayInfo> {
  const r = await invoke<ReplayInfo>("terminal_replay", {
    request: { sessionId },
  });
  // Handle old shape { bytes, truncated }
  if ((r as unknown as { discarded_bytes?: number }).discarded_bytes === undefined) {
    const legacy = r as unknown as { bytes: number[]; truncated: boolean };
    return {
      bytes: legacy.bytes,
      truncated: legacy.truncated,
      discarded_bytes: 0,
      next_sequence: 0,
      attachment_epoch: 0,
    };
  }
  return r;
}
