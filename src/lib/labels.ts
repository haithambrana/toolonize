/** Display labels shared across views. */
import type { LauncherType, RegistryEntry, SessionInfo } from "./ipc";
import { t, tk } from "../i18n";

export const TYPE_LABELS: Record<LauncherType, string> = {
  embedded_terminal: tk("Terminal"),
  local_command: tk("Local command"),
  remote_ssh: tk("Remote (SSH)"),
  external_gui: tk("External app"),
  unknown: tk("Needs review"),
};

export function entryName(e: RegistryEntry): string {
  return e.decision.alias ?? e.display_name;
}

const STATE_NAMES: Record<string, string> = {
  new: tk("New"),
  starting: tk("Starting"),
  running: tk("Running"),
  stopping: tk("Stopping"),
  restarting: tk("Restarting"),
  reconnecting: tk("Reconnecting"),
  closed: tk("Closed"),
};

export function processLabel(info: SessionInfo): string {
  const s = info.process_state;
  switch (s.kind) {
    case "exited":
      return s.signal
        ? t("Exited ({signal})", { signal: s.signal })
        : t("Exited with code {code}", { code: s.code });
    case "failed":
      return t("Failed to start: {reason}", { reason: s.reason });
    case "disconnected":
      return t("Disconnected");
    default:
      return t(STATE_NAMES[s.kind] ?? s.kind);
  }
}
