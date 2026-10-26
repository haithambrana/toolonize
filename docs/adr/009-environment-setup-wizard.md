# ADR-009: Environment setup wizard (one plan, one consent, one authorization)

Date: 2026-10-26
Status: **Implemented (Linux) / prepared (Windows, macOS) — pending human review**

## Context

The owner asked for a setup that checks every package and environment
requirement, and, when something is missing, asks permission once and installs
everything in one shot ("next, next, next"), with authorization taken once
before installing. Linux first; Windows and macOS prepared.

ToolOnize itself cannot do this from inside the app: on Linux the app cannot
start without WebKitGTK 4.1, and on Windows without WebView2. Installed
packages (`.deb`) already declare runtime dependencies, so the gap is (a) the
AppImage/other distributions at runtime and (b) contributors building from
source. The app must also never gain a generic "run this as root" path: the
WebView is untrusted (THREAT_MODEL §2) and no IPC command accepts command
text.

## Decision

Standalone, auditable scripts outside the app, under `scripts/setup/`
(`setup.sh` at the repository root dispatches by OS):

| Platform | Script | Status |
|---|---|---|
| Linux (apt, dnf, pacman, zypper) | `linux.sh` + `lib/common.sh` | Implemented; apt verified in CI and on Ubuntu 24.04 |
| Windows 10/11 | `windows.ps1` (winget) | Prepared; `-Check`/`-DryRun` run in CI; installs not yet verified |
| macOS | `macos.sh` | Prepared toolchain only; macOS is not a V1 platform (PRD §3) |

Flow, identical on every platform:

1. **Check** every requirement and print one report (`--check` stops here,
   exit 0 when all required items are present, 1 otherwise).
2. **Plan**: one list with every missing item and the exact command that will
   run (`--dry-run` stops here).
3. **One consent** (whiptail dialog, or plain-text prompts without a TTY or
   with `--text`; `--yes` for automation).
4. **One authorization**: all system packages go through a *single* privileged
   invocation — `sudo -- sh -c '<pm install> "$@"' sh <packages…>` (or
   `pkexec`, or directly when already root). On Windows a single elevated
   child process (one UAC prompt) runs every machine-wide winget install.
5. **User-level steps without privilege**: Node.js LTS and Rust (rustup).
6. **Verify** by re-running every check, then show next steps.

Security rules:

- Package names come from fixed in-script tables and travel as positional
  arguments, never through string interpolation. The elevated Windows child
  re-validates every item against its catalog.
- Downloads are HTTPS-only and checked against **SHA-256 digests pinned in the
  script** (Node.js `v24.22.1` tarballs, rustup-init `1.29.2`); a mismatch
  deletes the file and aborts that step. Bumping a version means updating
  the digest from the official checksum file.
- Remote install scripts are never piped to a shell (no `curl | sh`); on macOS
  Homebrew is not auto-installed.
- Nothing outside the user's home is touched without the single
  authorization; a pre-existing `~/.local/bin/node` that the wizard did not
  create is never replaced. Arch uses `pacman -S --needed` (no partial
  upgrade via `-Sy`).
- Optional tools (OpenSSH client, tmux, git) are offered separately and can be
  excluded (`--no-optional`).

## Consequences

- No new dependencies in the app; the scripts use only the platform's package
  manager, `curl`, `tar`, `sha256sum`/`shasum`, and optionally `whiptail`.
- CI dogfoods the Linux wizard to install the Tauri build dependencies, runs
  its stub-based test suite (`scripts/setup/tests/linux_test.sh`) and
  shellcheck, runs the Windows script under PowerShell 5.1 and 7, and runs
  the macOS script on a macOS runner.
- dnf/pacman/zypper package names follow the Tauri 2 prerequisites but have
  not been verified on those distributions; they are marked best-effort.
- A future in-app "environment" panel may *report* optional tools (ssh, tmux),
  but installing anything from inside the app would need a new ADR.
