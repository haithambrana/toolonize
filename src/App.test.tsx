import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import App from "./App";
import type { MemberView, WorkspaceView } from "./lib/ipc";
import { layoutFor } from "./layout/modes";
import { resetDialogs } from "./lib/dialogs";
import { resetStore } from "./state/store";
import { resetControllers } from "./terminal/controller";
import {
  calls,
  commandsCalled,
  core,
  entry,
  handlers,
  resetFakeCore,
  session,
} from "./test/fakeCore";
import { terminals } from "./test/fakeXterm";

function member(id: string, partial: Partial<MemberView> = {}): MemberView {
  return {
    id,
    launcher_id: `sys:shell:${id}`,
    member_type: "terminal",
    title: `Shell ${id}`,
    authorized_fingerprint: "fp",
    authorized_at: 1,
    auto_restart: false,
    detach_expected: false,
    added_at: 1,
    portable: null,
    color: null,
    status: { state: "ready", reason: null, launcher_name: id, current_fingerprint: "fp" },
    effective_type: "embedded_terminal",
    origin_label: "Built-in shell",
    interpreter: false,
    review_pending: false,
    ...partial,
  };
}

function workspace(members: MemberView[], partial: Partial<WorkspaceView> = {}): WorkspaceView {
  return {
    id: "ws1",
    name: "Project Phoenix",
    icon: null,
    created_at: 1,
    updated_at: 1,
    startup_policy: "ask",
    layout: {
      mode: "grid",
      json: layoutFor(
        "grid",
        members.map((m) => ({ id: m.id, title: m.title! }))
      ),
      focused_member: null,
    },
    staged_import: false,
    color: null,
    members,
    ...partial,
  };
}

async function emit(event: unknown) {
  await act(async () => {
    core.events!.send(event);
    await new Promise((r) => setTimeout(r, 60));
  });
}

describe("ToolOnize app", () => {
  beforeEach(() => {
    resetFakeCore();
    resetStore();
    resetDialogs();
    resetControllers();
    terminals.length = 0;
  });

  it("starts, subscribes to core events, and offers to create a workspace", async () => {
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: /One persistent workspace/ })
    ).toBeInTheDocument();
    expect(commandsCalled()).toContain("events_subscribe");
    expect(screen.getByText("● Live updates")).toBeInTheDocument();
  });

  it("creates a workspace through the dialog", async () => {
    const user = userEvent.setup();
    handlers.set("workspace_create", (a) => {
      core.workspaces = [workspace([], { id: "ws-new", name: a.name as string })];
      core.active = "ws-new";
      return "ws-new";
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Create a workspace" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "Project Phoenix");
    await user.click(within(dialog).getByRole("button", { name: "Create" }));
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "workspace_create")?.args).toEqual({
        name: "Project Phoenix",
      })
    );
    await emit({ type: "workspaces" });
    expect(await screen.findByRole("heading", { name: "Project Phoenix" })).toBeInTheDocument();
  });

  it("asks before starting anything when the policy is Ask (FR-024)", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    handlers.set("workspace_open", (a) =>
      a.choice ? { needs_choice: false, outcomes: [] } : { needs_choice: true, outcomes: [] }
    );
    const user = userEvent.setup();
    render(<App />);
    const banner = await screen.findByRole("region", { name: "Startup" });
    expect(commandsCalled()).not.toContain("session_start");
    await user.click(within(banner).getByRole("button", { name: "Start terminals" }));
    const opens = calls.filter((c) => c.cmd === "workspace_open");
    expect(opens.at(-1)?.args?.choice).toEqual({ start_terminals: true, launch_external: false });
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Startup" })).not.toBeInTheDocument()
    );
  });

  it("mode switches never recreate terminals or touch sessions (FR-031, T-LAYOUT-003/004)", async () => {
    const members = [member("a"), member("b")];
    core.workspaces = [workspace(members)];
    core.active = "ws1";
    core.sessions = [
      session({ session_id: "s-a", member_id: "a" }),
      session({ session_id: "s-b", member_id: "b" }),
    ];
    handlers.set("workspace_save_layout", (a) => {
      core.workspaces = [
        {
          ...core.workspaces[0],
          layout: {
            mode: a.mode as "tabs",
            json: a.layout,
            focused_member: (a.focusedMember as string) ?? null,
          },
        },
      ];
    });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Project Phoenix" });
    await waitFor(() => expect(terminals).toHaveLength(2));
    const instances = [...terminals];
    for (const mode of ["Tabs", "Master + Stack", "Focus", "Grid"]) {
      await user.click(screen.getByRole("radio", { name: mode }));
      await emit({ type: "workspaces" });
    }
    expect(terminals).toHaveLength(2);
    expect(terminals).toEqual(instances);
    for (const t of instances) expect(t.dispose).not.toHaveBeenCalled();
    const forbidden = [
      "session_start",
      "session_stop",
      "session_restart",
      "session_close",
      "member_launch",
    ];
    expect(commandsCalled().filter((c) => forbidden.includes(c))).toEqual([]);
    expect(calls.filter((c) => c.cmd === "workspace_save_layout").map((c) => c.args?.mode)).toEqual(
      ["tabs", "master_stack", "focus", "grid"]
    );
  });

  it("shows the exit banner and restarts only on request (FR-035, T-PTY-010)", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    handlers.set("session_restart", () =>
      session({ session_id: "s-a", member_id: "a", generation: 2 })
    );
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Running");
    await emit({
      type: "session",
      info: session({
        session_id: "s-a",
        member_id: "a",
        process_state: { kind: "exited", code: 2, signal: null },
      }),
    });
    expect((await screen.findAllByText("Exited with code 2")).length).toBeGreaterThan(0);
    expect(commandsCalled()).not.toContain("session_restart");
    await user.click(screen.getByRole("button", { name: "Restart" }));
    expect(commandsCalled()).toContain("session_restart");
  });

  it("remote members reconnect with honest SSH semantics (FR-036)", async () => {
    core.workspaces = [workspace([member("r", { member_type: "remote_command" })])];
    core.active = "ws1";
    core.sessions = [
      session({
        session_id: "s-r",
        member_id: "r",
        remote: true,
        process_state: { kind: "disconnected", code: 255 },
      }),
    ];
    handlers.set("session_reconnect", () => session({ session_id: "s-r", member_id: "r" }));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Reconnect" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Plain SSH starts a NEW remote shell");
    await user.click(within(dialog).getByRole("button", { name: "Reconnect" }));
    expect(commandsCalled()).toContain("session_reconnect");
  });

  it("Launchers view: review-required items offer approval but no run control (T-POLICY-005)", async () => {
    core.entries = [
      entry({
        launcher_id: "linux:desktop:Org.Example.Hostile.desktop",
        display_name: "Totally Safe Tool",
        origin: "desktop_dir",
        origin_label: "Desktop folder",
        effective_type: "unknown",
        executable: false,
        review_pending: true,
        classification: {
          proposed: "local_command",
          confidence: "medium",
          evidence: ["explicit interpreter execution (sh -c)"],
          interpreter: true,
          needs_review: true,
          blocked: false,
        },
      }),
    ];
    handlers.set("registry_approve", () => undefined);
    handlers.set("registry_acknowledge", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /Launchers/ }));
    const detail = await screen.findByRole("complementary", { name: /Totally Safe Tool/ });
    expect(
      within(detail).getByText(/runs code through a shell or interpreter/)
    ).toBeInTheDocument();
    for (const name of [/^run$/i, /^launch$/i, /^start$/i, /Add to workspace/]) {
      expect(within(detail).queryByRole("button", { name })).not.toBeInTheDocument();
    }
    await user.click(within(detail).getByRole("button", { name: "Approve this version" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Approve" }));
    expect(calls.find((c) => c.cmd === "registry_approve")?.args).toEqual({
      launcherId: "linux:desktop:Org.Example.Hostile.desktop",
      fingerprint: "fp-linux:desktop:Org.Example.Hostile.desktop",
    });
  });

  it("changed members explain re-review instead of starting (FR-015)", async () => {
    core.workspaces = [
      workspace([
        member("c", {
          status: {
            state: "changed",
            reason: "the launcher's command changed since you authorized it: re-review required",
            launcher_name: "c",
            current_fingerprint: "fp2",
          },
        }),
      ]),
    ];
    core.active = "ws1";
    render(<App />);
    expect((await screen.findAllByText(/re-review required/)).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Start" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Review…" })).toBeInTheDocument();
  });

  it("opens the command palette with the keyboard and runs an action (FR-054)", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    render(<App />);
    await screen.findByRole("heading", { name: "Project Phoenix" });
    fireEvent.keyDown(window, { code: "KeyP", key: "P", ctrlKey: true, shiftKey: true });
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    const input = within(palette).getByRole("combobox");
    fireEvent.change(input, { target: { value: "settings" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByText(/CRMonize\.com open-source project/)).toBeInTheDocument();
  });

  it("surfaces a recovered-state report on startup (FS-03)", async () => {
    handlers.set("app_startup_info", () => ({
      load_report: {
        generation: 1,
        recovered: true,
        problems: ["state.json: checksum mismatch"],
        restored_saved_at: 5,
        migrated_from: null,
      },
      platform: "linux",
      version: "0.1.0",
      backend: "portable-pty-0.9.0",
      state_dir: "/s",
      config_dir: "/c",
    }));
    render(<App />);
    expect(await screen.findByText(/State recovered from an earlier snapshot/)).toBeInTheDocument();
  });

  it("renames a terminal from its header; empty name resets (rename feature)", async () => {
    core.workspaces = [workspace([member("a"), member("b")])];
    core.active = "ws1";
    core.sessions = [
      session({ session_id: "s-a", member_id: "a" }),
      session({ session_id: "s-b", member_id: "b" }),
    ];
    handlers.set("member_update", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    const header = await screen.findByRole("button", { name: /Shell a — double-click to rename/ });
    await user.dblClick(header);
    const dialog = await screen.findByRole("dialog", { name: "Rename terminal" });
    const input = within(dialog).getByRole("textbox");
    await user.clear(input);
    await user.type(input, "API server logs");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(calls.find((c) => c.cmd === "member_update")?.args).toEqual({
      workspaceId: "ws1",
      memberId: "a",
      patch: { title: "API server logs" },
    });
    // No session was touched by renaming.
    expect(
      commandsCalled().filter((c) =>
        ["session_start", "session_restart", "session_stop"].includes(c)
      )
    ).toEqual([]);
  });

  it("terminal context menu copies the selection to the local clipboard", async () => {
    const copied: string[] = [];
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a", remote: true })];
    const user = userEvent.setup();
    // After setup(): user-event installs its own clipboard stub.
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (t: string) => void copied.push(t), readText: async () => "" },
    });
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    terminals[0].selection = "uptime output from the server";
    const host = document.querySelector(".terminal-host")!;
    fireEvent.contextMenu(host, { clientX: 50, clientY: 60 });
    const menu = await screen.findByRole("menu", { name: "Terminal actions" });
    const copyItem = within(menu).getByRole("menuitem", { name: /Copy/ });
    await user.click(copyItem);
    await waitFor(() => expect(copied).toEqual(["uptime output from the server"]));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("Ctrl+Shift+V pastes the local clipboard via the native read (WebKitGTK-safe)", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a", remote: true })];
    handlers.set("clipboard_read_text", () => "uname -a");
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    fireEvent.mouseDown(document.querySelector(".term-pane")!);
    fireEvent.keyDown(window, { code: "KeyV", key: "V", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(terminals[0].pasted).toEqual(["uname -a"]));
    expect(commandsCalled()).toContain("clipboard_read_text");
  });
  it("dropped SSH sessions show the auto-reconnect countdown and can stop it", async () => {
    core.workspaces = [workspace([member("r", { member_type: "remote_command" })])];
    core.active = "ws1";
    core.sessions = [
      session({
        session_id: "s-r",
        member_id: "r",
        remote: true,
        process_state: { kind: "disconnected", code: 255 },
      }),
    ];
    handlers.set("session_cancel_reconnect", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("button", { name: "Reconnect" });
    await emit({
      type: "reconnect",
      session_id: "s-r",
      active: true,
      attempt: 2,
      max_attempts: 8,
      delay_ms: 4000,
    });
    expect(await screen.findByText(/reconnecting in \d s \(attempt 2 of 8\)/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop reconnecting" }));
    expect(calls.find((c) => c.cmd === "session_cancel_reconnect")?.args).toEqual({
      sessionId: "s-r",
    });
    await emit({
      type: "reconnect",
      session_id: "s-r",
      active: false,
      attempt: 0,
      max_attempts: 8,
      delay_ms: 0,
    });
    expect(screen.queryByText(/Connection lost/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reconnect" })).toBeInTheDocument();
  });

  it("searches every open terminal and jumps to a match (Ctrl+Shift+G)", async () => {
    core.workspaces = [
      workspace([member("a"), member("b", { title: "Build server", color: "teal" })]),
    ];
    core.active = "ws1";
    core.sessions = [
      session({ session_id: "s-a", member_id: "a" }),
      session({ session_id: "s-b", member_id: "b" }),
    ];
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(2));
    terminals[0].bufferRows = [{ text: "all good", wrapped: false }];
    terminals[1].bufferRows = [
      { text: "compiling", wrapped: false },
      { text: "error: disk full", wrapped: false },
    ];
    fireEvent.keyDown(window, { code: "KeyG", key: "G", ctrlKey: true, shiftKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Search all terminals" });
    const input = within(dialog).getByRole("combobox");
    fireEvent.change(input, { target: { value: "disk" } });
    const hit = await within(dialog).findByRole("option", { name: /Build server · line 2/ });
    expect(hit).toHaveTextContent("error: disk full");
    expect(within(hit).getByRole("img", { name: "Teal tag" })).toBeInTheDocument();
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(terminals[1].selected).toEqual([7, 1, 4]));
    expect(screen.queryByRole("dialog", { name: "Search all terminals" })).not.toBeInTheDocument();
  });

  it("saves terminal output through the core and sets a color tag from the menu", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    handlers.set("scrollback_export", () => "/fixture/Downloads/toolonize-Shell-a.txt");
    handlers.set("member_update", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    terminals[0].bufferRows = [{ text: "$ uptime", wrapped: false }];
    const host = document.querySelector(".terminal-host")!;
    fireEvent.contextMenu(host, { clientX: 10, clientY: 10 });
    let menu = await screen.findByRole("menu", { name: "Terminal actions" });
    await user.click(within(menu).getByRole("menuitem", { name: "Save output to file" }));
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "scrollback_export")?.args).toEqual({
        name: "Shell a",
        text: "$ uptime\n",
      })
    );
    expect(await screen.findByText(/Saved to \/fixture\/Downloads/)).toBeInTheDocument();
    fireEvent.contextMenu(host, { clientX: 10, clientY: 10 });
    menu = await screen.findByRole("menu", { name: "Terminal actions" });
    await user.click(within(menu).getByRole("radio", { name: "Purple" }));
    expect(calls.find((c) => c.cmd === "member_update")?.args).toEqual({
      workspaceId: "ws1",
      memberId: "a",
      patch: { color: "purple" },
    });
  });

  it("lists SSH host aliases from the ssh config and adds one as a connection", async () => {
    handlers.set("ssh_hosts", () => ({
      hosts: ["build.example.com", "staging"],
      source: "/fixture/home/.ssh/config",
      notes: [],
    }));
    handlers.set("synthetic_add", () => "sys:ssh:staging");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /Launchers/ }));
    await user.click(await screen.findByRole("button", { name: "Read hosts from ~/.ssh/config" }));
    const list = await screen.findByRole("region", { name: "SSH hosts" });
    expect(within(list).getByText("build.example.com")).toBeInTheDocument();
    const row = within(list).getByText("staging").closest("li")!;
    await user.click(within(row).getByRole("button", { name: "Add" }));
    expect(calls.find((c) => c.cmd === "synthetic_add")?.args).toEqual({
      spec: { kind: "ssh", host: "staging" },
    });
  });

  it("Ctrl+click on a terminal link opens it through the core", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    handlers.set("open_url", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    terminals[0].bufferRows = [{ text: "docs: https://example.com/guide", wrapped: false }];
    const [link] = terminals[0].linksAt(1) as {
      text: string;
      activate: (e: MouseEvent, t: string) => void;
    }[];
    link.activate(new MouseEvent("click", { ctrlKey: true }), link.text);
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "open_url")?.args).toEqual({
        url: "https://example.com/guide",
      })
    );
    // OSC 8 targets are hidden behind text: confirm first.
    const handler = terminals[0].options.linkHandler as {
      activate: (e: MouseEvent, uri: string) => void;
    };
    handler.activate(new MouseEvent("click", { ctrlKey: true }), "https://example.org/hidden");
    const dialog = await screen.findByRole("dialog", { name: "Open link?" });
    expect(dialog).toHaveTextContent("https://example.org/hidden");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(calls.filter((c) => c.cmd === "open_url")).toHaveLength(1);
  });
});
