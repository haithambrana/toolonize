/** Throwaway M2 helpers for the real PTY -> Tauri Channel -> xterm.js proof. */

import { Channel, invoke } from "@tauri-apps/api/core";

const marker = new TextEncoder().encode("DONE_MARKER");

type StreamResult = {
  payloadBytes: number;
  streamedBytes: number;
  markerBytes: number;
  processExitCode: number;
};

export type PipelineResult = {
  payloadBytes: number;
  deliveredPayloadBytes: number;
  expectedSha256: string;
  deliveredSha256: string;
  exactByteIntegrity: boolean;
  xtermWriteCompleted: boolean;
  processExitCode: number;
  details: string;
};

export type ResizeResult = {
  requestedRows: number;
  requestedCols: number;
  observedRows: number;
  observedCols: number;
  processExitCode: number;
};

export type InputResult = {
  echoed: string;
  processExitCode: number;
};

function bytesFromMessage(message: Uint8Array | number[] | string): Uint8Array {
  if (typeof message === "string") return new TextEncoder().encode(message);
  return Uint8Array.from(message);
}

function concatBytes(chunks: Uint8Array[], total: number): Uint8Array {
  const output = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.length;
  }
  return output;
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const copy = Uint8Array.from(bytes);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", copy.buffer));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function runPtyPipelineSpike(
  termWrite: (data: string) => Promise<void>,
  onBytes: (bytes: number) => void,
  bytes: number = 256 * 1024
): Promise<PipelineResult> {
  const chunks: Uint8Array[] = [];
  let delivered = 0;
  let writeChain = Promise.resolve();
  const delivery = { expected: undefined as number | undefined };
  let resolveDelivery: (() => void) | undefined;
  const deliveryComplete = new Promise<void>((resolve) => {
    resolveDelivery = resolve;
  });

  const channel = new Channel<Uint8Array>();
  channel.onmessage = (message: Uint8Array | number[] | string) => {
    const data = bytesFromMessage(message);
    chunks.push(data);
    delivered += data.length;
    onBytes(data.length);
    writeChain = writeChain.then(() => termWrite(new TextDecoder().decode(data)));
    if (delivery.expected !== undefined && delivered === delivery.expected) resolveDelivery?.();
  };

  const result = await invoke<StreamResult>("spike_pty_stream", {
    channel,
    request: { bytes },
  });
  delivery.expected = result.streamedBytes;
  if (delivered === delivery.expected) resolveDelivery?.();
  await Promise.race([
    deliveryComplete,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error("timed out waiting for Tauri Channel delivery")), 5000)
    ),
  ]);
  await writeChain;

  const allBytes = concatBytes(chunks, delivered);
  const payload = allBytes.slice(0, result.payloadBytes);
  const deliveredMarker = allBytes.slice(result.payloadBytes);
  const expectedPayload = new Uint8Array(result.payloadBytes).fill(65);
  const [expectedSha256, deliveredSha256] = await Promise.all([
    sha256(expectedPayload),
    sha256(payload),
  ]);
  const exactByteIntegrity =
    result.processExitCode === 0 &&
    result.markerBytes === marker.length &&
    result.streamedBytes === result.payloadBytes + marker.length &&
    delivered === result.streamedBytes &&
    equalBytes(payload, expectedPayload) &&
    equalBytes(deliveredMarker, marker) &&
    expectedSha256 === deliveredSha256;

  return {
    payloadBytes: result.payloadBytes,
    deliveredPayloadBytes: payload.length,
    expectedSha256,
    deliveredSha256,
    exactByteIntegrity,
    xtermWriteCompleted: true,
    processExitCode: result.processExitCode,
    details: `payload ${result.payloadBytes}, streamed ${result.streamedBytes}, delivered ${delivered}, SHA-256 ${deliveredSha256}, exact ${exactByteIntegrity}`,
  };
}

export function testResize(rows: number, cols: number): Promise<ResizeResult> {
  return invoke<ResizeResult>("spike_resize", { rows, cols });
}

export function testInputEcho(input: string): Promise<InputResult> {
  return invoke<InputResult>("spike_input_echo", { input });
}
