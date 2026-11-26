# ADR-013: Split, folders, zoom, appearance, notifications, broadcast, snippets, presentation

Date: 2026-11-26
Status: **Implemented — pending human review** (features approved by the owner)

## Decisions

- **Quick split** (`Ctrl+Shift+E` right, `Ctrl+Shift+O` down, terminal menu):
  adds a default-shell member beside the focused pane by editing the saved
  layout JSON (`placeBeside`, pure and unit-tested). Tabs/Focus switch to
  Grid because two panes are then visible.
- **Folders.** A workspace project folder and a per-member start folder
  (set by split from the source terminal's current folder) are used only
  when the launcher defines no working directory. They must be existing
  absolute paths; a folder that later disappears is skipped (the shell
  starts at home) instead of blocking the start. A folder is not command
  text and does not change what runs; it is validated in Rust and never
  exported. The current folder comes from the shell's OSC 7 report or, for
  local terminals on Linux, `/proc/<pid>/cwd`; remote sessions report none.
- **Zoom** per terminal (`Ctrl+=`, `Ctrl+-`, `Ctrl+0`), -6..+12, stored on
  the member. **Appearance:** font family (charset-validated, it ends up in
  CSS), cursor style/blink, eight named color schemes; high contrast always
  keeps its own colors.
- **Desktop notifications** (tauri-plugin-notification, called from Rust
  only, no JS capability): when a terminal the user is not looking at exits,
  fails or disconnects, rings the bell, or a program asks via OSC 9 / OSC
  777. Settings switch, 6 per 30 s rate limit and length limits in Rust;
  "not looking" (window unfocused, terminal hidden, other workspace or
  screen) is decided in the UI. Sequences in replayed history never notify.
- **Broadcast** (`Ctrl+Shift+B`): keystrokes typed in one terminal are also
  written to every running terminal of the same workspace. It is the
  user's own typing, through the same write path, so no new execution path
  exists; the risk is typing into the wrong place, so: an explicit toggle,
  a warning toast with the count, a "Broadcast" chip on every receiving
  pane, and it turns off whenever the user switches workspace.
- **Snippets** (`Ctrl+Shift+S`): saved text (≤100, 8 KiB each) inserted
  like clipboard text — the multi-line paste warning applies and ToolOnize
  never executes it on its own.
- **Presentation mode** (`Ctrl+Shift+F11`): hides sidebar, toolbar and the
  project note drawer and adds 4 pt to terminal text.

## Found while implementing

The launcher folder watcher counted read events (open/close/access time) as
changes. Every scan reads the watched folders, so each scan triggered the
next: about three full scans per second for as long as the app ran. Only
real changes (create, modify, delete, rename, unknown) now trigger a rescan;
a regression test reads the watched folder repeatedly and fails on the old
behavior.
