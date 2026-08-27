/**
 * Spike helpers for PTY -> Rust -> Tauri Channel -> WebView -> xterm.js
 * Throwaway harness for M2, not product code.
 */

import { invoke, Channel } from "@tauri-apps/api/core";

/**
 * Run the full pipeline spike: spawn PTY via Rust, stream bytes via Channel to xterm.js.
 * Returns produced/delivered counts for lossless verification.
 */
export async function runPtyPipelineSpike(
  termWrite: (data: string) => void,
  onBytes: (bytes: number) => void,
  bytes: number = 512 * 1024
): Promise<{ produced: number; delivered: number; lossless: boolean; details: string }> {
  let delivered = 0;
  let channelText = "";

  const channel = new Channel<Uint8Array>();
  channel.onmessage = (chunk: Uint8Array | number[] | string) => {
    // Tauri may send as number[] or Uint8Array depending on serialization
    let data: Uint8Array;
    if (typeof chunk === "string") {
      data = new TextEncoder().encode(chunk);
    } else if (Array.isArray(chunk)) {
      data = new Uint8Array(chunk);
    } else {
      data = chunk as Uint8Array;
    }
    delivered += data.length;
    onBytes(data.length);
    const text = new TextDecoder().decode(data);
    channelText += text;
    termWrite(text);
  };

  const start = performance.now();
  const result: string = await invoke("spike_pty_stream", {
    channel,
    request: { bytes, seed: 42 },
  });
  const elapsed = performance.now() - start;

  // Verify DONE_MARKER present and lossless
  const hasMarker = channelText.includes("DONE_MARKER");
  const lossless = hasMarker && delivered >= bytes * 0.95;
  return {
    produced: bytes,
    delivered,
    lossless,
    details: `${result} | hasMarker ${hasMarker} | elapsed ${elapsed.toFixed(1)}ms | delivered ${delivered}`,
  };
}

export async function testResize(rows: number, cols: number): Promise<string> {
  const res: string = await invoke("spike_resize", { rows, cols });
  return res;
}

export async function testInputEcho(input: string): Promise<string> {
  const res: string = await invoke("spike_input_echo", { input });
  return res;
}
