/** Settings (FR-053/054): appearance, terminal, discovery, keyboard, data. */
import { useEffect, useMemo, useRef, useState } from "react";
import { api, TERMINAL_SCHEMES, type Settings, type Snippet } from "../lib/ipc";
import {
  ACTION_LABELS,
  ACTIONS,
  bindingFor,
  conflicts,
  DEFAULT_KEYMAP,
  effectiveKeymap,
  type Action,
} from "../lib/keymap";
import { backupNow, restoreFromFile } from "../lib/backup";
import { attempt, notify, useAppState } from "../state/store";
import { t, tk } from "../i18n";

function update(settings: Settings, patch: Partial<Settings>) {
  return attempt(() => api.updateSettings({ ...settings, ...patch }), t("Could not save settings"));
}

function AutostartToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => {
    let live = true;
    api.autostartGet().then(
      (v) => live && setEnabled(v),
      () => live && setEnabled(false)
    );
    return () => {
      live = false;
    };
  }, []);
  return (
    <label className="check">
      <input
        type="checkbox"
        checked={enabled === true}
        disabled={enabled === null}
        onChange={async (e) => {
          const want = e.target.checked;
          const now = await attempt(
            () => api.autostartSet(want),
            t("Could not change opening at login")
          );
          if (now !== undefined) setEnabled(now);
        }}
      />
      {t(
        "Open ToolOnize when I log in — with the switches below, your last session comes back after a restart"
      )}
    </label>
  );
}

function SessionMemoryCard() {
  const settings = useAppState((s) => s.settings);
  if (!settings) return null;
  return (
    <fieldset className="card card-wide">
      <legend>{t("Session memory")}</legend>
      <p>
        {t(
          "If ToolOnize closes or the computer loses power, it opens again with the same window, the same terminals in the same folders, and what they showed."
        )}
      </p>
      <AutostartToggle />
      <label className="check">
        <input
          type="checkbox"
          checked={settings.restore_terminals}
          onChange={(e) => void update(settings, { restore_terminals: e.target.checked })}
        />
        {t(
          "Start the terminals that were running last time (shells and SSH; commands only when set to restart automatically)"
        )}
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={settings.remember_terminal_content}
          onChange={(e) => void update(settings, { remember_terminal_content: e.target.checked })}
        />
        {t(
          "Remember what each terminal showed (kept on this computer only; turning this off deletes it)"
        )}
      </label>
      <div className="row-gap">
        <button
          type="button"
          className="btn btn-sm"
          onClick={async () => {
            const n = await attempt(
              () => api.forgetTerminalOutput(),
              t("Could not delete saved output")
            );
            if (n !== undefined) notify("info", `Deleted the saved output of ${n} terminal(s).`);
          }}
        >
          {t("Forget saved terminal output")}
        </button>
      </div>
      <p className="muted small">
        {t(
          "Saved output is cleaned so that nothing in it can act again (no clipboard writes, notifications or terminal queries) and is never part of backups, exports or diagnostics. Closing a terminal yourself forgets it."
        )}
      </p>
    </fieldset>
  );
}

function BackupCard() {
  const [withSettings, setWithSettings] = useState(true);
  const [restoreSettings, setRestoreSettings] = useState(false);
  const [busy, setBusy] = useState(false);
  const file = useRef<HTMLInputElement>(null);
  return (
    <fieldset className="card card-wide">
      <legend>{t("Backup & restore")}</legend>
      <p>
        {t(
          "One file with all workspaces, terminals and their options, layouts, notes, templates, launcher names and connections. It is saved to your Downloads folder and stays on this machine unless you move it."
        )}
      </p>
      <div className="row-gap backup-row">
        <button
          type="button"
          className="btn btn-sm btn-primary"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            await backupNow(withSettings);
            setBusy(false);
          }}
        >
          {t("Back up everything")}
        </button>
        <label className="check-inline">
          <input
            type="checkbox"
            checked={withSettings}
            onChange={(e) => setWithSettings(e.target.checked)}
          />
          {t("Include settings")}
        </label>
      </div>
      <div className="row-gap backup-row">
        <button
          type="button"
          className="btn btn-sm"
          disabled={busy}
          onClick={() => file.current?.click()}
        >
          {t("Restore from backup…")}
        </button>
        <label className="check-inline">
          <input
            type="checkbox"
            checked={restoreSettings}
            onChange={(e) => setRestoreSettings(e.target.checked)}
          />
          {t("Also replace my settings")}
        </label>
        <input
          ref={file}
          type="file"
          accept=".json,application/json"
          hidden
          aria-label={t("Backup file")}
          onChange={async (e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (!f) return;
            setBusy(true);
            await restoreFromFile(f, restoreSettings);
            setBusy(false);
          }}
        />
      </div>
      <p className="muted small">
        {t(
          "Restoring adds the workspaces next to the ones you have. For your safety nothing runs from a backup: restored workspaces wait for review and each terminal must be authorized again. Secrets, passwords and launcher approvals are never in a backup."
        )}
      </p>
    </fieldset>
  );
}

function KeyCapture({
  action,
  binding,
  onChange,
}: {
  action: Action;
  binding: string;
  onChange: (b: string | null) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  return (
    <span className="row-gap">
      <button
        type="button"
        className={capturing ? "btn btn-sm btn-primary kbd-capture" : "btn btn-sm kbd-capture"}
        aria-label={`Shortcut for ${ACTION_LABELS[action]}: ${binding}. Press to change.`}
        onClick={() => setCapturing(true)}
        onBlur={() => setCapturing(false)}
        onKeyDown={(e) => {
          if (!capturing) return;
          if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Escape") {
            setCapturing(false);
            return;
          }
          if (!e.ctrlKey && !e.altKey && !e.metaKey) return;
          setCapturing(false);
          onChange(bindingFor(e.nativeEvent));
        }}
      >
        {capturing ? "Press keys…" : <kbd>{binding}</kbd>}
      </button>
      {binding !== DEFAULT_KEYMAP[action] && (
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => onChange(null)}>
          {t("Reset")}
        </button>
      )}
    </span>
  );
}

const SCHEME_LABELS: Record<Settings["terminal_scheme"], string> = {
  auto: tk("Follow the app theme"),
  solarized_dark: "Solarized Dark",
  solarized_light: "Solarized Light",
  dracula: "Dracula",
  nord: "Nord",
  gruvbox_dark: "Gruvbox Dark",
  one_dark: "One Dark",
  tango_light: "Tango Light",
};

/** Saved on Enter/blur, not per keystroke (each save re-validates in Rust). */
function FontFamily({ settings }: { settings: Settings }) {
  const [value, setValue] = useState(settings.terminal_font_family);
  const save = () => {
    if (value.trim() !== settings.terminal_font_family) {
      void update(settings, { terminal_font_family: value.trim() });
    }
  };
  return (
    <label className="field">
      <span className="field-label">{t("Terminal font (leave empty for the default)")}</span>
      <input
        className="input"
        placeholder={t('"Fira Code", monospace')}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => e.key === "Enter" && save()}
      />
    </label>
  );
}

function Snippets({ settings }: { settings: Settings }) {
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  const save = async () => {
    const entry: Snippet = { name: name.trim(), text };
    if (!entry.name || !entry.text) return;
    const list = [...settings.snippets];
    if (editing === null) list.push(entry);
    else list[editing] = entry;
    const ok = await update(settings, { snippets: list });
    if (ok !== undefined) {
      setName("");
      setText("");
      setEditing(null);
    }
  };
  return (
    <fieldset className="card card-wide">
      <legend>{t("Snippets")}</legend>
      <p className="muted small">
        {t(
          "Saved text you insert into the focused terminal with Ctrl+Shift+S. It is pasted like clipboard text (the multi-line warning applies) — ToolOnize never runs it on its own. Add a line break at the end if you want it to run when inserted."
        )}
      </p>
      <ul className="plain-list">
        {settings.snippets.map((s, i) => (
          <li key={`${s.name}-${i}`} className="row-gap">
            <strong>{s.name}</strong>
            <code className="grow break small">{s.text.split("\n")[0].slice(0, 80)}</code>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => {
                setEditing(i);
                setName(s.name);
                setText(s.text);
              }}
            >
              {t("Edit")}
            </button>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                void update(settings, { snippets: settings.snippets.filter((_, j) => j !== i) })
              }
            >
              {t("Delete")}
            </button>
          </li>
        ))}
      </ul>
      <div className="snippet-form">
        <input
          className="input"
          aria-label={t("Snippet name")}
          placeholder={t("Name, e.g. Tail app log")}
          value={name}
          maxLength={60}
          onChange={(e) => setName(e.target.value)}
        />
        <textarea
          className="input mono"
          aria-label={t("Snippet text")}
          placeholder={t("tail -f /var/log/app.log")}
          rows={3}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <div className="row-gap">
          <button
            type="button"
            className="btn btn-sm btn-primary"
            disabled={!name.trim() || !text}
            onClick={() => void save()}
          >
            {editing === null ? "Add snippet" : "Save snippet"}
          </button>
          {editing !== null && (
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => {
                setEditing(null);
                setName("");
                setText("");
              }}
            >
              {t("Cancel")}
            </button>
          )}
        </div>
      </div>
    </fieldset>
  );
}

export function SettingsScreen() {
  const settings = useAppState((s) => s.settings);
  const startup = useAppState((s) => s.startup);
  const registry = useAppState((s) => s.registry);
  const keymap = useMemo(() => effectiveKeymap(settings?.keymap), [settings]);
  const clashes = useMemo(() => conflicts(keymap), [keymap]);
  if (!settings) return null;
  const shells = (registry?.entries ?? []).filter(
    (e) => e.launcher_id.startsWith("sys:shell:") || e.launcher_id.startsWith("sys:wsl:")
  );

  return (
    <section className="settings" aria-label={t("Settings")}>
      <header className="ws-header">
        <h1 className="ws-title">{t("Settings")}</h1>
      </header>
      <div className="settings-grid">
        <fieldset className="card">
          <legend>{t("Appearance")}</legend>
          <label className="field">
            <span className="field-label">{t("Language")}</span>
            <select
              className="input"
              value={settings.language}
              onChange={(e) =>
                void update(settings, { language: e.target.value as Settings["language"] })
              }
            >
              <option value="system">{t("Follow system")}</option>
              <option value="en">English</option>
              <option value="ar">العربية</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">{t("Theme")}</span>
            <select
              className="input"
              value={settings.theme}
              onChange={(e) =>
                void update(settings, { theme: e.target.value as Settings["theme"] })
              }
            >
              <option value="system">{t("Follow system")}</option>
              <option value="dark">{t("Dark")}</option>
              <option value="light">{t("Light")}</option>
              <option value="high_contrast">{t("High contrast")}</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">{t("Motion")}</span>
            <select
              className="input"
              value={settings.motion}
              onChange={(e) =>
                void update(settings, { motion: e.target.value as Settings["motion"] })
              }
            >
              <option value="system">{t("Follow system")}</option>
              <option value="reduced">{t("Reduced")}</option>
              <option value="full">{t("Full")}</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">{t("Terminal font size")}</span>
            <input
              className="input"
              type="number"
              min={8}
              max={32}
              value={settings.font_size}
              onChange={(e) => void update(settings, { font_size: Number(e.target.value) })}
            />
          </label>
          <FontFamily settings={settings} />
          <label className="field">
            <span className="field-label">{t("Terminal colors")}</span>
            <select
              className="input"
              value={settings.terminal_scheme}
              onChange={(e) =>
                void update(settings, {
                  terminal_scheme: e.target.value as Settings["terminal_scheme"],
                })
              }
            >
              {TERMINAL_SCHEMES.map((s) => (
                <option key={s} value={s}>
                  {t(SCHEME_LABELS[s])}
                </option>
              ))}
            </select>
            <span className="muted small">
              {t("High contrast theme always uses its own colors.")}
            </span>
          </label>
          <label className="field">
            <span className="field-label">{t("Cursor")}</span>
            <select
              className="input"
              value={settings.cursor_style}
              onChange={(e) =>
                void update(settings, { cursor_style: e.target.value as Settings["cursor_style"] })
              }
            >
              <option value="block">{t("Block")}</option>
              <option value="underline">{t("Underline")}</option>
              <option value="bar">{t("Bar")}</option>
            </select>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.cursor_blink}
              onChange={(e) => void update(settings, { cursor_blink: e.target.checked })}
            />
            {t("Blinking cursor")}
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.desktop_notifications}
              onChange={(e) => void update(settings, { desktop_notifications: e.target.checked })}
            />
            {t(
              "Desktop notifications when a terminal you are not looking at finishes, disconnects or rings the bell"
            )}
          </label>
        </fieldset>

        <fieldset className="card">
          <legend>{t("Terminals")}</legend>
          <label className="field">
            <span className="field-label">{t("Default shell for new terminals")}</span>
            <select
              className="input"
              value={settings.default_shell ?? ""}
              onChange={(e) => void update(settings, { default_shell: e.target.value || null })}
            >
              <option value="">{t("Automatic")}</option>
              {shells.map((s) => (
                <option key={s.launcher_id} value={s.launcher_id}>
                  {s.display_name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">{t("Scrollback lines per terminal")}</span>
            <input
              className="input"
              type="number"
              min={500}
              max={200000}
              step={500}
              value={settings.scrollback_lines}
              onChange={(e) => void update(settings, { scrollback_lines: Number(e.target.value) })}
            />
          </label>
          <label className="field">
            <span className="field-label">{t("Replay buffer per session (MiB)")}</span>
            <input
              className="input"
              type="number"
              min={2}
              max={64}
              value={settings.replay_buffer_mib}
              onChange={(e) => void update(settings, { replay_buffer_mib: Number(e.target.value) })}
            />
            <span className="muted small">
              {t(
                "Output kept by the core so a reloaded window can redraw running terminals. Applies to terminals started afterwards."
              )}
            </span>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.paste_warning}
              onChange={(e) => void update(settings, { paste_warning: e.target.checked })}
            />
            {t("Warn before pasting multiple lines")}
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.confirm_close_running}
              onChange={(e) => void update(settings, { confirm_close_running: e.target.checked })}
            />
            {t("Confirm before stopping a running terminal")}
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.copy_on_select}
              onChange={(e) => void update(settings, { copy_on_select: e.target.checked })}
            />
            {t("Copy selected text to the clipboard automatically")}
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.osc52_clipboard}
              onChange={(e) => void update(settings, { osc52_clipboard: e.target.checked })}
            />
            {t("Let terminal programs copy to my clipboard (OSC 52 — works over SSH, tmux, vim)")}
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.ssh_auto_reconnect}
              onChange={(e) => void update(settings, { ssh_auto_reconnect: e.target.checked })}
            />
            {t("Reconnect SSH terminals automatically when the connection drops")}
          </label>
          <p className="muted small">
            {t(
              "Copying always goes to this computer’s clipboard, also when the terminal is an SSH session. Programs can only write to the clipboard, never read it."
            )}
          </p>
        </fieldset>

        <Snippets settings={settings} />

        <fieldset className="card">
          <legend>{t("Discovery")}</legend>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.live_updates}
              onChange={(e) => void update(settings, { live_updates: e.target.checked })}
            />
            {t("Watch launcher folders for changes")}
          </label>
          <p className="muted small">
            {t(
              "Discovery only reads launcher metadata. It never runs a discovered program, resolves scripts, or uses the network. PATH is not scanned."
            )}
          </p>
        </fieldset>

        <fieldset className="card card-wide">
          <legend>{t("Keyboard")}</legend>
          {clashes.size > 0 && (
            <p className="alert alert-warn" role="alert">
              {t(
                "Conflicting shortcuts: {list}. Saving is blocked until each shortcut is unique.",
                { list: [...clashes.keys()].join(", ") }
              )}
            </p>
          )}
          <table className="table">
            <tbody>
              {ACTIONS.map((action) => (
                <tr key={action}>
                  <td>{ACTION_LABELS[action]}</td>
                  <td>
                    <KeyCapture
                      action={action}
                      binding={keymap[action]}
                      onChange={(b) => {
                        const next = { ...settings.keymap };
                        if (b === null || b === DEFAULT_KEYMAP[action]) delete next[action];
                        else next[action] = b;
                        void update(settings, { keymap: next });
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted small">
            {t("Shortcuts use Ctrl+Shift so terminal programs keep their own Ctrl keys.")}
          </p>
        </fieldset>

        <SessionMemoryCard />

        <BackupCard />

        <fieldset className="card card-wide">
          <legend>{t("Data & privacy")}</legend>
          <p>
            {t(
              "ToolOnize has no account, no cloud and no telemetry. Nothing leaves this machine except what your own terminals do."
            )}
          </p>
          {startup && (
            <dl className="kv">
              <div className="kv-row">
                <dt>{t("State")}</dt>
                <dd className="mono small break">{startup.state_dir}</dd>
              </div>
              <div className="kv-row">
                <dt>{t("Settings")}</dt>
                <dd className="mono small break">{startup.config_dir}</dd>
              </div>
              <div className="kv-row">
                <dt>{t("Version")}</dt>
                <dd>
                  {startup.version} · {startup.platform} · {startup.backend}
                </dd>
              </div>
            </dl>
          )}
          <p className="muted small">
            {t("To remove ToolOnize completely, uninstall it and delete these folders.")}
          </p>
          <div className="row-gap">
            <button
              type="button"
              className="btn btn-sm"
              onClick={async () => {
                const path = await attempt(
                  () => api.exportDiagnostics(),
                  t("Could not export diagnostics")
                );
                if (path) notify("info", `Diagnostics saved to ${path}`);
              }}
            >
              {t("Export diagnostics")}
            </button>
            <span className="muted small">
              {t(
                "For bug reports: versions, settings and recent log lines — no names, commands, output or notes."
              )}
            </span>
          </div>
          <p className="muted small">
            {t(
              "ToolOnize is a CRMonize.com open-source project (crmonize.com), licensed MIT OR Apache-2.0."
            )}
          </p>
        </fieldset>
      </div>
    </section>
  );
}
