import { lazy, Suspense, useEffect, useMemo } from "react";
import { CommandPalette } from "./components/CommandPalette";
import { DialogHost } from "./components/DialogHost";
import { GlobalSearch } from "./components/GlobalSearch";
import { TerminalMenu } from "./components/TerminalMenu";
import { runAction } from "./lib/commands";
import { actionFor, effectiveKeymap } from "./lib/keymap";
import { applyTheme } from "./lib/theme";
import { pruneControllers } from "./terminal/controller";
import { dismissNotice, init, setState, useAppState } from "./state/store";
import { Sidebar } from "./views/Sidebar";
import { WorkspaceScreen } from "./views/WorkspaceScreen";
import "@xterm/xterm/css/xterm.css";

// Throwaway M2 harness, included only in the dedicated spike build: with the
// flag unset the import is dead code and never reaches the bundle.
const SPIKE = import.meta.env.VITE_M2_SPIKE === "1" && import.meta.env.MODE !== "test";
const TerminalSpike = SPIKE ? lazy(() => import("./spike/TerminalSpike")) : null;

// Secondary screens load on first use; the workspace view stays in the
// main bundle so terminals appear without an extra round trip.
const LaunchersScreen = lazy(() =>
  import("./views/LaunchersScreen").then((m) => ({ default: m.LaunchersScreen }))
);
const SettingsScreen = lazy(() =>
  import("./views/SettingsScreen").then((m) => ({ default: m.SettingsScreen }))
);
const ActivityScreen = lazy(() =>
  import("./views/ActivityScreen").then((m) => ({ default: m.ActivityScreen }))
);

function Toasts() {
  const notices = useAppState((s) => s.notices);
  return (
    <div className="toasts" role="status" aria-live="polite">
      {notices.map((n) => (
        <div
          key={n.id}
          className={`toast toast-${n.level}`}
          role={n.level === "error" ? "alert" : undefined}
        >
          <span>{n.message}</span>
          <button
            type="button"
            className="icon-btn"
            aria-label="Dismiss"
            onClick={() => dismissNotice(n.id)}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

function useGlobalShortcuts() {
  const settings = useAppState((s) => s.settings);
  const keymap = useMemo(() => effectiveKeymap(settings?.keymap), [settings]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest?.(".kbd-capture")) return;
      const action = actionFor(event, keymap);
      if (!action) return;
      // Capture phase: handled before xterm.js sees the key.
      event.preventDefault();
      event.stopPropagation();
      void runAction(action);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [keymap]);
}

function useTheme() {
  const settings = useAppState((s) => s.settings);
  useEffect(() => {
    const update = () => setState({ resolvedTheme: applyTheme(settings) });
    update();
    if (typeof window.matchMedia !== "function") return;
    const queries = [
      "(prefers-color-scheme: light)",
      "(prefers-contrast: more)",
      "(forced-colors: active)",
      "(prefers-reduced-motion: reduce)",
    ].map((q) => window.matchMedia(q));
    queries.forEach((q) => q.addEventListener?.("change", update));
    return () => queries.forEach((q) => q.removeEventListener?.("change", update));
  }, [settings]);
}

function Shell() {
  const ready = useAppState((s) => s.ready);
  const fatal = useAppState((s) => s.fatal);
  const view = useAppState((s) => s.view);
  const sessions = useAppState((s) => s.sessions);
  useTheme();
  useGlobalShortcuts();
  useEffect(() => {
    void init();
  }, []);
  useEffect(() => {
    if (ready) pruneControllers(new Set(Object.keys(sessions)));
  }, [ready, sessions]);

  if (!ready) {
    return (
      <div className="splash" role="status" aria-live="polite">
        <span className="brand-mark" aria-hidden="true">
          ▣
        </span>{" "}
        Starting ToolOnize…
      </div>
    );
  }
  if (fatal) {
    return (
      <div className="splash">
        <div className="alert alert-danger" role="alert">
          <strong>ToolOnize could not reach its core.</strong>
          <p>{fatal}</p>
          <p className="muted small">
            Reload the window to retry. Running terminals are not affected by a reload.
          </p>
        </div>
      </div>
    );
  }
  return (
    <div className="app">
      <Sidebar />
      <main className="main" id="main">
        {view === "workspace" && <WorkspaceScreen />}
        <Suspense fallback={<div className="muted small screen-loading">Loading…</div>}>
          {view === "launchers" && <LaunchersScreen />}
          {view === "settings" && <SettingsScreen />}
          {view === "activity" && <ActivityScreen />}
        </Suspense>
      </main>
      <Toasts />
      <CommandPalette />
      <GlobalSearch />
      <TerminalMenu />
      <DialogHost />
    </div>
  );
}

export default function App() {
  if (TerminalSpike) {
    return (
      <Suspense fallback={<div className="muted small">Loading spike harness…</div>}>
        <TerminalSpike />
      </Suspense>
    );
  }
  return <Shell />;
}
