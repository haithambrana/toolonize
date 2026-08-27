import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { runPtyPipelineSpike, testResize, testInputEcho } from "./spike";

type SpikeState =
  | { status: "idle" }
  | { status: "running"; step: string }
  | { status: "success"; report: string }
  | { status: "error"; message: string };

export default function TerminalSpike() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [state, setState] = useState<SpikeState>({ status: "idle" });
  const [bytes, setBytes] = useState(0);

  useEffect(() => {
    if (!containerRef.current) return;
    // In jsdom (Vitest) xterm's CoreBrowserService expects window.matchMedia on parentWindow;
    // that is not fully implemented in jsdom, so we guard and fallback to no-op in test.
    if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
      // Provide a minimal mock for xterm in jsdom
      (window as unknown as { matchMedia: (q: string) => MediaQueryList }).matchMedia = (() =>
        ({
          matches: false,
          media: "",
          onchange: null,
          addListener: () => {},
          removeListener: () => {},
          addEventListener: () => {},
          removeEventListener: () => {},
          dispatchEvent: () => false,
        }) as unknown as MediaQueryList) as unknown as typeof window.matchMedia;
    }
    let term: Terminal | null = null;
    let fit: FitAddon | null = null;
    try {
      term = new Terminal({
        cursorBlink: true,
        fontFamily: "monospace",
        fontSize: 12,
        convertEol: true,
      });
      fit = new FitAddon();
      term.loadAddon(fit);
      term.open(containerRef.current);
      fit.fit();
      termRef.current = term;
      fitRef.current = fit;
    } catch {
      // In headless test, just create a mock terminal that buffers writes
      term = null;
      fit = null;
    }

    const onResize = () => {
      try {
        fit?.fit();
      } catch {
        // ignore resize fit error in headless
        void 0;
      }
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      try {
        term?.dispose();
      } catch {
        // ignore dispose error
        void 0;
      }
    };
  }, []);

  const run = async () => {
    const auto = new URLSearchParams(window.location.search).has("spikeAuto");
    try {
      const term = termRef.current;
      if (!term) throw new Error("xterm.js failed to initialize in the real WebView");
      term.clear();
      setBytes(0);
      setState({
        status: "running",
        step: "PTY -> Rust -> Tauri Channel -> WebView -> xterm.js",
      });
      const pipeline = await runPtyPipelineSpike(
        (data) => new Promise<void>((resolve) => term.write(data, resolve)),
        (n) => setBytes((prev) => prev + n),
        256 * 1024
      );
      term.writeln(`\r\n[Spike] ${pipeline.details}`);
      if (!pipeline.exactByteIntegrity || !pipeline.xtermWriteCompleted) {
        throw new Error(`exact pipeline validation failed: ${pipeline.details}`);
      }

      setState({ status: "running", step: "resize pipeline 24x80 -> 40x120" });
      const resize = await testResize(40, 120);
      const resizeOk =
        resize.requestedRows === 40 &&
        resize.requestedCols === 120 &&
        resize.observedRows === 40 &&
        resize.observedCols === 120 &&
        resize.processExitCode === 0;
      if (!resizeOk) throw new Error(`child-observed resize failed: ${JSON.stringify(resize)}`);
      term.writeln(`[Spike] resize: ${JSON.stringify(resize)}`);
      await testResize(24, 80);

      setState({ status: "running", step: "input return path WebView -> Rust -> PTY" });
      const input = "hello from WebView";
      const echo = await testInputEcho(input);
      const inputOk = echo.echoed === input && echo.processExitCode === 0;
      if (!inputOk) throw new Error(`input return failed: ${JSON.stringify(echo)}`);
      term.writeln(`[Spike] input echo: ${JSON.stringify(echo)}`);

      const report = {
        payloadBytes: pipeline.payloadBytes,
        deliveredPayloadBytes: pipeline.deliveredPayloadBytes,
        expectedSha256: pipeline.expectedSha256,
        deliveredSha256: pipeline.deliveredSha256,
        exactByteIntegrity: pipeline.exactByteIntegrity,
        xtermWriteCompleted: pipeline.xtermWriteCompleted,
        inputReturn: inputOk,
        realResize: resizeOk,
        processExitCode: pipeline.processExitCode,
      };
      setState({
        status: "success",
        report: JSON.stringify(report),
      });
      if (auto) {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("spike_complete", { report });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      termRef.current?.writeln(`\r\n[Spike] ERROR: ${msg}`);
      setState({ status: "error", message: msg });
      if (auto) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("spike_fail", { message: msg });
        } catch (invokeError) {
          console.error("M2_REAL_WEBVIEW_FAILURE", msg, invokeError);
        }
      }
    }
  };

  // Auto-run for CI when ?spikeAuto=1 is present (real WebView pipeline)
  useEffect(() => {
    if (new URLSearchParams(window.location.search).has("spikeAuto")) {
      const t = setTimeout(() => run(), 500);
      return () => clearTimeout(t);
    }
  }, []);

  return (
    <section className="card" aria-labelledby="spike-heading">
      <h2 id="spike-heading" className="card-title">
        M2 PTY Spike — Full Pipeline
      </h2>
      <p className="muted small">
        Throwaway harness: PTY → Rust → Tauri Channel → WebView → xterm.js. Verifies lossless
        transport, resize propagation, and input echo.
      </p>
      <div style={{ display: "flex", gap: "0.5rem", marginBottom: "0.5rem" }}>
        <button onClick={run} disabled={state.status === "running"}>
          Run Spike
        </button>
        <span className="muted small">Bytes delivered: {bytes}</span>
        {state.status === "running" && <span className="muted">Running: {state.step}…</span>}
        {state.status === "success" && <span className="status-ok">PASS: {state.report}</span>}
        {state.status === "error" && <span className="alert">FAIL: {state.message}</span>}
      </div>
      <div
        ref={containerRef}
        style={{
          width: "100%",
          height: "300px",
          background: "#1e1e1e",
          border: "1px solid #333",
          borderRadius: "4px",
          padding: "4px",
        }}
      />
      <p className="muted small">
        This harness is behind the <code>spike</code> feature and is not part of the M1 product
        surface. It is used for M2 evidence and Windows CI validation.
      </p>
    </section>
  );
}
