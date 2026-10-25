# Dependency inventory and license review (M10 supply-chain gate)

Snapshot: 2026-10-25, from `src-tauri/Cargo.lock` and `package-lock.json`.
Lockfiles are authoritative; nothing is upgraded automatically. Advisory
gating (cargo/npm audit as a blocking check) remains a human decision.

## Direct Rust dependencies

| Crate | Version | License | Why |
| --- | --- | --- | --- |
| tauri | 2.11.5 | Apache-2.0 OR MIT | App framework (ADR-001) |
| tauri-build | 2.6.3 | Apache-2.0 OR MIT | Build-time manifest/ACL generation |
| portable-pty | =0.9.0 (pinned) | MIT | PTY backend (ADR-004; upgrades require the M2 matrix) |
| notify | 8.2.1 | CC0-1.0 | inotify / ReadDirectoryChangesW watching (FR-005) — added in M5 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | Descriptor fingerprints, state checksums — added in M3/M8 |
| serde, serde_json | 1.x | MIT OR Apache-2.0 | IPC and state serialization |
| anyhow | 1.x | MIT OR Apache-2.0 | Error type returned by portable-pty |
| libc (unix) | 0.2.189 | MIT OR Apache-2.0 | Process-group signals for session cleanup |
| windows-sys (windows) | 0.59.0 | MIT OR Apache-2.0 | `SHGetKnownFolderPath`, Lxss registry read, `ShellExecuteW` — added in M6 |
| nix (optional, `spike` feature only) | 0.28.0 | MIT | M2 spike harness; not in production builds |

Dev-only: `proptest` 1.12 (MIT OR Apache-2.0, property tests),
`tempfile` 3.27 (MIT OR Apache-2.0), `tauri` with the `test` feature.

## Direct npm dependencies

| Package | Version | License | Why |
| --- | --- | --- | --- |
| react, react-dom | 19.2.8 | MIT | UI (ADR-001) |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | IPC client |
| @xterm/xterm | 5.5.0 | MIT | Terminal renderer (ADR-002) |
| @xterm/addon-fit | 0.10.0 | MIT | Terminal sizing |
| @xterm/addon-search | 0.15.0 (exact) | MIT | Scrollback search (FR-034) — added in M3 |

Removed in M10: `@xterm/addon-serialize`, `@xterm/addon-webgl` (unused).
| flexlayout-react | 0.10.8 (exact) | MIT | Docking layout (ADR-003) — added in M4 |

Dev-only tooling (all MIT except TypeScript, Apache-2.0): vite, vitest,
jsdom, Testing Library, eslint + plugins, prettier, typescript,
@tauri-apps/cli (Apache-2.0 OR MIT).

## Transitive license summary (Rust, 457 crates)

All crates are permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense,
Unicode-3.0, CC0) except the following, which are compatible with
distributing ToolOnize under MIT OR Apache-2.0 as unmodified dependencies:

| Crate | License | Note |
| --- | --- | --- |
| cssparser, cssparser-macros, selectors, dtoa-short, option-ext | MPL-2.0 | File-level copyleft; pulled in by Tauri/WRY. Unmodified; source available from crates.io. |
| r-efi | MIT OR Apache-2.0 OR LGPL-2.1-or-later | Used under MIT/Apache-2.0. |
| target-lexicon | Apache-2.0 WITH LLVM-exception | Build-time. |

Bundled installers must include third-party notices; generating an
aggregated `THIRD_PARTY_LICENSES` file is part of the release checklist.

## Policy

- No dependency is added without recording it here and in the commit
  message (AGENTS.md rule 6).
- GitHub Actions are pinned to full commit SHAs.
- `portable-pty` stays at `=0.9.0` until a reviewed ADR-004 upgrade.
