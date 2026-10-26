/** Settings (FR-053/054): appearance, terminal, discovery, keyboard, data. */
import { useMemo, useState } from "react";
import { api, type Settings } from "../lib/ipc";
import {
  ACTION_LABELS,
  ACTIONS,
  bindingFor,
  conflicts,
  DEFAULT_KEYMAP,
  effectiveKeymap,
  type Action,
} from "../lib/keymap";
import { attempt, useAppState } from "../state/store";

function update(settings: Settings, patch: Partial<Settings>) {
  return attempt(() => api.updateSettings({ ...settings, ...patch }), "Could not save settings");
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
          Reset
        </button>
      )}
    </span>
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
    <section className="settings" aria-label="Settings">
      <header className="ws-header">
        <h1 className="ws-title">Settings</h1>
      </header>
      <div className="settings-grid">
        <fieldset className="card">
          <legend>Appearance</legend>
          <label className="field">
            <span className="field-label">Theme</span>
            <select
              className="input"
              value={settings.theme}
              onChange={(e) =>
                void update(settings, { theme: e.target.value as Settings["theme"] })
              }
            >
              <option value="system">Follow system</option>
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="high_contrast">High contrast</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">Motion</span>
            <select
              className="input"
              value={settings.motion}
              onChange={(e) =>
                void update(settings, { motion: e.target.value as Settings["motion"] })
              }
            >
              <option value="system">Follow system</option>
              <option value="reduced">Reduced</option>
              <option value="full">Full</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">Terminal font size</span>
            <input
              className="input"
              type="number"
              min={8}
              max={32}
              value={settings.font_size}
              onChange={(e) => void update(settings, { font_size: Number(e.target.value) })}
            />
          </label>
        </fieldset>

        <fieldset className="card">
          <legend>Terminals</legend>
          <label className="field">
            <span className="field-label">Default shell for new terminals</span>
            <select
              className="input"
              value={settings.default_shell ?? ""}
              onChange={(e) => void update(settings, { default_shell: e.target.value || null })}
            >
              <option value="">Automatic</option>
              {shells.map((s) => (
                <option key={s.launcher_id} value={s.launcher_id}>
                  {s.display_name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-label">Scrollback lines per terminal</span>
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
            <span className="field-label">Replay buffer per session (MiB)</span>
            <input
              className="input"
              type="number"
              min={2}
              max={64}
              value={settings.replay_buffer_mib}
              onChange={(e) => void update(settings, { replay_buffer_mib: Number(e.target.value) })}
            />
            <span className="muted small">
              Output kept by the core so a reloaded window can redraw running terminals. Applies to
              terminals started afterwards.
            </span>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.paste_warning}
              onChange={(e) => void update(settings, { paste_warning: e.target.checked })}
            />
            Warn before pasting multiple lines
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.confirm_close_running}
              onChange={(e) => void update(settings, { confirm_close_running: e.target.checked })}
            />
            Confirm before stopping a running terminal
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.copy_on_select}
              onChange={(e) => void update(settings, { copy_on_select: e.target.checked })}
            />
            Copy selected text to the clipboard automatically
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.osc52_clipboard}
              onChange={(e) => void update(settings, { osc52_clipboard: e.target.checked })}
            />
            Let terminal programs copy to my clipboard (OSC 52 — works over SSH, tmux, vim)
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.ssh_auto_reconnect}
              onChange={(e) => void update(settings, { ssh_auto_reconnect: e.target.checked })}
            />
            Reconnect SSH terminals automatically when the connection drops
          </label>
          <p className="muted small">
            Copying always goes to this computer’s clipboard, also when the terminal is an SSH
            session. Programs can only write to the clipboard, never read it.
          </p>
        </fieldset>

        <fieldset className="card">
          <legend>Discovery</legend>
          <label className="check">
            <input
              type="checkbox"
              checked={settings.live_updates}
              onChange={(e) => void update(settings, { live_updates: e.target.checked })}
            />
            Watch launcher folders for changes
          </label>
          <p className="muted small">
            Discovery only reads launcher metadata. It never runs a discovered program, resolves
            scripts, or uses the network. PATH is not scanned.
          </p>
        </fieldset>

        <fieldset className="card card-wide">
          <legend>Keyboard</legend>
          {clashes.size > 0 && (
            <p className="alert alert-warn" role="alert">
              Conflicting shortcuts: {[...clashes.keys()].join(", ")}. Saving is blocked until each
              shortcut is unique.
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
            Shortcuts use Ctrl+Shift so terminal programs keep their own Ctrl keys.
          </p>
        </fieldset>

        <fieldset className="card card-wide">
          <legend>Data & privacy</legend>
          <p>
            ToolOnize has no account, no cloud and no telemetry. Nothing leaves this machine except
            what your own terminals do.
          </p>
          {startup && (
            <dl className="kv">
              <div className="kv-row">
                <dt>State</dt>
                <dd className="mono small break">{startup.state_dir}</dd>
              </div>
              <div className="kv-row">
                <dt>Settings</dt>
                <dd className="mono small break">{startup.config_dir}</dd>
              </div>
              <div className="kv-row">
                <dt>Version</dt>
                <dd>
                  {startup.version} · {startup.platform} · {startup.backend}
                </dd>
              </div>
            </dl>
          )}
          <p className="muted small">
            To remove ToolOnize completely, uninstall it and delete these folders.
          </p>
          <p className="muted small">
            ToolOnize is a CRMonize.com open-source project (crmonize.com), licensed MIT OR
            Apache-2.0.
          </p>
        </fieldset>
      </div>
    </section>
  );
}
