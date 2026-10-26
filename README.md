# ToolOnize

**Your existing dev tools. One persistent workspace.**

*ToolOnize is one of the [CRMonize.com](https://crmonize.com) open-source projects.*

ToolOnize is a **local-first developer workspace orchestrator** for **Linux and Windows**. It discovers the launchers and tools you already have — without running any of them — lets you review them, and composes terminals, SSH/tmux sessions and GUI applications into persistent docking workspaces.

> **Status: V1 implemented, pending human review.** Milestones M3–M10 were implemented in one pass after the owner waived per-milestone review pauses. No release has been published. See [`docs/product/STATUS.md`](docs/product/STATUS.md) for evidence and the honest list of open items (notably manual Windows/WebView2 verification and some performance budgets).

## What it does

- **Launcher discovery (read-only).** Linux XDG application entries with spec precedence (incl. Flatpak/Snap exports), the user's Desktop folder via `xdg-user-dirs`, and opt-in custom folders; Windows Start Menu and Desktop shortcuts via Known Folders. `.lnk` files are read from their stored metadata — ToolOnize never calls `IShellLink::Resolve`. Discovery never executes anything, never scans `PATH`, never uses the network. Folders are watched for changes.
- **Conservative review.** Every launcher is classified (Terminal, Local command, Remote/SSH, External app, or Needs review) with human-readable evidence. Entries from user-writable places, interpreter launchers (`sh -c`, `cmd /c`, PowerShell `-Command`, scripts) and anything ambiguous need an explicit review of the exact command before they can run.
- **Workspaces.** Members, a docking layout (Grid / Focus / Tabs / Master + Stack) and a startup policy (Ask / layout only / start terminals / also launch apps), restored after restart.
- **Embedded terminals.** bash/zsh/fish/sh, PowerShell, cmd, WSL distributions, your own `ssh`, tmux. Moving, maximizing, switching tabs or modes never restarts a terminal. Output delivery is lossless (acknowledged, with backpressure); terminals survive a window reload. Search, copy/paste with a multi-line paste warning, exit banner with restart, honest reconnect semantics (plain SSH starts a new remote shell; tmux/screen persist).
- **SSH comfort.** Pick hosts straight from your `~/.ssh/config` (only the `Host` aliases are read), automatic reconnect with backoff when a connection drops, and clipboard copy/paste that works like a local terminal (incl. OSC 52 from tmux/vim on the server).
- **Find things fast.** Ctrl+click links in terminal output, color tags for terminals and workspaces, save a terminal's output to a file, and search across all open terminals (Ctrl+Shift+G).
- **External apps.** Authorized GUI members launch detached through the OS; ToolOnize does not manage their windows.
- **Trust model.** Execution authorization is bound to `launcher_id + descriptor_fingerprint + workspace member`. If a launcher's command changes, its members keep their place but need re-review before the next start; running sessions are untouched. All decisions are recorded in a local, redacted audit log.
- **Local-first.** No account, no cloud, no telemetry. Non-secret workspace export/import (imports land in review; nothing runs).

## Set up your computer

One command checks everything ToolOnize needs, shows a single plan, asks for your consent and administrator authorization **once**, then installs everything in one go:

```bash
./setup.sh              # Linux (and macOS toolchain)
./setup.sh --check      # report only, change nothing
```

On Windows: `powershell -ExecutionPolicy Bypass -File scripts\setup\windows.ps1`. Details, flags and what is installed: [`docs/setup/SETUP.md`](docs/setup/SETUP.md).

## Build and run

Prerequisites (installed by the setup wizard): Node.js ≥ 20, Rust ≥ 1.77.2, and on Linux the WebKitGTK 4.1 development packages.

```bash
npm ci
npm run tauri -- dev      # development window
npm run tauri -- build    # installers: deb + AppImage (Linux), NSIS (Windows)
```

Tests and checks:

```bash
npm test && npm run lint && npm run typecheck
cd src-tauri && cargo test && cargo clippy --all-targets --all-features -- -D warnings
cargo run --release --example perf    # measurement harness (Linux)
```

State lives in `$XDG_STATE_HOME/toolonize` and settings in `$XDG_CONFIG_HOME/toolonize` (Linux) or `%APPDATA%\toolonize` (Windows); `TOOLONIZE_STATE_DIR` / `TOOLONIZE_CONFIG_DIR` override them. Deleting those folders removes everything ToolOnize stored.

## Architecture in one paragraph

A Rust core (Tauri 2) owns all privileged work: discovery, registry, classification, execution policy, PTY sessions (portable-pty 0.9.0 with ToolOnize mitigations), persistence. The React/TypeScript frontend is treated as untrusted: it can only call a fixed inventory of typed commands — none takes a program, arguments or command text — and execution is reachable only by naming a workspace member that the Rust policy then checks. Each terminal session has one long-lived xterm.js host that layout changes merely attach or park. Details: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) (§15 maps components to code).

## Documentation

| Document | Purpose |
|---|---|
| [`docs/product/STATUS.md`](docs/product/STATUS.md) | Milestone status, evidence, open items |
| [`docs/product/PRD.md`](docs/product/PRD.md) | Product requirements and V1 scope |
| [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) | System architecture and code map |
| [`docs/architecture/DOMAIN_MODEL.md`](docs/architecture/DOMAIN_MODEL.md) | Entities, invariants, state machines |
| [`docs/architecture/PLATFORM_ADAPTERS.md`](docs/architecture/PLATFORM_ADAPTERS.md) | Linux/Windows adapter contracts |
| [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) | Threats, mitigations, implementation status |
| [`docs/security/DEPENDENCIES.md`](docs/security/DEPENDENCIES.md) | Dependency inventory and licenses |
| [`docs/product/TEST_STRATEGY.md`](docs/product/TEST_STRATEGY.md) | Test layers, traceability, where suites live |
| [`docs/research/perf/BASELINE.md`](docs/research/perf/BASELINE.md) | Measured performance baseline |
| [`docs/release/RELEASE_CHECKLIST.md`](docs/release/RELEASE_CHECKLIST.md) | Release gates and manual smoke matrix |
| [`docs/setup/SETUP.md`](docs/setup/SETUP.md) | Setup wizard for Linux, Windows, macOS |
| [`docs/adr/`](docs/adr/) | Architecture Decision Records |

## Non-goals (V1)

IDE/editor, cloud sync, collaboration, plugin system, AI assistant, password manager, embedding foreign GUI windows, macOS, PATH-wide discovery, auto-updater, free-form custom commands. See [`docs/product/PRD.md`](docs/product/PRD.md) §3.

## Security

See [`SECURITY.md`](SECURITY.md) for reporting vulnerabilities and [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) for the model, including what V1 intentionally does not protect against.

## About

ToolOnize is developed as one of the [CRMonize.com](https://crmonize.com) open-source projects, together with its contributors. Copyright © 2026 CRMonize.com and the ToolOnize contributors.

## Licensing

Dual-licensed at your option under the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE). Unless explicitly stated otherwise, contributions are licensed under either at your option. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

Former discovery-phase codename was "Dev Command Center" — see [`docs/research/NAMING_RESEARCH.md`](docs/research/NAMING_RESEARCH.md). Preliminary naming research is not legal trademark clearance.
