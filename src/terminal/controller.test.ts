import { beforeEach, describe, expect, it, vi } from "vitest";
import { calls, core, frame, resetFakeCore, session } from "../test/fakeCore";
import { FakeTerminal, terminals } from "../test/fakeXterm";
import {
  getController,
  peekController,
  resetControllers,
  type ControllerOptions,
} from "./controller";

const policy = { copyOnSelect: false, osc52: true };

function options(confirm = vi.fn(async () => true)): ControllerOptions {
  return {
    fontSize: 13,
    scrollback: 1000,
    theme: {},
    confirmPaste: confirm,
    onDesync: vi.fn(),
    clipboardPolicy: () => policy,
    onCopied: vi.fn(),
    onContextMenu: vi.fn(),
  };
}

const clipboard: string[] = [];
Object.defineProperty(navigator, "clipboard", {
  configurable: true,
  value: {
    writeText: vi.fn(async (t: string) => {
      clipboard.push(t);
    }),
    readText: vi.fn(async () => clipboard.at(-1) ?? ""),
  },
});

const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

async function flush() {
  await new Promise((r) => setTimeout(r, 0));
}

function term(): FakeTerminal {
  return terminals[terminals.length - 1];
}

describe("TerminalController", () => {
  beforeEach(() => {
    clipboard.length = 0;
    policy.copyOnSelect = false;
    policy.osc52 = true;
    resetControllers();
    resetFakeCore();
    terminals.length = 0;
    core.sessions = [session({ session_id: "s1", member_id: "m1", end_offset: 0 })];
  });

  it("keeps one Terminal instance across mount/unmount/remount (T-LAYOUT-004)", async () => {
    const a = document.createElement("div");
    const b = document.createElement("div");
    document.body.append(a, b);
    const c1 = getController("s1", options);
    c1.mount(a);
    const instance = c1.term;
    c1.unmount(a);
    expect(c1.element.parentElement).not.toBe(a);
    expect(c1.element.isConnected).toBe(true); // parked, not destroyed
    const c2 = getController("s1", options);
    c2.mount(b);
    expect(c2).toBe(c1);
    expect(c2.term).toBe(instance);
    expect(term().dispose).not.toHaveBeenCalled();
    expect(term().opened).toBe(1);
    // Layout operations only report view transitions (T-STATE-001).
    const sessionCmds = calls.map((c) => c.cmd).filter((c) => c.startsWith("session_"));
    expect(sessionCmds).not.toContain("session_start");
    expect(sessionCmds).not.toContain("session_stop");
    expect(sessionCmds).not.toContain("session_restart");
    expect(calls.filter((c) => c.cmd === "session_view").map((c) => c.args?.event)).toEqual([
      "attach",
      "hide",
      "attach",
    ]);
  });

  it("writes contiguous frames and acknowledges after xterm wrote them (T-UI-001)", async () => {
    const c = getController("s1", options);
    await flush();
    const ch = core.attachments.get("s1")!;
    ch.output.send(frame(0, "hello "));
    ch.output.send(frame(6, "world"));
    expect(term().text).toBe("hello world");
    await new Promise((r) => setTimeout(r, 30));
    const acks = calls.filter((x) => x.cmd === "session_ack");
    expect(acks.at(-1)?.args).toMatchObject({ sessionId: "s1", offset: 11 });
    expect(c.deliveredOffset).toBe(11);
  });

  it("does not acknowledge bytes xterm has not finished writing (backpressure)", async () => {
    getController("s1", options);
    await flush();
    term().autoFlush = false;
    core.attachments.get("s1")!.output.send(frame(0, "x".repeat(100)));
    await new Promise((r) => setTimeout(r, 30));
    expect(calls.some((x) => x.cmd === "session_ack")).toBe(false);
    term().flush();
    await new Promise((r) => setTimeout(r, 30));
    expect(calls.filter((x) => x.cmd === "session_ack").at(-1)?.args?.offset).toBe(100);
  });

  it("treats an offset gap as explicit desync and re-attaches from what was written (T-PTY-007)", async () => {
    const opts = options();
    getController("s1", () => opts);
    await flush();
    const ch = core.attachments.get("s1")!;
    ch.output.send(frame(0, "abc"));
    ch.output.send(frame(10, "lost?"));
    expect(term().text).toBe("abc");
    expect(opts.onDesync).toHaveBeenCalled();
    await flush();
    const attaches = calls.filter((x) => x.cmd === "session_attach");
    expect(attaches).toHaveLength(2);
    expect(attaches[1].args?.fromOffset).toBe(3);
  });

  it("buffers frames that arrive before attach returns", async () => {
    let release: () => void = () => undefined;
    const original = (await import("../test/fakeCore")).handlers.get("session_attach")!;
    (await import("../test/fakeCore")).handlers.set("session_attach", async (a) => {
      const info = original(a);
      (a.output as { send: (f: ArrayBuffer) => void }).send(frame(0, "early"));
      await new Promise<void>((r) => (release = r));
      return info;
    });
    getController("s1", options);
    await flush();
    expect(term().text).toBe("");
    release();
    await flush();
    expect(term().text).toBe("early");
  });

  it("suppresses terminal answers while replaying history after reattach (FS-07)", async () => {
    core.sessions = [session({ session_id: "s1", member_id: "m1", end_offset: 7 })];
    getController("s1", options);
    await flush();
    const ch = core.attachments.get("s1")!;
    term().autoFlush = false;
    ch.output.send(frame(0, "\x1b[6nold"));
    term().type("\x1b[1;1R"); // xterm answering the replayed query
    expect(calls.some((x) => x.cmd === "session_write")).toBe(false);
    term().flush();
    term().type("ls\r");
    await flush();
    const writes = calls.filter((x) => x.cmd === "session_write");
    expect(writes).toHaveLength(1);
    expect(new TextDecoder().decode(Uint8Array.from(writes[0].args?.data as number[]))).toBe(
      "ls\r"
    );
  });

  it("resets and labels the view when history was trimmed", async () => {
    const { handlers } = await import("../test/fakeCore");
    const original = handlers.get("session_attach")!;
    handlers.set("session_attach", (a) => ({
      ...(original(a) as object),
      trimmed: true,
      start_offset: 500,
    }));
    getController("s1", options);
    await flush();
    expect(term().resets).toBe(1);
    expect(term().text).toContain("trimmed");
  });

  it("asks before multi-line paste and honours the answer (FR-033, T-UI-003)", async () => {
    const confirm = vi.fn(async () => false);
    const c = getController("s1", () => options(confirm));
    await c.paste("echo one");
    expect(term().pasted).toEqual(["echo one"]);
    expect(confirm).not.toHaveBeenCalled();
    await c.paste("rm -rf build\nmake");
    expect(confirm).toHaveBeenCalledWith("rm -rf build\nmake", false);
    expect(term().pasted).toEqual(["echo one"]);
    confirm.mockResolvedValueOnce(true);
    term().modes.bracketedPasteMode = true;
    await c.paste("a\nb");
    expect(confirm).toHaveBeenLastCalledWith("a\nb", true);
    expect(term().pasted).toEqual(["echo one", "a\nb"]);
  });

  it("intercepts native paste events for the warning", async () => {
    const confirm = vi.fn(async () => false);
    const c = getController("s1", () => options(confirm));
    const event = new Event("paste", { bubbles: true, cancelable: true }) as ClipboardEvent;
    Object.defineProperty(event, "clipboardData", { value: { getData: () => "x\ny" } });
    c.element.dispatchEvent(event);
    await flush();
    expect(event.defaultPrevented).toBe(true);
    expect(confirm).toHaveBeenCalled();
    expect(term().pasted).toEqual([]);
  });

  it("sends resize only when rows/cols change (T-UI-002)", async () => {
    const c = getController("s1", options);
    const div = document.createElement("div");
    document.body.append(div);
    c.mount(div);
    c.fitNow();
    c.fitNow();
    term().rows = 40;
    term().cols = 120;
    c.fitNow();
    const resizes = calls
      .filter((x) => x.cmd === "session_resize")
      .map((x) => [x.args?.rows, x.args?.cols]);
    expect(resizes).toEqual([
      [24, 80],
      [40, 120],
    ]);
  });

  it("disposes only on explicit request", async () => {
    const c = getController("s1", options);
    await flush();
    expect(peekController("s1")).toBe(c);
    resetControllers();
    expect(term().dispose).toHaveBeenCalledTimes(1);
    expect(calls.some((x) => x.cmd === "session_detach")).toBe(true);
  });

  it("OSC 52 from a program (e.g. tmux/vim over SSH) lands in the local clipboard", async () => {
    const opts = options();
    getController("s1", () => opts);
    await flush();
    term().osc(52, `c;${b64("copied on the server")}`);
    await flush();
    expect(clipboard).toEqual(["copied on the server"]);
    expect(opts.onCopied).toHaveBeenCalledWith(20, "program");
  });

  it("OSC 52 never reads the clipboard and respects the setting", async () => {
    getController("s1", options);
    await flush();
    clipboard.push("secret");
    expect(term().osc(52, "c;?")).toBe(true);
    expect(calls.some((x) => x.cmd === "session_write")).toBe(false);
    policy.osc52 = false;
    term().osc(52, `c;${b64("blocked")}`);
    await flush();
    expect(clipboard).toEqual(["secret"]);
  });

  it("OSC 52 in replayed history does not overwrite the clipboard", async () => {
    core.sessions = [session({ session_id: "s1", member_id: "m1", end_offset: 100 })];
    getController("s1", options);
    await flush();
    term().osc(52, `c;${b64("old")}`);
    await flush();
    expect(clipboard).toEqual([]);
  });

  it("copies the selection to the clipboard, and on select when enabled", async () => {
    vi.useFakeTimers();
    const opts = options();
    const c = getController("s1", () => opts);
    term().selection = "ls -la output";
    expect(await c.copyToClipboard()).toBe(true);
    expect(clipboard).toEqual(["ls -la output"]);
    term().selectionHandlers.forEach((h) => h());
    await vi.advanceTimersByTimeAsync(200);
    expect(clipboard).toHaveLength(1);
    policy.copyOnSelect = true;
    term().selection = "auto copied";
    term().selectionHandlers.forEach((h) => h());
    await vi.advanceTimersByTimeAsync(200);
    expect(clipboard.at(-1)).toBe("auto copied");
    vi.useRealTimers();
  });

  it("opens the context menu instead of the WebView menu", () => {
    const opts = options();
    const c = getController("s1", () => opts);
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 10,
      clientY: 20,
    });
    c.element.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(opts.onContextMenu).toHaveBeenCalledWith(10, 20);
  });
});
