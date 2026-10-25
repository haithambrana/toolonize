/** Right-click menu for terminals: clipboard, search, rename (ARIA menu). */
import { useEffect, useRef } from "react";
import { pasteInto } from "../lib/commands";
import { renameMember } from "../lib/actions";
import { peekController } from "../terminal/controller";
import { activeWorkspace, getState, notify, setState, useAppState } from "../state/store";

export function TerminalMenu() {
  const menu = useAppState((s) => s.termMenu);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menu) return;
    ref.current?.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus();
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setState({ termMenu: null });
    };
    window.addEventListener("mousedown", close, true);
    return () => window.removeEventListener("mousedown", close, true);
  }, [menu]);
  if (!menu) return null;
  const controller = peekController(menu.sessionId);
  const hasSelection = controller?.hasSelection() ?? false;
  const run = (fn: () => void | Promise<void>) => () => {
    setState({ termMenu: null });
    void Promise.resolve(fn()).finally(() => controller?.focus());
  };
  const items: { label: string; hint?: string; disabled?: boolean; action: () => void }[] = [
    {
      label: "Copy",
      hint: "Ctrl+Shift+C",
      disabled: !hasSelection,
      action: run(async () => {
        if (!(await controller?.copyToClipboard()))
          notify("error", "Could not copy to the clipboard");
      }),
    },
    {
      label: "Paste",
      hint: "Ctrl+Shift+V",
      action: run(() => controller && pasteInto(controller)),
    },
    { label: "Select all", hint: "Ctrl+Shift+A", action: run(() => controller?.selectAll()) },
    { label: "Clear screen", action: run(() => controller?.clearScreen()) },
    {
      label: "Find…",
      hint: "Ctrl+Shift+F",
      action: run(() => setState({ searchOpen: menu.sessionId })),
    },
    {
      label: "Rename terminal…",
      hint: "Ctrl+Shift+R",
      action: run(() => {
        const w = activeWorkspace(getState());
        if (w) return renameMember(w.id, menu.memberId);
      }),
    },
  ];
  const x = Math.min(menu.x, window.innerWidth - 230);
  const y = Math.min(menu.y, window.innerHeight - 260);
  return (
    <div
      ref={ref}
      className="context-menu"
      role="menu"
      aria-label="Terminal actions"
      style={{ left: x, top: y }}
      onKeyDown={(e) => {
        const buttons = [
          ...(ref.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? []),
        ];
        const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
        if (e.key === "Escape") {
          setState({ termMenu: null });
          controller?.focus();
        } else if (e.key === "ArrowDown") {
          e.preventDefault();
          buttons[(i + 1) % buttons.length]?.focus();
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          buttons[(i - 1 + buttons.length) % buttons.length]?.focus();
        }
      }}
    >
      {items.map((item) => (
        <button
          key={item.label}
          type="button"
          role="menuitem"
          className="context-item"
          disabled={item.disabled}
          onClick={item.action}
        >
          <span>{item.label}</span>
          {item.hint && <kbd>{item.hint}</kbd>}
        </button>
      ))}
    </div>
  );
}
