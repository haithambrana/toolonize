/**
 * Pane chrome in the layout's own title row (one row per pane instead of a
 * tab strip plus a second header): the tab shows the state dot, color tag
 * and name; the right side shows status chips and the pane's actions.
 */
import { memberTitle, stopSession } from "../lib/actions";
import { TagDot } from "../lib/colors";
import { processLabel } from "../lib/labels";
import { MemberNoteButton } from "../components/MemberNote";
import { sessionFor, setState, useAppState } from "../state/store";
import { t } from "../i18n";

function useMemberAndSession(workspaceId: string, memberId: string) {
  const member = useAppState((s) =>
    s.workspaces.find((w) => w.id === workspaceId)?.members.find((m) => m.id === memberId)
  );
  const session = useAppState((s) => sessionFor(s, workspaceId, memberId));
  return { member, session };
}

export function TabLeading({ workspaceId, memberId }: { workspaceId: string; memberId: string }) {
  const { member, session } = useMemberAndSession(workspaceId, memberId);
  if (!member) return null;
  return (
    <span className="tab-leading">
      <span
        className={`state-dot state-${session?.process_state.kind ?? "new"}`}
        aria-hidden="true"
      />
      <TagDot color={member.color} />
    </span>
  );
}

export function PaneActions({ workspaceId, memberId }: { workspaceId: string; memberId: string }) {
  const { member, session } = useMemberAndSession(workspaceId, memberId);
  const cwd = useAppState((s) => (session ? s.cwd[session.session_id] : undefined));
  const broadcasting = useAppState((s) => !!s.broadcast[workspaceId]);
  const searchOpen = useAppState((s) => !!session && s.searchOpen === session.session_id);
  if (!member) return null;
  const live = !!session && ["running", "starting"].includes(session.process_state.kind);
  return (
    <span className="pane-actions" onMouseDown={(e) => e.stopPropagation()}>
      {session && <span className="muted small pane-state">{processLabel(session)}</span>}
      {cwd && (
        <span
          className="pane-cwd muted small"
          title={cwd.host ? `${cwd.host}:${cwd.path}` : cwd.path}
        >
          {cwd.path.split(/[\\/]/).filter(Boolean).pop() ?? cwd.path}
        </span>
      )}
      {member.zoom !== 0 && (
        <span className="chip chip-info" title="Ctrl+0 resets the zoom">
          {member.zoom > 0 ? `+${member.zoom}` : member.zoom}
        </span>
      )}
      {broadcasting && live && (
        <span className="chip chip-warn" title={t("Typing goes to every running terminal here")}>
          {t("Broadcast")}
        </span>
      )}
      {session?.detach_expected && (
        <span
          className="chip chip-info"
          title={t("Persistence comes from tmux/screen, not from ToolOnize")}
        >
          tmux
        </span>
      )}
      <MemberNoteButton workspaceId={workspaceId} memberId={memberId} />
      {session && (
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={() => setState({ searchOpen: searchOpen ? null : session.session_id })}
          aria-pressed={searchOpen}
          title={t("Search scrollback")}
        >
          {t("Search")}
        </button>
      )}
      {session && live && (
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={() => void stopSession(session.session_id, memberTitle(member))}
        >
          {t("Stop")}
        </button>
      )}
    </span>
  );
}
