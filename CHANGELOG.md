# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- M2 PTY Backend Technical Spike (on `m2-pty-spike`, spike complete / human decision required): throwaway harness `tools/spike-pty` (portable-pty 0.9.0 + mitigations + direct `libc::openpty`/`CreatePseudoConsole`), deterministic synthetic fixtures, bounded lossless transport experiment, and full pipeline spike `PTY -> Rust -> Tauri Channel -> WebView -> xterm.js` (`src-tauri/src/commands/spike.rs` behind feature `spike`, `src/spike/TerminalSpike.tsx` with `@xterm/xterm` + `FitAddon` + `Channel`). Linux matrix 32 tests, 30 PASS, 2 NOT_VERIFIED (Windows-only hidden console), both backends PASS every MUST row; transport 2 MiB lossless true; simulated pipeline `524288 -> 524288 lossless true`. Report `docs/research/PTY_SPIKE_REPORT.md` + `docs/research/spike-m2/report.json`; ADR-004 remains `PROPOSED — SPIKE COMPLETE / HUMAN DECISION REQUIRED` pending Windows CI. Node24 GitHub Actions maintenance (verified pins: checkout `v6.1.0` `d23441a`, setup-node `v6.5.0` `2499707`, setup-python `v6.3.0` `ece7cb0`).
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
