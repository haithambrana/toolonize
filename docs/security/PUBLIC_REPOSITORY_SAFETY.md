# Public Repository Safety

Status: DRAFT — pending human review. This document defines the standing
rules and the release-time checklist that keep the repository safe for
public release (AGENTS.md rules 2, 3, 9).

---

## 1. Standing rules (apply to every change)

1. **No machine-specific data ever enters the repo**: no real hostnames,
   usernames, IP addresses (except RFC 5737/3849 documentation ranges),
   SSH configs/keys, tokens, password files, Tailscale/network state,
   personal launcher names or paths.
2. **All examples are fictional** (`example.com`, `alice@example`,
   `Org.Example.Tool.desktop`, `Example Tool.lnk`, `build-server` as an
   ssh *alias name* in docs).
3. Personal configuration lives outside the repository; the repo may contain
   `.example` templates only.
4. Screenshots/videos used in docs must be captured on sanitized fixtures,
   never the author's real system state.
5. Commit messages must not reference private hosts/projects.

## 2. Structural safeguards

- `.gitignore` covers env files, keys, local config patterns (present since
  repo birth; extended via reviewed PRs only).
- Secret scanning runs in CI over the working tree; before first public
  push, a full-history scan is mandatory (history rewrite if any hit).
- `docs/` uses only fictional fixture data; fixture generators are committed
  so binary test artifacts (e.g., synthetic `.lnk`) can be rebuilt rather
  than committed where feasible.

## 3. Pre-public-release checklist (naming & secret-safety gates)

Gate owner: human reviewer. All boxes must be evidenced in the release PR.

- [ ] Full-history secret scan clean (tool config + manual grep list from
      TEST_STRATEGY §9).
- [ ] Manual review of all docs for first-person machine references
      ("my server", home dirs with real usernames, `/home/<name>/...`).
- [ ] All IP addresses conform to RFC 5737/3849 or are removed.
- [ ] Fixture manifest confirms fictional data only.
- [ ] LICENSE chosen and applied; third-party license inventory current.
- [ ] SECURITY.md disclosure policy present.
- [ ] Naming-collision gate passed per PRD §17 (trademark search, repo/org
      availability, category-conflict check) — recorded as an ADR-note.
- [ ] README/docs truth pass: statuses correctly say researched / decided /
      proposed / planned / implemented; nothing claimed done that isn't.
- [ ] CI green including secret scan and link check.
- [ ] Release artifacts built by CI only; checksums published; provenance
      attestations evaluated per SEC-008; platform code signing present only
      if the signing-feasibility gate passed, with keys verified to be held
      outside the repository and outside CI secrets scope beyond the signing
      workflow.

## 4. Incident response (if a secret lands)

1. Revoke/rotate the credential immediately (assume compromised at push).
2. Remove from history via rewrite + force-push (documented command set in
   maintainer runbook — kept outside repo until needed).
3. Post-mortem note added to this file's changelog section (no sensitive
   detail), plus scanner rule added to prevent recurrence.

## 5. Documentation honesty taxonomy

Every capability statement in public docs should carry one of:
**researched** (evidence exists), **decided** (ADR Accepted),
**proposed** (ADR Proposed/Spike), **planned** (milestone-assigned),
**implemented** (tests pass). The discovery-phase tree contains zero
"implemented" claims — enforced by the truth pass above.

## 6. Changelog

- 2026-08-26: initial version during discovery phase; no incidents.
- 2026-08-26: release-integrity checklist item aligned with SEC-008 layering
  (checksums; attestations evaluated; native signing gated on feasibility).
  No incidents.
