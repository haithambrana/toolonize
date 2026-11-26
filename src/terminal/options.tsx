/** Controller options derived from settings + theme, and the paste warning. */
import { confirmDialog } from "../lib/dialogs";
import { openLink, pendingZoom } from "../lib/actions";
import { attention } from "../lib/attention";
import { terminalTheme, type ResolvedTheme } from "../lib/theme";
import type { Settings } from "../lib/ipc";
import { getState, notify, setState } from "../state/store";
import type { ControllerOptions } from "./controller";

export async function confirmPaste(text: string, bracketed: boolean): Promise<boolean> {
  if (getState().settings?.paste_warning === false) return true;
  const lines = text.split(/\r\n|\r|\n/);
  const preview =
    lines.slice(0, 8).join("\n") + (lines.length > 8 ? `\n… (${lines.length - 8} more lines)` : "");
  return confirmDialog({
    title: `Paste ${lines.length} lines?`,
    body: (
      <>
        <p>
          {bracketed
            ? "The program in this terminal enabled bracketed paste, so it receives the text as one paste rather than as typed commands."
            : "This terminal program did not enable bracketed paste: each line may run as soon as it is pasted."}
        </p>
        <pre className="paste-preview">{preview}</pre>
      </>
    ),
    confirm: "Paste",
    danger: !bracketed,
  });
}

/** Font size for a terminal: settings, plus presentation mode, plus zoom. */
export function terminalFontSize(settings: Settings | null, memberId?: string): number {
  const s = getState();
  const base = settings?.font_size ?? 13;
  const zoom = memberId
    ? (pendingZoom(memberId) ??
      s.workspaces.flatMap((w) => w.members).find((m) => m.id === memberId)?.zoom ??
      0)
    : 0;
  return Math.max(6, Math.min(48, base + zoom + (s.presentation ? 4 : 0)));
}

export function controllerOptions(
  settings: Settings | null,
  theme: ResolvedTheme,
  target?: { sessionId: string; memberId: string }
): ControllerOptions {
  return {
    clipboardPolicy: () => {
      const s = getState().settings;
      return { copyOnSelect: s?.copy_on_select ?? false, osc52: s?.osc52_clipboard ?? true };
    },
    onCopied: (chars, source) =>
      notify(
        "info",
        source === "program"
          ? `A program in this terminal copied ${chars} characters to your clipboard`
          : `Copied ${chars} characters`
      ),
    onContextMenu: target
      ? (x, y) => setState({ termMenu: { ...target, x, y }, focused: target.memberId })
      : undefined,
    onCwd: target
      ? (path, host) => setState((s) => ({ cwd: { ...s.cwd, [target.sessionId]: { path, host } } }))
      : undefined,
    onAttention: target ? (kind, message) => attention(target.sessionId, kind, message) : undefined,
    broadcastTargets: target ? () => broadcastTargets(target.sessionId) : undefined,
    fontSize: terminalFontSize(settings, target?.memberId),
    fontFamily: settings?.terminal_font_family ?? "",
    cursorStyle: settings?.cursor_style ?? "block",
    cursorBlink: settings?.cursor_blink ?? true,
    scrollback: settings?.scrollback_lines ?? 10_000,
    theme: terminalTheme(theme, settings?.terminal_scheme ?? "auto"),
    confirmPaste,
    onOpenLink: (url, hidden) => void openLink(url, hidden),
    onDesync: (reason) => notify("warning", `Terminal view resynchronized: ${reason}`),
  };
}

/** Running sessions of the same workspace while broadcast is on. */
export function broadcastTargets(sessionId: string): string[] {
  const s = getState();
  const self = s.sessions[sessionId];
  if (!self || !s.broadcast[self.workspace_id]) return [];
  return Object.values(s.sessions)
    .filter(
      (x) =>
        x.workspace_id === self.workspace_id &&
        x.session_id !== sessionId &&
        x.process_state.kind === "running"
    )
    .map((x) => x.session_id);
}
