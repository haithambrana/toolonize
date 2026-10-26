/**
 * Long-lived terminal host (ARCHITECTURE §6 "stable-host architecture").
 *
 * One `TerminalController` exists per session and owns its xterm.js
 * `Terminal` and host element. Layout components only *attach* the element
 * to a container and *park* it when unmounted; layout moves never dispose a
 * terminal or touch the PTY session (FR-031, T-LAYOUT-004). Disposal happens
 * only when the session is closed or its member is removed.
 *
 * Output delivery is offset-checked and acknowledged after xterm.js has
 * written each chunk, which is what drives the core's lossless backpressure.
 */
import { Channel } from "@tauri-apps/api/core";
import { Terminal, type IBufferRange, type ILink, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { api, type AttachInfo, type SessionEvent } from "../lib/ipc";
import { parseOsc52, writeClipboard } from "../lib/clipboard";

const ACK_BYTES = 64 * 1024;
const ACK_DELAY_MS = 16;

export interface ControllerOptions {
  fontSize: number;
  scrollback: number;
  theme: ITheme;
  /** Decide whether a paste may proceed (multi-line warning, FR-033). */
  confirmPaste: (text: string, bracketed: boolean) => Promise<boolean>;
  onDesync?: (reason: string) => void;
  onKey?: (event: KeyboardEvent) => boolean;
  /** Read live so settings changes apply to running terminals. */
  clipboardPolicy?: () => { copyOnSelect: boolean; osc52: boolean };
  /** Text was placed on the local clipboard (selection or OSC 52). */
  onCopied?: (chars: number, source: "selection" | "program") => void;
  onContextMenu?: (x: number, y: number) => void;
  /**
   * Ctrl+click on a link. `hidden` is true for OSC 8 hyperlinks, whose
   * target is not the visible text and must be shown before opening.
   */
  onOpenLink?: (url: string, hidden: boolean) => void;
}

/** One logical (unwrapped) line of the terminal buffer. */
export interface BufferLine {
  /** Buffer row where the line starts. */
  row: number;
  text: string;
}

export interface BufferMatch {
  row: number;
  col: number;
  length: number;
  line: string;
  index: number;
}

const URL_RE = /\bhttps?:\/\/[^\s"'<>`]+/gi;

/** URLs in a line of text, without trailing punctuation. */
export function findUrls(text: string): { index: number; url: string }[] {
  const out: { index: number; url: string }[] = [];
  for (const m of text.matchAll(URL_RE)) {
    let url = m[0];
    while (/[.,;:!?'")\]}>]$/.test(url)) {
      const last = url[url.length - 1];
      const open = { ")": "(", "]": "[", "}": "{" }[last];
      if (open && url.split(open).length > url.split(last).length - 1) break;
      url = url.slice(0, -1);
    }
    if (url.length > "https://".length) out.push({ index: m.index ?? 0, url });
  }
  return out;
}

let parking: HTMLDivElement | null = null;

/** Off-screen container that keeps unmounted terminals alive. */
function parkingLot(): HTMLDivElement {
  if (!parking || !parking.isConnected) {
    parking = document.createElement("div");
    parking.setAttribute("aria-hidden", "true");
    parking.style.cssText =
      "position:fixed;left:-10000px;top:0;width:800px;height:600px;overflow:hidden;visibility:hidden;";
    document.body.appendChild(parking);
  }
  return parking;
}

function frameOffset(frame: ArrayBuffer): number {
  const view = new DataView(frame, 0, 8);
  const low = view.getUint32(0, true);
  const high = view.getUint32(4, true);
  return high * 0x1_0000_0000 + low;
}

function toArrayBuffer(message: ArrayBuffer | Uint8Array | number[]): ArrayBuffer {
  if (message instanceof ArrayBuffer) return message;
  const bytes = message instanceof Uint8Array ? message : Uint8Array.from(message);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

export class TerminalController {
  readonly sessionId: string;
  readonly term: Terminal;
  readonly element: HTMLDivElement;
  readonly search: SearchAddon;
  private readonly fit: FitAddon;
  private readonly options: ControllerOptions;
  private opened = false;
  private disposed = false;
  private attachmentId: number | null = null;
  private attachSeq = 0;
  /** Next byte offset expected from the core. */
  private received = 0;
  /** Bytes xterm.js has finished writing. */
  private written = 0;
  private acked = 0;
  private ackTimer: ReturnType<typeof setTimeout> | undefined;
  /** Until this offset is written, we are replaying history. */
  private replayUntil = 0;
  private lastSize = { rows: 0, cols: 0 };
  private resizeTimer: ReturnType<typeof setTimeout> | undefined;
  private readonly encoder = new TextEncoder();
  private container: HTMLElement | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private acceptInput = true;
  private selectionTimer: ReturnType<typeof setTimeout> | undefined;
  /** Frames that arrived before the attach call returned its start offset. */
  private early: ArrayBuffer[] | null = null;

  constructor(sessionId: string, options: ControllerOptions) {
    this.sessionId = sessionId;
    this.options = options;
    this.element = document.createElement("div");
    this.element.className = "terminal-host";
    this.term = new Terminal({
      // OSC 8 hyperlinks: the target is hidden behind the text, so opening
      // asks first (onOpenLink(url, true)); only http(s) are accepted.
      linkHandler: {
        allowNonHttpProtocols: false,
        activate: (event, uri) => {
          if (event.ctrlKey || event.metaKey) this.options.onOpenLink?.(uri, true);
        },
        hover: (_event, uri) => this.hint(uri),
        leave: () => this.hint(null),
      },
      fontSize: options.fontSize,
      scrollback: options.scrollback,
      theme: options.theme,
      allowProposedApi: true,
      cursorBlink: true,
      fontFamily:
        '"JetBrains Mono", "Cascadia Mono", "DejaVu Sans Mono", "Ubuntu Mono", Menlo, Consolas, monospace',
      screenReaderMode: false,
    });
    this.fit = new FitAddon();
    this.search = new SearchAddon();
    this.term.loadAddon(this.fit);
    this.term.loadAddon(this.search);
    this.term.onData((data) => this.input(this.encoder.encode(data)));
    this.term.onBinary((data) => this.input(Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff)));
    this.term.attachCustomKeyEventHandler((event) => this.options.onKey?.(event) ?? true);
    // OSC 52: programs (tmux, vim, … also on a remote host over SSH) copy
    // into the LOCAL clipboard. Clipboard reads ("?") are never answered.
    this.term.parser.registerOscHandler(52, (data) => {
      if (this.isReplaying || !(this.options.clipboardPolicy?.().osc52 ?? true)) return true;
      const text = parseOsc52(data);
      if (text) {
        void writeClipboard(text).then(
          (ok) => ok && this.options.onCopied?.(text.length, "program")
        );
      }
      return true;
    });
    // Plain-text URLs: Ctrl+click opens them in the system browser.
    this.term.registerLinkProvider({
      provideLinks: (lineNumber, callback) => callback(this.linksAt(lineNumber - 1)),
    });
    this.term.onSelectionChange(() => {
      if (!this.options.clipboardPolicy?.().copyOnSelect) return;
      clearTimeout(this.selectionTimer);
      this.selectionTimer = setTimeout(() => {
        const text = this.term.getSelection();
        if (text) void writeClipboard(text);
      }, 150);
    });
    this.element.addEventListener("contextmenu", (event) => {
      if (!this.options.onContextMenu) return;
      event.preventDefault();
      this.options.onContextMenu(event.clientX, event.clientY);
    });
    // Paste interception for the multi-line warning (FR-033, T-TERM-02).
    this.element.addEventListener(
      "paste",
      (event) => {
        const text = event.clipboardData?.getData("text/plain") ?? "";
        event.preventDefault();
        event.stopPropagation();
        void this.paste(text);
      },
      true
    );
    parkingLot().appendChild(this.element);
  }

  get isDisposed(): boolean {
    return this.disposed;
  }

  get deliveredOffset(): number {
    return this.written;
  }

  get isReplaying(): boolean {
    return this.written < this.replayUntil;
  }

  /** Show the terminal inside `container` (layout attach). */
  mount(container: HTMLElement): void {
    if (this.disposed) return;
    this.container = container;
    container.appendChild(this.element);
    if (!this.opened) {
      this.term.open(this.element);
      this.opened = true;
    }
    this.resizeObserver?.disconnect();
    if (typeof ResizeObserver !== "undefined") {
      this.resizeObserver = new ResizeObserver(() => this.scheduleFit());
      this.resizeObserver.observe(container);
    }
    this.scheduleFit();
    void api.view(this.sessionId, "attach").catch(() => undefined);
  }

  /** Remove from the layout without destroying anything (layout detach). */
  unmount(container: HTMLElement): void {
    if (this.container !== container) return;
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.container = null;
    if (!this.disposed) {
      parkingLot().appendChild(this.element);
      void api.view(this.sessionId, "hide").catch(() => undefined);
    }
  }

  focus(): void {
    this.term.focus();
  }

  setInputEnabled(enabled: boolean): void {
    this.acceptInput = enabled;
  }

  private scheduleFit(): void {
    clearTimeout(this.resizeTimer);
    this.resizeTimer = setTimeout(() => this.fitNow(), 30);
  }

  fitNow(): void {
    if (this.disposed || !this.container || !this.opened) return;
    try {
      this.fit.fit();
    } catch {
      return; // not laid out yet
    }
    const { rows, cols } = this.term;
    if (rows !== this.lastSize.rows || cols !== this.lastSize.cols) {
      this.lastSize = { rows, cols };
      void api.resize(this.sessionId, rows, cols).catch(() => undefined);
    }
  }

  /** Attach (or re-attach) the output stream, resuming at what xterm wrote. */
  async attach(): Promise<AttachInfo | undefined> {
    if (this.disposed) return undefined;
    const seq = ++this.attachSeq;
    const output = new Channel<ArrayBuffer>();
    const events = new Channel<SessionEvent>();
    this.early = [];
    output.onmessage = (message) => {
      if (seq !== this.attachSeq) return;
      const frame = toArrayBuffer(message as ArrayBuffer);
      if (this.early) this.early.push(frame);
      else this.onFrame(frame);
    };
    events.onmessage = (event) => {
      if (seq !== this.attachSeq) return;
      if (event.type === "desync") {
        this.options.onDesync?.(event.reason);
        void this.attach();
      }
    };
    let info: AttachInfo;
    try {
      info = await api.attach(this.sessionId, this.written, output, events);
    } catch (error) {
      if (seq === this.attachSeq) this.early = null;
      throw error;
    }
    if (seq !== this.attachSeq || this.disposed) return info;
    this.attachmentId = info.attachment_id;
    if (info.trimmed) {
      // Older output left the bounded replay window: start from a clean
      // screen and say so instead of rendering a torn escape stream.
      this.term.reset();
      this.term.write(
        "\x1b[2m[ToolOnize] earlier output exceeded the replay buffer and was trimmed\x1b[0m\r\n"
      );
    }
    this.received = info.start_offset;
    this.written = info.start_offset;
    this.acked = info.start_offset;
    this.replayUntil = info.end_offset;
    const early = this.early ?? [];
    this.early = null;
    for (const frame of early) this.onFrame(frame);
    return info;
  }

  private onFrame(frame: ArrayBuffer): void {
    if (frame.byteLength < 8) return;
    const offset = frameOffset(frame);
    const data = new Uint8Array(frame, 8);
    if (offset !== this.received) {
      // Never paper over a gap: resynchronize explicitly from what we have.
      this.options.onDesync?.(`output gap (expected ${this.received}, got ${offset})`);
      void this.attach();
      return;
    }
    this.received += data.length;
    const end = this.received;
    this.term.write(data, () => {
      this.written = end;
      this.scheduleAck();
    });
  }

  private scheduleAck(): void {
    if (this.attachmentId === null) return;
    if (this.written - this.acked >= ACK_BYTES) {
      this.flushAck();
      return;
    }
    if (this.ackTimer === undefined) {
      this.ackTimer = setTimeout(() => this.flushAck(), ACK_DELAY_MS);
    }
  }

  private flushAck(): void {
    clearTimeout(this.ackTimer);
    this.ackTimer = undefined;
    if (this.attachmentId === null || this.written <= this.acked) return;
    this.acked = this.written;
    void api.ack(this.sessionId, this.attachmentId, this.written).catch(() => undefined);
  }

  private input(bytes: Uint8Array): void {
    // Replayed history makes xterm.js answer old terminal queries again; those
    // answers must not reach the new process.
    if (this.isReplaying || !this.acceptInput || this.disposed) return;
    void api.write(this.sessionId, bytes).catch(() => undefined);
  }

  async paste(text: string): Promise<void> {
    if (!text || this.disposed) return;
    const multiline = /[\r\n]/.test(text);
    if (multiline) {
      const ok = await this.options.confirmPaste(text, this.term.modes.bracketedPasteMode);
      if (!ok) return;
    }
    this.term.paste(text);
  }

  /** Copy the current selection to the local clipboard. */
  async copyToClipboard(): Promise<boolean> {
    const text = this.term.getSelection();
    if (!text) return false;
    const ok = await writeClipboard(text);
    if (ok) this.options.onCopied?.(text.length, "selection");
    return ok;
  }

  hasSelection(): boolean {
    return this.term.hasSelection();
  }

  selectAll(): void {
    this.term.selectAll();
  }

  clearScreen(): void {
    this.term.clear();
  }

  copySelection(): string {
    return this.term.getSelection();
  }

  private hint(url: string | null): void {
    this.element.title = url ? `Ctrl+click to open ${url}` : "";
  }

  /** Logical line containing buffer `row`, with wrapped rows joined. */
  private logicalLineAt(row: number): BufferLine | undefined {
    const buffer = this.term.buffer.active;
    let start = row;
    while (start > 0 && buffer.getLine(start)?.isWrapped) start--;
    if (!buffer.getLine(start)) return undefined;
    let text = "";
    let r = start;
    for (;;) {
      const line = buffer.getLine(r);
      if (!line) break;
      const next = buffer.getLine(r + 1);
      const continues = next?.isWrapped ?? false;
      text += line.translateToString(!continues);
      if (!continues) break;
      r++;
    }
    return { row: start, text };
  }

  private linksAt(row: number): ILink[] | undefined {
    const line = this.logicalLineAt(row);
    if (!line) return undefined;
    const cols = this.term.cols;
    const cell = (i: number) => ({ x: (i % cols) + 1, y: line.row + Math.floor(i / cols) + 1 });
    const links = findUrls(line.text)
      .map(({ index, url }): ILink => {
        const range: IBufferRange = { start: cell(index), end: cell(index + url.length - 1) };
        return {
          range,
          text: url,
          activate: (event, text) => {
            if (event.ctrlKey || event.metaKey) this.options.onOpenLink?.(text, false);
          },
          hover: (_event, text) => this.hint(text),
          leave: () => this.hint(null),
        };
      })
      .filter((l) => l.range.start.y <= row + 1 && l.range.end.y >= row + 1);
    return links.length ? links : undefined;
  }

  /** The whole buffer (scrollback + screen) as logical lines. */
  lines(): BufferLine[] {
    const buffer = this.term.buffer.active;
    const out: BufferLine[] = [];
    for (let row = 0; row < buffer.length; row++) {
      if (buffer.getLine(row)?.isWrapped) continue;
      const line = this.logicalLineAt(row);
      if (line) out.push(line);
    }
    return out;
  }

  /** Plain text of the buffer, trailing blank lines dropped. */
  textContent(): string {
    const lines = this.lines().map((l) => l.text);
    while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
    return lines.length ? lines.join("\n") + "\n" : "";
  }

  /** Matches of `query` across the buffer, up to `limit`. */
  find(query: string, caseSensitive: boolean, limit: number): BufferMatch[] {
    if (!query) return [];
    const needle = caseSensitive ? query : query.toLowerCase();
    const cols = this.term.cols;
    const out: BufferMatch[] = [];
    for (const line of this.lines()) {
      const hay = caseSensitive ? line.text : line.text.toLowerCase();
      let index = hay.indexOf(needle);
      while (index !== -1 && out.length < limit) {
        out.push({
          row: line.row + Math.floor(index / cols),
          col: index % cols,
          length: query.length,
          line: line.text,
          index,
        });
        index = hay.indexOf(needle, index + needle.length);
      }
      if (out.length >= limit) break;
    }
    return out;
  }

  /** Scroll to a match and select it. */
  reveal(match: Pick<BufferMatch, "row" | "col" | "length">): void {
    this.term.scrollToLine(Math.max(0, match.row - Math.floor(this.term.rows / 2)));
    this.term.select(match.col, match.row, match.length);
  }

  /** Visual separator when a fresh process starts in the same view. */
  markRestart(): void {
    const rows = this.term.rows;
    this.term.write(`\x1b[0m\r\n${"\r\n".repeat(Math.max(0, rows - 1))}\x1b[H\x1b[2J`);
  }

  applyOptions(options: Pick<ControllerOptions, "fontSize" | "scrollback" | "theme">): void {
    this.term.options.fontSize = options.fontSize;
    this.term.options.scrollback = options.scrollback;
    this.term.options.theme = options.theme;
    this.scheduleFit();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.attachSeq++;
    clearTimeout(this.ackTimer);
    clearTimeout(this.resizeTimer);
    this.resizeObserver?.disconnect();
    if (this.attachmentId !== null) {
      void api.detach(this.sessionId, this.attachmentId).catch(() => undefined);
    }
    this.term.dispose();
    this.element.remove();
  }
}

const controllers = new Map<string, TerminalController>();

export function getController(
  sessionId: string,
  options: () => ControllerOptions
): TerminalController {
  let controller = controllers.get(sessionId);
  if (!controller || controller.isDisposed) {
    controller = new TerminalController(sessionId, options());
    controllers.set(sessionId, controller);
    void controller.attach().catch(() => undefined);
  }
  return controller;
}

export function peekController(sessionId: string): TerminalController | undefined {
  return controllers.get(sessionId);
}

export function disposeController(sessionId: string): void {
  controllers.get(sessionId)?.dispose();
  controllers.delete(sessionId);
}

/** Drop controllers whose sessions no longer exist. */
export function pruneControllers(liveSessionIds: Set<string>): void {
  for (const id of [...controllers.keys()]) {
    if (!liveSessionIds.has(id)) disposeController(id);
  }
}

export function allControllers(): TerminalController[] {
  return [...controllers.values()];
}

/** Test helper. */
export function resetControllers(): void {
  for (const c of controllers.values()) c.dispose();
  controllers.clear();
}
