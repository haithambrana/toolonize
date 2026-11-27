/** Running sessions across workspaces and the local audit log. */
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type AuditEntry, type ViewState } from "../lib/ipc";
import { closeSession, selectWorkspace, stopSession } from "../lib/actions";
import { processLabel } from "../lib/labels";
import { attempt, notify, useAppState } from "../state/store";

const VIEW_LABELS: Record<ViewState, string> = {
  attached: "On screen",
  hidden: "In the background",
  detached: "Not shown",
};

const ACTION_LABELS: Record<string, string> = {
  "session.start": "Terminal started",
  "session.restart": "Terminal restarted",
  "session.reconnect": "Reconnected",
  "member.authorize": "Member authorized",
  "member.reauthorize": "Member re-authorized",
  "external.launch": "App launched",
  "member.launch": "App launched",
  "registry.approve": "Launcher approved",
  "registry.revoke": "Approval revoked",
  "url.open": "Link opened",
  "ssh.hosts.read": "SSH hosts read",
  "scrollback.export": "Output saved",
  "workspace.import": "Workspace imported",
  "backup.export": "Backup saved",
  "backup.restore": "Backup restored",
};

export function ActivityScreen() {
  const sessions = useAppState((s) => s.sessions);
  const workspaces = useAppState((s) => s.workspaces);
  const registry = useAppState((s) => s.registry);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [outcome, setOutcome] = useState<"" | AuditEntry["outcome"]>("");
  const load = useCallback(async () => {
    const entries = await attempt(() => api.audit(500));
    if (entries) setAudit(entries);
  }, []);
  useEffect(() => {
    void load();
  }, [load, sessions]);
  const list = useMemo(
    () => Object.values(sessions).sort((a, b) => a.started_at_ms - b.started_at_ms),
    [sessions]
  );
  const wsName = useCallback(
    (id: string | null) => (id && workspaces.find((w) => w.id === id)?.name) || "—",
    [workspaces]
  );
  const memberName = useCallback(
    (id: string | null) => {
      if (!id) return null;
      const m = workspaces.flatMap((w) => w.members).find((x) => x.id === id);
      return m ? (m.title ?? m.status.launcher_name ?? null) : null;
    },
    [workspaces]
  );
  const launcherName = useCallback(
    (id: string | null) => {
      if (!id) return "—";
      const e = registry?.entries.find((x) => x.launcher_id === id);
      return e ? (e.decision.alias ?? e.display_name) : id;
    },
    [registry]
  );
  const rows = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return audit
      .map((a) => ({
        ...a,
        label: ACTION_LABELS[a.action] ?? a.action,
        what: memberName(a.member_id) ?? launcherName(a.launcher_id),
        where: wsName(a.workspace_id),
      }))
      .filter((a) => !outcome || a.outcome === outcome)
      .filter(
        (a) =>
          !q ||
          [a.label, a.action, a.what, a.where, a.detail ?? ""].some((t) =>
            t.toLowerCase().includes(q)
          )
      );
  }, [audit, filter, outcome, memberName, launcherName, wsName]);

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
              <th>Shown</th>
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
                <td>{VIEW_LABELS[s.view_state]}</td>
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
      <h2 className="subhead">Execution audit log</h2>
      <p className="muted small">
        Every start, launch, approval and authorization decision is recorded locally. Command
        arguments, environment and terminal output are never logged.
      </p>
      <div className="row-gap audit-tools">
        <input
          className="input input-sm grow"
          placeholder="Filter by action, terminal, workspace or detail"
          aria-label="Filter the audit log"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <select
          className="input input-sm"
          aria-label="Outcome"
          value={outcome}
          onChange={(e) => setOutcome(e.target.value as typeof outcome)}
        >
          <option value="">All outcomes</option>
          <option value="allowed">Allowed</option>
          <option value="denied">Denied</option>
          <option value="failed">Failed</option>
          <option value="recorded">Recorded</option>
        </select>
        <button type="button" className="btn btn-sm" onClick={() => void load()}>
          Refresh
        </button>
        <button
          type="button"
          className="btn btn-sm"
          onClick={async () => {
            const path = await attempt(() => api.exportAudit(), "Could not export the audit log");
            if (path) notify("info", `Audit log saved to ${path}`);
          }}
        >
          Export
        </button>
      </div>
      <table className="table">
        <thead>
          <tr>
            <th>Time</th>
            <th>Action</th>
            <th>Outcome</th>
            <th>Terminal or launcher</th>
            <th>Workspace</th>
            <th>Detail</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((a, i) => (
            <tr key={`${a.at}-${i}`}>
              <td className="small">{new Date(a.at).toLocaleString()}</td>
              <td title={a.action}>{a.label}</td>
              <td>
                <span className={`chip chip-outcome-${a.outcome}`}>{a.outcome}</span>
              </td>
              <td className="small break" title={a.launcher_id ?? undefined}>
                {a.what}
              </td>
              <td className="small">{a.where}</td>
              <td className="small">{a.detail ?? ""}</td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className="muted small">
                {audit.length ? "Nothing matches the filter." : "No entries yet."}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
