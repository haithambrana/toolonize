# ADR-014: Full backup and restore, duplicate workspace, templates

Date: 2026-11-27
Status: **Implemented — pending human review** (requested by the owner)

## Context

Workspace exports (ADR-008, FR-025) move one workspace at a time and
deliberately leave out notes and machine details. The owner asked for a way
to move *everything* to a new machine or recover after a disk loss, to copy
a workspace, and to start new workspaces from a saved shape.

## Decision

- **Backup file.** One JSON file (`toolonize-backup`, schema 1) written to
  Downloads: all workspaces (members, titles, colors, zoom, folders,
  layouts), notes, templates, launcher aliases and hidden flags, custom
  roots, typed connections (ADR-008), and settings when the user keeps
  "Include settings" checked. Member authorizations and launcher approvals
  are removed before writing; secrets are never part of ToolOnize state.
- **Restore treats the file as hostile input** (T-CFG-01): at most 16 MiB,
  unknown top-level fields refused, format and schema checked, names,
  ids, colors, zoom, layouts (same validator as saved layouts) and notes
  (size limits) validated; folders that do not exist on this machine are
  dropped with a warning; a layout that does not match its members is
  dropped with a warning. Restored workspaces get **new ids**, are added
  next to the existing ones, are **staged for review**, have the startup
  policy **Ask**, and **every member must be authorized again**. Launcher
  approvals are never restored. Aliases only fill launchers that have no
  alias here. Settings are replaced only when the user asks for it, and an
  incompatible settings block skips only the settings. Notes are written
  first under the new ids and removed again if the workspaces cannot be
  saved, so a failed restore changes nothing.
- **Duplicate** copies a workspace with new ids, its layout rewritten to the
  new ids, and its notes. Member authorizations are kept: same machine, same
  launcher fingerprints (still re-checked at start), and the user asked for
  the copy.
- **Templates** store a workspace's shape (members, options, layout, startup
  policy) without the project folder, member start folders or notes; at
  most 50, unique names. A workspace from a template gets new ids.
  Templates in a backup are restored without authorizations.
- New commands: `workspace_duplicate`, `backup_export`, `backup_restore`,
  `template_list`, `template_save`, `template_create_workspace`,
  `template_delete`. No new dependencies.

## Consequences

- A backup holds the user's notes and folder paths; it is their file to
  protect. Settings › Backup & restore says so.
- Restoring is never a way to get something to run: the review and
  authorization flow of imports applies unchanged.
