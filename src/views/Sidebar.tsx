/** Navigation: workspaces, Launchers review queue, Activity, Settings. */
import { useMemo, useRef } from "react";
import { TagDot } from "../lib/colors";
import { api } from "../lib/ipc";
import { createWorkspace, importWorkspaceText, selectWorkspace, setView } from "../lib/actions";
import { newFromTemplate } from "../lib/backup";
import { readFileText } from "../lib/files";
import { formatProgress, installUpdate } from "../lib/updates";
import { attempt, notify, useAppState } from "../state/store";
import { t } from "../i18n";

function UpdateBadge() {
  const update = useAppState((s) => s.update);
  if (update.installing) {
    return (
      <p className="update-badge" role="status">
        {formatProgress(update.progress)}
      </p>
    );
  }
  if (!update.status?.available || !update.status.version) return null;
  return (
    <button
      type="button"
      className="update-badge update-badge-ready"
      onClick={() => void installUpdate()}
    >
      {t("Update to {version}", { version: update.status.version })}
    </button>
  );
}

function WatchBadge() {
  const watch = useAppState((s) => s.watch);
  if (watch.state === "live") {
    return (
      <span
        className="watch watch-live"
        title={`Watching ${watch.roots} launcher folder(s) for changes`}
      >
        {t("● Live updates")}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="watch watch-off"
      title={watch.state === "degraded" ? watch.reason : t("Live updates are off")}
      onClick={() => void attempt(() => api.rescan(), t("Rescan failed"))}
    >
      ○{" "}
      {watch.state === "degraded"
        ? t("Live updates degraded · Rescan")
        : t("Live updates off · Rescan")}
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
    <nav className="sidebar" aria-label={t("Main")}>
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          ▣
        </span>
        <span className="brand-name">{t("ToolOnize")}</span>
      </div>
      <div className="nav-section">
        <div className="nav-heading">
          <span>{t("Workspaces")}</span>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("New workspace")}
            title={t("New workspace")}
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
                <TagDot color={w.color} />
                <span className="nav-label">{w.name}</span>
                {w.staged_import && <span className="chip chip-warn">review</span>}
                {running[w.id] ? (
                  <span className="count" title={t("Running terminals")}>
                    {running[w.id]}
                  </span>
                ) : null}
              </button>
            </li>
          ))}
          {workspaces.length === 0 && (
            <li className="muted small nav-empty">{t("No workspaces yet")}</li>
          )}
        </ul>
        <button
          type="button"
          className="nav-item nav-subtle"
          onClick={() => fileInput.current?.click()}
        >
          {t("Import workspace…")}
        </button>
        <button
          type="button"
          className="nav-item nav-subtle"
          onClick={() => void newFromTemplate()}
        >
          {t("New from template…")}
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
              notify("error", t("Import file is too large"));
              return;
            }
            await importWorkspaceText(await readFileText(file));
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
          <span className="nav-label">{t("Launchers")}</span>
          {reviewCount > 0 && (
            <span className="count count-warn" title={t("Launchers waiting for review")}>
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
          <span className="nav-label">{t("Activity")}</span>
        </button>
        <button
          type="button"
          className={view === "settings" ? "nav-item nav-item-active" : "nav-item"}
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => setView("settings")}
        >
          <span className="nav-label">{t("Settings")}</span>
        </button>
      </div>
      <div className="sidebar-footer">
        <UpdateBadge />
        <WatchBadge />
      </div>
    </nav>
  );
}
