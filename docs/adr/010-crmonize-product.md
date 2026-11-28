# ADR-010: ToolOnize is a CRMonize.com product

Date: 2026-10-26
Status: **Accepted**

## Context

ToolOnize is developed and published by CRMonize.com as an open-source
product. Its public identity, metadata and repository content must say so
consistently, and the public repository must contain nothing private.

## Decision

- **Public identity:** "ToolOnize by CRMonize.com" (https://crmonize.com) in
  the README, package metadata (`package.json` author and homepage,
  `Cargo.toml` authors and homepage), installer metadata (`tauri.conf.json`
  publisher, homepage, copyright), the About section in Settings, and the
  setup wizards.
- **Copyright:** "CRMonize.com and the ToolOnize contributors". The license is
  MIT OR Apache-2.0.
- **Identifier:** the Tauri identifier is `com.crmonize.toolonize`, a
  reverse-DNS name under a domain CRMonize.com controls. ToolOnize's own state
  and settings live in `toolonize` directories that do not depend on the
  identifier.
- **No private data in the repository:** no personal names, personal
  accounts, e-mail addresses, credentials or private URLs in files or commit
  metadata. Release tooling takes the repository location from the build
  environment (`GITHUB_REPOSITORY`) instead of hard-coding it. Security
  reports go through GitHub's private vulnerability reporting
  ([SECURITY.md](../../SECURITY.md)).

## Verification

The working tree and full history were scanned for private keys, token
formats, private IP ranges, personal paths, e-mail addresses and
machine-specific configuration; gitleaks reports no findings. CI runs
`tools/check-public-safety.sh` on every push.

## Consequences

Contributors follow the same rules ([CONTRIBUTING.md](../../CONTRIBUTING.md),
[AGENTS.md](../../AGENTS.md)); fixtures use fictional data only.
