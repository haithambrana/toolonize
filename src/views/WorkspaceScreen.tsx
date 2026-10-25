/** Active workspace: header (modes, members, policy), startup prompt,
 * import-review banner, and the docking layout. */
import { useEffect, useMemo } from "react";
import { api, type LayoutMode, type StartupPolicy, type WorkspaceView } from "../lib/ipc";
import {
  addLauncher,
  createWorkspace,
  exportWorkspace,
  openWorkspace,
  setMode,
} from "../lib/actions";
import { addMemberDialog } from "../lib/addMember";
import { confirmDialog, promptDialog } from "../lib/dialogs";
import { TYPE_LABELS, entryName } from "../lib/labels";
import { WorkspaceLayout } from "../layout/WorkspaceLayout";
import { activeWorkspace, attempt, getState, setState, useAppState } from "../state/store";

const MODES: { mode: LayoutMode; label: string; hint: string }[] = [
  { mode: "grid", label: "Grid", hint: "Every member side by side" },
  { mode: "focus", label: "Focus", hint: "One member maximized" },
  { mode: "tabs", label: "Tabs", hint: "All members as tabs" },
  { mode: "master_stack", label: "Master + Stack", hint: "One large member, the rest stacked" },
];

const POLICIES: { value: StartupPolicy; label: string }[] = [
  { value: "ask", label: "Ask what to start" },
  { value: "restore_only", label: "Restore layout only" },
  { value: "restore_terminals", label: "Restore + start terminals" },
  { value: "restore_all", label: "Restore + terminals + external apps" },
];

function StartupPrompt({ w }: { w: WorkspaceView }) {
  const hasExternal = w.members.some((m) => m.member_type === "external_app");
  return (
    <div className="banner" role="region" aria-label="Startup">
      <span>Start this workspace’s members?</span>
      <span className="row-gap">
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={() => void openWorkspace(w, { start_terminals: true, launch_external: false })}
        >
          Start terminals
        </button>
        {hasExternal && (
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => void openWorkspace(w, { start_terminals: true, launch_external: true })}
          >
            Terminals + external apps
          </button>
        )}
        <button
          type="button"
          className="btn btn-sm"
          onClick={() =>
            setState((s) => ({ pendingChoice: { ...s.pendingChoice, [w.id]: false } }))
          }
        >
          Not now
        </button>
      </span>
    </div>
  );
}

function ImportBanner({ w }: { w: WorkspaceView }) {
  const pending = w.members.filter((m) => m.status.state !== "ready").length;
  return (
    <div className="banner banner-warn" role="region" aria-label="Imported workspace">
      <span>
        Imported workspace in review: {pending} member(s) still need review or are missing here.
        Nothing runs until you authorize it.
      </span>
      <button
        type="button"
        className="btn btn-sm"
        onClick={() => void attempt(() => api.acceptImport(w.id))}
      >
        Finish review
      </button>
    </div>
  );
}

function Header({ w }: { w: WorkspaceView }) {
  return (
    <header className="ws-header">
      <h1 className="ws-title">
        <button
          type="button"
          className="title-button"
          title="Rename workspace"
          onClick={async () => {
            const name = await promptDialog({
              title: "Rename workspace",
              label: "Name",
              initial: w.name,
              confirm: "Rename",
            });
            if (name) await attempt(() => api.renameWorkspace(w.id, name));
          }}
        >
          {w.name}
        </button>
      </h1>
      <div className="segmented" role="radiogroup" aria-label="Layout mode">
        {MODES.map((m) => (
          <button
            key={m.mode}
            type="button"
            role="radio"
            aria-checked={w.layout.mode === m.mode}
            className={w.layout.mode === m.mode ? "seg seg-active" : "seg"}
            title={m.hint}
            onClick={() => void setMode(w, m.mode)}
          >
            {m.label}
          </button>
        ))}
      </div>
      <div className="ws-actions">
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={() => void addMemberDialog(w)}
        >
          + Add member
        </button>
        {w.members.length > 0 && (
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => void openWorkspace(w, { start_terminals: true, launch_external: false })}
          >
            Start terminals
          </button>
        )}
        <label className="select-inline">
          <span className="sr-only">Startup policy</span>
          <select
            className="input input-sm"
            value={w.startup_policy}
            aria-label="Startup policy"
            onChange={(e) =>
              void attempt(() => api.setPolicy(w.id, e.target.value as StartupPolicy))
            }
          >
            {POLICIES.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={() => void exportWorkspace(w)}
        >
          Export
        </button>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={async () => {
            const running = Object.values(getState().sessions).filter(
              (s) => s.workspace_id === w.id
            ).length;
            const ok = await confirmDialog({
              title: `Delete ${w.name}?`,
              body: (
                <p>
                  The workspace, its layout and member authorizations are deleted
                  {running ? ` and ${running} running terminal(s) are closed` : ""}. Launchers stay
                  in the registry.
                </p>
              ),
              confirm: "Delete",
              danger: true,
            });
            if (ok) await attempt(() => api.deleteWorkspace(w.id));
          }}
        >
          Delete
        </button>
      </div>
    </header>
  );
}

function EmptyMembers({ w }: { w: WorkspaceView }) {
  const registry = useAppState((s) => s.registry);
  const first = useMemo(
    () =>
      registry?.entries.find(
        (e) => e.origin === "synthetic" && e.executable && e.launcher_id.startsWith("sys:shell:")
      ),
    [registry]
  );
  return (
    <div className="empty">
      <h2>{w.name} has no members yet</h2>
      <p className="muted">
        Add terminals (shells, SSH, tmux), discovered command-line tools, or external applications.
        Discovered launchers that need a human look first are waiting in the Launchers view.
      </p>
      <div className="row-gap">
        {first && (
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void addLauncher(w, first, true)}
          >
            Add {entryName(first)} terminal
          </button>
        )}
        <button type="button" className="btn" onClick={() => void addMemberDialog(w)}>
          Browse launchers…
        </button>
      </div>
      <p className="muted small">
        Supported member kinds: {Object.values(TYPE_LABELS).slice(0, 4).join(", ")}.
      </p>
    </div>
  );
}

export function WorkspaceScreen() {
  const w = useAppState(activeWorkspace);
  const pending = useAppState((s) => (w ? s.pendingChoice[w.id] : false));
  const opened = useAppState((s) => (w ? s.opened[w.id] : true));

  // Run the startup policy once per workspace per app session (FR-024).
  useEffect(() => {
    // Read the store synchronously so a double-invoked effect cannot open twice.
    if (!w || opened || getState().opened[w.id]) return;
    setState((s) => ({ opened: { ...s.opened, [w.id]: true } }));
    void openWorkspace(w, null);
  }, [w, opened]);

  if (!w) {
    return (
      <div className="empty">
        <h2>Your existing dev tools. One persistent workspace.</h2>
        <p className="muted">
          ToolOnize found the launchers on this machine without running any of them. Create a
          workspace, add the tools you use for one project, and ToolOnize restores the layout next
          time.
        </p>
        <button type="button" className="btn btn-primary" onClick={() => void createWorkspace()}>
          Create a workspace
        </button>
      </div>
    );
  }
  return (
    <section className="workspace" aria-label={`Workspace ${w.name}`}>
      <Header w={w} />
      {w.staged_import && <ImportBanner w={w} />}
      {pending && !w.staged_import && <StartupPrompt w={w} />}
      {w.members.length === 0 ? <EmptyMembers w={w} /> : <WorkspaceLayout workspace={w} />}
    </section>
  );
}
