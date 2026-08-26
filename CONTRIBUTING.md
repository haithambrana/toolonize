# Contributing to ToolOnize

Thanks for considering a contribution. ToolOnize is currently in `M0 — Repository Foundation` (pre-implementation). Application code has not been written yet.

## Ground rules

1. **Never include secrets or machine-specific data.** Do not paste real hostnames, usernames, IP addresses (except RFC 5737/3849 documentation ranges), SSH configs/keys, tokens, password files, Tailscale state, or personal launcher names/paths in issues, PRs, commits, or fixtures. See `docs/security/PUBLIC_REPOSITORY_SAFETY.md` and `docs/testing/FIXTURE_POLICY.md`.
2. **Tests are required for implementation changes.** Every milestone must have tests and explicit acceptance criteria (`AGENTS.md` rule 10, `docs/product/TEST_STRATEGY.md`).
3. **Architecture-affecting changes need an ADR.** Product scope, security policy, and public-release decisions are human-reviewed gates. Add or update a record under `docs/adr/`.
4. **Platform-specific behavior needs a parity-ledger update.** Document Linux/Windows differences in `docs/architecture/PLATFORM_ADAPTERS.md` §5.
5. **Dependencies are explicit.** Do not silently add dependencies (`AGENTS.md` rule 6). Propose, justify with evidence or an ADR, and get review.
6. **One cross-platform codebase.** Platform differences live behind the adapter traits; do not fork the product per OS (`AGENTS.md` rule 4).

## Development workflow (once code exists)

- Follow `docs/product/IMPLEMENTATION_PLAN.md` — milestones are vertical slices with gates.
- Do not weaken security to simplify implementation (`AGENTS.md` rule 7).
- Mark implementation status honestly in `docs/product/STATUS.md`; never claim a capability as implemented until its milestone's tests pass on both platforms.

## Licensing

Unless you explicitly state otherwise, any contribution you submit is licensed under **either**:

- [MIT License](LICENSE-MIT), **or**
- [Apache License 2.0](LICENSE-APACHE)

at the option of any downstream user (dual license MIT OR Apache-2.0). You affirm you have the right to contribute under these terms.

## Reporting issues

Include steps to reproduce with **fictional** data only. Redact any secret-shaped strings.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating you agree to its terms.
