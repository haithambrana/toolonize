/**
 * UI store: a cache/projection of Rust-owned state (DOMAIN_MODEL §5).
 * Every mutation goes through an IPC command; events from the core refresh
 * the cache.
 */
import { useSyncExternalStore } from "react";
import {
  api,
  errorMessage,
  type AppEvent,
  type NotesView,
  type RegistryView,
  type ResumeReport,
  type SessionInfo,
  type Settings,
  type StartupInfo,
  type WatchStatus,
  type WorkspaceView,
} from "../lib/ipc";

export type View = "workspace" | "launchers" | "settings" | "activity";

export interface Notice {
  id: number;
  level: "info" | "warning" | "error";
  message: string;
}

export interface AppState {
  ready: boolean;
  fatal: string | null;
  startup: StartupInfo | null;
  settings: Settings | null;
  workspaces: WorkspaceView[];
  active: string | null;
  registry: RegistryView | null;
  sessions: Record<string, SessionInfo>;
  watch: WatchStatus;
  notices: Notice[];
  view: View;
  paletteOpen: boolean;
  /** Member that last had focus in the active workspace. */
  focused: string | null;
  /** Last successful external launch per member (ms). */
  launched: Record<string, number>;
  /** Workspaces whose Ask startup policy is waiting for a choice. */
  pendingChoice: Record<string, boolean>;
  /** Workspaces whose startup policy already ran in this app session. */
  opened: Record<string, boolean>;
  /** Session whose scrollback search bar is open. */
  searchOpen: string | null;
  resolvedTheme: "dark" | "light" | "high_contrast";
  /** Open terminal context menu. */
  termMenu: { sessionId: string; memberId: string; x: number; y: number } | null;
  /** SSH auto-reconnect in progress, per session (`at`: next attempt, ms). */
  reconnecting: Record<string, { attempt: number; max: number; at: number }>;
  /** "Search all terminals" dialog. */
  globalSearchOpen: boolean;
  /** Sticky notes per workspace (ADR-012), as last saved or being typed. */
  notes: Record<string, NotesView>;
  /** Save state per note key (`<workspace>:<member>` or `<workspace>:*`). */
  noteStatus: Record<string, "saved" | "saving" | "error">;
  /** Members whose note panel is open. */
  noteOpen: Record<string, boolean>;
  projectNoteOpen: boolean;
  /** Folder last reported by each session's shell (OSC 7). */
  cwd: Record<string, { path: string; host: string }>;
  /** Workspaces whose terminals currently receive the same typing. */
  broadcast: Record<string, boolean>;
  /** Presentation mode: no sidebar or toolbar, larger terminal text. */
  presentation: boolean;
}

const initial: AppState = {
  ready: false,
  fatal: null,
  startup: null,
  settings: null,
  workspaces: [],
  active: null,
  registry: null,
  sessions: {},
  watch: { state: "off" },
  notices: [],
  view: "workspace",
  paletteOpen: false,
  focused: null,
  launched: {},
  pendingChoice: {},
  opened: {},
  searchOpen: null,
  resolvedTheme: "dark",
  termMenu: null,
  reconnecting: {},
  globalSearchOpen: false,
  notes: {},
  noteStatus: {},
  noteOpen: {},
  projectNoteOpen: false,
  cwd: {},
  broadcast: {},
  presentation: false,
};

let state: AppState = initial;
const listeners = new Set<() => void>();
let noticeId = 0;

export function getState(): AppState {
  return state;
}

export function setState(patch: Partial<AppState> | ((s: AppState) => Partial<AppState>)): void {
  const next = typeof patch === "function" ? patch(state) : patch;
  state = { ...state, ...next };
  for (const l of listeners) l();
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useAppState<T>(select: (s: AppState) => T): T {
  return useSyncExternalStore(subscribe, () => select(state));
}

/** Show a toast. The same message already on screen is not shown twice. */
export function notify(level: Notice["level"], message: string, ms?: number): void {
  if (getState().notices.some((n) => n.level === level && n.message === message)) return;
  const id = ++noticeId;
  setState((s) => ({ notices: [...s.notices.slice(-4), { id, level, message }] }));
  setTimeout(() => dismissNotice(id), ms ?? (level === "error" ? 9000 : 5000));
}

export function dismissNotice(id: number): void {
  setState((s) => ({ notices: s.notices.filter((n) => n.id !== id) }));
}

/** Run an IPC action and surface failures as a notice instead of throwing. */
export async function attempt<T>(
  action: () => Promise<T>,
  failure?: string
): Promise<T | undefined> {
  try {
    return await action();
  } catch (error) {
    notify("error", failure ? `${failure}: ${errorMessage(error)}` : errorMessage(error));
    return undefined;
  }
}

export async function refreshWorkspaces(): Promise<void> {
  const { workspaces, active } = await api.workspaces();
  setState({ workspaces, active });
}

export async function refreshRegistry(): Promise<void> {
  const registry = await api.registry();
  setState({ registry, watch: registry.watch });
}

export async function refreshSessions(): Promise<void> {
  const list = await api.sessions();
  setState({ sessions: Object.fromEntries(list.map((s) => [s.session_id, s])) });
}

export async function refreshSettings(): Promise<void> {
  setState({ settings: await api.settings() });
}

let registryTimer: ReturnType<typeof setTimeout> | undefined;
let workspaceTimer: ReturnType<typeof setTimeout> | undefined;

type SessionListener = (before: SessionInfo | undefined, after: SessionInfo) => void;
const sessionListeners = new Set<SessionListener>();

/** Observe session state changes (used for desktop notifications). */
export function onSessionChange(listener: SessionListener): () => void {
  sessionListeners.add(listener);
  return () => sessionListeners.delete(listener);
}

export function handleEvent(event: AppEvent): void {
  switch (event.type) {
    case "session": {
      const before = state.sessions[event.info.session_id];
      setState((s) => ({ sessions: { ...s.sessions, [event.info.session_id]: event.info } }));
      for (const l of sessionListeners) l(before, event.info);
      break;
    }
    case "session_closed":
      setState((s) => {
        const sessions = { ...s.sessions };
        delete sessions[event.session_id];
        const reconnecting = { ...s.reconnecting };
        delete reconnecting[event.session_id];
        return { sessions, reconnecting };
      });
      break;
    case "reconnect":
      setState((s) => {
        const reconnecting = { ...s.reconnecting };
        if (event.active) {
          reconnecting[event.session_id] = {
            attempt: event.attempt,
            max: event.max_attempts,
            at: Date.now() + event.delay_ms,
          };
        } else {
          delete reconnecting[event.session_id];
        }
        return { reconnecting };
      });
      break;
    case "registry":
      clearTimeout(registryTimer);
      registryTimer = setTimeout(() => void refreshRegistry().catch(() => undefined), 50);
      if (event.diff.added.length + event.diff.changed.length + event.diff.removed.length > 0) {
        notify(
          "info",
          `Launchers updated: ${event.diff.added.length} new, ${event.diff.changed.length} changed, ${event.diff.removed.length} removed`
        );
      }
      break;
    case "workspaces":
      clearTimeout(workspaceTimer);
      workspaceTimer = setTimeout(() => void refreshWorkspaces().catch(() => undefined), 30);
      break;
    case "settings":
      void refreshSettings().catch(() => undefined);
      break;
    case "watch":
      setState({ watch: event.status });
      break;
    case "notice":
      notify(
        event.level === "warning" ? "warning" : event.level === "error" ? "error" : "info",
        event.message
      );
      break;
  }
}

export async function init(): Promise<void> {
  try {
    await api.subscribe(handleEvent);
    const [startup, settings] = await Promise.all([api.startupInfo(), api.settings()]);
    setState({ startup, settings });
    await Promise.all([refreshWorkspaces(), refreshRegistry(), refreshSessions()]);
    setState({ ready: true });
    if (startup.load_report.recovered) {
      notify(
        "warning",
        `State recovered from an earlier snapshot. ${startup.load_report.problems.join(" ")}`
      );
    }
    for (const [level, message] of resumeMessages(startup.resume)) notify(level, message, 12000);
  } catch (error) {
    setState({ fatal: errorMessage(error), ready: true });
  }
}

/** Startup notices for what session memory restored (ADR-015). */
export function resumeMessages(r: ResumeReport | null): [Notice["level"], string][] {
  if (!r) return [];
  const out: [Notice["level"], string][] = [];
  if (r.restored > 0) {
    const what = `${r.restored} terminal${r.restored === 1 ? "" : "s"}`;
    out.push(
      r.crashed
        ? [
            "warning",
            `ToolOnize did not close normally last time. Restored ${what} with their last output.`,
          ]
        : ["info", `Restored ${what} from your last session.`]
    );
  } else if (r.crashed && r.left_stopped.length === 0 && r.failed.length === 0) {
    out.push([
      "info",
      "ToolOnize did not close normally last time. Your workspaces and notes are safe.",
    ]);
  }
  if (r.left_stopped.length) {
    out.push([
      "info",
      `Not started again automatically, because commands can have side effects: ${r.left_stopped.join(", ")}. Start them when you need them.`,
    ]);
  }
  if (r.failed.length) {
    out.push(["warning", `Could not restore: ${r.failed.join("; ")}.`]);
  }
  return out;
}

export function activeWorkspace(s: AppState): WorkspaceView | undefined {
  return s.workspaces.find((w) => w.id === s.active) ?? s.workspaces[0];
}

export function sessionFor(
  s: AppState,
  workspaceId: string,
  memberId: string
): SessionInfo | undefined {
  return Object.values(s.sessions).find(
    (x) => x.workspace_id === workspaceId && x.member_id === memberId
  );
}

/** Test helper. */
export function resetStore(): void {
  state = initial;
  for (const l of listeners) l();
}
