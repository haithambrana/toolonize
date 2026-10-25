/** Local command palette (M9): actions, workspaces, members. ARIA combobox. */
import { useMemo, useState } from "react";
import { ACTION_LABELS, ACTIONS, effectiveKeymap } from "../lib/keymap";
import { paletteItems, runAction, type PaletteItem } from "../lib/commands";
import { setState, useAppState } from "../state/store";

export function CommandPalette() {
  const open = useAppState((s) => s.paletteOpen);
  if (!open) return null;
  return <PaletteBody />;
}

function PaletteBody() {
  const settings = useAppState((s) => s.settings);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const keymap = useMemo(() => effectiveKeymap(settings?.keymap), [settings]);
  const all: PaletteItem[] = useMemo(
    () => [
      ...ACTIONS.filter((a) => a !== "palette").map((a) => ({
        id: `action:${a}`,
        label: ACTION_LABELS[a],
        hint: keymap[a],
        run: () => runAction(a),
      })),
      ...paletteItems(),
    ],
    [keymap]
  );
  const q = query.trim().toLowerCase();
  const items = all.filter((i) => !q || i.label.toLowerCase().includes(q));
  const close = () => setState({ paletteOpen: false });
  const choose = (item: PaletteItem | undefined) => {
    if (!item) return;
    close();
    void item.run();
  };
  const active = Math.min(index, Math.max(0, items.length - 1));
  return (
    <div
      className="dialog-backdrop palette-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && close()}
    >
      <div className="palette" role="dialog" aria-modal="true" aria-label="Command palette">
        <input
          className="input palette-input"
          autoFocus
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={items[active] ? `palette-${active}` : undefined}
          placeholder="Type a command"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              close();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setIndex((i) => Math.min(i + 1, items.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setIndex((i) => Math.max(i - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              choose(items[active]);
            }
          }}
        />
        <ul id="palette-list" className="palette-list" role="listbox">
          {items.map((item, i) => (
            <li
              key={item.id}
              id={`palette-${i}`}
              role="option"
              aria-selected={i === active}
              className={i === active ? "palette-item palette-item-active" : "palette-item"}
              onMouseEnter={() => setIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                choose(item);
              }}
            >
              <span>{item.label}</span>
              {item.hint && <kbd>{item.hint}</kbd>}
            </li>
          ))}
          {items.length === 0 && <li className="palette-item muted">No matching command</li>}
        </ul>
      </div>
    </div>
  );
}
