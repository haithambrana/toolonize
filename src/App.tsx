import { useEffect, useState } from "react";
import { ping, type PingResponse } from "./lib/ipc";
// Spike is dev-only and jsdom-incompatible; lazy-load to avoid breaking Vitest
import { lazy, Suspense } from "react";
const TerminalSpike = lazy(() => import("./spike/TerminalSpike"));

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
          Framework Shell — M1
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
            M1 proves the cross-platform shell and hardened IPC boundary. Terminal, workspace,
            launcher, PTY, layout, SSH, tmux and persistence features are planned for later
            milestones and are not present in this build.
          </p>
          <ul className="list">
            <li>Single custom command: app::ping (Tauri invoke: ping)</li>
            <li>No filesystem, shell, HTTP, or process plugins</li>
            <li>Capability restricted to main window, local content only</li>
          </ul>
        </section>

        {/* M2 Spike harness - throwaway, not product. Visible in dev for evidence, not in tests. */}
        {import.meta.env.DEV && import.meta.env.MODE !== "test" && (
          <Suspense fallback={<div className="muted small">Loading spike harness…</div>}>
            <TerminalSpike />
          </Suspense>
        )}
      </main>

      <footer className="footer">
        <small className="muted">
          ToolOnize M1 — framework shell only. No user data leaves this machine.
        </small>
      </footer>
    </div>
  );
}
