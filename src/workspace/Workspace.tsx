import { useEffect, useRef, useState, useCallback } from "react";
import type { TerminalProfile, SessionInfo } from "../terminal/terminalTypes";
import {
  terminalProfiles,
  terminalStart,
  terminalList,
  terminalAttach,
  terminalDetach,
  terminalClose,
  terminalRestart,
} from "../terminal/terminalClient";
import { TerminalView } from "../terminal/TerminalView";
import "./workspace.css";

const MIN_SIDEBAR = 200;
const MAX_SIDEBAR = 520;
const DEFAULT_SIDEBAR = 300;
const MOBILE_BREAKPOINT = 720;

export function Workspace() {
  const [profiles, setProfiles] = useState<TerminalProfile[]>([]);
  const [profilesError, setProfilesError] = useState<string | null>(null);
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(DEFAULT_SIDEBAR);
  const [sidebarVisible, setSidebarVisible] = useState(true);
  const [isMobile, setIsMobile] = useState(false);
  const dragRef = useRef<{ startX: number; startWidth: number } | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const refreshList = useCallback(async () => {
    try {
      const list = await terminalList();
      setSessions(list);
      setSelectedId((cur) => {
        if (list.length === 0) return null;
        if (cur && list.find((s) => s.session_id === cur)) return cur;
        return list[0].session_id;
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    terminalProfiles()
      .then((ps) => {
        if (cancelled) return;
        setProfiles(ps);
        const first = ps.find((p) => p.available) ?? ps[0];
        if (first) setSelectedProfile(first.id);
      })
      .catch((e) => {
        if (!cancelled) setProfilesError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Poll list periodically (renderer reload reattachment discovered via list).
  useEffect(() => {
    void refreshList();
    const id = window.setInterval(() => void refreshList(), 2000);
    return () => window.clearInterval(id);
  }, [refreshList]);

  // Responsive breakpoint: collapse sidebar into a drawer on narrow widths.
  useEffect(() => {
    const media = window.matchMedia;
    if (typeof media !== "function") {
      setIsMobile(false);
      setSidebarVisible(true);
      return;
    }
    const mql = media(`(max-width: ${MOBILE_BREAKPOINT}px)`);
    const apply = () => {
      setIsMobile(mql.matches);
      setSidebarVisible(!mql.matches);
    };
    apply();
    mql.addEventListener("change", apply);
    return () => mql.removeEventListener("change", apply);
  }, []);

  const handleStart = useCallback(async () => {
    if (!selectedProfile) return;
    setLoading(true);
    setError(null);
    try {
      const sess = await terminalStart(selectedProfile, 24, 80);
      setSessions((prev) => [...prev, sess]);
      setSelectedId(sess.session_id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [selectedProfile]);

  const selectedSession = sessions.find((s) => s.session_id === selectedId) ?? null;

  const runForSelected = useCallback(
    async (op: (id: string) => Promise<SessionInfo>) => {
      if (!selectedSession) return;
      try {
        const s = await op(selectedSession.session_id);
        setSessions((prev) => prev.map((x) => (x.session_id === s.session_id ? s : x)));
        await refreshList();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [selectedSession, refreshList]
  );

  const handleAttach = () => runForSelected((id) => terminalAttach(id).then((a) => a.session));
  const handleDetach = () => runForSelected((id) => terminalDetach(id));
  const handleRestart = () => runForSelected((id) => terminalRestart(id));
  const handleClose = () => runForSelected((id) => terminalClose(id));

  // Sidebar drag-resize via pointer events.
  const onDragStart = (e: React.PointerEvent) => {
    dragRef.current = { startX: e.clientX, startWidth: sidebarWidth };
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onDragMove = (e: React.PointerEvent) => {
    if (!dragRef.current) return;
    const delta = e.clientX - dragRef.current.startX;
    const next = Math.min(MAX_SIDEBAR, Math.max(MIN_SIDEBAR, dragRef.current.startWidth + delta));
    setSidebarWidth(next);
  };
  const onDragEnd = () => {
    dragRef.current = null;
  };

  return (
    <div className="workspace" ref={rootRef}>
      <header className="workspace-toolbar">
        <div className="workspace-brand">
          <span className="workspace-logo" aria-hidden>
            ▦
          </span>
          <div className="workspace-title">
            <strong>ToolOnize</strong>
            <span className="workspace-subtitle">Workspace — M4</span>
          </div>
        </div>

        {!isMobile && (
          <button
            type="button"
            className="icon-button"
            onClick={() => setSidebarVisible((v) => !v)}
            title={sidebarVisible ? "Hide sessions" : "Show sessions"}
            aria-label={sidebarVisible ? "Hide session list" : "Show session list"}
            aria-pressed={sidebarVisible}
          >
            {sidebarVisible ? "◧" : "◨"}
          </button>
        )}

        <div className="workspace-actions">
          <label className="field-label" htmlFor="profile-select">
            Profile
          </label>
          <select
            id="profile-select"
            value={selectedProfile}
            onChange={(e) => setSelectedProfile(e.target.value)}
            aria-label="Shell profile"
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id} disabled={!p.available}>
                {p.display_name}
                {p.available ? "" : " (unavailable)"}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="button button--primary"
            onClick={() => void handleStart()}
            disabled={loading || !selectedProfile}
          >
            {loading ? "Starting…" : "Start terminal"}
          </button>
        </div>

        <div className="workspace-status">
          <span className="pill">
            {sessions.length} session{sessions.length === 1 ? "" : "s"}
          </span>
        </div>
      </header>

      {(profilesError || error) && (
        <div className="alert alert-error" role="alert" style={{ margin: "8px 12px 0" }}>
          {profilesError ? `Profiles: ${profilesError}` : error}
        </div>
      )}

      <main className="workspace-main">
        {(sidebarVisible || !isMobile) && (
          <section
            className="workspace-sidebar"
            style={{ width: sidebarWidth }}
            aria-label="Sessions"
          >
            <div className="sidebar-header">
              <strong>Active sessions</strong>
              <button
                type="button"
                className="icon-button"
                onClick={() => void refreshList()}
                title="Refresh"
              >
                ↻
              </button>
            </div>

            <div className="session-list">
              {sessions.length === 0 && (
                <span className="muted-small">No sessions yet. Start one above.</span>
              )}
              {sessions.map((s) => (
                <button
                  key={s.session_id}
                  type="button"
                  className={`session-row ${s.session_id === selectedId ? "session-row--active" : ""}`}
                  onClick={() => {
                    setSelectedId(s.session_id);
                    if (isMobile) setSidebarVisible(false);
                  }}
                  aria-pressed={s.session_id === selectedId}
                >
                  <span className="session-row__id" title={s.session_id}>
                    {s.session_id.slice(0, 12)}
                  </span>
                  <span className="session-row__meta">
                    {s.profile_id} · {s.process_state.state} · gen {s.generation}
                  </span>
                </button>
              ))}
            </div>

            {selectedSession && (
              <div className="selected-actions">
                <button type="button" onClick={() => void handleAttach()}>
                  Attach
                </button>
                <button type="button" onClick={() => void handleDetach()}>
                  Detach
                </button>
                <button type="button" onClick={() => void handleRestart()}>
                  Restart
                </button>
                <button type="button" onClick={() => void handleClose()}>
                  Close
                </button>
              </div>
            )}
          </section>
        )}

        {sidebarVisible && !isMobile && (
          <div
            className="workspace-divider"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize session list"
            onPointerDown={onDragStart}
            onPointerMove={onDragMove}
            onPointerUp={onDragEnd}
            onPointerCancel={onDragEnd}
          />
        )}

        <section className="workspace-pane" aria-label="Terminal">
          {selectedSession ? (
            <TerminalView key={selectedSession.session_id} session={selectedSession} />
          ) : (
            <div className="workspace-empty">
              <p className="muted">Select or start a session to view its terminal.</p>
              <p className="muted-small">
                Terminal state (process, scrollback) is preserved across layout changes; renderer
                reload reattaches to the same SessionId.
              </p>
            </div>
          )}
        </section>
      </main>

      <footer className="workspace-statusbar">
        <span className="status-left">
          {selectedSession ? (
            <>
              <code className="mono">{selectedSession.session_id.slice(0, 16)}</code>
              <span>·</span>
              <span>
                {selectedSession.profile_id} · {selectedSession.process_state.state} ·{" "}
                {selectedSession.view_state} · {selectedSession.rows}×{selectedSession.cols}
              </span>
              {selectedSession.replay_truncated && (
                <span className="pill pill--warn">replay truncated</span>
              )}
            </>
          ) : (
            <span className="muted">No session selected</span>
          )}
        </span>
        <span className="status-right muted">
          portable-pty 0.9.0 · bounded lossless transport · no fs/shell/http/process in WebView
        </span>
      </footer>
    </div>
  );
}
