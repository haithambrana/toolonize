/**
 * User-intent actions shared by buttons, the command palette and keyboard
 * shortcuts. Each is an IPC request; the Rust core decides what happens.
 */
import {
  api,
  type ColorTag,
  type LayoutMode,
  type MemberView,
  type RegistryEntry,
  type WorkspaceView,
} from "./ipc";
import {
  layoutFor,
  placeBeside,
  reconcile,
  type LayoutMember,
  type SplitSide,
} from "../layout/modes";
import { confirmDialog, openDialog, promptDialog } from "./dialogs";
import { disposeController } from "../terminal/controller";
import { flushNote, hasNote, loadNotes } from "./notes";
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
  // Broadcast never follows the user into another workspace.
  setState({ broadcast: {} });
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

export interface AddOptions {
  /** Folder the new terminal starts in. */
  startDir?: string | null;
  /** Quick split: place the new pane next to this member. */
  beside?: { target: string; side: SplitSide };
}

export async function addLauncher(
  w: WorkspaceView,
  entry: RegistryEntry,
  start: boolean,
  opts: AddOptions = {}
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
  if (opts.startDir) {
    await attempt(
      () => api.updateMember(w.id, memberId, { start_dir: opts.startDir ?? "" }),
      "Could not use that folder"
    );
  }
  await refreshWorkspaces();
  const updated = getState().workspaces.find((x) => x.id === w.id);
  const member = updated?.members.find((m) => m.id === memberId);
  if (!updated || !member) return;
  const focus = memberId;
  setState({ focused: focus });
  if (opts.beside) {
    // Quick split: keep the user's arrangement and add a pane beside the
    // target. Tabs/Focus become Grid, since two panes are now visible.
    const members = layoutMembers(updated);
    const base =
      reconcile(updated.layout.json, members) ??
      layoutFor(updated.layout.mode, members, opts.beside.target);
    const self = members.find((m) => m.id === memberId)!;
    const mode = ["tabs", "focus"].includes(updated.layout.mode) ? "grid" : updated.layout.mode;
    await attempt(() =>
      api.saveLayout(
        updated.id,
        mode,
        placeBeside(base, opts.beside!.target, self, opts.beside!.side),
        focus
      )
    );
  } else {
    // Re-generate the preset for the current mode so the new member shows up.
    await attempt(() =>
      api.saveLayout(
        updated.id,
        updated.layout.mode,
        layoutFor(updated.layout.mode, layoutMembers(updated), focus),
        focus
      )
    );
  }
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
        {hasNote(w.id, m.id) && " Its note is kept: it moves into the project note."}
      </p>
    ),
    confirm: "Remove",
    danger: true,
  });
  if (!ok) return;
  await flushNote(w.id, m.id);
  await attempt(() => api.removeMember(w.id, m.id), "Could not remove member");
  if (session) disposeController(session.session_id);
  await loadNotes(w.id);
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

export async function setMemberColor(
  workspaceId: string,
  memberId: string,
  color: ColorTag | null
): Promise<void> {
  await attempt(
    () => api.updateMember(workspaceId, memberId, { color: color ?? "" }),
    "Could not change the color"
  );
}

export async function setWorkspaceColor(workspaceId: string, color: ColorTag | null) {
  await attempt(() => api.setWorkspaceColor(workspaceId, color), "Could not change the color");
}

/** Save a terminal's scrollback into the Downloads folder (the core picks the path). */
export async function saveOutput(text: string, name: string): Promise<void> {
  if (!text.trim()) {
    notify("info", "This terminal has no output to save yet");
    return;
  }
  const path = await attempt(() => api.exportScrollback(name, text), "Could not save the output");
  if (path) notify("info", `Saved to ${path}`);
}

/** Ctrl+click on a terminal link. OSC 8 targets are hidden, so they are shown first. */
export async function openLink(url: string, hidden: boolean): Promise<void> {
  if (hidden) {
    const ok = await confirmDialog({
      title: "Open link?",
      body: (
        <>
          <p>A program in this terminal links to:</p>
          <pre className="paste-preview">{url}</pre>
        </>
      ),
      confirm: "Open in browser",
    });
    if (!ok) return;
  }
  await attempt(() => api.openUrl(url), "Could not open the link");
}

/** Folder a terminal is in now: its shell's report (OSC 7, or the OS on
 * Linux) for local terminals, else the member's start folder, else the
 * workspace project folder. */
export async function currentFolder(w: WorkspaceView, memberId: string): Promise<string | null> {
  const s = getState();
  const m = w.members.find((x) => x.id === memberId);
  const session = sessionFor(s, w.id, memberId);
  if (session && !session.remote) {
    const reported = s.cwd[session.session_id]?.path;
    if (reported) return reported;
    const fromOs = await api.sessionCwd(session.session_id).catch(() => null);
    if (fromOs) return fromOs;
  }
  return m?.start_dir ?? w.project_dir ?? null;
}

/** Quick split: a new terminal right of / below the focused one, in the
 * same folder. */
export async function splitTerminal(side: SplitSide, targetId?: string): Promise<void> {
  const s = getState();
  const w = activeWorkspace(s);
  if (!w) {
    notify("info", "Create a workspace first.");
    return;
  }
  const shell = defaultShell();
  if (!shell) {
    notify("error", "No shell was found on this machine.");
    return;
  }
  const target = targetId ?? s.focused;
  const valid = target && w.members.some((m) => m.id === target) ? target : null;
  const startDir = valid ? await currentFolder(w, valid) : w.project_dir;
  await addLauncher(w, shell, true, {
    startDir,
    beside: valid ? { target: valid, side } : undefined,
  });
}

/** Zoom the user asked for but the core has not confirmed yet: quick
 * repeated presses build on it even if a refresh lands in between. */
const desiredZoom = new Map<string, number>();

export function pendingZoom(memberId: string): number | undefined {
  return desiredZoom.get(memberId);
}

/** Change (or reset) the font size of the focused terminal. */
export async function zoomTerminal(delta: number | "reset"): Promise<void> {
  const s = getState();
  const w = activeWorkspace(s);
  const m = w?.members.find((x) => x.id === s.focused);
  if (!w || !m) return;
  const current = desiredZoom.get(m.id) ?? m.zoom;
  const zoom = delta === "reset" ? 0 : Math.max(-6, Math.min(12, current + delta));
  if (zoom === current) return;
  desiredZoom.set(m.id, zoom);
  // Show it at once (and let the next key press build on it); the core's
  // refresh confirms or corrects it.
  setState((st) => ({
    workspaces: st.workspaces.map((x) =>
      x.id !== w.id
        ? x
        : { ...x, members: x.members.map((y) => (y.id === m.id ? { ...y, zoom } : y)) }
    ),
  }));
  const ok = await attempt(
    () => api.updateMember(w.id, m.id, { zoom }),
    "Could not change the size"
  );
  if (desiredZoom.get(m.id) === zoom) desiredZoom.delete(m.id);
  if (ok === undefined) await refreshWorkspaces();
}

/** Typing in one terminal goes to every running terminal of the workspace. */
export function toggleBroadcast(): void {
  const s = getState();
  const w = activeWorkspace(s);
  if (!w) return;
  const on = !s.broadcast[w.id];
  setState({ broadcast: { ...s.broadcast, [w.id]: on } });
  const running = Object.values(s.sessions).filter(
    (x) => x.workspace_id === w.id && x.process_state.kind === "running"
  ).length;
  notify(
    on ? "warning" : "info",
    on
      ? `Broadcast on: typing goes to all ${running} running terminals in ${w.name}`
      : "Broadcast off"
  );
}

export async function setProjectFolder(w: WorkspaceView): Promise<void> {
  const dir = await promptDialog({
    title: "Project folder",
    label: "New terminals in this workspace start in this folder (leave empty for your home)",
    initial: w.project_dir ?? "",
    confirm: "Save",
    validate: (v) =>
      v.trim() === "" || /^(\/|[A-Za-z]:[\\/])/.test(v.trim())
        ? null
        : "Use a full path, for example /home/me/project",
  });
  if (dir === undefined) return;
  const ok = await attempt(
    () => api.setProjectDir(w.id, dir.trim() || null),
    "Could not set the folder"
  );
  if (ok !== undefined)
    notify("info", dir.trim() ? `Project folder: ${dir.trim()}` : "Project folder cleared");
}
