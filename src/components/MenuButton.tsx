/** A button that opens a small ARIA menu (toolbar overflow). */
import { useEffect, useRef, useState, type ReactNode } from "react";

export interface MenuEntry {
  label: string;
  onSelect: () => void;
  checked?: boolean;
  danger?: boolean;
  /** Visual separator before this entry. */
  separator?: boolean;
}

export function MenuButton({
  label,
  entries,
  className = "btn btn-sm",
  children,
}: {
  label: string;
  entries: MenuEntry[];
  className?: string;
  children?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    wrap.current?.querySelector<HTMLButtonElement>('[role^="menuitem"]')?.focus();
    const close = (e: MouseEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", close, true);
    return () => window.removeEventListener("mousedown", close, true);
  }, [open]);
  const choose = (entry: MenuEntry) => {
    setOpen(false);
    button.current?.focus();
    entry.onSelect();
  };
  return (
    <div className="menu-wrap" ref={wrap}>
      <button
        ref={button}
        type="button"
        className={className}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        {children ?? label}
      </button>
      {open && (
        <div
          className="context-menu menu-dropdown"
          role="menu"
          aria-label={label}
          onKeyDown={(e) => {
            const items = [
              ...(wrap.current?.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]') ?? []),
            ];
            const i = items.indexOf(document.activeElement as HTMLButtonElement);
            if (e.key === "Escape") {
              e.preventDefault();
              setOpen(false);
              button.current?.focus();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              items[(i + 1) % items.length]?.focus();
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              items[(i - 1 + items.length) % items.length]?.focus();
            }
          }}
        >
          {entries.map((entry) => (
            <div key={entry.label}>
              {entry.separator && <hr className="menu-separator" />}
              <button
                type="button"
                role={entry.checked === undefined ? "menuitem" : "menuitemradio"}
                aria-checked={entry.checked}
                className={entry.danger ? "context-item context-danger" : "context-item"}
                onClick={() => choose(entry)}
              >
                <span>
                  {entry.checked !== undefined && (
                    <span className="menu-check" aria-hidden="true">
                      {entry.checked ? "●" : "○"}
                    </span>
                  )}
                  {entry.label}
                </span>
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
