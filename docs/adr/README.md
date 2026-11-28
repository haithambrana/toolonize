# Architecture Decision Records

Each record explains one significant decision: the context, what was decided, the alternatives, and the consequences. New decisions that change the architecture, the security model or the handling of user data get a new record; an existing record is changed only by a newer one that supersedes it.

| ADR | Decision | Status |
|---|---|---|
| [001](001-tauri-react-rust.md) | Tauri 2 + Rust core + React/TypeScript frontend | Accepted |
| [002](002-xtermjs-terminal-ui.md) | xterm.js as terminal UI | Accepted |
| [003](003-flexlayout-workspace-layout.md) | flexlayout-react for workspace docking layout | Accepted |
| [004](004-pty-backend.md) | PTY backend — portable-pty 0.9.0 with mitigations | Accepted |
| [005](005-launcher-discovery-not-execution.md) | Launcher discovery is not execution | Accepted |
| [006](006-persistence-atomic-snapshots.md) | Persistence uses atomic checksummed snapshot generations | Accepted |
| [007](007-windows-shortcut-policy.md) | Windows shortcut discovery reads stored metadata only | Accepted |
| [008](008-synthetic-connection-launchers.md) | Typed synthetic launchers for shells, SSH and tmux | Accepted |
| [009](009-environment-setup-wizard.md) | Environment setup wizard (one plan, one consent, one authorization) | Accepted |
| [010](010-crmonize-product.md) | ToolOnize is a CRMonize.com product | Accepted |
| [011](011-ssh-hosts-reconnect-links-export.md) | SSH connection manager, auto-reconnect, terminal links, output export | Accepted |
| [012](012-sticky-notes.md) | Sticky notes per terminal and per workspace | Accepted |
| [013](013-terminal-power-features.md) | Split, folders, zoom, appearance, notifications, broadcast, snippets, presentation | Accepted |
| [014](014-backup-duplicate-templates.md) | Full backup and restore, duplicate workspace, templates | Accepted |
| [015](015-session-memory.md) | Session memory — continue after closing the app or a power loss | Accepted |
| [016](016-terminal-env-startup-command.md) | Per-terminal environment variables and startup command | Accepted |
| [017](017-open-at-login.md) | Open at login | Accepted |
| [018](018-arabic-localization.md) | Arabic interface and right-to-left layout | Accepted |
| [019](019-in-app-updates.md) | In-app updates from signed GitHub releases | Accepted |
