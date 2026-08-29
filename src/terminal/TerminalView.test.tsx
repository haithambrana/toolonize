import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { TerminalView } from "./TerminalView";
import type { SessionInfo } from "./terminalTypes";

const xtermMocks = vi.hoisted(() => {
  const mockWrite = vi.fn((_data: unknown, cb?: () => void) => {
    if (cb) cb();
  });
  const mockWriteln = vi.fn();
  const mockOnData = vi.fn((cb: (d: string) => void) => {
    (mockOnData as unknown as { cb?: (d: string) => void }).cb = cb;
    return { dispose: vi.fn() };
  });
  const mockGetSelection = vi.fn(() => "selected text");
  const mockFindNext = vi.fn();
  const mockFindPrev = vi.fn();
  const mockFit = vi.fn();
  const mockPaste = vi.fn();
  class TerminalMock {
    open = vi.fn();
    loadAddon = vi.fn();
    write = mockWrite;
    writeln = mockWriteln;
    onData = mockOnData;
    getSelection = mockGetSelection;
    dispose = vi.fn();
    clear = vi.fn();
    paste = mockPaste;
    cols = 80;
    rows = 24;
  }
  class FitAddonMock {
    fit = mockFit;
    dispose = vi.fn();
  }
  class SearchAddonMock {
    findNext = mockFindNext;
    findPrevious = mockFindPrev;
    dispose = vi.fn();
  }
  return {
    mockWrite,
    mockWriteln,
    mockOnData,
    mockGetSelection,
    mockFindNext,
    mockFindPrev,
    mockFit,
    mockPaste,
    TerminalMock,
    FitAddonMock,
    SearchAddonMock,
  };
});

vi.mock("./terminalClient", () => ({
  terminalWrite: vi.fn(() => Promise.resolve(undefined)),
  terminalResize: vi.fn(() => Promise.resolve(undefined)),
  terminalAck: vi.fn(() => Promise.resolve(undefined)),
  terminalPoll: vi.fn(() =>
    Promise.resolve({ chunks: [], replayTruncated: false, nextSequence: 0 })
  ),
  terminalReplay: vi.fn(() =>
    Promise.resolve({
      bytes: [],
      truncated: false,
      discarded_bytes: 0,
      next_sequence: 0,
      attachment_epoch: 0,
    })
  ),
  terminalProfiles: vi.fn(() => Promise.resolve([])),
  terminalList: vi.fn(() => Promise.resolve([])),
  terminalAttach: vi.fn(() =>
    Promise.resolve({
      session: {
        session_id: "sess_00000001_deadbeef",
        generation: 1,
        profile_id: "bash",
        process_state: { state: "running" },
        view_state: "Attached",
        rows: 24,
        cols: 80,
        transport_state: "Normal",
        replay_truncated: false,
        exit_code: null,
      },
      attachment_epoch: 1,
      next_sequence: 0,
      acknowledged_up_to: null,
      replay_truncated: false,
      replay_discarded_bytes: 0,
    })
  ),
  terminalDetach: vi.fn(() => Promise.resolve({})),
  terminalHide: vi.fn(() => Promise.resolve({})),
  terminalShow: vi.fn(() => Promise.resolve({})),
  terminalClose: vi.fn(() => Promise.resolve({})),
  terminalRestart: vi.fn(() => Promise.resolve({})),
}));

import {
  terminalWrite,
  terminalAck,
  terminalPoll,
  terminalReplay,
  terminalAttach,
} from "./terminalClient";

vi.mock("@xterm/xterm", () => ({
  Terminal: xtermMocks.TerminalMock,
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: xtermMocks.FitAddonMock,
}));

vi.mock("@xterm/addon-search", () => ({
  SearchAddon: xtermMocks.SearchAddonMock,
}));

const baseSession: SessionInfo = {
  session_id: "sess_00000001_deadbeef",
  generation: 1,
  profile_id: "bash",
  process_state: { state: "running" },
  view_state: "Attached",
  rows: 24,
  cols: 80,
  transport_state: "Normal" as unknown as SessionInfo["transport_state"],
  replay_truncated: false,
  exit_code: null,
};

describe("TerminalView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(terminalPoll).mockResolvedValue({
      chunks: [],
      replayTruncated: false,
      nextSequence: 0,
    });
    vi.mocked(terminalReplay).mockResolvedValue({
      bytes: [],
      truncated: false,
      discarded_bytes: 0,
      next_sequence: 0,
      attachment_epoch: 0,
    });
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
        readText: vi.fn().mockResolvedValue("clipboard content"),
      },
    });
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders terminal shell and container", async () => {
    render(<TerminalView session={baseSession} />);
    expect(screen.getByLabelText("Terminal")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-container")).toBeInTheDocument();
  });

  it("loads profiles and handles start state (mocked)", async () => {
    render(<TerminalView session={baseSession} />);
    expect(screen.getByText(/sess_00000001/)).toBeInTheDocument();
  });

  it("shows running state and no exit banner when running", () => {
    render(<TerminalView session={baseSession} />);
    expect(screen.queryByText(/Process exited/)).not.toBeInTheDocument();
  });

  it("shows exit banner when process exited", () => {
    const exited: SessionInfo = {
      ...baseSession,
      process_state: { state: "exited", exit_code: 1 },
      exit_code: 1,
    };
    render(<TerminalView session={exited} />);
    expect(screen.getByText(/Process exited/)).toBeInTheDocument();
  });

  it("writes output chunks in order and acknowledges after xterm write", async () => {
    const chunks = [
      {
        session_id: baseSession.session_id,
        generation: 1,
        sequence: 0,
        bytes: Array.from(new TextEncoder().encode("hello ")),
      },
      {
        session_id: baseSession.session_id,
        generation: 1,
        sequence: 1,
        bytes: Array.from(new TextEncoder().encode("world")),
      },
    ];
    vi.mocked(terminalPoll).mockResolvedValueOnce({
      chunks,
      replayTruncated: false,
      nextSequence: 0,
    });
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(terminalPoll).toHaveBeenCalled(), { timeout: 1500 });
    await waitFor(() => expect(xtermMocks.mockWrite).toHaveBeenCalled(), { timeout: 1500 });
    await waitFor(() => expect(terminalAck).toHaveBeenCalledTimes(2), { timeout: 1500 });
    expect(terminalAck).toHaveBeenCalledWith(baseSession.session_id, 0);
    expect(terminalAck).toHaveBeenCalledWith(baseSession.session_id, 1);
  });

  it("forwards input via terminalWrite", async () => {
    render(<TerminalView session={baseSession} />);
    const cb = (xtermMocks.mockOnData as unknown as { cb?: (d: string) => void }).cb;
    expect(cb).toBeDefined();
    await act(async () => {
      cb!("echo hi\n");
    });
    await waitFor(() => expect(terminalWrite).toHaveBeenCalled());
    const call = vi.mocked(terminalWrite).mock.calls[0];
    expect(call[0]).toBe(baseSession.session_id);
    expect(ArrayBuffer.isView(call[1])).toBe(true);
  });

  it("requests resize via FitAddon", async () => {
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(xtermMocks.mockFit).toHaveBeenCalled());
  });

  it("attach/detach invariants: view state does not mutate process state (unit)", () => {
    const detached = { ...baseSession, view_state: "Detached" as const };
    const { rerender } = render(<TerminalView session={detached} />);
    expect(screen.getByText(/Detached/)).toBeInTheDocument();
    expect(screen.getByText(/running/)).toBeInTheDocument();
    rerender(<TerminalView session={{ ...baseSession, view_state: "Attached" }} />);
    expect(screen.getByText(/Attached/)).toBeInTheDocument();
  });

  it("search UI opens and triggers findNext/findPrevious", async () => {
    render(<TerminalView session={baseSession} />);
    const searchBtn = screen.getByRole("button", { name: "Search" });
    fireEvent.click(searchBtn);
    expect(screen.getByPlaceholderText("Search scrollback")).toBeInTheDocument();
    const input = screen.getByLabelText("Search terminal");
    fireEvent.change(input, { target: { value: "hello" } });
    expect(xtermMocks.mockFindNext).toHaveBeenCalledWith("hello");
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(xtermMocks.mockFindNext).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole("button", { name: "Prev" }));
    expect(xtermMocks.mockFindPrev).toHaveBeenCalledWith("hello");
  });

  it("copy selection uses clipboard and shows feedback", async () => {
    render(<TerminalView session={baseSession} />);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("selected text")
    );
    expect(await screen.findByText("Copied")).toBeInTheDocument();
  });

  it("multi-line paste warns via confirm (H14C: shared policy)", async () => {
    vi.mocked(navigator.clipboard.readText).mockResolvedValue("line1\nline2\nline3");
    render(<TerminalView session={baseSession} />);
    fireEvent.click(screen.getByRole("button", { name: "Paste" }));
    await waitFor(() => expect(window.confirm).toHaveBeenCalled());
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("Multi-line paste"));
    // H14A: toolbar paste routes through xterm's paste API, not terminalWrite.
    await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalled());
    expect(vi.mocked(terminalWrite)).not.toHaveBeenCalled();
  });

  it("IPC failure does not crash UI and remains usable", async () => {
    vi.mocked(terminalPoll).mockRejectedValue(new Error("ipc failed"));
    const { container } = render(<TerminalView session={baseSession} />);
    expect(container.querySelector(".terminal-container")).toBeInTheDocument();
    await new Promise((r) => setTimeout(r, 200));
    expect(screen.getByTestId("terminal-container")).toBeInTheDocument();
  });

  it("sequence gap surfaces error banner", async () => {
    const chunks = [
      { session_id: baseSession.session_id, generation: 1, sequence: 1, bytes: [65] },
    ];
    vi.mocked(terminalPoll).mockResolvedValueOnce({
      chunks,
      replayTruncated: false,
      nextSequence: 0,
    });
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(xtermMocks.mockWriteln).toHaveBeenCalled(), { timeout: 1500 });
    expect(xtermMocks.mockWriteln).toHaveBeenCalledWith(expect.stringContaining("sequence gap"));
  });

  it("replay truncation surfaces warning", async () => {
    const truncatedSession = { ...baseSession, replay_truncated: true };
    render(<TerminalView session={truncatedSession} />);
    expect(screen.getByText(/Replay truncated/)).toBeInTheDocument();
  });

  it("handles replay bytes on mount", async () => {
    vi.mocked(terminalReplay).mockResolvedValueOnce({
      bytes: Array.from(new TextEncoder().encode("replayed")),
      truncated: false,
      discarded_bytes: 0,
      next_sequence: 0,
      attachment_epoch: 0,
    });
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(terminalReplay).toHaveBeenCalledWith(baseSession.session_id));
    await waitFor(() => expect(xtermMocks.mockWrite).toHaveBeenCalled(), { timeout: 1500 });
  });

  it("serialized poll: only one poll active when xterm.write delayed", async () => {
    let pollCount = 0;
    vi.mocked(terminalPoll).mockImplementation(async () => {
      pollCount++;
      if (pollCount === 1) {
        return {
          chunks: [
            {
              session_id: baseSession.session_id,
              generation: 1,
              sequence: 0,
              bytes: [65],
            },
          ],
          replayTruncated: false,
          nextSequence: 1,
        };
      }
      return { chunks: [], replayTruncated: false, nextSequence: 1 };
    });
    // Delay xterm write 200ms (> poll interval 80ms)
    xtermMocks.mockWrite.mockImplementation((_data: unknown, cb?: () => void) => {
      setTimeout(() => cb && cb(), 200);
    });
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(pollCount).toBeGreaterThan(0), { timeout: 2000 });
    await new Promise((r) => setTimeout(r, 100));
    // Still only 1 poll should have been started because first poll's write not yet completed
    expect(pollCount).toBe(1);
    await waitFor(() => expect(pollCount).toBe(2), { timeout: 1000 });
    await waitFor(() => expect(terminalAck).toHaveBeenCalledWith(baseSession.session_id, 0), {
      timeout: 1000,
    });
    // No duplicate ack, no gap
    expect(xtermMocks.mockWriteln).not.toHaveBeenCalledWith(
      expect.stringContaining("sequence gap")
    );
  });

  it("generation change clears stale state and shows banner", async () => {
    const { rerender } = render(<TerminalView session={baseSession} />);
    await new Promise((r) => setTimeout(r, 200));
    const newSession = { ...baseSession, generation: 2 };
    rerender(<TerminalView session={newSession} />);
    await waitFor(() => expect(screen.getByText(/generation 1 -> 2/)).toBeInTheDocument(), {
      timeout: 1500,
    });
    expect(screen.getByText(/new stream cursor 0/)).toBeInTheDocument();
  });

  it("reattach via attach handshake establishes correct next_sequence without false gap", async () => {
    vi.mocked(terminalAttach).mockResolvedValueOnce({
      session: baseSession,
      attachment_epoch: 2,
      next_sequence: 5,
      acknowledged_up_to: 4,
      replay_truncated: false,
      replay_discarded_bytes: 0,
    });
    vi.mocked(terminalPoll).mockResolvedValueOnce({
      chunks: [{ session_id: baseSession.session_id, generation: 1, sequence: 5, bytes: [66] }],
      replayTruncated: false,
      nextSequence: 6,
    });
    render(<TerminalView session={baseSession} />);
    await waitFor(() => expect(terminalAttach).toHaveBeenCalled(), { timeout: 1500 });
    await waitFor(() => expect(terminalPoll).toHaveBeenCalled(), { timeout: 1500 });
    await waitFor(() => expect(xtermMocks.mockWrite).toHaveBeenCalled(), { timeout: 1500 });
    expect(xtermMocks.mockWriteln).not.toHaveBeenCalledWith(
      expect.stringContaining("sequence gap")
    );
    await waitFor(() => expect(terminalAck).toHaveBeenCalledWith(baseSession.session_id, 5), {
      timeout: 1500,
    });
  });

  describe("H14 paste policy (shared, xterm-paste, native-intercepted)", () => {
    // jsdom has no DataTransfer/ClipboardEvent; build a generic paste Event
    // whose clipboardData returns the supplied plain text (text/plain).
    const makePasteEvent = (text: string): Event => {
      const ev = new Event("paste", { bubbles: true, cancelable: true });
      Object.defineProperty(ev, "clipboardData", {
        value: { getData: (t: string) => (t === "text/plain" ? text : "") },
      });
      return ev;
    };

    beforeEach(() => {
      xtermMocks.mockPaste.mockClear();
    });

    it("toolbar single-line paste: no warning, paste exactly once via term.paste", async () => {
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("hello world");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      expect(window.confirm).not.toHaveBeenCalled();
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith("hello world");
      // Not routed via terminalWrite directly (H14A)
      expect(vi.mocked(terminalWrite)).not.toHaveBeenCalled();
    });

    it("toolbar multi-line paste: warning shown, Cancel -> zero sends", async () => {
      vi.spyOn(window, "confirm").mockReturnValue(false);
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("line1\nline2\nline3");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(window.confirm).toHaveBeenCalled());
      // Cancel -> zero bytes to PTY
      expect(xtermMocks.mockPaste).not.toHaveBeenCalled();
      expect(vi.mocked(terminalWrite)).not.toHaveBeenCalled();
    });

    it("toolbar multi-line paste: warning shown, Confirm -> exactly one paste", async () => {
      vi.spyOn(window, "confirm").mockReturnValue(true);
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("line1\nline2\nline3");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(window.confirm).toHaveBeenCalled());
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith("line1\nline2\nline3");
      expect(vi.mocked(terminalWrite)).not.toHaveBeenCalled();
    });

    it("native DOM paste: same warning policy on multi-line", async () => {
      const container = render(<TerminalView session={baseSession} />).getByTestId(
        "terminal-container"
      );
      const spy = vi.spyOn(window, "confirm").mockReturnValue(true);
      container.dispatchEvent(makePasteEvent("native\npaste\nmulti\nline"));
      await waitFor(() => expect(spy).toHaveBeenCalled());
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1000,
      });
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith("native\npaste\nmulti\nline");
    });

    it("native paste intercepted: xterm default path does NOT send a duplicate (exactly one paste)", async () => {
      const container = render(<TerminalView session={baseSession} />).getByTestId(
        "terminal-container"
      );
      const ev = makePasteEvent("single-line native");
      container.dispatchEvent(ev);
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1000,
      });
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith("single-line native");
      // preventDefault was called so nothing re-consumed/copy-sent
      expect(ev.defaultPrevented).toBe(true);
      expect(vi.mocked(terminalWrite)).not.toHaveBeenCalled();
    });

    it(">200-character single line: warning shown", async () => {
      const long = "x".repeat(201);
      vi.mocked(navigator.clipboard.readText).mockResolvedValue(long);
      render(<TerminalView session={baseSession} />);
      // Phase 1: Cancel -> zero sends
      vi.mocked(window.confirm).mockReturnValue(false);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(window.confirm).toHaveBeenCalled());
      expect(xtermMocks.mockPaste).not.toHaveBeenCalled();
      // Phase 2: Confirm -> pasted exactly once (no double)
      vi.mocked(window.confirm).mockReturnValue(true);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith(long);
    });

    it("Unicode/Arabic paste preserved verbatim", async () => {
      const arabic = "مرحبا بالعالم\nالسطر الثاني";
      vi.mocked(navigator.clipboard.readText).mockResolvedValue(arabic);
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      expect(xtermMocks.mockPaste).toHaveBeenCalledWith(arabic);
    });

    it("bracketed-paste mode off: no bracket markers added by ToolOnize", async () => {
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("plain");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      const arg = xtermMocks.mockPaste.mock.calls[0][0] as string;
      expect(arg).toBe("plain");
      expect(arg).not.toContain("\u001b[200~");
      expect(arg).not.toContain("\u001b[201~");
    });

    it("bracketed-paste mode on: xterm handles framing (ToolOnize must not double-wrap)", async () => {
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("payload");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      // ToolOnize only forwards the raw payload: xterm is responsible for any
      // bracketed framing. Assert no markers are injected by ToolOnize.
      const arg = xtermMocks.mockPaste.mock.calls[0][0] as string;
      expect(arg).toBe("payload");
      expect(arg).not.toContain("\u001b[200~");
      expect(arg).not.toContain("\u001b[201~");
      expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1);
    });

    it("no literal/doubled bracket markers caused by ToolOnize", async () => {
      vi.mocked(navigator.clipboard.readText).mockResolvedValue("line1\nline2");
      render(<TerminalView session={baseSession} />);
      fireEvent.click(screen.getByRole("button", { name: "Paste" }));
      await waitFor(() => expect(xtermMocks.mockPaste).toHaveBeenCalledTimes(1), {
        timeout: 1500,
      });
      const arg = xtermMocks.mockPaste.mock.calls[0][0] as string;
      const start = "\u001b[200~";
      const end = "\u001b[201~";
      // No literal markers at all — ToolOnize must not wrap or double-wrap.
      expect(arg.includes(start)).toBe(false);
      expect(arg.includes(end)).toBe(false);
    });

    it("component unmount removes the paste listener", async () => {
      const { unmount, getByTestId } = render(<TerminalView session={baseSession} />);
      const container = getByTestId("terminal-container");
      await waitFor(() => expect(xtermMocks.mockPaste).toBeDefined(), { timeout: 1000 });
      unmount();
      // After unmount, dispatching paste must NOT trigger term.paste
      xtermMocks.mockPaste.mockClear();
      container.dispatchEvent(makePasteEvent("after-unmount"));
      await new Promise((r) => setTimeout(r, 50));
      expect(xtermMocks.mockPaste).not.toHaveBeenCalled();
    });
  });
});
