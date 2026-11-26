/** The per-terminal sticky note: a toggle button and the floating note. */
import { toggleMemberNote } from "../lib/notes";
import { setState, useAppState } from "../state/store";
import { StickyNote } from "./StickyNote";
import { peekController } from "../terminal/controller";

export function MemberNoteButton({
  workspaceId,
  memberId,
  className = "btn btn-ghost btn-sm",
}: {
  workspaceId: string;
  memberId: string;
  className?: string;
}) {
  const open = useAppState((s) => !!s.noteOpen[memberId]);
  const noted = useAppState(
    (s) => (s.notes[workspaceId]?.members[memberId] ?? "").trim().length > 0
  );
  return (
    <button
      type="button"
      className={noted ? `${className} has-note` : className}
      aria-pressed={open}
      title="Terminal note (Ctrl+Shift+N)"
      onClick={() => toggleMemberNote(memberId)}
    >
      Note{noted && <span className="note-dot" aria-label="has a note" />}
    </button>
  );
}

export function MemberNoteOverlay({
  workspaceId,
  memberId,
  title,
  sessionId,
}: {
  workspaceId: string;
  memberId: string;
  title: string;
  /** Terminal to give focus back to when the note closes. */
  sessionId?: string;
}) {
  const open = useAppState((s) => !!s.noteOpen[memberId]);
  if (!open) return null;
  return (
    <StickyNote
      workspaceId={workspaceId}
      memberId={memberId}
      title={title}
      className="sticky-note-overlay"
      onClose={() => {
        setState((s) => ({ noteOpen: { ...s.noteOpen, [memberId]: false } }));
        if (sessionId) setTimeout(() => peekController(sessionId)?.focus(), 0);
      }}
    />
  );
}
