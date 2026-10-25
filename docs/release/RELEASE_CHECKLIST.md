# Release checklist

Releases are **drafts** created by `.github/workflows/release.yml` when a
`v*` tag is pushed. A human publishes them only after every box below is
checked. Nothing here may be claimed in release notes until it is true.

## 1. Gates (AGENTS.md rule 9, PRD §18)

- [ ] Human review of the milestones and ADRs marked "pending review" in
      `docs/product/STATUS.md` is complete.
- [ ] Naming-collision / trademark gate passed for the release name.
- [ ] Public-safety review: `tools/check-public-safety.sh` clean, gitleaks
      history scan clean, `docs/security/PUBLIC_REPOSITORY_SAFETY.md`
      checklist passed.
- [ ] CI green on the tagged commit (ubuntu + windows), repository-safety green.
- [ ] `docs/security/DEPENDENCIES.md` refreshed; third-party notices
      aggregated into the installers / release assets.

## 2. Integrity (SEC-008)

- [ ] `SHA256SUMS.txt` attached to the draft and verified locally.
- [ ] Build-provenance attestation present:
      `gh attestation verify <file> --repo <owner>/toolonize`.
- [ ] Release notes state that binaries are **not code-signed** (unless the
      signing-feasibility gate has provisioned signing).

## 3. Manual smoke matrix (clean VMs)

Use fictional data only. Record results (pass/fail + notes, no personal data)
in the release PR.

| Check | Ubuntu (deb) | Fedora/other (AppImage) | Windows 11 (NSIS) |
| --- | --- | --- | --- |
| Install, first launch shows the empty state | | | |
| Launchers view lists entries; nothing ran during discovery | | | |
| Create workspace, add default shell, type, resize, search | | | |
| Grid → Focus → Tabs → Master+Stack keeps the same running terminals | | | |
| Window reload (context menu → Reload) reattaches running terminals | | | |
| Quit and relaunch: layout restored, Ask prompt shown, nothing auto-started | | | |
| Desktop-folder / custom-root entry requires review before it can be added | | | |
| Edit a pinned launcher's command → member shows "re-review required" and cannot start | | | |
| External app member launches detached | | | |
| SSH member (fictional loopback host) → stop sshd → Disconnected → Reconnect copy is honest | | | |
| WSL distribution terminal | n/a | n/a | |
| Export (portable) contains no absolute paths/secrets; import lands in review | | | |
| Keyboard-only: palette, mode switch, member navigation | | | |
| High-contrast theme and reduced motion | | | |
| Screen reader smoke (Orca / NVDA): navigation, dialogs, buttons announced | | | |
| Uninstall leaves only the documented state/config folders | | | |

## 4. Publish

- [ ] Edit the draft notes (what changed, known limitations from STATUS.md).
- [ ] Publish.
