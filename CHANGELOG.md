# Changelog

All notable changes to ToolOnize are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and ToolOnize follows [Semantic Versioning](https://semver.org/).

## Unreleased

## 0.2.0 — 2026-11-28

The first public release of ToolOnize by [CRMonize.com](https://crmonize.com).

### Workspaces

- Workspaces group terminals, SSH sessions and apps per project, with four layouts (Grid, Focus, Tabs, Master + Stack). Moving, maximizing or switching layouts never restarts a terminal.
- Startup policy per workspace: ask, restore the layout only, start the terminals, or also launch the apps.
- **Session memory:** after a power loss or closing the app, ToolOnize reopens with the same window and the same terminals in the same folders, each showing its last output. Saved output is cleaned so nothing in it can run again; commands with side effects are not re-run unless you mark them "restart automatically".
- Sticky notes per terminal and per workspace, saved within a fraction of a second and safe across crashes (Ctrl+Shift+N / Ctrl+Shift+M).
- Full backup and restore, duplicate a workspace, workspace templates, and non-secret import/export (imports wait for review; nothing runs).
- Project folder per workspace, color tags for workspaces and terminals.

### Terminals

- Embedded terminals for bash, zsh, fish and sh, PowerShell, cmd, WSL distributions, your own `ssh`, and tmux. Lossless output with backpressure; terminals survive a window reload.
- Rename terminals (double-click, F2 or Ctrl+Shift+R), split right or down in the same folder, zoom per terminal, fonts, cursor styles and color schemes.
- Clipboard that works like a local terminal, including over SSH: Ctrl+Shift+C / Ctrl+Shift+V, right-click menu, optional copy-on-select, and OSC 52 so tmux or vim on a server can copy to your clipboard.
- Search in a terminal (Ctrl+Shift+F) or across all open terminals (Ctrl+Shift+G); Ctrl+click opens links; save a terminal's output to a file.
- Broadcast typing to several terminals, snippets, presentation mode, and desktop notifications when a terminal you are not looking at exits, disconnects or asks for attention.
- Per-terminal environment variables and a startup command (typed into the shell for you; anything restored or edited outside the app needs your confirmation first).

### SSH

- Pick hosts from your `~/.ssh/config` (only the `Host` aliases are read).
- Dropped SSH sessions reconnect automatically with backoff and a visible countdown.

### Launchers

- Finds the launchers you already have without running them: Linux application entries (including Flatpak and Snap) and your Desktop folder; Windows Start Menu and Desktop shortcuts.
- Every launcher is classified with readable evidence; anything ambiguous or user-writable needs your review of the exact command before it can run, and a changed launcher needs review again.
- Local, redacted audit log of every decision, with filters and export.

### App

- **In-app updates:** ToolOnize checks for a new release at start and every 6 hours (can be turned off) and updates with one click. Updates are signed and refused if the signature does not match; downgrades are refused. Your terminals come back after the update restart.
- **Open at login**, so a reboot brings your last session straight back.
- **English and Arabic** interface with a full right-to-left layout; terminals stay left-to-right.
- Command palette (Ctrl+Shift+P), remappable shortcuts, dark, light and high-contrast themes, reduced motion.
- Diagnostics export and a rotating diagnostic log for bug reports.

### Installation

- Packages for Linux (`.deb`, `.rpm`, AppImage) and Windows (installer), each with SHA-256 checksums and a build-provenance attestation.
- `./setup.sh` prepares a development machine with one plan, one confirmation and one administrator prompt (apt, dnf, pacman, zypper; winget on Windows).

### Known limitations

- The Windows build is built and tested automatically but has not yet been tested by hand.
- Installers are not code-signed, so Windows SmartScreen may show a warning.
- macOS is not supported yet, and there is no tray icon yet.
