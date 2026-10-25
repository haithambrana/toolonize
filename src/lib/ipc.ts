/**
 * Typed client for the ToolOnize IPC surface (src-tauri/src/commands).
 *
 * The frontend is untrusted by design: it can name workspaces, members and
 * launchers, send terminal bytes, and save layouts. It cannot pass a program,
 * argument list or command line anywhere; the Rust execution policy builds
 * every process.
 */
import { Channel, invoke } from "@tauri-apps/api/core";

export interface PingResponse {
  app_name: string;
  app_version: string;
  target_os: string;
  target_arch: string;
  status: string;
}

export type ProcessState =
  | { kind: "new" }
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "stopping" }
  | { kind: "closed" }
  | { kind: "exited"; code: number; signal: string | null }
  | { kind: "failed"; reason: string }
  | { kind: "restarting" }
  | { kind: "disconnected"; code: number }
  | { kind: "reconnecting" };

export type ViewState = "detached" | "attached" | "hidden";
export type ViewEvent = "attach" | "detach" | "hide" | "show";

export interface SessionInfo {
  session_id: string;
  workspace_id: string;
  member_id: string;
  title: string;
  process_state: ProcessState;
  view_state: ViewState;
  generation: number;
  pid: number | null;
  remote: boolean;
  detach_expected: boolean;
  rows: number;
  cols: number;
  base_offset: number;
  end_offset: number;
  attached: boolean;
  backend: string;
  started_at_ms: number;
}

export interface AttachInfo {
  attachment_id: number;
  start_offset: number;
  end_offset: number;
  trimmed: boolean;
  info: SessionInfo;
}

export type SessionEvent =
  { type: "state"; info: SessionInfo } | { type: "desync"; reason: string; end_offset: number };

export type LauncherType =
  "embedded_terminal" | "local_command" | "remote_ssh" | "external_gui" | "unknown";

export type OriginKind =
  | "xdg_user"
  | "xdg_system"
  | "flatpak"
  | "snap"
  | "desktop_dir"
  | "custom_root"
  | "programs_user"
  | "programs_common"
  | "desktop_user"
  | "desktop_public"
  | "synthetic";

export interface Descriptor {
  kind: "exec_template" | "shell_link" | "dbus_activatable" | "synthetic_system";
  program: string | null;
  args: string[];
  raw_args: string | null;
  exec_template: string | null;
  working_dir: string | null;
  terminal: boolean;
  try_exec: string | null;
  icon: string | null;
  path_resolved: boolean;
  source_path: string;
}

export interface Classification {
  proposed: LauncherType;
  confidence: "high" | "medium" | "low";
  evidence: string[];
  interpreter: boolean;
  needs_review: boolean;
  blocked: boolean;
}

export interface Decision {
  hidden: boolean;
  alias: string | null;
  approved_fingerprint: string | null;
  approved_at: number | null;
}

export interface RegistryEntry {
  launcher_id: string;
  display_name: string;
  platform: "linux" | "windows";
  origin: OriginKind;
  origin_label: string;
  origin_root: string;
  relative_path: string;
  descriptor: Descriptor;
  fingerprint: string;
  parse_status: "ok" | "needs_review" | "unavailable";
  issues: string[];
  classification: Classification;
  no_display: boolean;
  not_shown_here: boolean;
  comment: string | null;
  decision: Decision;
  change: "new" | "changed" | "unchanged";
  effective_type: LauncherType;
  review_pending: boolean;
  executable: boolean;
}

export interface SourceReport {
  origin: OriginKind;
  root: string;
  entries: number;
  truncated: boolean;
  errors: string[];
}

export type WatchStatus =
  { state: "live"; roots: number } | { state: "degraded"; reason: string } | { state: "off" };

export type SyntheticSpec =
  | { kind: "ssh"; host: string }
  | { kind: "ssh_tmux"; host: string; session: string }
  | { kind: "tmux"; session: string };

export interface RegistryView {
  entries: RegistryEntry[];
  reports: SourceReport[];
  last_scan_ms: number;
  custom_roots: string[];
  synthetic: SyntheticSpec[];
  removed: { launcher_id: string; display_name: string; removed_at: number }[];
  watch: WatchStatus;
}

export interface Diff {
  added: string[];
  changed: string[];
  removed: string[];
}

export type StartupPolicy = "ask" | "restore_only" | "restore_terminals" | "restore_all";
export type LayoutMode = "grid" | "focus" | "tabs" | "master_stack";
export type MemberType = "terminal" | "local_command" | "remote_command" | "external_app";
export type MemberState =
  "ready" | "changed" | "not_authorized" | "orphaned" | "blocked" | "unavailable";

export interface MemberView {
  id: string;
  launcher_id: string;
  member_type: MemberType;
  title: string | null;
  authorized_fingerprint: string | null;
  authorized_at: number | null;
  auto_restart: boolean;
  detach_expected: boolean;
  added_at: number;
  portable: { kind: string; value: string } | null;
  status: {
    state: MemberState;
    reason: string | null;
    launcher_name: string | null;
    current_fingerprint: string | null;
  };
  effective_type: LauncherType | null;
  origin_label: string | null;
  interpreter: boolean;
  review_pending: boolean;
}

export interface LayoutState {
  mode: LayoutMode;
  json: unknown;
  focused_member: string | null;
}

export interface WorkspaceView {
  id: string;
  name: string;
  icon: string | null;
  created_at: number;
  updated_at: number;
  startup_policy: StartupPolicy;
  layout: LayoutState;
  staged_import: boolean;
  members: MemberView[];
}

export interface OpenResult {
  needs_choice: boolean;
  outcomes: { member_id: string; outcome: string; reason: string | null }[];
}

export type Theme = "system" | "dark" | "light" | "high_contrast";

export interface Settings {
  theme: Theme;
  motion: "system" | "reduced" | "full";
  font_size: number;
  scrollback_lines: number;
  replay_buffer_mib: number;
  paste_warning: boolean;
  default_shell: string | null;
  keymap: Record<string, string>;
  live_updates: boolean;
  confirm_close_running: boolean;
  copy_on_select: boolean;
  osc52_clipboard: boolean;
}

export interface StartupInfo {
  load_report: {
    generation: number;
    recovered: boolean;
    problems: string[];
    restored_saved_at: number | null;
    migrated_from: number | null;
  };
  platform: string;
  version: string;
  backend: string;
  state_dir: string;
  config_dir: string;
}

export interface AuditEntry {
  at: number;
  action: string;
  outcome: "allowed" | "denied" | "failed" | "recorded";
  workspace_id: string | null;
  member_id: string | null;
  launcher_id: string | null;
  fingerprint: string | null;
  detail: string | null;
}

export interface ImportReport {
  workspace_id: string;
  members: number;
  unresolved: string[];
  warnings: string[];
}

export type AppEvent =
  | { type: "session"; info: SessionInfo }
  | { type: "session_closed"; session_id: string }
  | { type: "registry"; diff: Diff }
  | { type: "workspaces" }
  | { type: "settings" }
  | { type: "watch"; status: WatchStatus }
  | { type: "notice"; level: string; message: string };

export function ping(): Promise<PingResponse> {
  return invoke<PingResponse>("ping");
}

export const api = {
  startupInfo: () => invoke<StartupInfo>("app_startup_info"),
  subscribe: (onEvent: (event: AppEvent) => void) => {
    const events = new Channel<AppEvent>();
    events.onmessage = onEvent;
    return invoke<void>("events_subscribe", { events });
  },
  settings: () => invoke<Settings>("settings_get"),
  updateSettings: (settings: Settings) => invoke<void>("settings_update", { settings }),
  audit: (limit: number) => invoke<AuditEntry[]>("audit_recent", { limit }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  clipboardRead: () => invoke<string | null>("clipboard_read_text"),

  registry: () => invoke<RegistryView>("registry_view"),
  rescan: () => invoke<Diff>("registry_rescan"),
  setHidden: (launcherId: string, hidden: boolean) =>
    invoke<void>("registry_set_hidden", { launcherId, hidden }),
  setAlias: (launcherId: string, alias: string | null) =>
    invoke<void>("registry_set_alias", { launcherId, alias }),
  approve: (launcherId: string, fingerprint: string) =>
    invoke<void>("registry_approve", { launcherId, fingerprint }),
  revoke: (launcherId: string) => invoke<void>("registry_revoke", { launcherId }),
  acknowledge: (launcherIds: string[]) => invoke<void>("registry_acknowledge", { launcherIds }),
  addCustomRoot: (root: string) => invoke<void>("custom_root_add", { root }),
  removeCustomRoot: (root: string) => invoke<void>("custom_root_remove", { root }),
  addSynthetic: (spec: SyntheticSpec) => invoke<string>("synthetic_add", { spec }),
  removeSynthetic: (launcherId: string) => invoke<void>("synthetic_remove", { launcherId }),

  workspaces: () =>
    invoke<{ workspaces: WorkspaceView[]; active: string | null }>("workspace_list"),
  createWorkspace: (name: string) => invoke<string>("workspace_create", { name }),
  renameWorkspace: (workspaceId: string, name: string) =>
    invoke<void>("workspace_rename", { workspaceId, name }),
  deleteWorkspace: (workspaceId: string) => invoke<void>("workspace_delete", { workspaceId }),
  setActive: (workspaceId: string | null) => invoke<void>("workspace_set_active", { workspaceId }),
  setPolicy: (workspaceId: string, policy: StartupPolicy) =>
    invoke<void>("workspace_set_policy", { workspaceId, policy }),
  acceptImport: (workspaceId: string) => invoke<void>("workspace_accept_import", { workspaceId }),
  saveLayout: (
    workspaceId: string,
    mode: LayoutMode,
    layout: unknown,
    focusedMember: string | null
  ) => invoke<void>("workspace_save_layout", { workspaceId, mode, layout, focusedMember }),
  openWorkspace: (
    workspaceId: string,
    choice: { start_terminals: boolean; launch_external: boolean } | null
  ) => invoke<OpenResult>("workspace_open", { workspaceId, choice }),
  exportWorkspace: (workspaceId: string, machineBound: boolean) =>
    invoke<string>("workspace_export", { workspaceId, machineBound }),
  importWorkspace: (bundle: string) => invoke<ImportReport>("workspace_import", { bundle }),

  addMember: (workspaceId: string, launcherId: string, fingerprint: string, title: string | null) =>
    invoke<string>("member_add", { workspaceId, launcherId, fingerprint, title }),
  removeMember: (workspaceId: string, memberId: string) =>
    invoke<void>("member_remove", { workspaceId, memberId }),
  updateMember: (
    workspaceId: string,
    memberId: string,
    patch: { title?: string | null; auto_restart?: boolean; detach_expected?: boolean }
  ) => invoke<void>("member_update", { workspaceId, memberId, patch }),
  authorizeMember: (workspaceId: string, memberId: string, fingerprint: string) =>
    invoke<void>("member_authorize", { workspaceId, memberId, fingerprint }),
  launchMember: (workspaceId: string, memberId: string) =>
    invoke<number>("member_launch", { workspaceId, memberId }),

  startSession: (workspaceId: string, memberId: string, rows: number, cols: number) =>
    invoke<SessionInfo>("session_start", { workspaceId, memberId, rows, cols }),
  write: (sessionId: string, data: Uint8Array) =>
    invoke<void>("session_write", { sessionId, data: Array.from(data) }),
  resize: (sessionId: string, rows: number, cols: number) =>
    invoke<SessionInfo>("session_resize", { sessionId, rows, cols }),
  attach: (
    sessionId: string,
    fromOffset: number,
    output: Channel<ArrayBuffer>,
    events: Channel<SessionEvent>
  ) => invoke<AttachInfo>("session_attach", { sessionId, fromOffset, output, events }),
  detach: (sessionId: string, attachmentId: number) =>
    invoke<SessionInfo>("session_detach", { sessionId, attachmentId }),
  ack: (sessionId: string, attachmentId: number, offset: number) =>
    invoke<void>("session_ack", { sessionId, attachmentId, offset }),
  view: (sessionId: string, event: ViewEvent) =>
    invoke<SessionInfo>("session_view", { sessionId, event }),
  stop: (sessionId: string) => invoke<SessionInfo>("session_stop", { sessionId }),
  restart: (sessionId: string) => invoke<SessionInfo>("session_restart", { sessionId }),
  reconnect: (sessionId: string) => invoke<SessionInfo>("session_reconnect", { sessionId }),
  close: (sessionId: string) => invoke<void>("session_close", { sessionId }),
  sessions: () => invoke<SessionInfo[]>("session_list"),
};

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return JSON.stringify(error);
}

export function isProcessLive(state: ProcessState): boolean {
  return ["starting", "running", "stopping", "restarting", "reconnecting"].includes(state.kind);
}
