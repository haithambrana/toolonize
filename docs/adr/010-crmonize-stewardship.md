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
- The owner's GitHub account name appeared in four documentation files and in
  the author metadata of 52 commits.

## History rewrite (2026-10-26, approved by the owner)

With the owner's explicit approval, every branch (`main`,
`m1-framework-shell`, `m2-pty-spike`, `m3-terminal-session-manager` and the
working branch) was rewritten with `git filter-repo` and force-pushed with a
lease:

- Author and committer metadata of the owner's commits now read
  `CRMonize.com <noreply@crmonize.com>`; commits by GitHub and by Claude are
  unchanged.
- Every occurrence of the account name in file contents and messages was
  replaced by `<owner>`.
- Verified before pushing: same commit count on every branch, final trees
  differ only in those lines, zero occurrences in any object, and gitleaks
  8.30.1 (checksum-verified release) reports no leaks over all 58 commits,
  with both `.gitleaks.toml` and the default rules.

Residual (cannot be fixed from git): the repository URL itself contains the
account name; GitHub keeps the original commits of pull requests #1–#3
reachable through `refs/pull/*`; forks and caches made while the repository
was public keep the old history. To publish without any of that, create a
fresh repository under a CRMonize.com organization from the rewritten
history and archive or delete this one (a human decision).

## Consequences

Trademark/legal review of the names remains a release gate (STATUS.md).
