# ADR-019: In-app updates from signed GitHub releases

Date: 2026-11-27
Status: **Accepted**

## Decision

- **Mechanism.** The official `tauri-plugin-updater` (2.12.x, default
  features off, no `zip`), driven only from Rust: the WebView can call
  `update_check` and `update_install`, never the plugin's own commands (no
  capability granted) and never with a URL.
- **Trust.** Every installer of a release is signed (minisign/Ed25519) by
  the release workflow with the project's private key (repository secret).
  The public key is built into release builds; `install` refuses any file
  whose signature does not match, before writing anything.
  `requireSignedVersion` binds each signature to its version, so a crafted
  manifest cannot force a downgrade to an older genuine build (needs Tauri
  CLI ≥ 2.12, which writes `version:` into the signature; the CLI was raised
  to 2.12.4 for this — checked in the CLI source and end to end). The endpoint
  is HTTPS (GitHub).
- **Publishing is a manual step.** Releases are created as drafts; installed copies read
  `releases/latest/download/latest.json`, which only resolves to a
  **published** release. Nothing reaches users until a maintainer publishes it.
- **No account names or keys in the repository.** The endpoint
  (`https://github.com/${GITHUB_REPOSITORY}/…`) and the public key
  (repository variable) are injected at build time by the workflow
  (`--config`). Builds without them (development, self-built, unsigned
  releases) do not load the updater at all and say so in Settings.
- **User experience.** Automatic check 15 s after start and every 6 hours
  (Settings › Updates, on by default), "Check now", a sidebar entry
  "Update to X.Y.Z", release notes in the confirmation. Installing needs a
  click; the session is saved first and the terminals come back after the
  restart (ADR-015). AppImage replaces itself, deb/rpm install through the
  system package tool (administrator password), Windows runs the NSIS
  installer in passive mode (session saved in the plugin's before-exit
  hook).
- **Privacy.** The check sends GitHub the IP address and the app version,
  nothing else; Settings and "Data & privacy" say so; it can be turned off.
- **Maintainer setup.** `scripts/release/setup-updater.sh` creates the key
  outside the repository and stores secret, password and public key with
  the GitHub CLI; `docs/release/UPDATES.md` describes releasing.
- New dependency recorded in `docs/security/DEPENDENCIES.md`; cargo-deny
  allows CDLA-Permissive-2.0 for `webpki-root-certs` only.

## Verification

End to end with a throwaway key (never committed): installed 0.1.0
AppImage → automatic check found 0.1.1 → signature verified → file replaced
(SHA-256 equals the served 0.1.1) → restart as 0.1.1 with its terminal
restored. Refused, nothing written: an update signed with a different key;
a genuine 0.1.1 file announced as 0.1.3 (`requireSignedVersion`). Found and
fixed on the way: Tauri CLI 2.11 wrote no version into signatures (CLI
raised to 2.12.4); the version shown in Settings came from the crate
instead of the release version.

## Consequences

- An update channel is a code-execution path: its security rests on the
  private key. Losing it means installed copies cannot accept new updates
  (they need one manual reinstall); leaking it means an attacker who can
  also serve the manifest could ship code. The key never enters the
  repository or CI logs, and publishing stays a manual step.
- The app now makes one network request of its own (the check); the
  earlier "no network" statements were updated.
