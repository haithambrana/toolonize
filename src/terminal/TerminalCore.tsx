import { useEffect, useState, useCallback } from "react";
import type { TerminalProfile, SessionInfo } from "./terminalTypes";
import {
  terminalProfiles,
  terminalStart,
  terminalList,
  terminalAttach,
  terminalDetach,
  terminalClose,
  terminalRestart,
} from "./terminalClient";
import { TerminalView } from "./TerminalView";
import "./terminal.css";

export function TerminalCore() {
  const [profiles, setProfiles] = useState<TerminalProfile[]>([]);
  const [profilesError, setProfilesError] = useState<string | null>(null);
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshList = useCallback(async () => {
    try {
      const list = await terminalList();
      setSessions(list);
      if (list.length > 0 && !selectedId) {
        setSelectedId(list[0].session_id);
      } else if (list.length === 0) {
        setSelectedId(null);
      } else if (selectedId && !list.find((s) => s.session_id === selectedId)) {
        // Selected session disappeared (closed)
        setSelectedId(list[0]?.session_id ?? null);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [selectedId]);

  useEffect(() => {
    let cancelled = false;
    terminalProfiles()
      .then((ps) => {
        if (cancelled) return;
        setProfiles(ps);
        const first = ps.find((p) => p.available);
        if (first) setSelectedProfile(first.id);
        else if (ps.length > 0) setSelectedProfile(ps[0].id);
      })
      .catch((e) => {
        if (!cancelled) setProfilesError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Poll list periodically (renderer reload reattachment)
  useEffect(() => {
    void refreshList();
    const id = window.setInterval(() => void refreshList(), 2000);
    return () => window.clearInterval(id);
  }, [refreshList]);

  const handleStart = useCallback(async () => {
    if (!selectedProfile) return;
    setLoading(true);
    setError(null);
    try {
      // Default dimensions — FitAddon will resize shortly after mount
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

  const handleAttach = async () => {
    if (!selectedSession) return;
    try {
      const s = await terminalAttach(selectedSession.session_id);
      setSessions((prev) => prev.map((x) => (x.session_id === s.session_id ? s : x)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDetach = async () => {
    if (!selectedSession) return;
    try {
      const s = await terminalDetach(selectedSession.session_id);
      setSessions((prev) => prev.map((x) => (x.session_id === s.session_id ? s : x)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleClose = async () => {
    if (!selectedSession) return;
    try {
      await terminalClose(selectedSession.session_id);
      await refreshList();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRestart = async () => {
    if (!selectedSession) return;
    try {
      const s = await terminalRestart(selectedSession.session_id);
      setSessions((prev) => prev.map((x) => (x.session_id === s.session_id ? s : x)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="terminal-core">
      <div className="terminal-core__header">
        <h2>Terminal Core — M3</h2>
        <span className="muted-small">
          portable-pty 0.9.0 + ToolOnize mitigations · {sessions.length} session(s)
        </span>
      </div>

      {profilesError && (
        <div className="alert alert-error" role="alert">
          Failed to load profiles: {profilesError}
        </div>
      )}

      <div className="card" style={{ padding: 12 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <label htmlFor="profile-select">Profile:</label>
          <select
            id="profile-select"
            value={selectedProfile}
            onChange={(e) => setSelectedProfile(e.target.value)}
            aria-label="Shell profile"
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id} disabled={!p.available}>
                {p.display_name} ({p.id}){p.available ? "" : " — unavailable"}
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={() => void handleStart()}
            disabled={loading || !selectedProfile}
          >
            {loading ? "Starting…" : "Start terminal"}
          </button>
          <button type="button" onClick={() => void refreshList()}>
            Refresh list
          </button>
        </div>
        {error && (
          <div className="alert alert-error" role="alert" style={{ marginTop: 8 }}>
            {error}
          </div>
        )}
        <p className="muted-small" style={{ marginTop: 8 }}>
          Frontend may only request opaque profile ids. Executable/argv construction stays
          Rust-side. No raw exec from WebView. DSR/CPR stateful handling, writer-lifetime guard, and
          bounded lossless transport with ack are active. Renderer reload reattaches via{" "}
          <code>terminal_list</code> + replay.
        </p>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "340px 1fr", gap: 12, minHeight: 380 }}>
        <div className="session-list" aria-label="Active sessions">
          <strong>Active sessions</strong>
          {sessions.length === 0 && (
            <span className="muted-small">No sessions yet. Start one above.</span>
          )}
          {sessions.map((s) => (
            <button
              key={s.session_id}
              type="button"
              className={`session-row ${s.session_id === selectedId ? "session-row--active" : ""}`}
              onClick={() => setSelectedId(s.session_id)}
              aria-pressed={s.session_id === selectedId}
            >
              <span className="session-row__id" title={s.session_id}>
                {s.session_id.slice(0, 16)}
              </span>
              <span style={{ fontSize: "0.85rem" }}>
                {s.profile_id} · {s.process_state.state} · {s.view_state} · gen {s.generation}
              </span>
            </button>
          ))}
          {selectedSession && (
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginTop: 8 }}>
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
          {selectedSession && (
            <dl className="details" style={{ marginTop: 8, fontSize: "0.85rem" }}>
              <div className="detail-row">
                <dt>Session ID</dt>
                <dd style={{ wordBreak: "break-all" }}>{selectedSession.session_id}</dd>
              </div>
              <div className="detail-row">
                <dt>Generation</dt>
                <dd>{selectedSession.generation}</dd>
              </div>
              <div className="detail-row">
                <dt>Process state</dt>
                <dd>{JSON.stringify(selectedSession.process_state)}</dd>
              </div>
              <div className="detail-row">
                <dt>View state</dt>
                <dd>{selectedSession.view_state}</dd>
              </div>
              <div className="detail-row">
                <dt>Size</dt>
                <dd>
                  {selectedSession.rows}×{selectedSession.cols}
                </dd>
              </div>
              <div className="detail-row">
                <dt>Transport</dt>
                <dd>{JSON.stringify(selectedSession.transport_state)}</dd>
              </div>
              {selectedSession.replay_truncated && (
                <dd style={{ color: "#ffdf5d" }}>Replay truncated</dd>
              )}
            </dl>
          )}
        </div>

        <div style={{ minHeight: 400 }}>
          {selectedSession ? (
            <TerminalView key={selectedSession.session_id} session={selectedSession} />
          ) : (
            <div className="card" style={{ padding: 24, textAlign: "center" }}>
              <p className="muted">Select or start a session to view its terminal.</p>
              <p className="muted-small">
                Renderer reload test: start a session, emit <code>BEFORE_RELOAD</code>, reload
                WebView, list should still show same SessionId/generation, reattach, receive{" "}
                <code>AFTER_RELOAD</code>.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="card" style={{ padding: 12 }}>
        <h3 style={{ margin: 0, fontSize: "0.95rem" }}>M3 notes</h3>
        <ul className="list" style={{ fontSize: "0.85rem" }}>
          <li>
            Transport: chunk 4096, capacity 65536, high 49152, low 16384, hard 65536, replay 65536 —
            bounded lossless, backpressure, sequence-acked, no silent drop.
          </li>
          <li>
            DSR/CPR: stateful detector across splits; CPR = ESC[&lt;rows&gt;;&lt;cols&gt;R (24;80
            fallback).
          </li>
          <li>
            Writer lifetime: owned by Rust session; detach/reload does not drop ConPTY input writer.
          </li>
          <li>
            View vs process: attach/detach/hide/show never mutates process state or generation
            (tested).
          </li>
          <li>
            Full app exit terminates local children; renderer reload survives (reattach via list).
          </li>
          <li>
            No FlexLayout yet — M4 owns docking. No launcher discovery, persistence, or workspace.
          </li>
        </ul>
      </div>
    </div>
  );
}
