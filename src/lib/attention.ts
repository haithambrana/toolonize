/**
 * Desktop notifications for terminals the user is not looking at: a process
 * exits or fails, an SSH connection drops, the bell rings, or a program asks
 * for a notification (OSC 9 / OSC 777). The core applies the Settings switch
 * and a rate limit; here we only decide whether the user would miss it.
 */
import { api, type SessionInfo } from "./ipc";
import { peekController } from "../terminal/controller";
import { getState, onSessionChange } from "../state/store";

/** True when this terminal is on screen in a focused, visible window. */
export function userSees(sessionId: string): boolean {
  const c = peekController(sessionId);
  const s = getState();
  const session = s.sessions[sessionId];
  return (
    !!c?.isMounted &&
    session?.workspace_id === s.active &&
    s.view === "workspace" &&
    document.visibilityState === "visible" &&
    document.hasFocus()
  );
}

function titleOf(sessionId: string): string {
  return getState().sessions[sessionId]?.title ?? "Terminal";
}

export function attention(sessionId: string, kind: "bell" | "program", message: string): void {
  if (userSees(sessionId)) return;
  const title = titleOf(sessionId);
  const body = kind === "bell" ? "The terminal rang the bell" : message;
  void api.desktopNotify(title, body).catch(() => undefined);
}

const ENDED = ["exited", "failed", "disconnected"];

export function sessionEndedMessage(info: SessionInfo): string | null {
  const s = info.process_state;
  if (s.kind === "exited") return s.code === 0 ? "Finished" : `Exited with code ${s.code}`;
  if (s.kind === "failed") return `Failed: ${s.reason}`;
  if (s.kind === "disconnected") return "Connection lost";
  return null;
}

let installed = false;

export function installAttention(): void {
  if (installed) return;
  installed = true;
  onSessionChange((before, after) => {
    const wasLive = before && !ENDED.includes(before.process_state.kind);
    if (!wasLive || !ENDED.includes(after.process_state.kind)) return;
    if (userSees(after.session_id)) return;
    const message = sessionEndedMessage(after);
    if (message) void api.desktopNotify(after.title, message).catch(() => undefined);
  });
}
