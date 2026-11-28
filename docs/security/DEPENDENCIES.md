# Dependency inventory and license review

From `src-tauri/Cargo.lock` and `package-lock.json`. Lockfiles are
authoritative; nothing is upgraded automatically. `cargo deny` and
`npm audit` run as blocking checks in CI (see "Supply-chain checks").

## Direct Rust dependencies

| Crate | Version | License | Why |
| --- | --- | --- | --- |
| tauri | 2.11.5 | Apache-2.0 OR MIT | App framework (ADR-001) |
| tauri-build | 2.6.3 | Apache-2.0 OR MIT | Build-time manifest/ACL generation |
| portable-pty | =0.9.0 (pinned) | MIT | PTY backend (ADR-004; upgrades must pass the PTY regression tests on Linux and Windows) |
| notify | 8.2.1 | CC0-1.0 | inotify / ReadDirectoryChangesW watching for launcher changes |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | Descriptor fingerprints, state checksums |
| serde, serde_json | 1.x | MIT OR Apache-2.0 | IPC and state serialization |
| anyhow | 1.x | MIT OR Apache-2.0 | Error type returned by portable-pty |
| libc (unix) | 0.2.189 | MIT OR Apache-2.0 | Process-group signals for session cleanup |
| windows-sys (windows) | 0.59.0 | MIT OR Apache-2.0 | `SHGetKnownFolderPath`, Lxss registry read, `ShellExecuteW`, Run key for open at login |
| gtk (linux) | 0.18.2 | MIT | Native clipboard read for terminal paste (WebKitGTK blocks the web API); already in Tauri's tree — added with the clipboard feature |
| tauri-plugin-notification | 2.4.0 | Apache-2.0 OR MIT | Desktop notifications (terminal exited, bell, program request), called from Rust only — no JS capability granted; pulls notify-rust (Linux D-Bus) — added 2026-11-26 |
| tauri-plugin-updater | 2.12.0 (`~2.12.0`, default features off: `rustls-tls`, `system-proxy`; **no `zip`**) | Apache-2.0 OR MIT | In-app updates (ADR-019), called from Rust only — no JS capability granted; signature check (minisign/Ed25519) against the public key built into release builds. The default `zip` feature is disabled: it pulled `zip` 4.6.1 with six RustSec advisories and is only needed for archive-format updates. Pulls reqwest/rustls; `webpki-root-certs` (Mozilla root list) is CDLA-Permissive-2.0, allowed for that crate only in `deny.toml`. Pinned to 2.12.x because 2.14 requires Tauri 2.12 — added 2026-11-27 |

Dev-only: `proptest` 1.12 (MIT OR Apache-2.0, property tests),
`tempfile` 3.27 (MIT OR Apache-2.0), `tauri` with the `test` feature.

## Direct npm dependencies

| Package | Version | License | Why |
| --- | --- | --- | --- |
| react, react-dom | 19.2.8 | MIT | UI (ADR-001) |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | IPC client |
| @xterm/xterm | 5.5.0 | MIT | Terminal renderer (ADR-002) |
| @xterm/addon-fit | 0.10.0 | MIT | Terminal sizing |
| @xterm/addon-search | 0.15.0 (exact) | MIT | Scrollback search |
| flexlayout-react | 0.10.8 (exact) | MIT | Docking layout (ADR-003) |

Dev-only tooling (all MIT except TypeScript, Apache-2.0): vite, vitest,
jsdom, Testing Library, eslint + plugins, prettier, typescript,
@tauri-apps/cli (Apache-2.0 OR MIT). `@tauri-apps/cli` was raised from
2.11.4 to 2.12.4 on 2026-11-27 (build tool only): 2.12 records the version in
the update signature's trusted comment, which `requireSignedVersion`
(ADR-019, downgrade protection) needs; 2.11 signatures carry no version.

## Transitive license summary (Rust, 457 crates)

All crates are permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense,
Unicode-3.0, CC0) except the following, which are compatible with
distributing ToolOnize under MIT OR Apache-2.0 as unmodified dependencies:

| Crate | License | Note |
| --- | --- | --- |
| cssparser, cssparser-macros, selectors, dtoa-short, option-ext | MPL-2.0 | File-level copyleft; pulled in by Tauri/WRY. Unmodified; source available from crates.io. |
| r-efi | MIT OR Apache-2.0 OR LGPL-2.1-or-later | Used under MIT/Apache-2.0. |
| target-lexicon | Apache-2.0 WITH LLVM-exception | Build-time. |

Third-party license notices for bundled dependencies are listed in the
release checklist ([RELEASE_CHECKLIST.md](../release/RELEASE_CHECKLIST.md)).

## Setup-wizard downloads (not app dependencies)

`scripts/setup/` (ADR-009) installs toolchains for contributors. It pins
Node.js `v24.22.1` and rustup-init `1.29.2` by SHA-256 digest; system
packages come from the distribution's package manager, Windows tools from
winget. Nothing here is linked into or shipped with ToolOnize.

## Supply-chain checks

- CI job `supply-chain`: `npm audit --audit-level=high` and
  `cargo deny check` (RustSec advisories, licenses, sources; policy in
  `src-tauri/deny.toml`). Both fail the build.
- Fixed when the checks were introduced (2026-11-26): dev tooling `vite` 6.4.3 → 6.4.6,
  `vitest` 3.2 → 4.1.12 (pulls fixed `tinypool`), `typescript-eslint` →
  8.74 (fixed `minimatch`/`brace-expansion`); Rust transitive `url` 2.5.8 →
  2.5.9 (RUSTSEC-2026-0592) and `cookie` 0.18.2 → 0.18.3
  (RUSTSEC-2026-0594) via `cargo update -p` (no other crate changed).
  Production npm dependencies had no advisories.

## Policy

- No dependency is added without recording it here, with its license and
  reason, and in the commit message.
- GitHub Actions are pinned to full commit SHAs.
- `portable-pty` stays at `=0.9.0` until an upgrade passes the ADR-004
  regression tests.
