import { useEffect, useRef, useState, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import "@xterm/xterm/css/xterm.css";
import "./terminal.css";
import type { SessionInfo } from "./terminalTypes";
import {
  terminalWrite,
  terminalResize,
  terminalAck,
  terminalPoll,
  terminalReplay,
  terminalAttach,
} from "./terminalClient";

type Props = {
  session: SessionInfo;
  onExit?: (s: SessionInfo) => void;
};

const SCROLLBACK = 5000;
const POLL_INTERVAL_MS = 80;
const RESIZE_DEBOUNCE_MS = 150;

function isExitState(s: SessionInfo): boolean {
  const st = s.process_state as { state: string; exit_code?: number };
  return st.state === "exited" || st.state === "failed" || st.state === "closed";
}

export function TerminalView({ session }: Props) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const pendingAck = useRef<Map<number, boolean>>(new Map());
  const searchQuery = useRef<string>("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);
  const [replayTruncated, setReplayTruncated] = useState(session.replay_truncated);
  const sessionIdRef = useRef(session.session_id);
  const generationRef = useRef(session.generation);
  const expectedSeqRef = useRef<number>(0);
  const epochRef = useRef<number>(0);
  const pollInFlightRef = useRef<boolean>(false);
  const cancelledRef = useRef<boolean>(false);
  const generationBannerRef = useRef<string | null>(null);
  const [generationBanner, setGenerationBanner] = useState<string | null>(null);

  useEffect(() => {
    sessionIdRef.current = session.session_id;
    // H10: generation change must invalidate old delivery state
    if (generationRef.current !== session.generation) {
      const oldGen = generationRef.current;
      const newGen = session.generation;
      generationRef.current = newGen;
      // Invalidate pending ACKs from prior generation
      pendingAck.current.clear();
      // Reset expected sequence for new generation (new Transport starts at 0)
      expectedSeqRef.current = 0;
      epochRef.current = 0;
      const msg = `[restart] generation ${oldGen} -> ${newGen} — new stream cursor 0`;
      generationBannerRef.current = msg;
      setGenerationBanner(msg);
      const term = termRef.current;
      if (term) {
        // H10: explicit xterm behavior on generation change — clear stale content and show banner
        term.clear();
        term.writeln(`\r\n${msg}`);
      }
    }
    setReplayTruncated(session.replay_truncated);
  }, [session.session_id, session.generation, session.replay_truncated]);

  const getDimensions = useCallback(() => {
    const fit = fitRef.current;
    const term = termRef.current;
    if (!fit || !term) return null;
    try {
      fit.fit();
    } catch {
      return null;
    }
    return { cols: term.cols, rows: term.rows };
  }, []);

  const handleResize = useCallback(async () => {
    const dims = getDimensions();
    if (!dims) return;
    if (dims.rows === 0 || dims.cols === 0) return;
    if (dims.rows > 500 || dims.cols > 1000) return;
    try {
      await terminalResize(sessionIdRef.current, dims.rows, dims.cols);
    } catch {
      // Resize failures are non-fatal
    }
  }, [getDimensions]);

  // Create one Terminal instance per mounted TerminalView
  useEffect(() => {
    if (!containerRef.current) return;
    cancelledRef.current = false;
    pollInFlightRef.current = false;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "block",
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      fontSize: 13,
      lineHeight: 1.1,
      scrollback: SCROLLBACK,
      allowTransparency: false,
      convertEol: false,
    });

    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);

    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;

    term.open(containerRef.current);
    try {
      fit.fit();
    } catch {
      // ignore
    }

    const dispData = term.onData((data: string) => {
      const enc = new TextEncoder();
      const bytes = enc.encode(data);
      terminalWrite(sessionIdRef.current, bytes).catch(() => {});
    });

    // H14B: intercept native paste (keyboard Ctrl+V, context-menu paste) at the
    // container in CAPTURE phase BEFORE xterm's own paste handler consumes it.
    // preventDefault() stops xterm's default paste path so the clipboard data is
    // not sent twice; we route the text through the SAME paste policy.
    const containerElement = containerRef.current;
    const pasteHandler = (e: ClipboardEvent) => {
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (!text) return;
      e.preventDefault();
      pasteText(text);
    };
    containerElement.addEventListener("paste", pasteHandler, true);

    let resizeTimer: number | null = null;
    const ro = new ResizeObserver(() => {
      if (resizeTimer) window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        void handleResize();
      }, RESIZE_DEBOUNCE_MS);
    });
    ro.observe(containerRef.current);

    // H2/H3: establish correct delivery cursor via attach handshake
    // H5: fetch bounded replay with truncation flag
    let pollTimer: number | null = null;
    let gapNotified = false;

    const initAndPoll = async () => {
      try {
        // Attach to get cursor: epoch + next_sequence (H2)
        const attach = await terminalAttach(sessionIdRef.current);
        if (cancelledRef.current) return;
        epochRef.current = attach.attachment_epoch;
        expectedSeqRef.current = attach.next_sequence;
        if (attach.replay_truncated) setReplayTruncated(true);

        // Replay bounded history
        const replay = await terminalReplay(sessionIdRef.current);
        if (cancelledRef.current) return;
        if (replay.bytes.length > 0) {
          const chunk = new Uint8Array(replay.bytes);
          term.write(chunk);
        }
        if (replay.truncated) setReplayTruncated(true);
      } catch {
        // If attach/replay fails, fallback to 0 and continue polling
        if (!cancelledRef.current) {
          expectedSeqRef.current = 0;
        }
      }

      // H4: strictly serialized poll loop (no overlapping setInterval)
      const pollOnce = async () => {
        if (cancelledRef.current) return;
        if (pollInFlightRef.current) return; // guard ensures max 1 concurrent
        pollInFlightRef.current = true;
        try {
          const { chunks, replayTruncated } = await terminalPoll(sessionIdRef.current, 16);
          if (cancelledRef.current) return;
          if (replayTruncated) setReplayTruncated(true);
          chunks.sort((a, b) => a.sequence - b.sequence);
          for (const ch of chunks) {
            if (cancelledRef.current) break;
            // H10: generation check — stale generation chunks are ignored safely
            if (ch.generation !== generationRef.current) {
              continue;
            }
            // H3: epoch check via sequence — stale in-flight cleared on attach, so any
            // unexpectedly low sequence is a gap, not a redelivery
            if (ch.sequence !== expectedSeqRef.current) {
              if (!gapNotified) {
                term.writeln("\r\n[transport] sequence gap detected — desynchronized");
                gapNotified = true;
              }
              break;
            }
            pendingAck.current.set(ch.sequence, true);
            const bytes = new Uint8Array(ch.bytes);
            await new Promise<void>((resolve) => {
              term.write(bytes, () => resolve());
            });
            if (cancelledRef.current) break;
            try {
              await terminalAck(sessionIdRef.current, ch.sequence);
              pendingAck.current.delete(ch.sequence);
              expectedSeqRef.current = ch.sequence + 1;
            } catch {
              break;
            }
          }
        } catch {
          // Poll errors non-fatal
        } finally {
          pollInFlightRef.current = false;
        }
      };

      const loop = async () => {
        while (!cancelledRef.current) {
          await pollOnce();
          if (cancelledRef.current) break;
          await new Promise<void>((resolve) => {
            pollTimer = window.setTimeout(() => resolve(), POLL_INTERVAL_MS);
          });
        }
      };
      void loop();
    };

    void initAndPoll();

    return () => {
      cancelledRef.current = true;
      if (pollTimer) window.clearTimeout(pollTimer);
      if (resizeTimer) window.clearTimeout(resizeTimer);
      // H14B: remove the paste listener during cleanup.
      containerElement.removeEventListener("paste", pasteHandler, true);
      ro.disconnect();
      dispData.dispose();
      search.dispose();
      fit.dispose();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      searchRef.current = null;
      pollInFlightRef.current = false;
    };
    // Do NOT recreate on ordinary React rerender; only when session id changes is handled via sessionIdRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCopy = useCallback(async () => {
    const term = termRef.current;
    if (!term) return;
    const sel = term.getSelection();
    if (!sel) {
      setCopyFeedback("No selection");
      setTimeout(() => setCopyFeedback(null), 1500);
      return;
    }
    try {
      await navigator.clipboard.writeText(sel);
      setCopyFeedback("Copied");
    } catch {
      try {
        const ta = document.createElement("textarea");
        ta.value = sel;
        document.body.appendChild(ta);
        ta.select();
        document.execCommand("copy");
        document.body.removeChild(ta);
        setCopyFeedback("Copied");
      } catch {
        setCopyFeedback("Copy failed");
      }
    }
    setTimeout(() => setCopyFeedback(null), 1500);
  }, []);

  // H14: single shared paste policy. Every user paste path (toolbar button,
  // keyboard/native paste, context-menu paste) must route through this function
  // so behavior is identical and exactly one paste reaches the PTY.
  const pasteText = useCallback((raw: string): boolean => {
    if (!raw) return false;
    const lines = raw.split("\n");
    // H14C: warn when multi-line (>1 line) OR large (>200 chars).
    if (lines.length > 1 || raw.length > 200) {
      const ok = window.confirm(
        `Paste warning: clipboard contains ${lines.length} line(s) / ${raw.length} chars.\n` +
          "Multi-line paste will be sent as typed. Continue?"
      );
      // Cancel -> zero bytes sent to the PTY.
      if (!ok) return false;
    }
    const term = termRef.current;
    if (!term) return false;
    // H14A: use xterm's public paste API. term.paste() performs the terminal's
    // paste transformations and respects bracketed-paste mode itself; we must
    // NOT manually wrap with ESC[200~ / ESC[201~ and must NOT call
    // terminalWrite directly. xterm fires its onData handler, which is the
    // single existing input path -> exactly one send.
    term.paste(raw);
    return true;
  }, []);

  const handlePaste = useCallback(async () => {
    let text = "";
    try {
      text = await navigator.clipboard.readText();
    } catch {
      setCopyFeedback("Paste read failed");
      setTimeout(() => setCopyFeedback(null), 1500);
      return;
    }
    pasteText(text);
  }, [pasteText]);

  const handleSearch = useCallback((dir: "next" | "prev") => {
    const addon = searchRef.current;
    if (!addon) return;
    const q = searchQuery.current;
    if (!q) return;
    if (dir === "next") addon.findNext(q);
    else addon.findPrevious(q);
  }, []);

  const exitBanner = isExitState(session)
    ? (() => {
        const st = session.process_state as { state: string; exit_code?: number; reason?: string };
        const code = st.exit_code ?? session.exit_code;
        const msg =
          st.state === "exited"
            ? `Process exited (code ${code ?? "?"})`
            : st.state === "failed"
              ? `Process failed: ${st.reason ?? "unknown"}`
              : `Session ${st.state}`;
        return msg;
      })()
    : null;

  return (
    <div className="terminal-view">
      <div className="terminal-toolbar" role="toolbar" aria-label="Terminal controls">
        <button type="button" onClick={() => setSearchOpen((v) => !v)} aria-pressed={searchOpen}>
          Search
        </button>
        <button type="button" onClick={() => void handleCopy()}>
          Copy
        </button>
        <button type="button" onClick={() => void handlePaste()}>
          Paste
        </button>
        <span className="spacer" />
        <span className="muted-small" aria-live="polite">
          {session.session_id.slice(0, 16)}… gen {session.generation} · {session.view_state} ·{" "}
          {session.process_state.state}
        </span>
        {copyFeedback && <span className="muted-small">{copyFeedback}</span>}
      </div>

      {searchOpen && (
        <div className="terminal-search" role="search">
          <input
            type="text"
            placeholder="Search scrollback"
            aria-label="Search terminal"
            value={searchText}
            onChange={(e) => {
              const v = e.target.value;
              setSearchText(v);
              searchQuery.current = v;
              if (v && searchRef.current) searchRef.current.findNext(v);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                handleSearch(e.shiftKey ? "prev" : "next");
              } else if (e.key === "Escape") {
                setSearchOpen(false);
              }
            }}
          />
          <button type="button" onClick={() => handleSearch("next")}>
            Next
          </button>
          <button type="button" onClick={() => handleSearch("prev")}>
            Prev
          </button>
          <button type="button" onClick={() => setSearchOpen(false)}>
            Close
          </button>
        </div>
      )}

      {replayTruncated && (
        <div className="terminal-banner terminal-banner--truncated" role="status">
          Replay truncated — scrollback beyond the bounded replay cap is not available after reload.
        </div>
      )}

      {generationBanner && (
        <div className="terminal-banner terminal-banner--restart" role="status" aria-live="polite">
          {generationBanner}
        </div>
      )}

      {exitBanner && (
        <div className="terminal-banner terminal-banner--exit" role="status" aria-live="polite">
          {exitBanner} — <em>restart will retain SessionId and bump generation</em>
        </div>
      )}

      <div
        ref={containerRef}
        className="terminal-container"
        data-testid="terminal-container"
        aria-label="Terminal"
      />
    </div>
  );
}

export default TerminalView;
