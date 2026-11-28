/**
 * Local system clipboard access for terminals.
 *
 * Text copied in a terminal — including over SSH, where the selection is just
 * bytes ToolOnize already displays — goes to the clipboard of the machine
 * ToolOnize runs on. The async Clipboard API is tried first; WebKitGTK can
 * refuse it, so writes fall back to a hidden textarea + execCommand("copy").
 */
import { api } from "./ipc";

export async function writeClipboard(text: string): Promise<boolean> {
  if (!text) return false;
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the legacy path
  }
  const previous = document.activeElement as HTMLElement | null;
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.cssText = "position:fixed;left:-10000px;top:0;opacity:0;";
  document.body.appendChild(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  area.remove();
  previous?.focus?.();
  return ok;
}

/** Read clipboard text for a paste: native read first (WebKitGTK blocks the
 * web API), then the web API. Null when nothing could be read. */
export async function readClipboard(): Promise<string | null> {
  try {
    const text = await api.clipboardRead();
    if (text !== null) return text;
  } catch {
    // fall back to the web API
  }
  try {
    if (navigator.clipboard?.readText) return await navigator.clipboard.readText();
  } catch {
    // blocked by the WebView
  }
  return null;
}

/** Max bytes a terminal program may place on the clipboard via OSC 52. */
export const OSC52_MAX_BYTES = 1024 * 1024;

/**
 * Parse an OSC 52 payload (`<targets>;<base64>`). Returns the text to copy,
 * or null for queries (`?`, never answered: that would let a remote program
 * read your clipboard), malformed data, or oversized payloads.
 */
export function parseOsc52(data: string): string | null {
  const sep = data.indexOf(";");
  if (sep < 0) return null;
  const payload = data.slice(sep + 1);
  if (payload === "?" || payload.length === 0) return null;
  if (payload.length > Math.ceil((OSC52_MAX_BYTES * 4) / 3) + 4) return null;
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(payload)) return null;
  try {
    const binary = atob(payload);
    const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
}
