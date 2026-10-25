/** Content of one workspace tab: a terminal, an external app card, or a
 * status card explaining why the member cannot run yet. */
import type { MemberView, WorkspaceView } from "../lib/ipc";
import { reviewMember } from "../lib/review";
import { launchMember, memberTitle, removeMember, renameMember, startMember } from "../lib/actions";
import { TerminalPane } from "../terminal/TerminalPane";
import { sessionFor, setState, useAppState } from "../state/store";

const STATE_TEXT: Record<string, string> = {
  changed: "Changed — re-review required",
  not_authorized: "Not authorized yet",
  orphaned: "Launcher missing",
  blocked: "Needs review",
  unavailable: "Unavailable",
};

function StatusCard({ w, m }: { w: WorkspaceView; m: MemberView }) {
  return (
    <div className="member-card">
      <h3 className="member-card-title">{memberTitle(m)}</h3>
      <p className="chip chip-warn">{STATE_TEXT[m.status.state] ?? m.status.state}</p>
      {m.status.reason && <p className="muted">{m.status.reason}</p>}
      <div className="row-gap">
        <button type="button" className="btn btn-primary" onClick={() => void reviewMember(w, m)}>
          Review…
        </button>
        <button type="button" className="btn" onClick={() => void renameMember(w.id, m.id)}>
          Rename
        </button>
        <button type="button" className="btn" onClick={() => void removeMember(w, m)}>
          Remove
        </button>
      </div>
    </div>
  );
}

function ExternalCard({ w, m }: { w: WorkspaceView; m: MemberView }) {
  const launched = useAppState((s) => s.launched[m.id]);
  return (
    <div className="member-card">
      <h3 className="member-card-title">{memberTitle(m)}</h3>
      <p className="muted">
        External application{m.origin_label ? ` · ${m.origin_label}` : ""}. It opens as a normal
        window outside ToolOnize; ToolOnize does not manage its windows.
      </p>
      <div className="row-gap">
        <button type="button" className="btn btn-primary" onClick={() => void launchMember(w, m)}>
          Launch
        </button>
        <button type="button" className="btn" onClick={() => void renameMember(w.id, m.id)}>
          Rename
        </button>
        <button type="button" className="btn" onClick={() => void removeMember(w, m)}>
          Remove
        </button>
      </div>
      {launched && (
        <p className="muted small" role="status">
          Launched at {new Date(launched).toLocaleTimeString()}
        </p>
      )}
    </div>
  );
}

function IdleCard({ w, m }: { w: WorkspaceView; m: MemberView }) {
  return (
    <div className="member-card">
      <h3 className="member-card-title">{memberTitle(m)}</h3>
      <p className="muted">
        {m.member_type === "remote_command"
          ? "Remote session through your own SSH configuration."
          : "Not running."}
        {m.detach_expected && " Persistence comes from tmux/screen."}
      </p>
      <div className="row-gap">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void startMember(w, m)}
          data-testid="start-member"
        >
          Start
        </button>
        <button type="button" className="btn" onClick={() => void renameMember(w.id, m.id)}>
          Rename
        </button>
        <button type="button" className="btn" onClick={() => void removeMember(w, m)}>
          Remove
        </button>
      </div>
    </div>
  );
}

export function MemberPane({ workspaceId, memberId }: { workspaceId: string; memberId: string }) {
  const w = useAppState((s) => s.workspaces.find((x) => x.id === workspaceId));
  const session = useAppState((s) => sessionFor(s, workspaceId, memberId));
  const m = w?.members.find((x) => x.id === memberId);
  if (!w || !m) return <div className="member-card muted">This member was removed.</div>;
  if (session) return <TerminalPane workspace={w} member={m} session={session} />;
  return (
    <div className="member-pane" onMouseDown={() => setState({ focused: m.id })}>
      {m.status.state !== "ready" ? (
        <StatusCard w={w} m={m} />
      ) : m.member_type === "external_app" ? (
        <ExternalCard w={w} m={m} />
      ) : (
        <IdleCard w={w} m={m} />
      )}
    </div>
  );
}
