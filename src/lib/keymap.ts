/**
 * Keyboard shortcuts for core actions (FR-054). Defaults avoid plain
 * Ctrl+<letter> so terminal programs keep their keys; every binding can be
 * remapped in Settings (validated again in Rust).
 */
export type Action =
  | "palette"
  | "workspace.next"
  | "workspace.prev"
  | "mode.grid"
  | "mode.focus"
  | "mode.tabs"
  | "mode.master_stack"
  | "terminal.search"
  | "terminal.copy"
  | "terminal.paste"
  | "terminal.new"
  | "member.next"
  | "member.prev"
  | "member.rename"
  | "terminal.select_all"
  | "focus.toggle"
  | "view.launchers"
  | "view.settings";

export const ACTION_LABELS: Record<Action, string> = {
  palette: "Open command palette",
  "workspace.next": "Next workspace",
  "workspace.prev": "Previous workspace",
  "mode.grid": "Grid layout",
  "mode.focus": "Focus layout",
  "mode.tabs": "Tabs layout",
  "mode.master_stack": "Master + Stack layout",
  "terminal.search": "Search terminal scrollback",
  "terminal.copy": "Copy terminal selection",
  "terminal.paste": "Paste into terminal",
  "terminal.new": "Add a terminal to this workspace",
  "member.next": "Focus next member",
  "member.prev": "Focus previous member",
  "member.rename": "Rename the focused terminal",
  "terminal.select_all": "Select all terminal text",
  "focus.toggle": "Toggle focus on current member",
  "view.launchers": "Open the Launchers review queue",
  "view.settings": "Open Settings",
};

export const DEFAULT_KEYMAP: Record<Action, string> = {
  palette: "Ctrl+Shift+P",
  "workspace.next": "Ctrl+Shift+]",
  "workspace.prev": "Ctrl+Shift+[",
  "mode.grid": "Ctrl+Shift+1",
  "mode.focus": "Ctrl+Shift+2",
  "mode.tabs": "Ctrl+Shift+3",
  "mode.master_stack": "Ctrl+Shift+4",
  "terminal.search": "Ctrl+Shift+F",
  "terminal.copy": "Ctrl+Shift+C",
  "terminal.paste": "Ctrl+Shift+V",
  "terminal.new": "Ctrl+Shift+T",
  "member.next": "Ctrl+Shift+ArrowRight",
  "member.prev": "Ctrl+Shift+ArrowLeft",
  "member.rename": "Ctrl+Shift+R",
  "terminal.select_all": "Ctrl+Shift+A",
  "focus.toggle": "Ctrl+Shift+Enter",
  "view.launchers": "Ctrl+Shift+L",
  "view.settings": "Ctrl+,",
};

export const ACTIONS = Object.keys(DEFAULT_KEYMAP) as Action[];

const CODE_KEYS: Record<string, string> = {
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
};

/** Layout-independent key name from a KeyboardEvent. */
export function keyName(event: Pick<KeyboardEvent, "code" | "key">): string {
  const code = event.code ?? "";
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad") && /\d$/.test(code)) return code.slice(-1);
  if (CODE_KEYS[code]) return CODE_KEYS[code];
  if (code) return code;
  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

export function bindingFor(
  event: Pick<KeyboardEvent, "ctrlKey" | "shiftKey" | "altKey" | "metaKey" | "code" | "key">
): string {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.shiftKey) parts.push("Shift");
  if (event.altKey) parts.push("Alt");
  if (event.metaKey) parts.push("Meta");
  parts.push(keyName(event));
  return parts.join("+");
}

export function effectiveKeymap(
  overrides: Record<string, string> | undefined
): Record<Action, string> {
  const map = { ...DEFAULT_KEYMAP };
  for (const [action, binding] of Object.entries(overrides ?? {})) {
    if ((ACTIONS as string[]).includes(action)) map[action as Action] = binding;
  }
  return map;
}

export function actionFor(
  event: Parameters<typeof bindingFor>[0],
  keymap: Record<Action, string>
): Action | undefined {
  if (!event.ctrlKey && !event.altKey && !event.metaKey) return undefined;
  const binding = bindingFor(event);
  return ACTIONS.find((a) => keymap[a] === binding);
}

/** Bindings that collide (returned as binding -> actions). */
export function conflicts(keymap: Record<Action, string>): Map<string, Action[]> {
  const by = new Map<string, Action[]>();
  for (const a of ACTIONS) by.set(keymap[a], [...(by.get(keymap[a]) ?? []), a]);
  return new Map([...by].filter(([, actions]) => actions.length > 1));
}
