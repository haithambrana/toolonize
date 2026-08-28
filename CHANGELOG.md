# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- M2 PTY Backend Technical Spike on `m2-pty-spike`: throwaway portable/direct backend harness, explicit platform reports, exact SHA-256 output checks, child-observed resize, readiness-synchronized Ctrl+C, split-aware DSR handling, safe bounded ConPTY ownership/teardown, real slow-consumer backpressure, and a fail-closed real Tauri/WebKitGTK/xterm.js auto-run. Fresh Linux xvfb and Windows hosted evidence pass, including 31/31 Windows records for both backends and exact 256 KiB SHA-256 delivery. `HUMAN_M2_GATE=APPROVED` and ADR-004 is accepted: ToolOnize V1 selects `portable-pty` 0.9.0 with ToolOnize-owned mitigations on Linux and Windows. Direct native implementations remain spike-verified fallback/reference paths only; patched forks are not selected. Dependency upgrades require the relevant M2 Linux and Windows regression matrix. PR #2 remains draft pending final CI review and human merge. Node24 GitHub Actions maintenance uses immutable verified pins.
- M1 Cross-Platform Framework Shell (complete, human-approved, and merged): Tauri 2 + React + TypeScript framework shell with single hardened IPC command `app::ping` (mapped to Tauri command `ping`), capability-restricted IPC, typed frontend wrapper `src/lib/ipc.ts`, and minimal professional UI shell displaying app identity and sanitized IPC data. No terminal, workspace, launcher, PTY, layout, SSH, tmux, or persistence features - deferred to M3+.
- M1 CI matrix (`.github/workflows/ci.yml`): ubuntu + windows, Node LTS + stable Rust, deterministic `npm ci`, frontend and Rust quality gates, and Tauri build smoke.
- Repository foundation (M0): `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, dual licensing (`LICENSE-MIT`, `LICENSE-APACHE`), ADR index, fixture policy, secret-scanning config (`.gitleaks.toml`), repository-safety workflow (`.github/workflows/repository-safety.yml`), documentation traceability checker (`tools/check-doc-traceability.py` with tests), and public-safety checker (`tools/check-public-safety.sh`).
- Product identity finalized as **ToolOnize** (human-approved; former discovery-phase codename was "Dev Command Center"). Tauri identifier direction `com.toolonize.desktop`, CLI/binary `toolonize`, config-directory direction `toolonize`.
- Architecture, PRD, threat model, implementation plan, test strategy, and roadmap marked human-approved for progression into M0/M1.

### Changed
- ADR-004 accepted `portable-pty` 0.9.0 behind `PtyBackend` for Linux and Windows with mandatory split-DSR, input-writer lifetime, bounded-lossless transport, resize, integrity, Ctrl+C, UTF-8/VT, cleanup, concurrency, and explicit-timeout regressions. Direct native implementations remain non-production fallback/reference spike paths.
- Project status and roadmap updated to record M0/M1 complete, human-approved, and merged; M2 complete and human-approved with ADR-004 accepted and PR #2 pending final merge; and M3 not started. No terminal/workspace/launcher product features are claimed as done.
- Repository safety controls transitioned from M0 (forbid all app artifacts) to M1 (allow `src/`, `src-tauri/`, manifests, lockfiles; retain secret/path scanning).

### Notes
- No version has been released. Only the completed M1 framework shell exists as production application code; M2 is an approved technical spike and M3 has not started. User-facing ToolOnize functionality remains planned.
- Preliminary naming research is not legal trademark clearance; formal review remains a later release gate if required.
