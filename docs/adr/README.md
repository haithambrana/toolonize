# Architecture Decision Records

Index of decisions affecting ToolOnize (formerly discovery-phase codename "Dev Command Center"). Statuses are human-approved gates; do not change them without an ADR and human review.

| ADR | Title | Status |
|---|---|---|
| [001](001-tauri-react-rust.md) | Tauri 2 + Rust core + React/TypeScript frontend | **Accepted** |
| [002](002-xtermjs-terminal-ui.md) | xterm.js terminal UI | **Accepted** |
| [003](003-flexlayout-workspace-layout.md) | flexlayout-react leading choice; requires M4 live-terminal state-preservation gate | **Conditional** — leading choice; must pass the M4 gate (direct mounting vs portal/host fallback; dockview fallback only if proven necessary) |
| [004](004-pty-backend-spike-required.md) | portable-pty 0.9.0 + ToolOnize mitigations on Linux and Windows | **Accepted** - direct native paths remain spike fallback/reference only |
| [005](005-launcher-discovery-not-execution.md) | Launcher discovery is not execution | **Accepted** |

Do not upgrade ADR-003 without its M4 state-preservation gate. ADR-004 was accepted after the M2 Linux, Windows, and real-WebView evidence passed. Any portable-pty upgrade must rerun the relevant M2 matrix on both platforms before adoption; `Cargo.lock` remains authoritative.

Preliminary naming research is not legal trademark clearance; formal legal/trademark review remains a later release gate if required.
