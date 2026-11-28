# Security Policy

ToolOnize runs your shells, SSH sessions and tools, so we take its security seriously.

## Supported versions

| Version | Supported |
|---|---|
| 0.2.x (latest release) | ✅ |
| older | ❌ — ToolOnize updates itself; install the latest release |

## Reporting a vulnerability

**Please do not open a public issue for a suspected vulnerability.**

Report it privately through GitHub: open the repository's **Security** tab and choose **[Report a vulnerability](../../security/advisories/new)**. Please include:

- the affected version and platform;
- steps to reproduce, using fictional data only (no real hosts, keys or credentials);
- the impact you expect and, if you have one, a suggested fix.

We will acknowledge your report, keep you informed while we work on a fix, and publish a security advisory when the fix is released. We are happy to credit you in the advisory unless you prefer to stay anonymous. Please give us reasonable time to fix the issue before disclosing it publicly.

## Security model in brief

- **The interface is untrusted.** The WebView can only call a fixed list of typed commands; none accepts a program, arguments or a command line. Starting anything goes through the execution policy in the Rust core.
- **Discovery never executes.** Launchers found on your system are read, classified and shown for review; changed launchers need review again before they run.
- **No credential storage.** SSH uses your own `ssh`, agent and config. ToolOnize stores no passwords or keys.
- **Signed updates.** Every installer and update is signed; the app refuses an update whose signature does not match its built-in key, and refuses downgrades.
- **Local data stays local.** No account, no telemetry. Saved terminal output lives in a folder readable only by you and is never included in backups, exports or diagnostics.

The full analysis is in [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md).

## Verifying downloads

Every release lists SHA-256 checksums in `SHA256SUMS.txt` and carries a GitHub build-provenance attestation. You can check a file with `sha256sum -c SHA256SUMS.txt --ignore-missing`, or with the `gh attestation verify` command shown in the release notes.
