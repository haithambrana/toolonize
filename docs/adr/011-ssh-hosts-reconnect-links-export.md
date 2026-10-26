# ADR-011: SSH connection manager, auto-reconnect, terminal links, output export

Date: 2026-10-26
Status: **Implemented — pending human review** (features chosen by the owner)

## Context

The owner picked five additions: Ctrl+click links, an SSH connection manager,
SSH auto-reconnect, color tags, and scrollback export/global search. Three of
them touch security rules: THREAT_MODEL T-TERM-03 said ToolOnize never reads
`~/.ssh`; opening links turns untrusted terminal output into an action; and
export writes files on the user's disk.

## Decisions

### SSH connection manager (amends T-TERM-03)

- On an explicit click ("Read hosts from ~/.ssh/config") the core reads
  `~/.ssh/config` and files it `Include`s **from inside `~/.ssh` only**
  (canonicalized, so symlinks cannot escape; 256 KiB per file, 32 files,
  depth 4, 500 hosts). Nothing is read at startup or in the background.
- Only `Host` aliases leave `sshconfig.rs`. Host names, users, ports,
  identity files, proxy commands and every other option are never returned,
  stored or logged. Patterns (`*`, `?`), negations (`!`) and anything outside
  the existing launcher grammar (`shells::valid_host`, no leading `-`) are
  dropped, so an alias can never become an ssh option.
- Keys, `known_hosts` and all other files in `~/.ssh` are never opened.
- Adding a host creates the same typed synthetic launcher as typing it
  (ADR-008); it still needs workspace authorization, and connecting runs the
  user's own `ssh`, which reads the full config itself.
- The audit log records only the number of aliases read.

### SSH auto-reconnect

- The session layer already marks a remote session `Disconnected` when ssh
  exits with 255 (connection failed or dropped). A normal `exit` returns the
  remote shell's status, and a user Stop ends as `Closed`; neither reconnects.
- Backoff 2, 4, 8, 16, 30, 30… seconds, at most 8 attempts; a connection that
  stayed up 60 s starts again from attempt 1. Each attempt is the existing
  `reconnect_session` path, so the execution policy re-checks authorization.
- The UI shows the countdown with "Reconnect now" and "Stop reconnecting"
  (`session_cancel_reconnect`). Setting `ssh_auto_reconnect` (default on).
- Plain SSH still starts a *new* remote shell; only tmux/screen persist
  (FR-036 wording unchanged).

### Terminal links

- Plain-text `http(s)` URLs open only on **Ctrl+click** (Cmd on macOS), never
  on a plain click, through the existing audited, allowlisted `open_url`.
- OSC 8 hyperlinks hide their target behind text, so the target is shown in a
  confirmation dialog first; non-http(s) OSC 8 schemes are refused by xterm.js
  and again by the Rust allowlist.

### Output export and global search

- "Save output to file" sends the text and a display name only. The core
  picks the user's Downloads folder (`XDG_DOWNLOAD_DIR` / the Downloads Known
  Folder), builds a sanitized name (`toolonize-<name>-<UTC stamp>.txt`),
  creates it with `create_new` (never overwrites), caps it at 32 MiB, and
  returns the path. The WebView never passes a path.
- "Search all terminals" (Ctrl+Shift+G) searches the in-memory scrollback of
  open terminals in the frontend; nothing new crosses the IPC boundary.

### Color tags

- A fixed palette of eight ids, validated in Rust for members and
  workspaces; carried through export/import, where unknown values are dropped
  with a warning (cosmetic data never fails an import). Colors never affect
  execution and are always paired with a text label for accessibility.

## Consequences

- New commands: `ssh_hosts`, `scrollback_export`, `workspace_set_color`,
  `session_cancel_reconnect` (inventory, build manifest and capability file
  updated together; the inventory tests enforce it). No new dependencies.
- T-TERM-03 now reads: ToolOnize reads only `Host` aliases from the user's
  ssh config, on request, and never key material.
