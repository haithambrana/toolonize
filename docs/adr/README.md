# Architecture Decision Records

Index of decisions affecting ToolOnize (formerly discovery-phase codename "Dev Command Center"). Statuses are human-approved gates; do not change them without an ADR and human review.

| ADR | Title | Status |
|---|---|---|
| [001](001-tauri-react-rust.md) | Tauri 2 + Rust core + React/TypeScript frontend | **Accepted** |
| [002](002-xtermjs-terminal-ui.md) | xterm.js terminal UI | **Accepted** |
| [003](003-flexlayout-workspace-layout.md) | flexlayout-react with a stable terminal-host architecture | **Accepted (M4 gate passed in automated tests) — pending human review** |
| [004](004-pty-backend-spike-required.md) | portable-pty 0.9.0 + ToolOnize mitigations on Linux and Windows | **Accepted** - direct native paths remain spike fallback/reference only |
| [005](005-launcher-discovery-not-execution.md) | Launcher discovery is not execution | **Accepted** |
| [006](006-persistence-atomic-snapshots.md) | Persistence via atomic checksummed snapshot generations | **Implemented — pending human review** |
| [007](007-windows-shortcut-policy.md) | Windows `.lnk` discovery reads stored metadata only; duplicates kept; packaged apps deferred | **Implemented — pending human review and Windows runtime verification** |
| [008](008-synthetic-connection-launchers.md) | Typed synthetic launchers for shells, SSH and tmux | **Implemented — pending human review** |
| [009](009-environment-setup-wizard.md) | Environment setup wizard: one plan, one consent, one authorization | **Implemented (Linux) / prepared (Windows, macOS) — pending human review** |
| [010](010-crmonize-stewardship.md) | ToolOnize is a CRMonize.com open-source project | **Implemented — pending human review** |
| [011](011-ssh-hosts-reconnect-links-export.md) | SSH connection manager, auto-reconnect, terminal links, output export (amends T-TERM-03) | **Implemented — pending human review** |

ADR-003's M4 state-preservation gate passed in automated tests (see the ADR); ADR-006..011 record decisions made while implementing M3–M10 in one pass after the owner waived per-milestone review pauses on 2026-10-25. They await human review. ADR-004 was accepted after the M2 Linux, Windows, and real-WebView evidence passed. Any portable-pty upgrade must rerun the relevant M2 matrix on both platforms before adoption; `Cargo.lock` remains authoritative.

Preliminary naming research is not legal trademark clearance; formal legal/trademark review remains a later release gate if required.
