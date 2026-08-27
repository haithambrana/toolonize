# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- M1 Cross-Platform Framework Shell (in progress on `m1-framework-shell`): Tauri 2 + React + TypeScript framework shell with single hardened IPC command `app::ping` (mapped to Tauri command `ping`), capability-restricted IPC, typed frontend wrapper `src/lib/ipc.ts`, and minimal professional UI shell displaying app identity and sanitized IPC data. No terminal, workspace, launcher, PTY, layout, SSH, tmux, or persistence features — deferred to M2+.
- M1 CI matrix (`.github/workflows/ci.yml`): ubuntu + windows, Node LTS + stable Rust, deterministic `npm ci`, frontend and Rust quality gates, and Tauri build smoke.
- Repository foundation (M0): `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, dual licensing (`LICENSE-MIT`, `LICENSE-APACHE`), ADR index, fixture policy, secret-scanning config (`.gitleaks.toml`), repository-safety workflow (`.github/workflows/repository-safety.yml`), documentation traceability checker (`tools/check-doc-traceability.py` with tests), and public-safety checker (`tools/check-public-safety.sh`).
- Product identity finalized as **ToolOnize** (human-approved; former discovery-phase codename was "Dev Command Center"). Tauri identifier direction `com.toolonize.desktop`, CLI/binary `toolonize`, config-directory direction `toolonize`.
- Architecture, PRD, threat model, implementation plan, test strategy, and roadmap marked human-approved for progression into M0/M1.

### Changed
- `docs/product/STATUS.md` and `README.md` updated to reflect M0 complete, public repository created (`haithambrana/toolonize`), and M1 framework shell in progress. No terminal/workspace/launcher features claimed as done.
- Repository safety controls transitioned from M0 (forbid all app artifacts) to M1 (allow `src/`, `src-tauri/`, manifests, lockfiles; retain secret/path scanning).

### Notes
- No version has been released. Only the M1 framework shell exists/in progress; user-facing ToolOnize functionality remains planned.
- Preliminary naming research is not legal trademark clearance; formal review remains a later release gate if required.
