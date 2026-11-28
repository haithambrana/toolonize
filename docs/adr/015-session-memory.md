# ADR-015: Session memory — continue after closing the app or a power loss

Date: 2026-11-27
Status: **Accepted**

## Context

When the laptop suddenly turns off, or the user closes everything, they
want to get back the same window with the same content and continue as if
they had never closed it.

Before this change ToolOnize restored workspaces, layouts and notes, but
every terminal started empty, in its default folder, and only when the
workspace's startup policy allowed it.

## Decision

- **What is remembered** (in `<state>/memory/`, written through the atomic
  `persist::Store` of ADR-006):
  - the terminals that are running, with their size and — for local shells —
    their current folder (read from the process, e.g. `/proc/<pid>/cwd`);
    saved within about a second of any start/stop and every 15 seconds
    (a `cd` produces no event);
  - the last part of each terminal's output (at most 256 KiB of cleaned
    text from the last 1 MiB of output) and its width, every 15 seconds for
    terminals with new output, and at a normal exit;
  - the window's size, place and maximized state.
- **Crash detection.** The session file carries `clean_exit`, set only at a
  normal exit, so the next start knows whether the previous run ended
  abruptly. A normal exit saves everything *before* stopping terminals, so
  closing the app is remembered the same way as a power loss.
- **What comes back at the next start** (before the window opens):
  - shells and SSH sessions that were running start again in the same folder
    and size, through the normal execution policy (authorized member,
    unchanged launcher fingerprint, workspace not in review staging) —
    refusals are reported, never bypassed;
  - **commands are not started again automatically**, because they can have
    side effects (a deploy script must not run twice). They are listed, and
    start by hand — except members the user marked "restart automatically";
  - each terminal first shows its saved output, then a dim
    "── restored from your previous session ──" line, then the new shell.
    The view shows the saved text at the width it was written at and then
    fits the pane, so lines the shell wrapped come back intact;
  - the window returns to its place if that place is still on a connected
    monitor (clamped to it);
  - workspaces that got terminals back count as opened, so their "Ask"
    policy does not ask again.
- **Replay safety.** Saved output is reduced to printable text, line
  controls and harmless formatting (SGR colors, erase-in-line, moves within
  the line). Removed: all OSC strings (clipboard writes, titles, folders,
  notifications, hyperlinks), DCS/APC/PM/SOS strings, device and mode
  queries whose answers would be typed into the new shell (DSR, DA, DECRQM,
  DECRQSS, window reports), full resets, mode changes, C1 controls, BEL,
  and everything shown on the alternate screen (full-screen programs).
  The text is cleaned when saved and again when loaded. Unit tests include
  a deterministic fuzz test that only safe escapes ever survive.
- **Privacy.** Terminal output can contain secrets, like a shell history
  file does. It stays on this computer in a folder readable only by the
  user (0700 on Unix; the per-user profile on Windows); it is never part of
  backups, exports, diagnostics, the audit log or the diagnostic log
  (tested). Settings › Session memory has two switches — "Start the
  terminals that were running last time" and "Remember what each terminal
  showed" (turning it off deletes what was saved) — and a "Forget saved
  terminal output" button. Closing a terminal yourself, removing it, or
  deleting its workspace forgets its output; leftovers of members that no
  longer exist are removed at startup.
- **Defaults.** Both switches are on: this is the behaviour users ask for,
  and it matches what users expect from terminals that restore sessions.
- New command: `memory_forget`. No new dependencies.

## Consequences

- At most the last ~15 seconds of output (and ~1 second of start/stop
  changes) can be missing after a sudden power loss; notes keep their own
  0.4-second autosave (ADR-012).
- A restarted shell is a new process: programs that were running inside it
  (an editor, a server started by hand) are not running again — their last
  output is shown. tmux/screen members keep their remote sessions.
- On Windows the folder of a local shell is not read from the process yet,
  so it starts in the member/project folder; the rest works the same.
- On Wayland the compositor decides window placement; size and maximized
  state are still restored.
