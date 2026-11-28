# Contributing to ToolOnize

Thank you for your interest in ToolOnize. ToolOnize is developed by [CRMonize.com](https://crmonize.com) as an open-source product, and contributions of every size are welcome: bug reports, documentation fixes, translations, and code.

## Getting set up

```bash
./setup.sh            # Linux / macOS: installs Node.js, Rust and the system packages once
npm ci
npm run tauri -- dev  # starts the app with hot reload
```

On Windows run `powershell -ExecutionPolicy Bypass -File scripts\setup\windows.ps1` instead of `./setup.sh`. Details: [`docs/setup/SETUP.md`](docs/setup/SETUP.md).

## Before you open a pull request

Run the same checks CI runs:

```bash
npm run typecheck && npm run lint && npm run format:check && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
bash tools/check-public-safety.sh
```

A pull request should:

- explain **what** changes and **why** (link the issue if there is one);
- include tests for the behaviour it adds or fixes;
- keep the change focused: one topic per pull request.

## Ground rules

1. **No secrets or personal data.** Never commit real hostnames, usernames, IP addresses, SSH configuration, keys, tokens or private paths, in code, tests, fixtures, issues or screenshots. Use fictional data (`example.com`, `192.0.2.0/24`); see [`docs/testing/FIXTURE_POLICY.md`](docs/testing/FIXTURE_POLICY.md). CI scans every push.
2. **Security is not traded for convenience.** The WebView is untrusted: anything that starts a process goes through the execution policy in the Rust core. Read [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) before changing IPC commands, execution, persistence or updates.
3. **One cross-platform codebase.** Platform differences live behind the adapters described in [`docs/architecture/PLATFORM_ADAPTERS.md`](docs/architecture/PLATFORM_ADAPTERS.md); document any Linux/Windows difference there.
4. **Dependencies are deliberate.** A new dependency needs a reason in the pull request and an entry in [`docs/security/DEPENDENCIES.md`](docs/security/DEPENDENCIES.md). `cargo deny` and `npm audit` run in CI.
5. **Significant decisions get a record.** Changes to the architecture, the security model or user data handling come with an Architecture Decision Record in [`docs/adr/`](docs/adr/README.md).
6. **Every string is translatable.** User-facing text goes through `t()` / `tk()` with an Arabic entry in `src/i18n/ar.ts`; a test checks that both catalogs match.

## Translations

ToolOnize ships in English and Arabic, including a full right-to-left layout. Catalogs map the English text to its translation (see `src/i18n/ar.ts`); adding a language takes a new catalog plus a small change in `src/i18n/index.ts`. Please open an issue first so we can coordinate.

## Reporting bugs and ideas

Use the issue templates. Include your OS, ToolOnize version (Settings › Updates) and steps to reproduce, with fictional data only. Security problems go through [SECURITY.md](SECURITY.md), never a public issue.

## Licensing

Unless you state otherwise, any contribution you submit is licensed under **either** the [MIT License](LICENSE-MIT) **or** the [Apache License 2.0](LICENSE-APACHE), at the option of the user, without additional terms. You confirm that you have the right to contribute under these terms.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By taking part you agree to its terms.
