/**
 * TerminalView for one session: mounts the session's long-lived controller,
 * shows process state, the exit/disconnect banner, and scrollback search.
 */
import { useEffect, useRef, useState } from "react";
import type { MemberView, SessionInfo, WorkspaceView } from "../lib/ipc";
import {
  closeSession,
  memberTitle,
  reconnectSession,
  restartSession,
  renameMember,
  stopSession,
} from "../lib/actions";
import { processLabel } from "../lib/labels";
import { api } from "../lib/ipc";
import { TagDot } from "../lib/colors";
import { setState, useAppState } from "../state/store";
import { getController, peekController } from "./controller";
import { controllerOptions } from "./options";

function SearchBar({ sessionId }: { sessionId: string }) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [found, setFound] = useState<boolean | null>(null);
  const run = (backwards: boolean, incremental = false) => {
    const c = peekController(sessionId);
    if (!c || !query) {
      setFound(null);
      return;
    }
    const opts = { caseSensitive, incremental };
    setFound(backwards ? c.search.findPrevious(query, opts) : c.search.findNext(query, opts));
  };
  const close = () => {
    peekController(sessionId)?.search.clearDecorations();
    setState({ searchOpen: null });
    peekController(sessionId)?.focus();
  };
  return (
    <div className="term-search" role="search">
      <input
        className="input input-sm"
        placeholder="Search scrollback"
        aria-label="Search scrollback"
        autoFocus
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setFound(null);
        }}
        onKeyUp={(e) => {
          if (e.key.length === 1 || e.key === "Backspace") run(false, true);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            run(e.shiftKey);
          }
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
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
      <button
        type="button"
        className="btn btn-sm"
        onClick={() => run(true)}
        aria-label="Previous match"
      >
        ↑
      </button>
      <button
        type="button"
        className="btn btn-sm"
        onClick={() => run(false)}
        aria-label="Next match"
      >
        ↓
      </button>
      {found === false && (
        <span className="muted small" role="status">
          No match
        </span>
      )}
      <button type="button" className="btn btn-sm" onClick={close} aria-label="Close search">
        ✕
      </button>
    </div>
  );
}

/** Seconds left until `at`, refreshed every second. */
function useCountdown(at: number | undefined): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (at === undefined) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [at]);
  return at === undefined ? 0 : Math.max(0, Math.ceil((at - now) / 1000));
}

function ExitBanner({ info, member }: { info: SessionInfo; member: MemberView }) {
  const s = info.process_state;
  const retry = useAppState((st) => st.reconnecting[info.session_id]);
  const seconds = useCountdown(retry?.at);
  if (!["exited", "failed", "disconnected", "closed"].includes(s.kind)) return null;
  const remote = info.remote;
  if (retry && s.kind === "disconnected") {
    return (
      <div className="exit-banner exit-banner-warn" role="status">
        <span>
          <strong>Connection lost</strong>
          <span className="muted">
            {" "}
            — reconnecting {seconds > 0 ? `in ${seconds} s` : "now"} (attempt {retry.attempt} of{" "}
            {retry.max})
          </span>
        </span>
        <span className="exit-actions">
          <button
            type="button"
            className="btn btn-sm btn-primary"
            onClick={() => void reconnectSession(info.session_id, info.detach_expected)}
          >
            Reconnect now
          </button>
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => void api.cancelReconnect(info.session_id).catch(() => undefined)}
          >
            Stop reconnecting
          </button>
        </span>
      </div>
    );
  }
  return (
    <div
      className={`exit-banner ${s.kind === "exited" && s.code === 0 ? "" : "exit-banner-warn"}`}
      role="status"
    >
      <span>
        <strong>{processLabel(info)}</strong>
        {remote && s.kind === "disconnected" && (
          <span className="muted">
            {" "}
            —{" "}
            {info.detach_expected
              ? "your tmux/screen session keeps running on the remote side."
              : "plain SSH cannot resume the old remote shell."}
          </span>
        )}
      </span>
      <span className="exit-actions">
        {remote ? (
          <button
            type="button"
            className="btn btn-sm btn-primary"
            onClick={() => void reconnectSession(info.session_id, info.detach_expected)}
          >
            Reconnect
          </button>
        ) : (
          <button
            type="button"
            className="btn btn-sm btn-primary"
            onClick={() => void restartSession(info.session_id)}
          >
            Restart
          </button>
        )}
        <button
          type="button"
          className="btn btn-sm"
          onClick={() => void closeSession(info.session_id)}
        >
          Close
        </button>
        {member.auto_restart && <span className="muted small">Auto-restart on</span>}
      </span>
    </div>
  );
}

export function TerminalPane({
  workspace,
  member,
  session,
}: {
  workspace: WorkspaceView;
  member: MemberView;
  session: SessionInfo;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const settings = useAppState((s) => s.settings);
  const theme = useAppState((s) => s.resolvedTheme);
  const searchOpen = useAppState((s) => s.searchOpen === session.session_id);
  const isFocused = useAppState((s) => s.focused === member.id);
  const sessionId = session.session_id;
  const generation = useRef(session.generation);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const controller = getController(sessionId, () =>
      controllerOptions(settings, theme, { sessionId, memberId: member.id })
    );
    controller.mount(node);
    return () => controller.unmount(node);
    // Options are applied separately; remounting must not depend on them.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  useEffect(() => {
    if (!isFocused) return;
    const t = setTimeout(() => peekController(sessionId)?.focus(), 50);
    return () => clearTimeout(t);
  }, [sessionId, isFocused]);

  useEffect(() => {
    peekController(sessionId)?.applyOptions(controllerOptions(settings, theme));
  }, [sessionId, settings, theme]);

  useEffect(() => {
    if (session.generation > generation.current) peekController(sessionId)?.markRestart();
    generation.current = session.generation;
  }, [sessionId, session.generation]);

  const live = ["running", "starting"].includes(session.process_state.kind);
  return (
    <div
      className="term-pane"
      onFocusCapture={() => setState({ focused: member.id })}
      onMouseDown={() => setState({ focused: member.id })}
    >
      <div className={member.color ? `pane-header pane-tag tag-${member.color}` : "pane-header"}>
        <span className={`state-dot state-${session.process_state.kind}`} aria-hidden="true" />
        <TagDot color={member.color} />
        <button
          type="button"
          className="pane-title title-button"
          title="Double-click to rename"
          aria-label={`${memberTitle(member)} — double-click to rename`}
          onDoubleClick={() => void renameMember(workspace.id, member.id)}
        >
          {memberTitle(member)}
        </button>
        <span className="muted small">{processLabel(session)}</span>
        {session.detach_expected && (
          <span
            className="chip chip-info"
            title="Persistence comes from tmux/screen, not from ToolOnize"
          >
            tmux
          </span>
        )}
        <span className="pane-actions">
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={() => setState({ searchOpen: searchOpen ? null : sessionId })}
            aria-pressed={searchOpen}
            title="Search scrollback"
          >
            Search
          </button>
          {live ? (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => void stopSession(sessionId, memberTitle(member))}
            >
              Stop
            </button>
          ) : null}
        </span>
      </div>
      {searchOpen && <SearchBar sessionId={sessionId} />}
      <div
        ref={ref}
        className="term-container"
        data-session={sessionId}
        data-workspace={workspace.id}
        aria-label={`Terminal: ${memberTitle(member)}`}
      />
      <ExitBanner info={session} member={member} />
    </div>
  );
}
