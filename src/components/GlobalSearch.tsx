/** Search the scrollback of every open terminal at once (ARIA combobox). */
import { useEffect, useMemo, useState } from "react";
import { selectWorkspace, setView, memberTitle } from "../lib/actions";
import { focusMember } from "../lib/commands";
import { peekController, type BufferMatch } from "../terminal/controller";
import { getState, setState, useAppState } from "../state/store";
import { TagDot } from "../lib/colors";
import type { ColorTag } from "../lib/ipc";

const MAX_RESULTS = 300;
const PER_TERMINAL = 100;

interface Hit extends BufferMatch {
  sessionId: string;
  workspaceId: string;
  memberId: string;
  where: string;
  color: ColorTag | null;
}

export function GlobalSearch() {
  const open = useAppState((s) => s.globalSearchOpen);
  return open ? <SearchBody /> : null;
}

function excerpt(hit: Hit) {
  const start = Math.max(0, hit.index - 40);
  const before = (start > 0 ? "…" : "") + hit.line.slice(start, hit.index);
  const match = hit.line.slice(hit.index, hit.index + hit.length);
  const after = hit.line.slice(hit.index + hit.length, hit.index + hit.length + 60);
  return (
    <span className="mono small">
      {before}
      <mark>{match}</mark>
      {after}
    </span>
  );
}

function SearchBody() {
  const sessions = useAppState((s) => s.sessions);
  const workspaces = useAppState((s) => s.workspaces);
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [index, setIndex] = useState(0);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(query), 120);
    return () => clearTimeout(t);
  }, [query]);
  const hits = useMemo(() => {
    const out: Hit[] = [];
    if (!debounced) return out;
    for (const w of workspaces) {
      for (const m of w.members) {
        const session = Object.values(sessions).find(
          (x) => x.workspace_id === w.id && x.member_id === m.id
        );
        const c = session && peekController(session.session_id);
        if (!c) continue;
        for (const match of c.find(debounced, caseSensitive, PER_TERMINAL)) {
          out.push({
            ...match,
            sessionId: session.session_id,
            workspaceId: w.id,
            memberId: m.id,
            where: `${w.name} › ${memberTitle(m)}`,
            color: m.color ?? w.color,
          });
          if (out.length >= MAX_RESULTS) return out;
        }
      }
    }
    return out;
  }, [debounced, caseSensitive, sessions, workspaces]);
  const close = () => setState({ globalSearchOpen: false });
  const choose = async (hit: Hit | undefined) => {
    if (!hit) return;
    close();
    if (getState().active !== hit.workspaceId) await selectWorkspace(hit.workspaceId);
    setView("workspace");
    focusMember(hit.memberId);
    // Give the layout a moment to show the pane before scrolling it.
    setTimeout(() => {
      const c = peekController(hit.sessionId);
      c?.reveal(hit);
      c?.focus();
    }, 80);
  };
  const active = Math.min(index, Math.max(0, hits.length - 1));
  return (
    <div
      className="dialog-backdrop palette-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && close()}
    >
      <div className="palette" role="dialog" aria-modal="true" aria-label="Search all terminals">
        <div className="row-gap">
          <input
            className="input palette-input grow"
            autoFocus
            role="combobox"
            aria-expanded="true"
            aria-controls="global-search-list"
            aria-activedescendant={hits[active] ? `gs-${active}` : undefined}
            placeholder="Search the output of all open terminals"
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
                setIndex((i) => Math.min(i + 1, hits.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => Math.max(i - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                void choose(hits[active]);
              }
            }}
          />
          <label className="check-inline">
            <input
              type="checkbox"
              checked={caseSensitive}
              onChange={(e) => setCaseSensitive(e.target.checked)}
            />
            Aa
          </label>
        </div>
        <ul id="global-search-list" className="palette-list" role="listbox">
          {hits.map((hit, i) => (
            <li
              key={`${hit.sessionId}:${hit.row}:${hit.col}`}
              id={`gs-${i}`}
              role="option"
              aria-selected={i === active}
              className={i === active ? "palette-item palette-item-active" : "palette-item"}
              onMouseEnter={() => setIndex(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                void choose(hit);
              }}
            >
              <span className="search-hit">
                <span className="small muted">
                  <TagDot color={hit.color} /> {hit.where} · line {hit.row + 1}
                </span>
                {excerpt(hit)}
              </span>
            </li>
          ))}
          {debounced && hits.length === 0 && (
            <li className="palette-item muted">No match in the open terminals</li>
          )}
          {hits.length >= MAX_RESULTS && (
            <li className="palette-item muted small">
              Showing the first {MAX_RESULTS} matches; refine the search.
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}
