/** Display labels shared across views. */
import type { LauncherType, RegistryEntry, SessionInfo } from "./ipc";

export const TYPE_LABELS: Record<LauncherType, string> = {
  embedded_terminal: "Terminal",
  local_command: "Local command",
  remote_ssh: "Remote (SSH)",
  external_gui: "External app",
  unknown: "Needs review",
};

export function entryName(e: RegistryEntry): string {
  return e.decision.alias ?? e.display_name;
}

export function processLabel(info: SessionInfo): string {
  const s = info.process_state;
  switch (s.kind) {
    case "exited":
      return s.signal ? `Exited (${s.signal})` : `Exited with code ${s.code}`;
    case "failed":
      return `Failed to start: ${s.reason}`;
    case "disconnected":
      return "Disconnected";
    default:
      return s.kind.charAt(0).toUpperCase() + s.kind.slice(1);
  }
}
