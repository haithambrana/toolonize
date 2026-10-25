/** Running sessions across workspaces and the local audit log. */
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type AuditEntry } from "../lib/ipc";
import { closeSession, selectWorkspace, stopSession } from "../lib/actions";
import { processLabel } from "../lib/labels";
import { attempt, useAppState } from "../state/store";

export function ActivityScreen() {
  const sessions = useAppState((s) => s.sessions);
  const workspaces = useAppState((s) => s.workspaces);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const load = useCallback(async () => {
    const entries = await attempt(() => api.audit(200));
    if (entries) setAudit(entries);
  }, []);
  useEffect(() => {
    void load();
  }, [load, sessions]);
  const list = useMemo(
    () => Object.values(sessions).sort((a, b) => a.started_at_ms - b.started_at_ms),
    [sessions]
  );
  const wsName = (id: string) => workspaces.find((w) => w.id === id)?.name ?? "—";

  return (
    <section className="activity" aria-label="Activity">
      <header className="ws-header">
        <h1 className="ws-title">Activity</h1>
      </header>
      <h2 className="subhead">Terminal sessions</h2>
      {list.length === 0 ? (
        <p className="muted">
          No sessions. Terminals keep running while ToolOnize is open, even if the window reloads.
        </p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Terminal</th>
              <th>Workspace</th>
              <th>State</th>
              <th>View</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {list.map((s) => (
              <tr key={s.session_id}>
                <td>{s.title}</td>
                <td>
                  <button
                    type="button"
                    className="link"
                    onClick={() => void selectWorkspace(s.workspace_id)}
                  >
                    {wsName(s.workspace_id)}
                  </button>
                </td>
                <td>{processLabel(s)}</td>
                <td>{s.view_state}</td>
                <td className="row-gap">
                  {s.process_state.kind === "running" && (
                    <button
                      type="button"
                      className="btn btn-sm"
                      onClick={() => void stopSession(s.session_id, s.title)}
                    >
                      Stop
                    </button>
                  )}
                  <button
                    type="button"
                    className="btn btn-sm"
                    onClick={() => void closeSession(s.session_id)}
                  >
                    Close
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <h2 className="subhead">
        Execution audit log{" "}
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => void load()}>
          Refresh
        </button>
      </h2>
      <p className="muted small">
        Every start, launch, approval and authorization decision is recorded locally. Command
        arguments, environment and terminal output are never logged.
      </p>
      <table className="table">
        <thead>
          <tr>
            <th>Time</th>
            <th>Action</th>
            <th>Outcome</th>
            <th>Launcher</th>
            <th>Detail</th>
          </tr>
        </thead>
        <tbody>
          {audit.map((a, i) => (
            <tr key={`${a.at}-${i}`}>
              <td className="small">{new Date(a.at).toLocaleTimeString()}</td>
              <td className="mono small">{a.action}</td>
              <td>
                <span className={`chip chip-outcome-${a.outcome}`}>{a.outcome}</span>
              </td>
              <td className="mono small break">{a.launcher_id ?? "—"}</td>
              <td className="small">{a.detail ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
