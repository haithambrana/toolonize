# ADR-010: ToolOnize is a CRMonize.com open-source project

Date: 2026-10-26
Status: **Implemented — pending human review** (requested by the owner)

## Context

The owner asked to introduce the whole project as one of the CRMonize.com
open-source projects, and to make sure no critical or sensitive information
remains in the repository, which started as a private project.

## Decision

- Public identity: "ToolOnize — a CRMonize.com open-source project"
  (https://crmonize.com) in the README, package metadata (`package.json`
  author/homepage, `Cargo.toml` authors/homepage), installer metadata
  (`tauri.conf.json` publisher, homepage, copyright) and the setup wizards.
- Copyright line: "CRMonize.com and the ToolOnize contributors". The license
  stays MIT OR Apache-2.0.
- Tauri identifier changes from `com.toolonize.desktop` to
  `com.crmonize.toolonize`: reverse-DNS identifiers should use a domain the
  publisher controls. No release has shipped, so nothing migrates; ToolOnize's
  own state and settings live in `toolonize` directories that do not depend on
  the identifier.
- No personal names, personal accounts, e-mail addresses or private URLs in
  the tree. Repository references use `<owner>/toolonize` until the public
  location is decided. No contact e-mail is published until CRMonize.com
  designates one (SECURITY.md).

## Sensitive-information review (2026-10-26)

Scanned the working tree and the full git history (`git log --all -p`) for
private keys, token formats (GitHub, Slack, AWS), private and CGNAT IPv4
ranges, personal home paths, e-mail addresses and machine-specific
launchers/SSH configuration. Findings:

- No credentials, keys, tokens, private IPs, SSH configs or real launcher
  files, in the tree or in history. Fixtures use `example.com`, RFC 5737
  addresses and fictional names only.
- The owner's GitHub account name appeared in four documentation files; it is
  removed from the tree by this change. It remains in older commits' history
  and in commit author metadata (GitHub `noreply` addresses only). Removing it
  from history requires a history rewrite and force-push, which is a human
  decision (AGENTS.md rule 9) and was not done.
- A dedicated secret scanner (e.g. gitleaks) over full history is still
  recommended before the public release; it is listed in the release
  checklist.

## Consequences

Trademark/legal review of the names remains a release gate (STATUS.md).
