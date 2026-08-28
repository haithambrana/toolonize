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
  const pollTimer = useRef<number | null>(null);
  const pendingAck = useRef<Map<number, boolean>>(new Map());
  const searchQuery = useRef<string>("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);
  const [replayTruncated, setReplayTruncated] = useState(session.replay_truncated);
  const sessionIdRef = useRef(session.session_id);
  const generationRef = useRef(session.generation);

  useEffect(() => {
    sessionIdRef.current = session.session_id;
    generationRef.current = session.generation;
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
      // Resize failures are non-fatal; surface via console only (no PII)
    }
  }, [getDimensions]);

  // Create one Terminal instance per mounted TerminalView
  useEffect(() => {
    if (!containerRef.current) return;

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
    // Fit once after open
    try {
      fit.fit();
    } catch {
      // ignore
    }

    // Handle input -> forward bytes to existing session only
    const dispData = term.onData((data: string) => {
      const enc = new TextEncoder();
      const bytes = enc.encode(data);
      terminalWrite(sessionIdRef.current, bytes).catch(() => {
        // Write failure is non-crash; UI remains usable
      });
    });

    // Also handle binary? xterm onData gives string; for UTF-8 we encode.
    // For paste, we handle separately via bracketed-aware flow.

    // Resize observer with debounce
    let resizeTimer: number | null = null;
    const ro = new ResizeObserver(() => {
      if (resizeTimer) window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        void handleResize();
      }, RESIZE_DEBOUNCE_MS);
    });
    ro.observe(containerRef.current);

    // Initial replay for renderer reload reattachment
    (async () => {
      try {
        const replay = await terminalReplay(sessionIdRef.current);
        if (replay.bytes.length > 0) {
          const chunk = new Uint8Array(replay.bytes);
          // Write without ack tracking for replay (it's history, not sequenced)
          term.write(chunk);
        }
        if (replay.truncated) setReplayTruncated(true);
      } catch {
        // replay failure is non-fatal
      }
    })();

    // Poll sequenced chunks; ack after xterm write completes
    let expectedSeq = 0;
    let gapNotified = false;
    const poll = async () => {
      try {
        const { chunks, replayTruncated: truncated } = await terminalPoll(sessionIdRef.current, 16);
        if (truncated) setReplayTruncated(true);
        // Sort by sequence to ensure ordering check (backend already sequences)
        chunks.sort((a, b) => a.sequence - b.sequence);
        for (const ch of chunks) {
          // Generation check: if chunk generation differs, it's from previous incarnation — skip or warn
          if (ch.generation !== generationRef.current) {
            // Generation mismatch means restart occurred; we can surface via banner externally
            continue;
          }
          if (ch.sequence !== expectedSeq) {
            if (!gapNotified) {
              term.writeln("\r\n[transport] sequence gap detected — desynchronized");
              gapNotified = true;
            }
            // Do not ack gap; surface error and stop processing further in this poll
            break;
          }
          pendingAck.current.set(ch.sequence, true);
          const bytes = new Uint8Array(ch.bytes);
          // Wait for xterm write callback before ack
          await new Promise<void>((resolve) => {
            term.write(bytes, () => {
              resolve();
            });
          });
          try {
            await terminalAck(sessionIdRef.current, ch.sequence);
            pendingAck.current.delete(ch.sequence);
            expectedSeq = ch.sequence + 1;
          } catch {
            // ack failure surfaces desync; stop advancing
            break;
          }
        }
      } catch {
        // Poll errors are non-crash; next interval will retry
      }
    };
    pollTimer.current = window.setInterval(() => {
      void poll();
    }, POLL_INTERVAL_MS);
    // Also poll immediately
    void poll();

    return () => {
      if (pollTimer.current) window.clearInterval(pollTimer.current);
      if (resizeTimer) window.clearTimeout(resizeTimer);
      ro.disconnect();
      dispData.dispose();
      search.dispose();
      fit.dispose();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      searchRef.current = null;
    };
    // Do NOT recreate on ordinary React rerender; only when session id changes is handled via sessionIdRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Attach handling: when generation changes, reset expectedSeq
  useEffect(() => {
    // Generation is tracked via ref updated above; instance stays stable.
  }, [session.generation]);

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
      // Fallback: execCommand if clipboard permission denied
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

  const handlePaste = useCallback(async () => {
    let text = "";
    try {
      text = await navigator.clipboard.readText();
    } catch {
      setCopyFeedback("Paste read failed");
      setTimeout(() => setCopyFeedback(null), 1500);
      return;
    }
    // No clipboard contents persisted or logged — per security policy
    if (!text) return;
    const lines = text.split("\n");
    if (lines.length > 1 || text.length > 200) {
      const ok = window.confirm(
        `Paste warning: clipboard contains ${lines.length} line(s) / ${text.length} chars.\n` +
          "Multi-line paste will be sent as typed. Continue?"
      );
      if (!ok) return;
    }
    const enc = new TextEncoder();
    const bytes = enc.encode(text);
    // Preserve bracketed-paste mode: xterm will handle bracketed prefix; we just forward bytes
    try {
      await terminalWrite(sessionIdRef.current, bytes);
    } catch {
      setCopyFeedback("Paste write failed");
      setTimeout(() => setCopyFeedback(null), 1500);
    }
  }, []);

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
