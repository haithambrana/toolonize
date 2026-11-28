# ADR-008: Typed synthetic launchers for shells, SSH and tmux

Date: 2026-10-25
Status: **Accepted**

## Context

ToolOnize embeds bash/sh, PowerShell, cmd, WSL distributions, the
user's `ssh`, and tmux attach. Many machines have no `.desktop`/`.lnk` entry
for "ssh to my build host". A free-form command editor is out of scope,
and DOMAIN_MODEL allows "trusted synthetic system launchers produced by the
native core" with fixed typed fields. THREAT_MODEL T-TERM-03 forbids reading
`~/.ssh`.

## Decision

The core generates synthetic launchers (origin `synthetic`) from typed,
validated fields only:

| Kind | Fields | argv built by the core |
|---|---|---|
| Shell | discovered on this machine (`$SHELL`, bash, zsh, fish, sh; pwsh, Windows PowerShell, cmd) | `<shell>` (+ `-l` for the login shell, `-NoLogo` for PowerShell) |
| WSL | distribution name read from the per-user Lxss registry key | `wsl.exe -d <name>` |
| SSH | host alias | `ssh -- <host>` |
| SSH + tmux | host alias, session name | `ssh -t -- <host> tmux new-session -A -s <session>` |
| tmux | session name | `tmux new-session -A -s <session>` |

- Host grammar: 1–253 chars of `[A-Za-z0-9._-@:[]]`, not starting with `-`;
  `--` additionally ends ssh option parsing. Session grammar: 1–64 chars of
  `[A-Za-z0-9_-]`, not starting with `-`. Anything else is rejected.
- ToolOnize never reads ssh configuration or keys; host aliases resolve
  through the user's own ssh config when ssh runs.
- Synthetic launchers still require explicit authorization: they only run as
  workspace members, through the same execution policy, and are audited.
- tmux-based members are marked `detach_expected`; the UI states that
  persistence comes from tmux, and that a plain SSH reconnect starts a new
  remote shell.

## Consequences

Users can build SSH/tmux workspaces without hand-written launcher files,
without ToolOnize ever accepting free-form command text.

## Links

`src-tauri/src/shells.rs`, `src-tauri/src/policy.rs`; DOMAIN_MODEL "Member", THREAT_MODEL T-TERM-03, T-WEB-01.
