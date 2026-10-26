# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- M3 Production Terminal Session Manager on `m3-terminal-session-manager`: production `PtyBackend` trait + `PortablePtyBackend` (portable-pty 0.9.0), stateful DSR/CPR detector (ESC[6n across splits, exactly-once CPR ESC[<rows>;<cols>R), writer-lifetime guard, bounded lossless transport (chunk 4096, capacity 65536, high 49152, low 16384, hard 65536, replay 65536, sequenced, ack-after-xterm-write, backpressure, no silent drop), `ProcessSessionState`/`ViewAttachmentState` orthogonal machines with validated transitions, `SessionManager` registry (opaque SessionId, stable across reload, generation bump on restart, per-session isolation), safe opaque profile layer (no raw exec from WebView), typed Tauri commands `terminal_*` (semantic `terminal::*`), capability-restricted to main window, xterm 5.x `TerminalView` with Fit+Search addons, copy/paste with multi-line warning (bracketed-paste aware), resize via FitAddon with validation, exit banner, restart/close/detach reattach, replay for renderer reload survival, and comprehensive Rust + frontend contract tests (state machine, DSR splits, 256 KiB SHA-256 integrity, backpressure, concurrent isolation, reload reattach, clipboard/search/resize). No FlexLayout/workspace/launcher/persistence yet (M4+).
- M2 PTY Backend Technical Spike on `m2-pty-spike`: throwaway portable/direct backend harness, explicit platform reports, exact SHA-256 output checks, child-observed resize, readiness-synchronized Ctrl+C, split-aware DSR handling, safe bounded ConPTY ownership/teardown, real slow-consumer backpressure, and a fail-closed real Tauri/WebKitGTK/xterm.js auto-run. Fresh Linux xvfb and Windows hosted evidence pass, including 31/31 Windows records for both backends and exact 256 KiB SHA-256 delivery. `HUMAN_M2_GATE=APPROVED` and ADR-004 is accepted: ToolOnize V1 selects `portable-pty` 0.9.0 with ToolOnize-owned mitigations on Linux and Windows. Direct native implementations remain spike-verified fallback/reference paths only; patched forks are not selected. Dependency upgrades require the relevant M2 Linux and Windows regression matrix. PR #2 remains draft pending final CI review and human merge. Node24 GitHub Actions maintenance uses immutable verified pins.
- M1 Cross-Platform Framework Shell (complete, human-approved, and merged): Tauri 2 + React + TypeScript framework shell with single hardened IPC command `app::ping` (mapped to Tauri command `ping`), capability-restricted IPC, typed frontend wrapper `src/lib/ipc.ts`, and minimal professional UI shell displaying app identity and sanitized IPC data. No terminal, workspace, launcher, PTY, layout, SSH, tmux, or persistence features - deferred to M3+.
- M1 CI matrix (`.github/workflows/ci.yml`): ubuntu + windows, Node LTS + stable Rust, deterministic `npm ci`, frontend and Rust quality gates, and Tauri build smoke.
- Repository foundation (M0): `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, dual licensing (`LICENSE-MIT`, `LICENSE-APACHE`), ADR index, fixture policy, secret-scanning config (`.gitleaks.toml`), repository-safety workflow (`.github/workflows/repository-safety.yml`), documentation traceability checker (`tools/check-doc-traceability.py` with tests), and public-safety checker (`tools/check-public-safety.sh`).
- Product identity finalized as **ToolOnize** (human-approved; former discovery-phase codename was "Dev Command Center"). Tauri identifier direction `com.toolonize.desktop`, CLI/binary `toolonize`, config-directory direction `toolonize`.
- Architecture, PRD, threat model, implementation plan, test strategy, and roadmap marked human-approved for progression into M0/M1.

### Changed
- ADR-004 accepted `portable-pty` 0.9.0 behind `PtyBackend` for Linux and Windows with mandatory split-DSR, input-writer lifetime, bounded-lossless transport, resize, integrity, Ctrl+C, UTF-8/VT, cleanup, concurrency, and explicit-timeout regressions. Direct native implementations remain non-production fallback/reference spike paths.
- Project status and roadmap updated to record M0/M1/M2 complete, human-approved, and merged; M3 in progress (production terminal lifecycle core in progress). No workspace/layout/launcher/persistence features are claimed as done.
- Repository safety controls transitioned from M0 (forbid all app artifacts) to M1 (allow `src/`, `src-tauri/`, manifests, lockfiles; retain secret/path scanning).
- Frontend now includes `@xterm/addon-search` 0.16.0 for in-terminal search (M3), alongside xterm 5.x and fit/serialize/webgl.

### Notes
- No version has been released. Only the completed M1 framework shell exists as merged production application code; M2 is an approved technical spike merged as evidence; M3 production terminal lifecycle core is in progress on `m3-terminal-session-manager`. User-facing workspace/layout/launcher functionality remains planned for M4+.
- Preliminary naming research is not legal trademark clearance; formal review remains a later release gate if required.
