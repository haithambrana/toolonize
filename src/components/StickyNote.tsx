/** Sticky note editor for a terminal member or the workspace project note. */
import { useEffect, useRef } from "react";
import { editNote, flushNote, noteKey } from "../lib/notes";
import { useAppState } from "../state/store";

const STATUS_TEXT = {
  saved: "Saved",
  saving: "Saving…",
  error: "Not saved yet — retrying",
} as const;

export function StickyNote({
  workspaceId,
  memberId,
  title,
  onClose,
  className,
}: {
  workspaceId: string;
  /** null = the workspace's project note */
  memberId: string | null;
  title: string;
  onClose: () => void;
  className?: string;
}) {
  const text = useAppState((s) => {
    const v = s.notes[workspaceId];
    if (!v) return "";
    return memberId === null ? v.project : (v.members[memberId] ?? "");
  });
  const status = useAppState((s) => s.noteStatus[noteKey(workspaceId, memberId)] ?? "saved");
  const loaded = useAppState((s) => workspaceId in s.notes);
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  const close = () => {
    void flushNote(workspaceId, memberId);
    onClose();
  };

  const label = memberId === null ? "Project note" : `Note for ${title}`;
  return (
    <section
      className={`sticky-note ${className ?? ""}`}
      aria-label={label}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <header className="sticky-note-header">
        <span className="sticky-note-title">{label}</span>
        <span
          className={`sticky-note-status sticky-note-${status}`}
          role="status"
          aria-live="polite"
        >
          {STATUS_TEXT[status]}
        </span>
        <button type="button" className="icon-btn" aria-label={`Close ${label}`} onClick={close}>
          ✕
        </button>
      </header>
      <textarea
        ref={ref}
        className="sticky-note-text"
        aria-label={label}
        placeholder={
          memberId === null
            ? "Notes for the whole project — saved automatically."
            : "Notes for this terminal — saved automatically."
        }
        value={text}
        disabled={!loaded}
        spellCheck={false}
        onChange={(e) => editNote(workspaceId, memberId, e.target.value)}
        onBlur={() => void flushNote(workspaceId, memberId)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        }}
      />
    </section>
  );
}
