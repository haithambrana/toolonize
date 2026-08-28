export type ProcessSessionState =
  | { state: "new" }
  | { state: "starting" }
  | { state: "running" }
  | { state: "exited"; exit_code: number }
  | { state: "failed"; reason: string }
  | { state: "stopping" }
  | { state: "closed" }
  | { state: "restarting" }
  | { state: "disconnected" }
  | { state: "reconnecting" };

export type ViewAttachmentState = "Detached" | "Attached" | "Hidden";

export type TransportState = "Normal" | { Desynchronized: { reason: string } } | "Backpressured";

export interface TerminalProfile {
  id: string;
  display_name: string;
  kind: string;
  available: boolean;
}

export interface SessionInfo {
  session_id: string;
  generation: number;
  profile_id: string;
  process_state: ProcessSessionState;
  view_state: ViewAttachmentState;
  rows: number;
  cols: number;
  transport_state: TransportState;
  replay_truncated: boolean;
  exit_code: number | null;
}

export interface OutputChunk {
  session_id: string;
  generation: number;
  sequence: number;
  bytes: number[];
}

export type TerminalEventKind =
  | "SessionCreated"
  | "StateChanged"
  | "OutputChunk"
  | "TransportStateChanged"
  | "Exited"
  | "Failed"
  | "ReplayTruncated";

export interface TerminalEvent {
  kind: TerminalEventKind;
  session_id: string;
  payload?: unknown;
}
