/** Minimal xterm.js stand-in for jsdom: records writes, disposals, pastes. */
import { vi } from "vitest";

export const terminals: FakeTerminal[] = [];

export class FakeTerminal {
  rows = 24;
  cols = 80;
  options: Record<string, unknown>;
  modes = { bracketedPasteMode: false };
  written: string[] = [];
  pasted: string[] = [];
  disposed = false;
  opened = 0;
  resets = 0;
  focused = 0;
  selection = "";
  private dataHandlers: ((d: string) => void)[] = [];
  private pendingCallbacks: (() => void)[] = [];
  autoFlush = true;
  dispose = vi.fn(() => {
    this.disposed = true;
  });

  constructor(options: Record<string, unknown>) {
    this.options = { ...options };
    terminals.push(this);
  }
  loadAddon() {}
  open() {
    this.opened++;
  }
  onData(handler: (d: string) => void) {
    this.dataHandlers.push(handler);
    return { dispose() {} };
  }
  onBinary() {
    return { dispose() {} };
  }
  attachCustomKeyEventHandler() {}
  write(data: string | Uint8Array, callback?: () => void) {
    this.written.push(typeof data === "string" ? data : new TextDecoder().decode(data));
    if (callback) {
      if (this.autoFlush) callback();
      else this.pendingCallbacks.push(callback);
    }
  }
  flush() {
    const cbs = this.pendingCallbacks;
    this.pendingCallbacks = [];
    cbs.forEach((cb) => cb());
  }
  paste(text: string) {
    this.pasted.push(text);
  }
  reset() {
    this.resets++;
  }
  focus() {
    this.focused++;
  }
  getSelection() {
    return this.selection;
  }
  /** Simulate the user typing (or xterm answering a terminal query). */
  type(data: string) {
    this.dataHandlers.forEach((h) => h(data));
  }
  get text() {
    return this.written.join("");
  }
}

export class FakeFitAddon {
  fit() {}
}

export class FakeSearchAddon {
  findNext = vi.fn(() => true);
  findPrevious = vi.fn(() => true);
  clearDecorations = vi.fn();
}
