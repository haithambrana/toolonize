# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added
- SSH connection manager: Launchers › Connections reads the `Host` aliases from `~/.ssh/config` (and Includes inside `~/.ssh`) on request and adds them as SSH connections; nothing else from the ssh config is read (ADR-011).
- SSH auto-reconnect: a dropped SSH terminal reconnects automatically with backoff (up to 8 attempts) and shows a countdown with "Reconnect now" / "Stop reconnecting"; can be turned off in Settings.
- Ctrl+click opens links in terminal output (OSC 8 links show their target first).
- Color tags for terminals and workspaces, shown on tabs, pane headers, the sidebar and search results.
- "Save output to file" (to the Downloads folder) and "Search all terminals" (Ctrl+Shift+G) across every open terminal.
- Setup wizard (ADR-009): `./setup.sh` checks every package and toolchain requirement, shows one plan, asks for consent and administrator authorization once, installs everything in one transaction, then verifies. Linux implemented (apt, dnf, pacman, zypper — each verified end to end in CI, including a full build; whiptail or plain text; `--check`, `--dry-run`, `--yes`, `--profile dev|run`); Windows (`scripts/setup/windows.ps1`, winget with one UAC prompt) and macOS (toolchain only) prepared. Node.js and rustup downloads are pinned to SHA-256 digests. CI now installs the Linux build dependencies through the wizard. No new dependencies.

- Rename terminals: double-click (or F2 on) a tab, double-click the pane title, the terminal right-click menu, the command palette, or Ctrl+Shift+R. The name is saved with the workspace and shown for the running session without restarting it; an empty name resets to the launcher name.
- Clipboard that behaves like a local terminal, including over SSH: Ctrl+Shift+C / right-click → Copy puts the selection on this computer's clipboard; Ctrl+Shift+V / right-click → Paste reads it natively (works on WebKitGTK); optional copy-on-select; OSC 52 support so tmux/vim/neovim on a remote host can copy into the local clipboard (write-only, can be turned off). Terminal right-click menu with Copy, Paste, Select all, Clear screen, Find and Rename.
- V1 implementation of M3–M10 (pending human review; per-milestone pauses waived by the owner on 2026-10-25): Rust session manager on portable-pty with lossless acknowledged output and renderer-reload reattach; Linux and Windows launcher discovery with change watching; conservative classifier, review approvals and member authorizations bound to descriptor fingerprints; execution policy, detached external launches, audit log; atomic checksummed persistence with recovery; non-secret import/export; FlexLayout workspaces with Grid/Focus/Tabs/Master+Stack; launchers review queue, settings, command palette, themes incl. high contrast; deb/AppImage/NSIS packaging and a draft-only release workflow with SHA-256 sums and build provenance. ADR-006..008 added; ADR-003 gate passed in automated tests.
- New dependencies: `notify` 8, `sha2` 0.10, `windows-sys` 0.59 (Windows), `libc` (Unix, now non-optional), `flexlayout-react` 0.10.8, `@xterm/addon-search` 0.15.0; dev: `proptest`, `tempfile`. See `docs/security/DEPENDENCIES.md`.
- M2 PTY Backend Technical Spike on `m2-pty-spike`: throwaway portable/direct backend harness, explicit platform reports, exact SHA-256 output checks, child-observed resize, readiness-synchronized Ctrl+C, split-aware DSR handling, safe bounded ConPTY ownership/teardown, real slow-consumer backpressure, and a fail-closed real Tauri/WebKitGTK/xterm.js auto-run. Fresh Linux xvfb and Windows hosted evidence pass, including 31/31 Windows records for both backends and exact 256 KiB SHA-256 delivery. `HUMAN_M2_GATE=APPROVED` and ADR-004 is accepted: ToolOnize V1 selects `portable-pty` 0.9.0 with ToolOnize-owned mitigations on Linux and Windows. Direct native implementations remain spike-verified fallback/reference paths only; patched forks are not selected. Dependency upgrades require the relevant M2 Linux and Windows regression matrix. PR #2 remains draft pending final CI review and human merge. Node24 GitHub Actions maintenance uses immutable verified pins.
- M1 Cross-Platform Framework Shell (complete, human-approved, and merged): Tauri 2 + React + TypeScript framework shell with single hardened IPC command `app::ping` (mapped to Tauri command `ping`), capability-restricted IPC, typed frontend wrapper `src/lib/ipc.ts`, and minimal professional UI shell displaying app identity and sanitized IPC data. No terminal, workspace, launcher, PTY, layout, SSH, tmux, or persistence features - deferred to M3+.
- M1 CI matrix (`.github/workflows/ci.yml`): ubuntu + windows, Node LTS + stable Rust, deterministic `npm ci`, frontend and Rust quality gates, and Tauri build smoke.
- Repository foundation (M0): `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, dual licensing (`LICENSE-MIT`, `LICENSE-APACHE`), ADR index, fixture policy, secret-scanning config (`.gitleaks.toml`), repository-safety workflow (`.github/workflows/repository-safety.yml`), documentation traceability checker (`tools/check-doc-traceability.py` with tests), and public-safety checker (`tools/check-public-safety.sh`).
- Product identity finalized as **ToolOnize** (human-approved; former discovery-phase codename was "Dev Command Center"). Tauri identifier direction `com.toolonize.desktop`, CLI/binary `toolonize`, config-directory direction `toolonize`.
- Architecture, PRD, threat model, implementation plan, test strategy, and roadmap marked human-approved for progression into M0/M1.

### Changed
- ToolOnize is presented as a CRMonize.com open-source project (ADR-010): README, package and installer metadata, copyright line "CRMonize.com and the ToolOnize contributors". Tauri identifier is now `com.crmonize.toolonize` (was `com.toolonize.desktop`; no release had shipped).
- ADR-004 accepted `portable-pty` 0.9.0 behind `PtyBackend` for Linux and Windows with mandatory split-DSR, input-writer lifetime, bounded-lossless transport, resize, integrity, Ctrl+C, UTF-8/VT, cleanup, concurrency, and explicit-timeout regressions. Direct native implementations remain non-production fallback/reference spike paths.
- Project status and roadmap updated to record M0/M1 complete, human-approved, and merged; M2 complete and human-approved with ADR-004 accepted and PR #2 pending final merge; and M3 not started. No terminal/workspace/launcher product features are claimed as done.
- Repository safety controls transitioned from M0 (forbid all app artifacts) to M1 (allow `src/`, `src-tauri/`, manifests, lockfiles; retain secret/path scanning).

### Notes
- No version has been released. Only the completed M1 framework shell exists as production application code; M2 is an approved technical spike and M3 has not started. User-facing ToolOnize functionality remains planned.
- Preliminary naming research is not legal trademark clearance; formal review remains a later release gate if required.

### Security
- Sensitive-information review of the tree and full git history: no credentials, keys, tokens, private IPs or machine-specific files found. With the owner's approval, all branches were rewritten to remove the owner's personal account name from file contents and commit metadata (now `CRMonize.com`), then force-pushed; gitleaks 8.30.1 reports no leaks over the full history (ADR-010).

