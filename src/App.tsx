import { lazy, Suspense } from "react";
// Spike/reload harnesses are special builds and jsdom-incompatible; lazy-load.
const Workspace = lazy(() =>
  import("./workspace/Workspace").then((m) => ({ default: m.Workspace }))
);
const TerminalSpike = lazy(() => import("./spike/TerminalSpike"));
const M3ReloadHarness = lazy(() => import("./m3-harness/M3ReloadHarness"));

export default function App() {
  // M2 spike and M3 reload builds are isolated (single consumer) harnesses.
  if (import.meta.env.VITE_M2_SPIKE === "1" && import.meta.env.MODE !== "test") {
    return (
      <Suspense fallback={<div className="muted">Loading spike harness…</div>}>
        <TerminalSpike />
      </Suspense>
    );
  }
  if (import.meta.env.VITE_M3_RELOAD === "1" && import.meta.env.MODE !== "test") {
    return (
      <Suspense fallback={<div className="muted">Loading M3 reload harness…</div>}>
        <M3ReloadHarness />
      </Suspense>
    );
  }

  // Production: M4 Workspace / Layout Core.
  return (
    <Suspense
      fallback={
        <div className="muted" style={{ padding: 24 }}>
          Loading workspace…
        </div>
      }
    >
      <Workspace />
    </Suspense>
  );
}
