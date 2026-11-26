/** Snippet picker: saved text pasted into the focused terminal like
 * clipboard text (the multi-line paste warning applies). Never executed by
 * ToolOnize itself. */
import { useState } from "react";
import { openDialog } from "./dialogs";
import type { Snippet } from "./ipc";
import type { TerminalController } from "../terminal/controller";
import { getState, notify, setState } from "../state/store";

function Picker({ snippets, close }: { snippets: Snippet[]; close: (s?: Snippet) => void }) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const q = query.trim().toLowerCase();
  const items = snippets.filter(
    (s) => !q || s.name.toLowerCase().includes(q) || s.text.toLowerCase().includes(q)
  );
  const active = Math.min(index, Math.max(0, items.length - 1));
  return (
    <div className="dialog-body">
      <input
        className="input"
        role="combobox"
        aria-expanded="true"
        aria-controls="snippet-list"
        aria-label="Find a snippet"
        placeholder="Find a snippet"
        data-autofocus
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setIndex(0);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setIndex((i) => Math.min(i + 1, items.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setIndex((i) => Math.max(i - 1, 0));
          } else if (e.key === "Enter" && items[active]) {
            e.preventDefault();
            close(items[active]);
          }
        }}
      />
      <ul id="snippet-list" className="palette-list" role="listbox">
        {items.map((s, i) => (
          <li
            key={s.name}
            role="option"
            aria-selected={i === active}
            className={i === active ? "palette-item palette-item-active" : "palette-item"}
            onMouseEnter={() => setIndex(i)}
            onMouseDown={(e) => {
              e.preventDefault();
              close(s);
            }}
          >
            <span className="search-hit">
              <span>{s.name}</span>
              <span className="mono small muted">{s.text.split("\n")[0].slice(0, 80)}</span>
            </span>
          </li>
        ))}
        {items.length === 0 && <li className="palette-item muted">No matching snippet</li>}
      </ul>
    </div>
  );
}

export async function snippetPicker(c: TerminalController): Promise<void> {
  const snippets = getState().settings?.snippets ?? [];
  if (snippets.length === 0) {
    notify("info", "No snippets yet: add them in Settings → Snippets");
    setState({ view: "settings" });
    return;
  }
  const chosen = await openDialog<Snippet>("Insert snippet", (close) => (
    <Picker snippets={snippets} close={close} />
  ));
  if (chosen) {
    await c.paste(chosen.text);
    c.focus();
  }
}
