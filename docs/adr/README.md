# Architecture Decision Records

Index of decisions affecting ToolOnize (formerly discovery-phase codename "Dev Command Center"). Statuses are human-approved gates; do not change them without an ADR and human review.

| ADR | Title | Status |
|---|---|---|
| [001](001-tauri-react-rust.md) | Tauri 2 + Rust core + React/TypeScript frontend | **Accepted** |
| [002](002-xtermjs-terminal-ui.md) | xterm.js terminal UI | **Accepted** |
| [003](003-flexlayout-workspace-layout.md) | flexlayout-react leading choice; requires M4 live-terminal state-preservation gate | **Conditional** — leading choice; must pass the M4 gate (direct mounting vs portal/host fallback; dockview fallback only if proven necessary) |
| [004](004-pty-backend-spike-required.md) | PTY backend — spike required | **Proposed / Spike Required** — must be resolved during M2 |
| [005](005-launcher-discovery-not-execution.md) | Launcher discovery is not execution | **Accepted** |

Do not upgrade ADR-003 or ADR-004 without their respective gates. ADR-003 remains conditional on the M4 state-preservation proof (see `docs/product/IMPLEMENTATION_PLAN.md` M4). ADR-004 remains Proposed / Spike Required and must be resolved during the M2 critical integration risk gate.

Preliminary naming research is not legal trademark clearance; formal legal/trademark review remains a later release gate if required.
