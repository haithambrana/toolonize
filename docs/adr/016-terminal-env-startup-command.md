# ADR-016: Per-terminal environment variables and startup command

Date: 2026-11-27
Status: **Accepted**

## Context

Users want a terminal that opens with project variables set (for example
`NODE_ENV`, `API_URL`) and, for shells, a first command typed for them (for
example `npm run dev` or `cd src && make watch`). The IPC rule is that no
command accepts a program, argument list or command line.

## Decision

- **Model.** Each member has a `LaunchConfig`: at most 32 environment
  variables (`NAME=value`, names `[A-Za-z_][A-Za-z0-9_]*` up to 64 chars,
  values up to 4 KiB, no NUL, no duplicates) and an optional startup command
  (one line, up to 1024 chars).
- **Where it applies.** Variables apply to embedded members (shells, SSH,
  commands) and are added on top of the inherited environment; never to
  external GUI apps. The startup command applies only to shells and SSH
  terminals.
- **The startup command is typed, not executed.** After the shell prints
  its first output (or after 3 s) the core writes the line plus Enter into
  the terminal — the same path as the user's own typing (`session_write`),
  which the WebView can already use. It is never passed to `exec`, never
  turned into arguments, and adds no new execution path; the rule that no command line is executed
  from IPC still holds. It is typed again after
  a restart or SSH reconnect, and after a session-memory resume (ADR-015),
  because "when this terminal starts" is what the user configured.
- **Refused variables** (defense in depth, not the main control): names
  that make the dynamic loader or a shell run extra code — `LD_*`, `DYLD_*`,
  `BASH_FUNC_*`, `BASH_ENV`, `ENV`, `PROMPT_COMMAND`, `PS0`, `PS4`,
  `SHELLOPTS`, `BASHOPTS`, `IFS`, `ZDOTDIR`, `GCONV_PATH`, `NODE_OPTIONS`,
  `PERL5OPT`, `RUBYOPT`, `PYTHONSTARTUP`, `JAVA_TOOL_OPTIONS`, `COMSPEC`,
  `PATHEXT`.
- **Confirmation is the control.** The configuration carries a digest of
  what the user confirmed; saving it in Terminal settings confirms it. A
  configuration that is not confirmed — restored from a backup, or changed
  outside the app — does not start: the core refuses with a clear reason and
  the UI shows "Check settings…" instead of Start.
- **Secrets.** Values may be tokens: backups keep names and the startup
  command but blank every value; workspace exports keep neither; the audit
  log records counts only ("2 environment variable(s), startup command
  set"); the diagnostic log and diagnostics never contain them. The state
  and settings folders are now private to the user (0700 on Unix).
- New command `member_confirm_launch`; `member_update` takes `env` and
  `startup_command`. No new dependencies.

## Consequences

- A compromised WebView gains nothing it did not have (it could already
  type into a running shell). A tampered state file or a hostile backup
  cannot make a terminal start with new variables or a new command without
  the user seeing and saving it.
- The denylist is not exhaustive by design; it blocks the well-known loader
  and shell hooks so that a confirmed-looking configuration cannot hide
  code behind a harmless program.
