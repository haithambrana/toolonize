# ADR-017: Open at login (tray icon deferred)

Date: 2026-11-27
Status: **Implemented (open at login) — tray icon deferred; pending human review**

## Context

With session memory (ADR-015), opening ToolOnize at login brings the last
session back after a restart without any action. The enhancement list also
proposed a tray icon with "close to tray".

## Decision

- **Open at login** uses each platform's per-user mechanism, with no new
  dependency: an XDG autostart desktop entry on Linux
  (`~/.config/autostart/toolonize.desktop`), the `ToolOnize` value under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on Windows
  (`RegGetValueW`/`RegSetKeyValueW`/`RegDeleteKeyValueW`, existing
  `windows-sys` features), and a LaunchAgent on macOS.
- The entry only names ToolOnize's own executable (the AppImage file when
  running from one) plus `--autostart`; it is generated from the running
  executable's path, quoted per the Desktop Entry spec / XML-escaped, and
  written atomically. "On" means the entry exists **and** starts this
  executable, so a stale entry from an old install location shows as off.
- Off by default; Settings › Session memory has the switch, which always
  shows the system's real state. Changes are recorded in the audit log.
- Commands `autostart_get`, `autostart_set`.

## Deferred: tray icon / close to tray

Not implemented in this pass, on purpose:

- it needs Tauri's `tray-icon` feature, i.e. new crates (`tray-icon`,
  `libappindicator`), and on Linux a runtime library
  (libayatana-appindicator) that is missing on several desktops — without a
  load check the app can panic at start;
- "close to tray" changes what closing the window means (terminals keep
  running in the background), which is a product decision for the owner.

Session memory plus open-at-login already cover "continue where I left
off". The tray can follow as its own change with the dependency recorded in
`docs/security/DEPENDENCIES.md`, a `dlopen` check, and a setup-wizard
package entry.
