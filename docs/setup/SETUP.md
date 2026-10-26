# Setting up a computer for ToolOnize

ToolOnize is a [CRMonize.com](https://crmonize.com) open-source project. The
setup wizards check everything ToolOnize needs, show **one plan**, ask for
your **consent once**, take **administrator authorization once**, install
everything in one go and verify the result. Design and security rules:
[ADR-009](../adr/009-environment-setup-wizard.md).

## Quick start

| System | Command |
|---|---|
| Linux | `./setup.sh` (same as `bash scripts/setup/linux.sh`) |
| Windows | `powershell -ExecutionPolicy Bypass -File scripts\setup\windows.ps1` |
| macOS (toolchain only, not a V1 platform) | `./setup.sh` (same as `bash scripts/setup/macos.sh`) |

The wizard pages are: **Welcome → Check results → (optional tools?) →
Install plan → one authorization → Finished**. Nothing changes until you
accept the plan. Cancel at any page and nothing is installed.

## Options (Linux; macOS takes the same flags where they apply)

| Flag | Meaning |
|---|---|
| `--check` | Only report. Exit 0 when every required item is present, 1 otherwise. |
| `--dry-run` | Show the full plan, including the exact package-manager command. |
| `--profile dev` | (default) Everything needed to build from source. |
| `--profile run` | Only the runtime libraries the built app needs (e.g. for the AppImage). |
| `--yes` | Accept the plan without prompting (automation/CI). |
| `--no-optional` | Leave out the optional tools (OpenSSH client, tmux, git). |
| `--skip node\|rust\|packages` | Skip one area, e.g. when you manage Node with nvm. |
| `--text` | Plain-text prompts even when `whiptail` is installed. |

Windows: `-Check`, `-DryRun`, `-Yes`, `-NoOptional`, `-SetupProfile dev|run`.

Exit codes: 0 ok · 1 missing or failed · 2 usage · 3 cancelled · 4 unsupported.

## What gets checked

| Area | Linux (apt names) | Windows | macOS |
|---|---|---|---|
| Runtime | `libwebkit2gtk-4.1-0`, `xdg-utils` | WebView2 Runtime | — |
| Build (dev) | `build-essential pkg-config curl xz-utils file libssl-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev patchelf` | VS 2022 Build Tools (C++ workload) | Xcode Command Line Tools |
| Node.js (dev) | ≥ 20 with npm; else Node.js 24.22.1 LTS for your user | ≥ 20; else `OpenJS.NodeJS.LTS` | same as Linux |
| Rust (dev) | ≥ 1.77.2; else rustup for your user | ≥ 1.77.2; else `Rustlang.Rustup` | same as Linux |
| Optional | `ssh`, `tmux`, `git` | `ssh` (Windows feature), Git | `tmux` (Homebrew) |

Fedora (dnf), Arch (pacman) and openSUSE (zypper) use the equivalent package
names from the Tauri 2 prerequisites; only apt (Debian/Ubuntu) is verified so
far. On other distributions, install the equivalents of the apt list above by
hand and rerun with `--skip packages`.

## How authorization works

- **Linux**: all system packages are installed by a single `sudo` (or
  `pkexec`) call that runs one package-manager transaction. Node.js and Rust
  are installed for your user without root: Node.js into `~/.local/opt`
  (linked from `~/.local/bin`), Rust via rustup into `~/.cargo`.
- **Windows**: one UAC prompt starts an elevated helper that runs every
  machine-wide `winget install`; rustup is then installed for your user.
- **macOS**: no administrator step; Apple's installer handles the Command
  Line Tools.

Every download is HTTPS-only and must match a SHA-256 digest pinned in the
script, otherwise it is deleted and the step stops.
