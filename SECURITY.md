# Security Policy

## Supported versions

ToolOnize is in `M0 — Repository Foundation`. No application release has been published and no version is supported yet. Once releases exist, this table will list supported lines.

| Version | Supported |
|---|---|
| (pre-release) | — |

## Reporting a vulnerability

**Do not open a public issue for a suspected vulnerability.**

Until a public GitHub Security Advisory channel is activated (before the first public release), use the following process:

1. Contact the maintainers through the private channel announced in the repository's first public release announcement. If no channel has been announced yet, hold the report and contact the human Product/Technical Lead out-of-band.
2. Include: affected component, steps to reproduce with fictional data only (no real credentials/hosts), impact assessment, and any suggested mitigation.
3. Allow time for triage before any public disclosure. We will acknowledge receipt and coordinate a disclosure timeline with you.

**Never post vulnerabilities publicly** (issues, discussions, social media) until a fix and advisory are coordinated.

We will credit reporters in the advisory unless you prefer to remain anonymous.

## Scope

V1 threat model: see `docs/security/THREAT_MODEL.md`. In particular:

- The WebView is treated as untrusted; execution policy lives in the Rust core.
- No plaintext credential storage; SSH delegates to the user's agent/config.
- Launcher parsing must remain robust against hostile inputs.

## Disclosure expectations

We follow coordinated disclosure. We will keep you informed of remediation progress and publish an advisory once a fix is available.

Do not include a personal contact email here unless it is already approved as a public security contact.
