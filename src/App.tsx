import { useEffect, useState } from "react";
import { ping, type PingResponse } from "./lib/ipc";
// Spike is dev-only and jsdom-incompatible; lazy-load to avoid breaking Vitest
import { lazy, Suspense } from "react";
const TerminalSpike = lazy(() => import("./spike/TerminalSpike"));
const TerminalCore = lazy(() =>
  import("./terminal/TerminalCore").then((m) => ({ default: m.TerminalCore }))
);
const M3ReloadHarness = lazy(() => import("./m3-harness/M3ReloadHarness"));

type IpcState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; data: PingResponse }
  | { status: "error"; message: string };

export default function App() {
  const [ipc, setIpc] = useState<IpcState>({ status: "idle" });

  useEffect(() => {
    let cancelled = false;
    setIpc({ status: "loading" });
    ping()
      .then((data) => {
        if (!cancelled) setIpc({ status: "success", data });
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err);
        if (!cancelled) setIpc({ status: "error", message });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="shell">
      <header className="header">
        <h1 className="title">ToolOnize</h1>
        <p className="tagline">Your existing dev tools. One persistent workspace.</p>
        <span className="badge" aria-label="Milestone">
          Terminal Core — M3
        </span>
      </header>

      <main className="main">
        <section className="card" aria-labelledby="ipc-heading">
          <h2 id="ipc-heading" className="card-title">
            IPC Status
          </h2>

          {ipc.status === "loading" && (
            <p className="muted" role="status" aria-live="polite">
              Contacting Rust core…
            </p>
          )}

          {ipc.status === "error" && (
            <div className="alert alert-error" role="alert">
              <strong>IPC failed</strong>
              <p className="alert-message">{ipc.message}</p>
              <p className="muted small">The UI remains usable. Retry by reloading the window.</p>
            </div>
          )}

          {ipc.status === "success" && (
            <dl className="details">
              <div className="detail-row">
                <dt>Application</dt>
                <dd>{ipc.data.app_name}</dd>
              </div>
              <div className="detail-row">
                <dt>Version</dt>
                <dd>{ipc.data.app_version}</dd>
              </div>
              <div className="detail-row">
                <dt>Target OS</dt>
                <dd>{ipc.data.target_os}</dd>
              </div>
              <div className="detail-row">
                <dt>Architecture</dt>
                <dd>{ipc.data.target_arch}</dd>
              </div>
              <div className="detail-row">
                <dt>IPC status</dt>
                <dd>
                  <span className="status-ok">{ipc.data.status}</span>
                </dd>
              </div>
            </dl>
          )}

          {ipc.status === "idle" && <p className="muted">Waiting to contact backend…</p>}
        </section>

        <section className="card" aria-labelledby="scope-heading">
          <h2 id="scope-heading" className="card-title">
            Scope
          </h2>
          <p className="muted">
            M3 delivers the production terminal lifecycle core (portable-pty 0.9.0 + mitigations)
            with session manager, lossless transport, and xterm TerminalView. Workspace/layout (M4)
            and launcher discovery (M5/M6) remain planned.
          </p>
          <ul className="list">
            <li>
              IPC: ping +
              terminal_profiles/start/list/attach/detach/write/resize/ack/close/restart/poll/replay
            </li>
            <li>No filesystem, shell, HTTP, or process plugins; no raw exec from WebView</li>
            <li>Capability restricted to main window, local content only</li>
            <li>View vs process state orthogonal; renderer reload reattaches to same SessionId</li>
          </ul>
        </section>

        <section className="card" aria-labelledby="terminal-heading" style={{ padding: 16 }}>
          <h2 id="terminal-heading" className="card-title">
            Terminal Core — M3
          </h2>
          <Suspense fallback={<div className="muted small">Loading terminal core…</div>}>
            <TerminalCore />
          </Suspense>
        </section>

        {/* Throwaway M2 harness, included only in the dedicated spike build. */}
        {import.meta.env.VITE_M2_SPIKE === "1" && import.meta.env.MODE !== "test" && (
          <Suspense fallback={<div className="muted small">Loading spike harness…</div>}>
            <TerminalSpike />
          </Suspense>
        )}
        {import.meta.env.VITE_M3_RELOAD === "1" && import.meta.env.MODE !== "test" && (
          <Suspense fallback={<div className="muted small">Loading M3 harness…</div>}>
            <M3ReloadHarness />
          </Suspense>
        )}
      </main>

      <footer className="footer">
        <small className="muted">
          ToolOnize M3 — terminal core. No user data leaves this machine. Full app exit terminates
          local children; renderer reload survives. Portable-pty 0.9.0 + mitigations (DSR stateful,
          writer lifetime, lossless transport).
        </small>
      </footer>
    </div>
  );
}
