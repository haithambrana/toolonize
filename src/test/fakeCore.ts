/**
 * In-memory stand-in for the Rust core behind `@tauri-apps/api/core`, so
 * component tests exercise the real IPC client and store. It records every
 * invoke and implements just enough behavior for UI flows.
 */
import { vi } from "vitest";
import type { AttachInfo, RegistryEntry, SessionInfo, Settings, WorkspaceView } from "../lib/ipc";

export class FakeChannel<T = unknown> {
  onmessage: (message: T) => void = () => undefined;
  send(message: T) {
    this.onmessage(message);
  }
}

export interface Call {
  cmd: string;
  args: Record<string, unknown> | undefined;
}

export const calls: Call[] = [];
export const handlers = new Map<string, (args: Record<string, unknown>) => unknown>();

export const invoke = vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
  calls.push({ cmd, args });
  const handler = handlers.get(cmd);
  if (!handler) throw new Error(`fake core: unhandled command ${cmd}`);
  return handler(args ?? {});
});

export const settings: Settings = {
  theme: "dark",
  motion: "system",
  font_size: 13,
  scrollback_lines: 10000,
  replay_buffer_mib: 4,
  paste_warning: true,
  default_shell: null,
  keymap: {},
  live_updates: true,
  confirm_close_running: false,
  copy_on_select: false,
  osc52_clipboard: true,
  ssh_auto_reconnect: true,
};

export function entry(partial: Partial<RegistryEntry> & { launcher_id: string }): RegistryEntry {
  return {
    display_name: partial.launcher_id,
    platform: "linux",
    origin: "xdg_system",
    origin_label: "System applications",
    origin_root: "/fixture/applications",
    relative_path: `${partial.launcher_id}.desktop`,
    descriptor: {
      kind: "exec_template",
      program: "example-tool",
      args: [],
      raw_args: null,
      exec_template: "example-tool",
      working_dir: null,
      terminal: true,
      try_exec: null,
      icon: null,
      path_resolved: true,
      source_path: `/fixture/applications/${partial.launcher_id}.desktop`,
    },
    fingerprint: `fp-${partial.launcher_id}`,
    parse_status: "ok",
    issues: [],
    classification: {
      proposed: "local_command",
      confidence: "high",
      evidence: ["Terminal=true"],
      interpreter: false,
      needs_review: false,
      blocked: false,
    },
    no_display: false,
    not_shown_here: false,
    comment: null,
    decision: { hidden: false, alias: null, approved_fingerprint: null, approved_at: null },
    change: "unchanged",
    effective_type: "local_command",
    review_pending: false,
    executable: true,
    ...partial,
  };
}

export function session(
  partial: Partial<SessionInfo> & { session_id: string; member_id: string }
): SessionInfo {
  return {
    workspace_id: "ws1",
    title: partial.member_id,
    process_state: { kind: "running" },
    view_state: "detached",
    generation: 1,
    pid: 100,
    remote: false,
    detach_expected: false,
    rows: 24,
    cols: 80,
    base_offset: 0,
    end_offset: 0,
    attached: false,
    backend: "portable-pty-0.9.0",
    started_at_ms: 1,
    ...partial,
  };
}

export const core = {
  workspaces: [] as WorkspaceView[],
  active: null as string | null,
  entries: [] as RegistryEntry[],
  sessions: [] as SessionInfo[],
  events: null as FakeChannel | null,
  attachments: new Map<string, { output: FakeChannel; events: FakeChannel; id: number }>(),
};

let nextAttachment = 1;

export function resetFakeCore() {
  calls.length = 0;
  handlers.clear();
  core.workspaces = [];
  core.active = null;
  core.entries = [];
  core.sessions = [];
  core.events = null;
  core.attachments.clear();
  const ok = () => undefined;
  handlers.set("events_subscribe", (a) => {
    core.events = a.events as FakeChannel;
  });
  handlers.set("app_startup_info", () => ({
    load_report: {
      generation: 0,
      recovered: false,
      problems: [],
      restored_saved_at: null,
      migrated_from: null,
    },
    platform: "linux",
    version: "0.1.0",
    backend: "portable-pty-0.9.0",
    state_dir: "/fixture/state",
    config_dir: "/fixture/config",
  }));
  handlers.set("settings_get", () => settings);
  handlers.set("settings_update", ok);
  handlers.set("workspace_list", () => ({ workspaces: core.workspaces, active: core.active }));
  handlers.set("registry_view", () => ({
    entries: core.entries,
    reports: [],
    last_scan_ms: 1,
    custom_roots: [],
    synthetic: [],
    removed: [],
    watch: { state: "live", roots: 1 },
  }));
  handlers.set("session_list", () => core.sessions);
  handlers.set("workspace_set_active", (a) => {
    core.active = a.workspaceId as string;
  });
  handlers.set("workspace_open", () => ({ needs_choice: false, outcomes: [] }));
  handlers.set("workspace_save_layout", ok);
  handlers.set("session_view", (a) => core.sessions.find((s) => s.session_id === a.sessionId));
  handlers.set("session_resize", (a) => core.sessions.find((s) => s.session_id === a.sessionId));
  handlers.set("session_ack", ok);
  handlers.set("session_write", ok);
  handlers.set("session_detach", ok);
  handlers.set("session_attach", (a): AttachInfo => {
    const id = nextAttachment++;
    core.attachments.set(a.sessionId as string, {
      output: a.output as FakeChannel,
      events: a.events as FakeChannel,
      id,
    });
    const info = core.sessions.find((s) => s.session_id === a.sessionId)!;
    return {
      attachment_id: id,
      start_offset: a.fromOffset as number,
      end_offset: info?.end_offset ?? 0,
      trimmed: false,
      info,
    };
  });
  handlers.set("audit_recent", () => []);
}

/** Build an output frame like the Rust pump does. */
export function frame(offset: number, text: string): ArrayBuffer {
  const bytes = new TextEncoder().encode(text);
  const out = new Uint8Array(8 + bytes.length);
  new DataView(out.buffer).setUint32(0, offset % 0x1_0000_0000, true);
  new DataView(out.buffer).setUint32(4, Math.floor(offset / 0x1_0000_0000), true);
  out.set(bytes, 8);
  return out.buffer;
}

export function commandsCalled(): string[] {
  return calls.map((c) => c.cmd);
}
