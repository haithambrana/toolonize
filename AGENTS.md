# Guidelines for coding agents

These rules apply to anyone, human or AI agent, who changes this repository. [CONTRIBUTING.md](CONTRIBUTING.md) has the full contributor guide.

## Hard rules

1. **No secrets or machine-specific data** in the repository: no credentials, tokens, keys, password files, SSH configuration, real hostnames, usernames, IP addresses or private paths. Fixtures use fictional data only ([`docs/testing/FIXTURE_POLICY.md`](docs/testing/FIXTURE_POLICY.md)). The repository is public.
2. **Never weaken security to simplify an implementation.** The WebView is untrusted; process execution is reachable only through the Rust execution policy.
3. **One cross-platform codebase** with platform adapters for Linux and Windows; no per-OS forks.
4. **No silent dependencies.** Every new dependency is justified in the change and recorded in [`docs/security/DEPENDENCIES.md`](docs/security/DEPENDENCIES.md).
5. **Decisions that change the architecture, the security model or user data handling need an ADR** in [`docs/adr/`](docs/adr/README.md).
6. **Every change has tests**, and the checks below pass before it is pushed.

## Working in the code

- **IPC commands** are registered in four places that tests keep in sync: `ALLOWED_COMMANDS` in `src-tauri/src/commands/mod.rs`, the command list in `src-tauri/build.rs`, `src-tauri/capabilities/default.json`, and `generate_handler!` in `src-tauri/src/lib.rs`.
- **User-facing strings** use `t()` / `tk()` from `src/i18n`, with an Arabic entry in `src/i18n/ar.ts`.
- **State files** are written through `persist::Store` (atomic, checksummed); never write state files directly.

## Checks

```bash
npm run typecheck && npm run lint && npm run format:check && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
bash tools/check-public-safety.sh
```
