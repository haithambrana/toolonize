# Engineering Constitution

## Project phase
Discovery and architecture only.

## Current hard rules
1. Do not implement application features until the PRD, architecture, threat model, and implementation plan are approved.
2. Never copy machine-specific launchers, SSH configuration, credentials, IP addresses, usernames, tokens, password files, or private paths into the repository.
3. The repository must remain safe for eventual public release.
4. Prefer one cross-platform codebase with platform adapters for Linux and Windows.
5. Product decisions require evidence or an ADR.
6. Do not silently add dependencies.
7. Do not weaken security to simplify implementation.
8. Do not modify the existing Python/GTK prototype.
9. Do not push or create a public GitHub repository until naming and secret-safety gates pass.
10. Every implementation milestone must have tests and explicit acceptance criteria.

## Current status
The existing Python/GTK application is a disposable proof of concept and is not part of this repository.

## Authority
Architecture, product scope, security policy, and public-release decisions are human-reviewed gates.
