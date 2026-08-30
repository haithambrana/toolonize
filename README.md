# ToolOnize

**Your existing dev tools. One persistent workspace.**

ToolOnize is a **local-first developer workspace orchestrator** for **Linux and Windows** (V1). It discovers the launchers and tools you already have, lets you compose them into persistent workspaces, embeds your terminal sessions, and launches your GUI tools — with explicit review before anything executes.

> **Status: M3 Production Terminal Session Manager — in progress** on branch `m3-terminal-session-manager`. M0/M1/M2 complete, human-approved, and merged. M3 delivers the production PTY lifecycle core (portable-pty 0.9.0 + ToolOnize mitigations) with session manager, lossless transport, and xterm TerminalView. See [`docs/product/STATUS.md`](docs/product/STATUS.md).

Former discovery-phase codename was "Dev Command Center" — see [`docs/research/NAMING_RESEARCH.md`](docs/research/NAMING_RESEARCH.md). Preliminary naming research is not legal trademark clearance.

---

## Planned V1 capabilities

- **Launcher discovery** — Linux XDG application entries (incl. Flatpak/Snap exports, the user's Desktop directory via `xdg-user-dirs`, and opt-in custom roots) and Windows Known Folders / `.lnk` shortcuts. Discovery is metadata-only; nothing discovered ever executes automatically.
- **Conservative classification & review** — every discovered launcher is classified as Embedded Terminal candidate, Local Command, Remote/SSH, External GUI, or Unknown/Needs Review. Unknown is the safe default.
- **Persistent workspaces** — members + docking layout (Grid / Focus / Tabs / Master+Stack) + startup policy, restored after restart.
- **Embedded PTY terminal members** — bash/sh, PowerShell, cmd, WSL distros, the user's `ssh`, and tmux attach interop. Resize, drag, and mode switches never respawn the underlying PTY.
- **Authorized external GUI launching** — external apps launch detached via the OS mechanism; ToolOnize does not manage foreign windows.
- **SSH/tmux interoperability** — delegates to the user's own `ssh` and `tmux`/`screen`; honest reconnection semantics (plain SSH reconnects start a new remote session unless the remote itself persists).
- **Conservative authorization** — execution is bound to `launcher_id + descriptor_fingerprint + workspace/member scope` and lives in the Rust core; the WebView never receives raw spawn power.

**Non-goals for V1:** IDE/editor, cloud sync, collaboration, plugin system, AI assistant, password manager, macOS support, PATH-wide discovery, auto-updater. See [`docs/product/PRD.md`](docs/product/PRD.md) §3 and §15.

Implementation has **progressed** through M2 PTY spike (portable-pty 0.9.0 + mitigations selected per ADR-004) and is now in **M3** (production terminal lifecycle core) on branch `m3-terminal-session-manager` — session manager, process/view state separation, lossless bound transport with ack, DSR/CPR mitigations, and xterm TerminalView. The existing Python/GTK prototype in the separate repository remains disposable and not part of this codebase (per `AGENTS.md`). Workspace/layout docking (M4) and launcher discovery (M5/M6) remain planned.

---

## Documentation

| Document | Purpose |
|---|---|
| [`docs/product/PRD.md`](docs/product/PRD.md) | Product requirements and V1 scope |
| [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) | System architecture and component boundaries |
| [`docs/architecture/DOMAIN_MODEL.md`](docs/architecture/DOMAIN_MODEL.md) | Domain entities, invariants, state machines |
| [`docs/architecture/PLATFORM_ADAPTERS.md`](docs/architecture/PLATFORM_ADAPTERS.md) | Linux/Windows adapter contracts |
| [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) | Assets, trust boundaries, threats, mitigations |
| [`docs/security/PUBLIC_REPOSITORY_SAFETY.md`](docs/security/PUBLIC_REPOSITORY_SAFETY.md) | Rules that keep the repo safe for public release |
| [`docs/product/ROADMAP.md`](docs/product/ROADMAP.md) | Phased milestones M0–M10 |
| [`docs/product/IMPLEMENTATION_PLAN.md`](docs/product/IMPLEMENTATION_PLAN.md) | Per-milestone work units, tests, gates |
| [`docs/product/TEST_STRATEGY.md`](docs/product/TEST_STRATEGY.md) | Test layers, fixtures, traceability matrix |
| [`docs/product/STATUS.md`](docs/product/STATUS.md) | Current phase and gate status |
| [`docs/adr/`](docs/adr/) | Architecture Decision Records |
| [`docs/testing/FIXTURE_POLICY.md`](docs/testing/FIXTURE_POLICY.md) | Fictional-fixture rules |

---

## Project phase

`M3 — Production Terminal Session Manager` is in progress on branch `m3-terminal-session-manager`. `M0/M1/M2` complete, human-approved, and merged; `ADR-004` accepted (portable-pty 0.9.0 + mitigations).

```
M0: COMPLETE / HUMAN APPROVED / MERGED
M1: COMPLETE / HUMAN APPROVED / MERGED
M2: COMPLETE / HUMAN APPROVED / MERGED
ADR-004: ACCEPTED (portable-pty 0.9.0 + ToolOnize mitigations)
M3: IN PROGRESS — Production Terminal Session Manager
Public repository: CREATED — haithambrana/toolonize
Repository safety CI: GREEN
```

## Platforms

V1 targets **Linux and Windows** via one cross-platform codebase with platform adapters. macOS is not in V1 scope.

## Security model (summary)

- Discovery never executes.
- Only explicitly pinned/authorized members are executable; authorization is checked in Rust against `launcher_id + descriptor_fingerprint + scope`.
- No generic exec IPC command exists; every WebView→Rust command is typed, validated, and capability-scoped (Tauri 2).
- No plaintext credential storage; SSH delegates to the user's agent/config.
- See [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) and [`SECURITY.md`](SECURITY.md).

## Licensing

Dual-licensed at your option:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

Unless explicitly stated otherwise, contributions are licensed under **either** MIT **or** Apache-2.0 at your option. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

Preliminary naming research is not legal trademark clearance; formal review remains a later release gate if required.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). **Do not include secrets, machine-specific launchers, SSH configuration, credentials, IP addresses, usernames, tokens, or private paths** in issues, PRs, or fixtures.

## Questions

For product direction see the PRD and ADRs. For repository usage see `AGENTS.md`.
