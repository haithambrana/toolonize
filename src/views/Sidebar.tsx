/** Navigation: workspaces, Launchers review queue, Activity, Settings. */
import { useMemo, useRef } from "react";
import { api } from "../lib/ipc";
import { createWorkspace, importWorkspaceText, selectWorkspace, setView } from "../lib/actions";
import { attempt, notify, useAppState } from "../state/store";

function WatchBadge() {
  const watch = useAppState((s) => s.watch);
  if (watch.state === "live") {
    return (
      <span
        className="watch watch-live"
        title={`Watching ${watch.roots} launcher folder(s) for changes`}
      >
        ● Live updates
      </span>
    );
  }
  return (
    <button
      type="button"
      className="watch watch-off"
      title={watch.state === "degraded" ? watch.reason : "Live updates are off"}
      onClick={() => void attempt(() => api.rescan(), "Rescan failed")}
    >
      ○ {watch.state === "degraded" ? "Live updates degraded" : "Live updates off"} · Rescan
    </button>
  );
}

export function Sidebar() {
  const workspaces = useAppState((s) => s.workspaces);
  const active = useAppState((s) => s.active);
  const view = useAppState((s) => s.view);
  const registry = useAppState((s) => s.registry);
  const sessions = useAppState((s) => s.sessions);
  const fileInput = useRef<HTMLInputElement>(null);
  const reviewCount = useMemo(
    () => registry?.entries.filter((e) => e.review_pending && !e.decision.hidden).length ?? 0,
    [registry]
  );
  const running = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const s of Object.values(sessions)) {
      if (s.process_state.kind === "running")
        counts[s.workspace_id] = (counts[s.workspace_id] ?? 0) + 1;
    }
    return counts;
  }, [sessions]);

  return (
    <nav className="sidebar" aria-label="Main">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          ▣
        </span>
        <span className="brand-name">ToolOnize</span>
      </div>
      <div className="nav-section">
        <div className="nav-heading">
          <span>Workspaces</span>
          <button
            type="button"
            className="icon-btn"
            aria-label="New workspace"
            title="New workspace"
            onClick={() => void createWorkspace()}
          >
            +
          </button>
        </div>
        <ul className="nav-list">
          {workspaces.map((w) => (
            <li key={w.id}>
              <button
                type="button"
                className={
                  view === "workspace" && active === w.id ? "nav-item nav-item-active" : "nav-item"
                }
                aria-current={view === "workspace" && active === w.id ? "page" : undefined}
                onClick={() => void selectWorkspace(w.id)}
              >
                <span className="nav-label">{w.name}</span>
                {w.staged_import && <span className="chip chip-warn">review</span>}
                {running[w.id] ? (
                  <span className="count" title="Running terminals">
                    {running[w.id]}
                  </span>
                ) : null}
              </button>
            </li>
          ))}
          {workspaces.length === 0 && <li className="muted small nav-empty">No workspaces yet</li>}
        </ul>
        <button
          type="button"
          className="nav-item nav-subtle"
          onClick={() => fileInput.current?.click()}
        >
          Import workspace…
        </button>
        <input
          ref={fileInput}
          type="file"
          accept=".json,application/json"
          hidden
          onChange={async (e) => {
            const file = e.target.files?.[0];
            e.target.value = "";
            if (!file) return;
            if (file.size > 1024 * 1024) {
              notify("error", "Import file is too large");
              return;
            }
            await importWorkspaceText(await file.text());
          }}
        />
      </div>
      <div className="nav-section">
        <button
          type="button"
          className={view === "launchers" ? "nav-item nav-item-active" : "nav-item"}
          aria-current={view === "launchers" ? "page" : undefined}
          onClick={() => setView("launchers")}
        >
          <span className="nav-label">Launchers</span>
          {reviewCount > 0 && (
            <span className="count count-warn" title="Launchers waiting for review">
              {reviewCount}
            </span>
          )}
        </button>
        <button
          type="button"
          className={view === "activity" ? "nav-item nav-item-active" : "nav-item"}
          aria-current={view === "activity" ? "page" : undefined}
          onClick={() => setView("activity")}
        >
          <span className="nav-label">Activity</span>
        </button>
        <button
          type="button"
          className={view === "settings" ? "nav-item nav-item-active" : "nav-item"}
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => setView("settings")}
        >
          <span className="nav-label">Settings</span>
        </button>
      </div>
      <div className="sidebar-footer">
        <WatchBadge />
      </div>
    </nav>
  );
}
