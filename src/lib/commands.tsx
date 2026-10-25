/** Keyboard/palette action dispatcher (FR-054). */
import { addTerminal, selectWorkspace, setMode, setView } from "./actions";
import { cycle } from "../layout/modes";
import { addMemberDialog } from "./addMember";
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

function focusMember(memberId: string | null) {
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
    case "terminal.copy": {
      const text = focusedController()?.copySelection();
      if (text) {
        await navigator.clipboard
          ?.writeText(text)
          .catch(() => notify("error", "Clipboard is not available"));
      }
      return;
    }
    case "terminal.paste": {
      const c = focusedController();
      if (!c) return;
      try {
        const text = await navigator.clipboard.readText();
        await c.paste(text);
      } catch {
        notify(
          "warning",
          "The system blocked clipboard access. Use Shift+Insert or the middle mouse button to paste."
        );
      }
      return;
    }
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
  items.push({ id: "view:activity", label: "Open Activity", run: () => setView("activity") });
  return items;
}
