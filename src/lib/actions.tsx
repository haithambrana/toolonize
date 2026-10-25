/**
 * User-intent actions shared by buttons, the command palette and keyboard
 * shortcuts. Each is an IPC request; the Rust core decides what happens.
 */
import {
  api,
  type LayoutMode,
  type MemberView,
  type RegistryEntry,
  type WorkspaceView,
} from "./ipc";
import { layoutFor, type LayoutMember } from "../layout/modes";
import { confirmDialog, openDialog, promptDialog } from "./dialogs";
import { disposeController } from "../terminal/controller";
import {
  activeWorkspace,
  attempt,
  getState,
  notify,
  refreshWorkspaces,
  sessionFor,
  setState,
  type View,
} from "../state/store";

export function memberTitle(m: MemberView): string {
  return m.title ?? m.status.launcher_name ?? m.launcher_id;
}

export function layoutMembers(w: WorkspaceView): LayoutMember[] {
  return w.members.map((m) => ({ id: m.id, title: memberTitle(m) }));
}

export function setView(view: View): void {
  setState({ view, paletteOpen: false });
}

export async function selectWorkspace(id: string): Promise<void> {
  setState({ view: "workspace", active: id });
  await attempt(() => api.setActive(id));
}

export async function setMode(
  w: WorkspaceView,
  mode: LayoutMode,
  focused?: string | null
): Promise<void> {
  const focus =
    focused ?? getState().focused ?? w.layout.focused_member ?? w.members[0]?.id ?? null;
  const json = layoutFor(mode, layoutMembers(w), focus);
  await attempt(() => api.saveLayout(w.id, mode, json, focus), "Could not change layout");
}

export async function startMember(w: WorkspaceView, m: MemberView): Promise<void> {
  const existing = sessionFor(getState(), w.id, m.id);
  if (existing) return;
  const info = await attempt(
    () => api.startSession(w.id, m.id, 24, 80),
    `Could not start ${memberTitle(m)}`
  );
  if (info) {
    setState((s) => ({ sessions: { ...s.sessions, [info.session_id]: info }, focused: m.id }));
  }
}

export async function launchMember(w: WorkspaceView, m: MemberView): Promise<void> {
  const pid = await attempt(
    () => api.launchMember(w.id, m.id),
    `Could not launch ${memberTitle(m)}`
  );
  if (pid !== undefined) setState((s) => ({ launched: { ...s.launched, [m.id]: Date.now() } }));
}

export async function restartSession(sessionId: string): Promise<void> {
  await attempt(() => api.restart(sessionId), "Restart failed");
}

export async function reconnectSession(sessionId: string, detachExpected: boolean): Promise<void> {
  const ok = await confirmDialog({
    title: "Reconnect",
    body: (
      <p>
        Reconnecting runs the command again through your own SSH configuration.{" "}
        {detachExpected
          ? "This member uses tmux/screen, so your remote session resumes if it is still running."
          : "Plain SSH starts a NEW remote shell: programs from the previous connection are not resumed."}
      </p>
    ),
    confirm: "Reconnect",
  });
  if (!ok) return;
  await attempt(() => api.reconnect(sessionId), "Reconnect failed");
}

export async function stopSession(sessionId: string, title: string): Promise<void> {
  const ok =
    !getState().settings?.confirm_close_running ||
    (await confirmDialog({
      title: `Stop ${title}?`,
      body: <p>The process and everything it started in this terminal will be terminated.</p>,
      confirm: "Stop",
      danger: true,
    }));
  if (ok) await attempt(() => api.stop(sessionId), "Stop failed");
}

export async function closeSession(sessionId: string): Promise<void> {
  await attempt(() => api.close(sessionId), "Close failed");
  disposeController(sessionId);
}

export async function openWorkspace(
  w: WorkspaceView,
  choice: { start_terminals: boolean; launch_external: boolean } | null
) {
  const result = await attempt(() => api.openWorkspace(w.id, choice), "Could not open workspace");
  if (!result) return;
  if (choice !== null || result.needs_choice) {
    setState((s) => ({ pendingChoice: { ...s.pendingChoice, [w.id]: result.needs_choice } }));
  }
  const denied = result.outcomes.filter((o) => o.outcome === "denied");
  if (denied.length) {
    notify(
      "warning",
      `${denied.length} member(s) could not start: ${denied.map((d) => d.reason).join("; ")}`
    );
  }
}

export function defaultShell(): RegistryEntry | undefined {
  const s = getState();
  const entries = s.registry?.entries ?? [];
  const preferred = s.settings?.default_shell;
  const shells = entries.filter(
    (e) => e.origin === "synthetic" && e.launcher_id.startsWith("sys:shell:") && e.executable
  );
  return shells.find((e) => e.launcher_id === preferred) ?? shells[0];
}

export async function addLauncher(
  w: WorkspaceView,
  entry: RegistryEntry,
  start: boolean
): Promise<void> {
  // Distinguish repeated members of the same launcher ("bash 2", "bash 3").
  const same = w.members.filter((m) => m.launcher_id === entry.launcher_id).length;
  const base = entry.decision.alias ?? entry.display_name;
  const title = same > 0 ? `${base} ${same + 1}`.slice(0, 80) : null;
  const memberId = await attempt(
    () => api.addMember(w.id, entry.launcher_id, entry.fingerprint, title),
    `Could not add ${entry.display_name}`
  );
  if (!memberId) return;
  await refreshWorkspaces();
  const updated = getState().workspaces.find((x) => x.id === w.id);
  const member = updated?.members.find((m) => m.id === memberId);
  if (!updated || !member) return;
  // Re-generate the preset for the current mode so the new member shows up.
  const focus = memberId;
  setState({ focused: focus });
  await attempt(() =>
    api.saveLayout(
      updated.id,
      updated.layout.mode,
      layoutFor(updated.layout.mode, layoutMembers(updated), focus),
      focus
    )
  );
  if (start && member.member_type !== "external_app") await startMember(updated, member);
}

export async function addTerminal(): Promise<void> {
  const w = activeWorkspace(getState());
  if (!w) {
    notify("info", "Create a workspace first.");
    return;
  }
  const shell = defaultShell();
  if (!shell) {
    notify("error", "No shell was found on this machine.");
    return;
  }
  await addLauncher(w, shell, true);
}

export async function removeMember(w: WorkspaceView, m: MemberView): Promise<void> {
  const session = sessionFor(getState(), w.id, m.id);
  const ok = await confirmDialog({
    title: `Remove ${memberTitle(m)}?`,
    body: (
      <p>
        The member leaves this workspace{session ? " and its running terminal is closed" : ""}. The
        launcher itself stays in the registry.
      </p>
    ),
    confirm: "Remove",
    danger: true,
  });
  if (!ok) return;
  await attempt(() => api.removeMember(w.id, m.id), "Could not remove member");
  if (session) disposeController(session.session_id);
}

export async function exportWorkspace(w: WorkspaceView): Promise<void> {
  const machineBound = await openDialog<boolean>("Export workspace", (close) => (
    <>
      <div className="dialog-body">
        <p>
          Exports contain names, layout and startup policy. Members travel as portable hints (for
          example a Desktop File ID or an SSH alias) — never as command lines, credentials or
          secrets. Importing always lands in review: nothing runs until it is authorized again.
        </p>
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)}>
          Cancel
        </button>
        <button type="button" className="btn" onClick={() => close(true)}>
          Machine-bound export
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => close(false)}
          data-autofocus
        >
          Portable export
        </button>
      </div>
    </>
  ));
  if (machineBound === undefined) return;
  const text = await attempt(() => api.exportWorkspace(w.id, machineBound), "Export refused");
  if (!text) return;
  await openDialog<void>(
    "Workspace export",
    (close) => (
      <>
        <div className="dialog-body">
          <textarea
            className="input code-area"
            readOnly
            value={text}
            rows={14}
            aria-label="Exported JSON"
          />
        </div>
        <div className="dialog-actions">
          <button
            type="button"
            className="btn"
            onClick={() => {
              const blob = new Blob([text], { type: "application/json" });
              const a = document.createElement("a");
              a.href = URL.createObjectURL(blob);
              a.download = `${w.name.replace(/[^\w.-]+/g, "_")}.toolonize.json`;
              a.click();
              URL.revokeObjectURL(a.href);
            }}
          >
            Save file…
          </button>
          <button
            type="button"
            className="btn btn-primary"
            data-autofocus
            onClick={() => {
              void navigator.clipboard?.writeText(text).then(
                () => notify("info", "Copied to clipboard"),
                () => notify("error", "Clipboard is not available")
              );
              close(undefined);
            }}
          >
            Copy and close
          </button>
        </div>
      </>
    ),
    { wide: true }
  );
}

export async function importWorkspaceText(text: string): Promise<void> {
  const report = await attempt(() => api.importWorkspace(text), "Import rejected");
  if (!report) return;
  await refreshWorkspaces();
  setState({ active: report.workspace_id, view: "workspace" });
  notify(
    report.unresolved.length || report.warnings.length ? "warning" : "info",
    `Imported ${report.members} member(s) for review.` +
      (report.unresolved.length ? ` Not found here: ${report.unresolved.join(", ")}.` : "") +
      (report.warnings.length ? ` ${report.warnings.join("; ")}.` : "")
  );
}

export async function createWorkspace(): Promise<void> {
  const name = await promptDialog({ title: "New workspace", label: "Name", confirm: "Create" });
  if (!name) return;
  const id = await attempt(() => api.createWorkspace(name), "Could not create workspace");
  if (id) await selectWorkspace(id);
}

/** Rename a workspace member (and its running terminal). Empty resets. */
export async function renameMember(workspaceId: string, memberId: string): Promise<void> {
  const w = getState().workspaces.find((x) => x.id === workspaceId);
  const m = w?.members.find((x) => x.id === memberId);
  if (!w || !m) return;
  const name = await promptDialog({
    title: "Rename terminal",
    label: "Name (leave empty to use the launcher's name)",
    initial: m.title ?? memberTitle(m),
    confirm: "Rename",
    validate: (v) =>
      v.trim().length > 80
        ? "At most 80 characters"
        : [...v].some((c) => c.charCodeAt(0) < 32)
          ? "Single line only"
          : null,
  });
  if (name === undefined) return;
  await saveMemberTitle(workspaceId, memberId, name);
}

export async function saveMemberTitle(workspaceId: string, memberId: string, name: string) {
  await attempt(
    () => api.updateMember(workspaceId, memberId, { title: name.trim() }),
    "Could not rename"
  );
}
