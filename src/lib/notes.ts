/**
 * Sticky notes (ADR-012): one note per terminal member and one project note
 * per workspace. Typing updates the store at once and is saved shortly after
 * the user pauses (and immediately on blur, on closing the note and when the
 * window is hidden). The core writes each save durably (fsync + atomic
 * rename), so at most the last fraction of a second of typing can be lost
 * to a power cut. Failed saves keep the text and retry with backoff.
 */
import { api, errorMessage } from "./ipc";
import { getState, notify, setState } from "../state/store";

export const SAVE_DELAY_MS = 400;
const RETRY_MS = [1000, 2000, 5000, 10000];

interface Pending {
  timer?: ReturnType<typeof setTimeout>;
  /** Text waiting to be saved; undefined once saved. */
  text?: string;
  inFlight: boolean;
  failures: number;
}

const pending = new Map<string, Pending>();

export function noteKey(workspaceId: string, memberId: string | null): string {
  return `${workspaceId}:${memberId ?? "*"}`;
}

function parseKey(key: string): [string, string | null] {
  const i = key.lastIndexOf(":");
  const member = key.slice(i + 1);
  return [key.slice(0, i), member === "*" ? null : member];
}

export async function loadNotes(workspaceId: string): Promise<void> {
  try {
    const view = await api.notes(workspaceId);
    setState((s) => {
      // Text typed but not yet saved wins over what the core returns.
      const merged = { ...view, members: { ...view.members } };
      for (const [key, p] of pending) {
        const [ws, member] = parseKey(key);
        if (ws !== workspaceId || p.text === undefined) continue;
        if (member === null) merged.project = p.text;
        else merged.members[member] = p.text;
      }
      return { notes: { ...s.notes, [workspaceId]: merged } };
    });
  } catch (error) {
    notify("error", `Could not load notes: ${errorMessage(error)}`);
  }
}

export function noteText(workspaceId: string, memberId: string | null): string {
  const view = getState().notes[workspaceId];
  if (!view) return "";
  return memberId === null ? view.project : (view.members[memberId] ?? "");
}

export function hasNote(workspaceId: string, memberId: string | null): boolean {
  return noteText(workspaceId, memberId).trim().length > 0;
}

function setStatus(key: string, status: "saved" | "saving" | "error") {
  setState((s) => ({ noteStatus: { ...s.noteStatus, [key]: status } }));
}

/** The user typed: update the view now, save after a short pause. */
export function editNote(workspaceId: string, memberId: string | null, text: string): void {
  setState((s) => {
    const view = s.notes[workspaceId] ?? {
      project: "",
      project_updated_at: 0,
      members: {},
      trash: [],
    };
    const next =
      memberId === null
        ? { ...view, project: text }
        : { ...view, members: { ...view.members, [memberId]: text } };
    return { notes: { ...s.notes, [workspaceId]: next } };
  });
  const key = noteKey(workspaceId, memberId);
  const p = pending.get(key) ?? { inFlight: false, failures: 0 };
  p.text = text;
  clearTimeout(p.timer);
  p.timer = setTimeout(() => void flushNote(workspaceId, memberId), SAVE_DELAY_MS);
  pending.set(key, p);
  setStatus(key, "saving");
}

/** Save now if there is unsaved text. Resolves when saved (or failed). */
export async function flushNote(workspaceId: string, memberId: string | null): Promise<void> {
  const key = noteKey(workspaceId, memberId);
  const p = pending.get(key);
  if (!p || p.text === undefined || p.inFlight) return;
  clearTimeout(p.timer);
  const text = p.text;
  p.inFlight = true;
  try {
    await api.saveNote(workspaceId, memberId, text);
    p.failures = 0;
    if (p.text === text) {
      p.text = undefined;
      setStatus(key, "saved");
    }
  } catch (error) {
    p.failures += 1;
    setStatus(key, "error");
    const message = errorMessage(error);
    // Rejections the core will never accept (too large, gone) are not retried.
    const permanent = /at most|not found/.test(message);
    if (p.failures === 1 || permanent) notify("error", `Note not saved: ${message}`);
    if (!permanent) {
      const delay = RETRY_MS[Math.min(p.failures - 1, RETRY_MS.length - 1)];
      p.timer = setTimeout(() => void flushNote(workspaceId, memberId), delay);
    }
  } finally {
    p.inFlight = false;
  }
  // Text typed while the save was in flight is saved next.
  if (p.text !== undefined && p.text !== text && p.failures === 0) {
    await flushNote(workspaceId, memberId);
  }
}

export async function flushAllNotes(): Promise<void> {
  await Promise.all([...pending.keys()].map((key) => flushNote(...parseKey(key))));
}

let installed = false;

/** Save everything when the window is hidden or closed. */
export function installNoteFlush(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  const flush = () => void flushAllNotes();
  window.addEventListener("pagehide", flush);
  window.addEventListener("beforeunload", flush);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") flush();
  });
}

export async function restoreNotes(trashId: string, workspaceId: string): Promise<void> {
  try {
    await flushNote(workspaceId, null);
    await api.restoreNotes(trashId, workspaceId);
    await loadNotes(workspaceId);
    notify("info", "Notes restored into the project note");
  } catch (error) {
    notify("error", `Could not restore notes: ${errorMessage(error)}`);
  }
}

export function toggleMemberNote(memberId: string): void {
  setState((s) => ({ noteOpen: { ...s.noteOpen, [memberId]: !s.noteOpen[memberId] } }));
}

/** Test helper. */
export function resetNotes(): void {
  for (const p of pending.values()) clearTimeout(p.timer);
  pending.clear();
}
