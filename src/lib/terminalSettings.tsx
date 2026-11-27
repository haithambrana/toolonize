/**
 * Terminal settings (ADR-016): environment variables and a startup command
 * for one terminal. Saving confirms them; settings that came from a backup
 * or import do not run until the user saves (confirms) them here.
 */
import { useState } from "react";
import type { EnvVar, MemberView, WorkspaceView } from "./ipc";
import { api } from "./ipc";
import { openDialog } from "./dialogs";
import { memberTitle } from "./actions";
import { attempt, getState, notify, sessionFor } from "../state/store";

/** "NAME=value" per line; blank lines and lines starting with # are skipped. */
export function parseEnv(text: string): { env: EnvVar[]; error: string | null } {
  const env: EnvVar[] = [];
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) return { env, error: `Line ${i + 1}: use NAME=value` };
    env.push({ name: line.slice(0, eq).trim(), value: line.slice(eq + 1) });
  }
  return { env, error: null };
}

export function formatEnv(env: EnvVar[]): string {
  return env.map((v) => `${v.name}=${v.value}`).join("\n");
}

function SettingsBody({
  m,
  close,
}: {
  m: MemberView;
  close: (v: { env: EnvVar[]; startup_command: string } | undefined) => void;
}) {
  const [envText, setEnvText] = useState(formatEnv(m.launch.env));
  const [command, setCommand] = useState(m.launch.startup_command ?? "");
  const parsed = parseEnv(envText);
  const shellLike = m.member_type === "terminal" || m.member_type === "remote_command";
  const blanks = m.launch.env.filter((v) => v.value === "").map((v) => v.name);
  const commandError = [...command].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127)
    ? "One line only"
    : command.length > 1024
      ? "At most 1024 characters"
      : null;
  const error = parsed.error ?? commandError;
  return (
    <>
      <div className="dialog-body">
        {!m.launch_confirmed && (
          <p className="callout callout-warn" role="alert">
            These settings came from a backup or import and have not run on this computer. Check
            them, then press Save to allow them.
            {blanks.length > 0 &&
              ` Values are never stored in backups — fill in ${blanks.join(", ")}.`}
          </p>
        )}
        <label className="field">
          <span>Environment variables — one NAME=value per line</span>
          <textarea
            className="input code-area"
            rows={6}
            spellCheck={false}
            value={envText}
            placeholder={"API_URL=http://localhost:8080\nNODE_ENV=development"}
            onChange={(e) => setEnvText(e.target.value)}
          />
        </label>
        <label className="field">
          <span>Startup command — typed into the shell when the terminal starts</span>
          <input
            className="input mono"
            value={command}
            disabled={!shellLike}
            placeholder={shellLike ? "npm run dev" : "Only for shells and SSH terminals"}
            onChange={(e) => setCommand(e.target.value)}
          />
        </label>
        {error && (
          <p className="field-error" role="alert">
            {error}
          </p>
        )}
        <p className="muted small">
          Kept on this computer only. Backups keep the names but never the values; workspace exports
          keep neither. Variables that make programs run extra code (such as LD_PRELOAD or BASH_ENV)
          are refused.
        </p>
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)}>
          Cancel
        </button>
        <button
          type="button"
          className="btn btn-primary"
          disabled={error !== null}
          data-autofocus
          onClick={() => close({ env: parsed.env, startup_command: shellLike ? command : "" })}
        >
          Save
        </button>
      </div>
    </>
  );
}

export async function terminalSettings(w: WorkspaceView, m: MemberView): Promise<void> {
  const result = await openDialog<{ env: EnvVar[]; startup_command: string }>(
    `Terminal settings — ${memberTitle(m)}`,
    (close) => <SettingsBody m={m} close={close} />,
    { wide: true }
  );
  if (!result) return;
  const saved = await attempt(
    () => api.updateMember(w.id, m.id, result).then(() => true),
    "Could not save terminal settings"
  );
  if (!saved) return;
  notify(
    "info",
    sessionFor(getState(), w.id, m.id)
      ? "Saved. Restart the terminal to use the new settings."
      : "Saved."
  );
}
