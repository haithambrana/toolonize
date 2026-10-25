/** Controller options derived from settings + theme, and the paste warning. */
import { confirmDialog } from "../lib/dialogs";
import { terminalTheme, type ResolvedTheme } from "../lib/theme";
import type { Settings } from "../lib/ipc";
import { getState, notify } from "../state/store";
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

export function controllerOptions(
  settings: Settings | null,
  theme: ResolvedTheme
): ControllerOptions {
  return {
    fontSize: settings?.font_size ?? 13,
    scrollback: settings?.scrollback_lines ?? 10_000,
    theme: terminalTheme(theme),
    confirmPaste,
    onDesync: (reason) => notify("warning", `Terminal view resynchronized: ${reason}`),
  };
}
