# ADR-012: Sticky notes per terminal and per workspace

Date: 2026-11-26
Status: **Accepted**

## Context

Developers keep notes next to their terminals: what a server is for, the
next step, a command to remember. ToolOnize needs a sticky note for every
terminal and one for the whole project, reachable by a shortcut, that is
never lost — not even if the laptop shuts down.

## Decision

- **Model.** One note per workspace member (any member type) and one project
  note per workspace. Plain text, at most 256 KiB per note and 4 MiB per
  workspace. Only NUL is refused.
- **Storage.** One file per workspace (`<state>/notes/notes-<id>.json`)
  through the existing `persist::Store`: temp file → fsync → generation
  rotation → atomic rename → fsync of the folder, with a checksum and two
  older generations. A power cut leaves either the new or the previous
  complete save; a torn or corrupt newest file falls back to the previous
  generation (tested by truncating the file, and manually with `kill -9`).
- **Autosave.** The UI saves 400 ms after typing pauses, immediately on blur,
  when a note closes and when the window is hidden or closed. At most the
  last fraction of a second of typing can be lost to a sudden power loss.
  Failed saves keep the text on screen and retry with backoff
  (1, 2, 5, 10 s); a permanent rejection (size) is reported, not retried.
- **Never silently deleted.** Removing a member moves its note into the
  project note. Deleting a workspace first moves its notes into a "recently
  deleted" list (30 days, 50 entries), restorable into another project note;
  if that save fails, the workspace is not deleted.
- **Shortcuts.** `Ctrl+Shift+N` toggles the focused terminal's note,
  `Ctrl+Shift+M` the project note; both remappable. Inside a note the
  terminal clipboard/search shortcuts act on the text field.
- **Privacy.** Notes stay local: never in the audit log, the diagnostic log,
  diagnostics exports or workspace exports (tested).
- New commands: `notes_get`, `note_save`, `notes_restore`.

## Consequences

- Notes are user data; full backups (ADR-014) include them, workspace exports
  do not.
