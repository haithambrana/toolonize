/** Keyboard/palette action dispatcher (FR-054). */
import {
  addTerminal,
  renameMember,
  selectWorkspace,
  setMode,
  setView,
  splitTerminal,
  toggleBroadcast,
  zoomTerminal,
} from "./actions";
import { snippetPicker } from "./snippets";
import { terminalSettings } from "./terminalSettings";
import { backupNow, duplicateWorkspace, newFromTemplate, saveAsTemplate } from "./backup";
import { readClipboard } from "./clipboard";
import type { TerminalController } from "../terminal/controller";
import { cycle } from "../layout/modes";
import { addMemberDialog } from "./addMember";
import { toggleMemberNote } from "./notes";
import { peekController } from "../terminal/controller";
import { activeWorkspace, getState, notify, sessionFor, setState } from "../state/store";
import type { Action } from "./keymap";

function focusedController() {
  const s = getState();
  const w = activeWorkspace(s);
  if (!w || !s.focused) return undefined;
  const session = sessionFor(s, w.id, s.focused);
  return session ? peekController(session.session_id) : undefined;
}

export function focusMember(memberId: string | null) {
  if (!memberId) return;
  setState({ focused: memberId });
  const s = getState();
  const w = activeWorkspace(s);
  const session = w && sessionFor(s, w.id, memberId);
  if (session) peekController(session.session_id)?.focus();
}

export async function runAction(action: Action): Promise<void> {
  const s = getState();
  const w = activeWorkspace(s);
  switch (action) {
    case "palette":
      setState({ paletteOpen: !s.paletteOpen });
      return;
    case "workspace.next":
    case "workspace.prev": {
      const ids = s.workspaces.map((x) => x.id);
      const next = cycle(ids, w?.id ?? null, action === "workspace.next" ? 1 : -1);
      if (next) await selectWorkspace(next);
      return;
    }
    case "mode.grid":
    case "mode.focus":
    case "mode.tabs":
    case "mode.master_stack":
      if (w) await setMode(w, action.slice(5) as "grid" | "focus" | "tabs" | "master_stack");
      return;
    case "focus.toggle":
      if (w) await setMode(w, w.layout.mode === "focus" ? "grid" : "focus", s.focused);
      return;
    case "member.next":
    case "member.prev":
      if (w)
        focusMember(
          cycle(
            w.members.map((m) => m.id),
            s.focused,
            action === "member.next" ? 1 : -1
          )
        );
      return;
    case "terminal.search": {
      const c = focusedController();
      if (c) setState({ searchOpen: s.searchOpen === c.sessionId ? null : c.sessionId });
      return;
    }
    case "notes.terminal":
      if (!w || !s.focused) {
        notify("info", "Click a terminal first, then press the shortcut again");
        return;
      }
      toggleMemberNote(s.focused);
      return;
    case "notes.project":
      setState({ projectNoteOpen: !s.projectNoteOpen, view: "workspace" });
      return;
    case "terminal.split_right":
      await splitTerminal("right");
      return;
    case "terminal.split_down":
      await splitTerminal("bottom");
      return;
    case "terminal.zoom_in":
      await zoomTerminal(1);
      return;
    case "terminal.zoom_out":
      await zoomTerminal(-1);
      return;
    case "terminal.zoom_reset":
      await zoomTerminal("reset");
      return;
    case "terminal.broadcast":
      toggleBroadcast();
      return;
    case "terminal.snippets": {
      const c = focusedController();
      if (!c) {
        notify("info", "Click a terminal first, then insert a snippet");
        return;
      }
      await snippetPicker(c);
      return;
    }
    case "view.presentation":
      setState({ presentation: !s.presentation });
      return;
    case "terminal.search_all":
      setState({ globalSearchOpen: !s.globalSearchOpen });
      return;
    case "terminal.copy": {
      const c = focusedController();
      if (c?.hasSelection() && !(await c.copyToClipboard())) {
        notify("error", "Could not copy to the clipboard");
      }
      return;
    }
    case "terminal.paste": {
      const c = focusedController();
      if (c) await pasteInto(c);
      return;
    }
    case "terminal.select_all":
      focusedController()?.selectAll();
      return;
    case "member.rename":
      if (w && s.focused) await renameMember(w.id, s.focused);
      return;
    case "terminal.new":
      await addTerminal();
      return;
    case "view.launchers":
      setView("launchers");
      return;
    case "view.settings":
      setView("settings");
      return;
  }
}

export interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  run: () => void | Promise<void>;
}

export function paletteItems(): PaletteItem[] {
  const s = getState();
  const w = activeWorkspace(s);
  const items: PaletteItem[] = [];
  for (const ws of s.workspaces) {
    items.push({
      id: `ws:${ws.id}`,
      label: `Go to workspace: ${ws.name}`,
      run: () => selectWorkspace(ws.id),
    });
  }
  if (w) {
    items.push({
      id: "add-member",
      label: `Add member to ${w.name}…`,
      run: () => addMemberDialog(w),
    });
    for (const m of w.members.filter((x) => x.member_type !== "external_app")) {
      items.push({
        id: `settings:${m.id}`,
        label: `Terminal settings: ${m.title ?? m.status.launcher_name ?? m.launcher_id}`,
        run: () => terminalSettings(w, m),
      });
    }
    for (const m of w.members) {
      items.push({
        id: `member:${m.id}`,
        label: `Focus member: ${m.title ?? m.status.launcher_name ?? m.launcher_id}`,
        run: () => {
          setView("workspace");
          focusMember(m.id);
        },
      });
    }
  }
  if (w) {
    items.push(
      {
        id: "ws:duplicate",
        label: `Duplicate workspace ${w.name}`,
        run: () => duplicateWorkspace(w),
      },
      { id: "ws:template", label: `Save ${w.name} as template…`, run: () => saveAsTemplate(w) }
    );
  }
  items.push(
    { id: "ws:from-template", label: "New workspace from template…", run: () => newFromTemplate() },
    { id: "backup", label: "Back up everything now", run: () => void backupNow(false) },
    { id: "view:activity", label: "Open Activity", run: () => setView("activity") }
  );
  return items;
}

/** Paste the local clipboard into a terminal (multi-line warning applies). */
export async function pasteInto(c: TerminalController): Promise<void> {
  const text = await readClipboard();
  if (text === null) {
    notify(
      "warning",
      "The system blocked clipboard access. Use Shift+Insert or the middle mouse button to paste."
    );
    return;
  }
  await c.paste(text);
}
