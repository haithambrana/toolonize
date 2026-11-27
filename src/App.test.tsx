import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { MemberView, WorkspaceView } from "./lib/ipc";
import { layoutFor, membersIn } from "./layout/modes";
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
  settings,
} from "./test/fakeCore";
import { terminals } from "./test/fakeXterm";
import { resetNotes } from "./lib/notes";
import { setLanguage } from "./i18n";

const user = () => userEvent.setup();

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
    start_dir: null,
    zoom: 0,
    launch: { env: [], startup_command: null, confirmed: null },
    launch_confirmed: true,
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
    project_dir: null,
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
    resetNotes();
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
      resume: null,
    }));
    render(<App />);
    expect(await screen.findByText(/State recovered from an earlier snapshot/)).toBeInTheDocument();
  });

  it("renames a terminal (Ctrl+Shift+R); empty name resets (rename feature)", async () => {
    core.workspaces = [workspace([member("a"), member("b")])];
    core.active = "ws1";
    core.sessions = [
      session({ session_id: "s-a", member_id: "a" }),
      session({ session_id: "s-b", member_id: "b" }),
    ];
    handlers.set("member_update", () => undefined);
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(2));
    fireEvent.mouseDown(document.querySelectorAll(".term-pane")[0]);
    fireEvent.keyDown(window, { code: "KeyR", key: "R", ctrlKey: true, shiftKey: true });
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
  it("exports diagnostics from Settings and shows where they were saved", async () => {
    handlers.set("diagnostics_export", () => "/fixture/Downloads/toolonize-diagnostics.txt");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("button", { name: "Export diagnostics" }));
    expect(
      await screen.findByText(/Diagnostics saved to \/fixture\/Downloads/)
    ).toBeInTheDocument();
  });
  it("Ctrl+Shift+N opens the focused terminal's note; typing is saved and shown as present", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    fireEvent.mouseDown(document.querySelector(".term-pane")!);
    fireEvent.keyDown(window, { code: "KeyN", key: "N", ctrlKey: true, shiftKey: true });
    const note = await screen.findByRole("textbox", { name: "Note for Shell a" });
    expect(note).toHaveFocus();
    fireEvent.change(note, { target: { value: "deploy with --dry-run first" } });
    await waitFor(() => expect(core.notes.ws1?.members.a).toBe("deploy with --dry-run first"));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Note( has a note)?$/ })).toHaveClass("has-note");
    // Ctrl+Shift+V inside the note pastes into the note, not into the terminal.
    fireEvent.keyDown(note, { code: "KeyV", key: "V", ctrlKey: true, shiftKey: true });
    expect(calls.some((c) => c.cmd === "clipboard_read_text")).toBe(false);
    // Escape closes the note and gives the keyboard back to the terminal.
    const focusedBefore = terminals[0].focused;
    fireEvent.keyDown(note, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "Note for Shell a" })).not.toBeInTheDocument();
    await waitFor(() => expect(terminals[0].focused).toBeGreaterThan(focusedBefore));
  });

  it("Ctrl+Shift+M opens the project note, and saved notes come back after a restart", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.notes.ws1 = { project: "", members: { a: "kept from last time" } };
    const { unmount } = render(<App />);
    await screen.findByRole("heading", { name: "Project Phoenix" });
    fireEvent.keyDown(window, { code: "KeyM", key: "M", ctrlKey: true, shiftKey: true });
    const project = await screen.findByRole("textbox", { name: "Project note" });
    await waitFor(() => expect(project).not.toBeDisabled());
    fireEvent.change(project, { target: { value: "release on Thursday" } });
    await waitFor(() => expect(core.notes.ws1.project).toBe("release on Thursday"));
    unmount();
    // "Restart": a fresh UI state reads what the core saved.
    resetStore();
    render(<App />);
    await screen.findByRole("heading", { name: "Project Phoenix" });
    await user().click(await screen.findByRole("button", { name: /^Note( has a note)?$/ }));
    expect(await screen.findByRole("textbox", { name: "Note for Shell a" })).toHaveValue(
      "kept from last time"
    );
    await user().click(screen.getByRole("button", { name: /^Notes( has a note)?$/ }));
    expect(await screen.findByRole("textbox", { name: "Project note" })).toHaveValue(
      "release on Thursday"
    );
  });

  it("removing a terminal saves its note first and reloads the project note", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    handlers.set("member_remove", () => {
      core.notes.ws1 = { project: "— Note from the removed terminal “Shell a” —\nx", members: {} };
    });
    const u = user();
    render(<App />);
    await u.click(await screen.findByRole("button", { name: /^Note( has a note)?$/ }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Note for Shell a" }), {
      target: { value: "x" },
    });
    await u.click(screen.getByRole("button", { name: "Remove" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Its note is kept: it moves into the project note.");
    await u.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(commandsCalled()).toContain("member_remove"));
    const order = calls.map((c) => c.cmd);
    expect(order.indexOf("note_save")).toBeLessThan(order.indexOf("member_remove"));
    expect(order.lastIndexOf("notes_get")).toBeGreaterThan(order.indexOf("member_remove"));
  });
  describe("terminal power features", () => {
    const shell = () =>
      entry({
        launcher_id: "sys:shell:bash",
        display_name: "bash",
        origin: "synthetic",
        origin_label: "Built-in (ToolOnize)",
        executable: true,
        effective_type: "embedded_terminal",
      });

    function setupSplit() {
      core.entries = [shell()];
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      handlers.set("session_cwd", () => "/srv/app");
      handlers.set("member_add", () => {
        const w = core.workspaces[0];
        core.workspaces = [{ ...w, members: [...w.members, member("b")] }];
        return "b";
      });
      handlers.set("member_update", () => undefined);
      handlers.set("session_start", (a) => {
        const info = session({ session_id: "s-b", member_id: a.memberId as string });
        core.sessions = [...core.sessions, info];
        return info;
      });
    }

    it("Ctrl+Shift+E splits right: new shell in the same folder, beside the focused pane", async () => {
      setupSplit();
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      fireEvent.mouseDown(document.querySelector(".term-pane")!);
      fireEvent.keyDown(window, { code: "KeyE", key: "E", ctrlKey: true, shiftKey: true });
      await waitFor(() => expect(commandsCalled()).toContain("session_start"));
      expect(calls.find((c) => c.cmd === "member_update")?.args).toEqual({
        workspaceId: "ws1",
        memberId: "b",
        patch: { start_dir: "/srv/app" },
      });
      const save = calls.filter((c) => c.cmd === "workspace_save_layout").at(-1)!.args!;
      expect(save.mode).toBe("grid");
      expect(membersIn(save.layout)).toEqual(["a", "b"]);
      expect(JSON.stringify(save.layout)).toContain('"id":"ts-b"');
    });

    it("Ctrl+= and Ctrl+0 change and reset the focused terminal's size", async () => {
      core.workspaces = [workspace([member("a", { zoom: 2 })])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      // Like the real core: the change is stored and reported back.
      handlers.set("member_update", (args) => {
        const zoom = (args.patch as { zoom: number }).zoom;
        const w = core.workspaces[0];
        core.workspaces = [{ ...w, members: w.members.map((m) => ({ ...m, zoom })) }];
        core.events?.send({ type: "workspaces" });
      });
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      expect(terminals[0].options.fontSize).toBe(15);
      fireEvent.mouseDown(document.querySelector(".term-pane")!);
      // Rapid presses build on each other before the core replies.
      fireEvent.keyDown(window, { code: "Equal", key: "=", ctrlKey: true });
      fireEvent.keyDown(window, { code: "Equal", key: "=", ctrlKey: true });
      fireEvent.keyDown(window, { code: "Equal", key: "=", ctrlKey: true });
      await waitFor(() => expect(terminals[0].options.fontSize).toBe(18));
      fireEvent.keyDown(window, { code: "Minus", key: "-", ctrlKey: true });
      fireEvent.keyDown(window, { code: "Digit0", key: "0", ctrlKey: true });
      await waitFor(() =>
        expect(calls.filter((c) => c.cmd === "member_update").map((c) => c.args?.patch)).toEqual([
          { zoom: 3 },
          { zoom: 4 },
          { zoom: 5 },
          { zoom: 4 },
          { zoom: 0 },
        ])
      );
    });

    it("broadcast sends typing to every running terminal of the workspace", async () => {
      core.workspaces = [workspace([member("a"), member("b")])];
      core.active = "ws1";
      core.sessions = [
        session({ session_id: "s-a", member_id: "a" }),
        session({ session_id: "s-b", member_id: "b" }),
      ];
      const u = user();
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(2));
      await u.click(screen.getByRole("button", { name: "Broadcast" }));
      expect(await screen.findAllByText("Broadcast", { selector: ".chip" })).toHaveLength(2);
      terminals[0].type("ls\r");
      await waitFor(() =>
        expect(
          calls.filter((c) => c.cmd === "session_write").map((c) => c.args?.sessionId)
        ).toEqual(["s-a", "s-b"])
      );
      await u.click(screen.getByRole("button", { name: "Broadcast on" }));
      terminals[0].type("x");
      await waitFor(() => expect(calls.filter((c) => c.cmd === "session_write")).toHaveLength(3));
    });

    it("Ctrl+Shift+S inserts a saved snippet into the focused terminal", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      handlers.set("settings_get", () => ({
        ...settings,
        snippets: [
          { name: "Tail app log", text: "tail -f /var/log/app.log" },
          { name: "Disk usage", text: "df -h" },
        ],
      }));
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      fireEvent.mouseDown(document.querySelector(".term-pane")!);
      fireEvent.keyDown(window, { code: "KeyS", key: "S", ctrlKey: true, shiftKey: true });
      const dialog = await screen.findByRole("dialog", { name: "Insert snippet" });
      fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "disk" } });
      fireEvent.keyDown(within(dialog).getByRole("combobox"), { key: "Enter" });
      await waitFor(() => expect(terminals[0].pasted).toEqual(["df -h"]));
    });

    it("presentation mode hides the sidebar and toolbar and enlarges text", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      const u = user();
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      fireEvent.keyDown(window, { code: "F11", key: "F11", ctrlKey: true, shiftKey: true });
      expect(await screen.findByRole("button", { name: /Exit presentation/ })).toBeInTheDocument();
      expect(screen.queryByRole("navigation", { name: "Main" })).not.toBeInTheDocument();
      await waitFor(() => expect(terminals[0].options.fontSize).toBe(17));
      await u.click(screen.getByRole("button", { name: /Exit presentation/ }));
      expect(screen.getByRole("navigation", { name: "Main" })).toBeInTheDocument();
    });

    it("notifies on the desktop when a terminal you are not looking at ends", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a", title: "Build" })];
      handlers.set("desktop_notify", () => true);
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      const focus = vi.spyOn(document, "hasFocus").mockReturnValue(false);
      await emit({
        type: "session",
        info: session({
          session_id: "s-a",
          member_id: "a",
          title: "Build",
          process_state: { kind: "exited", code: 2, signal: null },
        }),
      });
      expect(calls.find((c) => c.cmd === "desktop_notify")?.args).toEqual({
        title: "Build",
        body: "Exited with code 2",
      });
      // Looking at it: no notification.
      focus.mockReturnValue(true);
      await emit({
        type: "session",
        info: session({ session_id: "s-a", member_id: "a", title: "Build" }),
      });
      await emit({
        type: "session",
        info: session({
          session_id: "s-a",
          member_id: "a",
          title: "Build",
          process_state: { kind: "exited", code: 0, signal: null },
        }),
      });
      expect(calls.filter((c) => c.cmd === "desktop_notify")).toHaveLength(1);
      focus.mockRestore();
    });

    it("sets the project folder from the toolbar", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      handlers.set("workspace_set_project_dir", () => undefined);
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "More ▾" }));
      await u.click(screen.getByRole("menuitem", { name: "Project folder…" }));
      const dialog = await screen.findByRole("dialog", { name: "Project folder" });
      await u.type(within(dialog).getByRole("textbox"), "relative/path");
      expect(within(dialog).getByRole("button", { name: "Save" })).toBeDisabled();
      await u.clear(within(dialog).getByRole("textbox"));
      await u.type(within(dialog).getByRole("textbox"), "/home/me/phoenix");
      await u.click(within(dialog).getByRole("button", { name: "Save" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "workspace_set_project_dir")?.args).toEqual({
          workspaceId: "ws1",
          dir: "/home/me/phoenix",
        })
      );
    });
  });
  it("shows each pane's name, state and actions in one title row", async () => {
    core.workspaces = [workspace([member("a"), member("b")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    expect(document.querySelector(".pane-header")).toBeNull();
    // The running pane's actions sit in the layout's title row.
    const actions = document.querySelectorAll(
      ".flexlayout__tabset_header .pane-actions, .flexlayout__tab_toolbar .pane-actions"
    );
    expect(actions.length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByRole("button", { name: "Search" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Stop" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: /^Note( has a note)?$/ })).toHaveLength(2);
    // FlexLayout renders tab buttons twice (one copy for measuring).
    expect(document.querySelectorAll(".tab-leading .state-running").length).toBeGreaterThan(0);
    expect(document.querySelectorAll(".tab-leading .state-new").length).toBeGreaterThan(0);
  });

  it("shows Start terminals only while a terminal is not running; More menu holds the rest", async () => {
    core.workspaces = [workspace([member("a")])];
    core.active = "ws1";
    core.sessions = [session({ session_id: "s-a", member_id: "a" })];
    handlers.set("workspace_set_policy", () => undefined);
    const u = user();
    render(<App />);
    await waitFor(() => expect(terminals).toHaveLength(1));
    expect(screen.queryByRole("button", { name: "Start terminals" })).not.toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "More ▾" }));
    const menu = screen.getByRole("menu", { name: "More workspace actions" });
    expect(
      within(menu).getByRole("menuitemradio", { name: /On open: Ask what to start/ })
    ).toHaveAttribute("aria-checked", "true");
    await u.click(within(menu).getByRole("menuitemradio", { name: /Restore \+ start terminals/ }));
    expect(calls.find((c) => c.cmd === "workspace_set_policy")?.args).toEqual({
      workspaceId: "ws1",
      policy: "restore_terminals",
    });
    await emit({
      type: "session",
      info: session({
        session_id: "s-a",
        member_id: "a",
        process_state: { kind: "exited", code: 0, signal: null },
      }),
    });
    expect(await screen.findByRole("button", { name: "Start terminals" })).toBeInTheDocument();
  });

  it("Activity shows names instead of internal ids and filters the audit log", async () => {
    core.workspaces = [workspace([member("a", { title: "API server" })])];
    core.active = "ws1";
    core.sessions = [
      session({ session_id: "s-a", member_id: "a", title: "API server", view_state: "hidden" }),
    ];
    handlers.set("audit_recent", () => [
      {
        at: 2,
        action: "session.start",
        outcome: "allowed",
        workspace_id: "ws1",
        member_id: "a",
        launcher_id: "sys:shell:a",
        fingerprint: null,
        detail: null,
      },
      {
        at: 1,
        action: "url.open",
        outcome: "denied",
        workspace_id: null,
        member_id: null,
        launcher_id: null,
        fingerprint: null,
        detail: "scheme not allowed",
      },
    ]);
    handlers.set("audit_export", () => "/fixture/Downloads/toolonize-audit-log.txt");
    const u = user();
    render(<App />);
    await u.click(await screen.findByRole("button", { name: "Activity" }));
    expect(await screen.findByText("In the background")).toBeInTheDocument();
    expect(await screen.findByText("Terminal started")).toBeInTheDocument();
    expect(screen.getAllByText("API server").length).toBeGreaterThan(1);
    expect(screen.queryByText("sys:shell:a")).not.toBeInTheDocument();
    await u.selectOptions(screen.getByRole("combobox", { name: "Outcome" }), "denied");
    expect(screen.queryByText("Terminal started")).not.toBeInTheDocument();
    expect(screen.getByText("Link opened")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Export" }));
    expect(await screen.findByText(/Audit log saved to/)).toBeInTheDocument();
  });

  describe("backup, duplicate and templates (ADR-014)", () => {
    it("backs up everything from Settings, with or without settings", async () => {
      handlers.set("backup_export", () => "/fixture/Downloads/toolonize-backup.json");
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      await u.click(await screen.findByRole("button", { name: "Back up everything" }));
      expect(await screen.findByText(/Backup saved to \/fixture\/Downloads/)).toBeInTheDocument();
      await u.click(screen.getByRole("checkbox", { name: "Include settings" }));
      await u.click(screen.getByRole("button", { name: "Back up everything" }));
      await waitFor(() =>
        expect(calls.filter((c) => c.cmd === "backup_export").map((c) => c.args)).toEqual([
          { includeSettings: true },
          { includeSettings: false },
        ])
      );
    });

    it("saves typed-but-unsaved note text before the backup is written", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      handlers.set("backup_export", () => "/fixture/Downloads/b.json");
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Notes" }));
      const area = await screen.findByRole("textbox", { name: "Project note" });
      await waitFor(() => expect(area).not.toBeDisabled());
      fireEvent.change(area, { target: { value: "deploy at 5" } });
      // Straight to the palette action, before the save delay ran out.
      await act(async () => {
        const { backupNow } = await import("./lib/backup");
        await backupNow(false);
      });
      const order = commandsCalled().filter((c) => c === "note_save" || c === "backup_export");
      expect(order).toEqual(["note_save", "backup_export"]);
      expect(core.notes.ws1.project).toBe("deploy at 5");
    });

    it("restores a backup file into review and reports what came back", async () => {
      handlers.set("backup_restore", () => {
        core.workspaces = [
          workspace([member("a", { authorized_fingerprint: null })], {
            id: "ws-r",
            name: "Phoenix (restored)",
            staged_import: true,
          }),
        ];
        return {
          workspaces: 1,
          members: 1,
          notes: 2,
          templates: 1,
          connections: 0,
          settings_restored: false,
          warnings: [],
        };
      });
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      await screen.findByRole("button", { name: "Restore from backup…" });
      const input = screen.getByLabelText("Backup file");
      const file = new File(['{"format":"toolonize-backup"}'], "backup.json", {
        type: "application/json",
      });
      await u.upload(input, file);
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "backup_restore")?.args).toEqual({
          contents: '{"format":"toolonize-backup"}',
          restoreSettings: false,
        })
      );
      expect(
        await screen.findByText(
          /Restored 1 workspace\(s\) with 1 terminal\(s\), 2 note\(s\), 1 template\(s\)\. Review and authorize/
        )
      ).toBeInTheDocument();
      const nav = screen.getByRole("navigation", { name: "Main" });
      expect(await within(nav).findByText("Phoenix (restored)")).toBeInTheDocument();
      expect(within(nav).getByText("review")).toBeInTheDocument();
    });

    it("refuses oversized backup files and shows the core's reason for bad ones", async () => {
      handlers.set("backup_restore", () => {
        throw "not a ToolOnize backup";
      });
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      await u.click(await screen.findByRole("checkbox", { name: "Also replace my settings" }));
      const input = screen.getByLabelText("Backup file");
      const big = new File(["{}"], "big.json", { type: "application/json" });
      Object.defineProperty(big, "size", { value: 17 * 1024 * 1024 });
      await u.upload(input, big);
      expect(await screen.findByText(/too large/)).toBeInTheDocument();
      expect(commandsCalled()).not.toContain("backup_restore");
      await u.upload(input, new File(["nope"], "x.json", { type: "application/json" }));
      expect(await screen.findByText(/not a ToolOnize backup/)).toBeInTheDocument();
      expect(calls.find((c) => c.cmd === "backup_restore")?.args).toEqual({
        contents: "nope",
        restoreSettings: true,
      });
    });

    it("duplicates a workspace and saves it as a template from the More menu", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      handlers.set("workspace_duplicate", () => {
        core.workspaces = [
          ...core.workspaces,
          workspace([member("a2")], { id: "ws2", name: "Project Phoenix (copy)" }),
        ];
        return "ws2";
      });
      handlers.set("template_save", () => "tpl_1");
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "More ▾" }));
      await u.click(screen.getByRole("menuitem", { name: "Duplicate workspace" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "workspace_duplicate")?.args).toEqual({
          workspaceId: "ws1",
        })
      );
      expect(
        await screen.findByRole("heading", { name: "Project Phoenix (copy)" })
      ).toBeInTheDocument();
      expect(calls.filter((c) => c.cmd === "workspace_set_active").at(-1)?.args).toEqual({
        workspaceId: "ws2",
      });

      await u.click(screen.getByRole("button", { name: "More ▾" }));
      await u.click(screen.getByRole("menuitem", { name: "Save as template…" }));
      const dialog = await screen.findByRole("dialog", { name: "Save as template" });
      const name = within(dialog).getByRole("textbox");
      expect(name).toHaveValue("Project Phoenix (copy)");
      await u.clear(name);
      expect(within(dialog).getByRole("button", { name: "Save template" })).toBeDisabled();
      await u.type(name, "Dev pair");
      await u.click(within(dialog).getByRole("button", { name: "Save template" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "template_save")?.args).toEqual({
          workspaceId: "ws2",
          name: "Dev pair",
        })
      );
      expect(await screen.findByText("Template “Dev pair” saved.")).toBeInTheDocument();
    });

    it("creates a workspace from a template and deletes templates", async () => {
      let templates = [
        { id: "tpl_1", name: "Dev pair", created_at: 1, members: 2 },
        { id: "tpl_2", name: "Ops", created_at: 2, members: 3 },
      ];
      handlers.set("template_list", () => templates);
      handlers.set("template_delete", (a) => {
        templates = templates.filter((t) => t.id !== a.templateId);
      });
      handlers.set("template_create_workspace", (a) => {
        core.workspaces = [
          workspace([member("x"), member("y")], { id: "ws-t", name: a.name as string }),
        ];
        return "ws-t";
      });
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "New from template…" }));
      let dialog = await screen.findByRole("dialog", { name: "New workspace from template" });
      expect(await within(dialog).findByText("Dev pair")).toBeInTheDocument();
      expect(within(dialog).getByText("3 terminal(s)")).toBeInTheDocument();
      await u.click(within(dialog).getByRole("button", { name: "Delete template Ops" }));
      expect(within(dialog).getByRole("alert")).toHaveTextContent("Delete “Ops”?");
      await u.click(within(dialog).getByRole("button", { name: "Keep" }));
      expect(commandsCalled()).not.toContain("template_delete");
      await u.click(within(dialog).getByRole("button", { name: "Delete template Ops" }));
      await u.click(within(dialog).getByRole("button", { name: "Yes, delete" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "template_delete")?.args).toEqual({
          templateId: "tpl_2",
        })
      );
      dialog = await screen.findByRole("dialog", { name: "New workspace from template" });
      await waitFor(() => expect(within(dialog).queryByText("Ops")).toBeNull());
      await u.click(within(dialog).getByRole("button", { name: "Use template Dev pair" }));
      const prompt = await screen.findByRole("dialog", { name: "New workspace" });
      await u.type(within(prompt).getByLabelText("Name"), "Billing");
      await u.click(within(prompt).getByRole("button", { name: "Create" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "template_create_workspace")?.args).toEqual({
          templateId: "tpl_1",
          name: "Billing",
        })
      );
      expect(await screen.findByRole("heading", { name: "Billing" })).toBeInTheDocument();
    });

    it("explains how to make a template when there are none", async () => {
      handlers.set("template_list", () => []);
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "New from template…" }));
      expect(await screen.findByText(/No templates yet/)).toBeInTheDocument();
    });
  });

  describe("session memory (ADR-015)", () => {
    const startup = (resume: unknown) => ({
      load_report: {
        generation: 1,
        recovered: false,
        problems: [],
        restored_saved_at: null,
        migrated_from: null,
      },
      platform: "linux",
      version: "0.1.0",
      backend: "portable-pty-0.9.0",
      state_dir: "/s",
      config_dir: "/c",
      resume,
    });

    it("tells the user what came back after the computer lost power", async () => {
      handlers.set("app_startup_info", () =>
        startup({
          crashed: true,
          restored: 2,
          left_stopped: ["Deploy"],
          failed: ["Build: changed"],
        })
      );
      render(<App />);
      expect(
        await screen.findByText(
          "ToolOnize did not close normally last time. Restored 2 terminals with their last output."
        )
      ).toBeInTheDocument();
      // Shown once even though dev mode runs startup twice.
      const { notify } = await import("./state/store");
      act(() =>
        notify(
          "warning",
          "ToolOnize did not close normally last time. Restored 2 terminals with their last output."
        )
      );
      expect(screen.getAllByText(/did not close normally/)).toHaveLength(1);
      expect(screen.getByText(/Not started again automatically.*Deploy/)).toBeInTheDocument();
      expect(screen.getByText("Could not restore: Build: changed.")).toBeInTheDocument();
    });

    it("says so quietly after a normal close, and nothing when there was nothing", async () => {
      handlers.set("app_startup_info", () =>
        startup({ crashed: false, restored: 1, left_stopped: [], failed: [] })
      );
      const { unmount } = render(<App />);
      expect(
        await screen.findByText("Restored 1 terminal from your last session.")
      ).toBeInTheDocument();
      unmount();
      resetStore();
      handlers.set("app_startup_info", () =>
        startup({ crashed: false, restored: 0, left_stopped: [], failed: [] })
      );
      render(<App />);
      await screen.findByRole("button", { name: "Settings" });
      expect(screen.queryByText(/Restored|did not close/)).toBeNull();
    });

    it("restored output is shown in the terminal before new output", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a", end_offset: 0 })];
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      await waitFor(() => expect(core.attachments.get("s-a")).toBeDefined());
      const prelude =
        "\x1b[0m$ make\r\nok\r\n\x1b[0m\x1b[2m── restored from your previous session ──\x1b[0m\r\n";
      await act(async () => {
        const { frame } = await import("./test/fakeCore");
        core.attachments.get("s-a")!.output.send(frame(0, prelude) as never);
        const bytes = new TextEncoder().encode(prelude).length;
        core.attachments.get("s-a")!.output.send(frame(bytes, "$ ") as never);
        await new Promise((r) => setTimeout(r, 30));
      });
      const written = terminals[0].written.join("");
      expect(written).toContain("restored from your previous session");
      expect(written.indexOf("$ make")).toBeLessThan(written.indexOf("restored from"));
    });

    it("Settings: open at login reflects and changes the system entry", async () => {
      let on = false;
      handlers.set("autostart_get", () => on);
      handlers.set("autostart_set", (a) => (on = a.enabled as boolean));
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      const box = await screen.findByRole("checkbox", { name: /Open ToolOnize when I log in/ });
      await waitFor(() => expect(box).not.toBeDisabled());
      expect(box).not.toBeChecked();
      await u.click(box);
      await waitFor(() => expect(box).toBeChecked());
      expect(calls.find((c) => c.cmd === "autostart_set")?.args).toEqual({ enabled: true });
      await u.click(box);
      await waitFor(() => expect(box).not.toBeChecked());
      // The system refused: the box shows the real state and says why.
      handlers.set("autostart_set", () => {
        throw "could not write the login entry";
      });
      await u.click(box);
      expect(await screen.findByText(/could not write the login entry/)).toBeInTheDocument();
      expect(box).not.toBeChecked();
    });

    it("Settings: turn restarting and remembering off, and forget saved output", async () => {
      handlers.set("memory_forget", () => 3);
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      await u.click(
        await screen.findByRole("checkbox", { name: /Start the terminals that were running/ })
      );
      await u.click(screen.getByRole("checkbox", { name: /Remember what each terminal showed/ }));
      const updates = calls.filter((c) => c.cmd === "settings_update").map((c) => c.args!.settings);
      expect(updates).toEqual([
        expect.objectContaining({ restore_terminals: false }),
        expect.objectContaining({ remember_terminal_content: false }),
      ]);
      await u.click(screen.getByRole("button", { name: "Forget saved terminal output" }));
      expect(
        await screen.findByText("Deleted the saved output of 3 terminal(s).")
      ).toBeInTheDocument();
    });
  });

  describe("terminal settings: environment and startup command (ADR-016)", () => {
    it("edits variables and the startup command from the terminal's menu", async () => {
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      handlers.set("member_update", () => undefined);
      const u = user();
      render(<App />);
      await waitFor(() => expect(terminals).toHaveLength(1));
      fireEvent.contextMenu(document.querySelector(".terminal-host")!, {
        clientX: 50,
        clientY: 60,
      });
      await u.click(await screen.findByRole("menuitem", { name: /Terminal settings/ }));
      const dialog = await screen.findByRole("dialog", { name: /Terminal settings/ });
      const env = within(dialog).getByRole("textbox", { name: /Environment variables/ });
      await u.type(env, "NODE_ENV=development{enter}not a pair");
      expect(within(dialog).getByRole("alert")).toHaveTextContent("Line 2: use NAME=value");
      expect(within(dialog).getByRole("button", { name: "Save" })).toBeDisabled();
      await u.clear(env);
      await u.type(env, "NODE_ENV=development{enter}# comment{enter}{enter}API_URL=http://x:1/a=b");
      await u.type(within(dialog).getByRole("textbox", { name: /Startup command/ }), "npm run dev");
      await u.click(within(dialog).getByRole("button", { name: "Save" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "member_update")?.args).toEqual({
          workspaceId: "ws1",
          memberId: "a",
          patch: {
            env: [
              { name: "NODE_ENV", value: "development" },
              { name: "API_URL", value: "http://x:1/a=b" },
            ],
            startup_command: "npm run dev",
          },
        })
      );
      expect(
        await screen.findByText("Saved. Restart the terminal to use the new settings.")
      ).toBeInTheDocument();
    });

    it("restored settings must be checked before the terminal can start", async () => {
      core.workspaces = [
        workspace([
          member("a", {
            launch: {
              env: [{ name: "API_TOKEN", value: "" }],
              startup_command: "make watch",
              confirmed: null,
            },
            launch_confirmed: false,
          }),
        ]),
      ];
      core.active = "ws1";
      handlers.set("member_update", (a) => {
        const m = core.workspaces[0].members[0];
        core.workspaces = [
          workspace([
            {
              ...m,
              launch: {
                env: (a.patch as { env: never[] }).env,
                startup_command: "make watch",
                confirmed: "d",
              },
              launch_confirmed: true,
            },
          ]),
        ];
      });
      const u = user();
      render(<App />);
      expect(
        await screen.findByText("Terminal settings need your confirmation")
      ).toBeInTheDocument();
      expect(screen.queryByTestId("start-member")).toBeNull();
      await u.click(screen.getByRole("button", { name: "Check settings…" }));
      const dialog = await screen.findByRole("dialog", { name: /Terminal settings/ });
      expect(within(dialog).getByRole("alert")).toHaveTextContent(
        "Values are never stored in backups — fill in API_TOKEN."
      );
      const env = within(dialog).getByRole("textbox", { name: /Environment variables/ });
      expect(env).toHaveValue("API_TOKEN=");
      await u.type(env, "abc123");
      expect(within(dialog).getByRole("textbox", { name: /Startup command/ })).toHaveValue(
        "make watch"
      );
      await u.click(within(dialog).getByRole("button", { name: "Save" }));
      await waitFor(() =>
        expect(calls.find((c) => c.cmd === "member_update")?.args?.patch).toEqual({
          env: [{ name: "API_TOKEN", value: "abc123" }],
          startup_command: "make watch",
        })
      );
      await emit({ type: "workspaces" });
      expect(await screen.findByTestId("start-member")).toBeInTheDocument();
    });

    it("commands get variables but no startup command", async () => {
      core.workspaces = [workspace([member("a", { member_type: "local_command" })])];
      core.active = "ws1";
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings…" }));
      const dialog = await screen.findByRole("dialog", { name: /Terminal settings/ });
      expect(within(dialog).getByRole("textbox", { name: /Startup command/ })).toBeDisabled();
    });
  });

  describe("Arabic interface", () => {
    afterEach(() => {
      settings.language = "en";
      setLanguage("en");
    });

    it("shows the interface in Arabic, right-to-left, with terminals left-to-right", async () => {
      settings.language = "ar";
      core.workspaces = [workspace([member("a")])];
      core.active = "ws1";
      core.sessions = [session({ session_id: "s-a", member_id: "a" })];
      render(<App />);
      expect(await screen.findByText("مساحات العمل")).toBeInTheDocument();
      expect(document.documentElement.dir).toBe("rtl");
      expect(screen.getByRole("button", { name: "الإعدادات" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "+ إضافة عنصر" })).toBeInTheDocument();
      await waitFor(() => expect(terminals).toHaveLength(1));
      expect(document.querySelector(".term-container")?.getAttribute("dir")).toBe("ltr");
      // Main toolbar and sidebar: no English left (names and shortcuts aside).
      const toolbar = document.querySelector(".ws-actions")!.textContent!;
      expect(toolbar).not.toMatch(/Notes|Broadcast|Add member|More/);
    });

    it("switching the language in Settings applies it at once", async () => {
      handlers.set("settings_update", (a) => {
        Object.assign(settings, a.settings as object);
        core.events?.send({ type: "settings" });
      });
      const u = user();
      render(<App />);
      await u.click(await screen.findByRole("button", { name: "Settings" }));
      await u.selectOptions(await screen.findByRole("combobox", { name: "Language" }), "ar");
      expect(await screen.findByText("ذاكرة الجلسة")).toBeInTheDocument();
      expect(document.documentElement.dir).toBe("rtl");
    });
  });
});
