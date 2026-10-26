# Project Status

Phase: **V1 implemented (M3–M10) — pending human review and verification**

Architecture: HUMAN APPROVED for progression into M0.
Naming: ToolOnize — HUMAN APPROVED. ToolOnize is the approved
working/public product identity (former discovery-phase codename was
"Dev Command Center" — historical naming research preserved in
docs/research/NAMING_RESEARCH.md). Preliminary naming research is not legal
trademark clearance; formal legal/trademark review remains a later release
gate if required.

```
M0:  COMPLETE / HUMAN APPROVED / MERGED
M1:  COMPLETE / HUMAN APPROVED / MERGED
M2:  COMPLETE / HUMAN APPROVED / MERGED (ADR-004 accepted)
M3:  IMPLEMENTED — pending human review
M4:  IMPLEMENTED — pending human review (ADR-003 gate passed in automated tests)
M5:  IMPLEMENTED — pending human review
M6:  IMPLEMENTED — pending human review and Windows runtime verification (ADR-007)
M7:  IMPLEMENTED — pending human review
M8:  IMPLEMENTED — pending human review (ADR-006)
M9:  IMPLEMENTED — pending human review; NFR budgets partly measured (Linux only)
M10: IMPLEMENTED (packaging + draft-release pipeline) — no release published
Selected PTY backend: portable-pty 0.9.0 + ToolOnize mitigations
Repository: CREATED on GitHub (public release under CRMonize.com pending release gates)
```

Steward: **CRMonize.com** — ToolOnize is a CRMonize.com open-source project
(ADR-010). Tauri identifier: com.crmonize.toolonize · binary: toolonize ·
config directory: toolonize.

## Gate waiver (2026-10-25)

The repository owner asked for the app to be completed and, when asked how
to handle the per-milestone human gates in AGENTS.md, chose **"All
milestones, no pauses"**: M3–M10 were implemented in one pass without
stopping for review between milestones. This waives the *pauses*, not the
reviews: every milestone, ADR-003's status change and ADR-006..008 remain
open for human review, and public-release gates (AGENTS.md rule 9, M10
release checklist) are untouched. No release has been tagged or published.

## What exists now

| Milestone | Delivered | Evidence |
|---|---|---|
| M3 Terminal lifecycle core | `PtyBackend` on portable-pty with the ADR-004 mitigations; session manager with orthogonal process/view state machines; lossless ack-driven output log with water marks, stall → explicit desync; restart/reconnect; process-group cleanup; long-lived xterm.js hosts; search, copy/paste with multi-line warning, exit banner | Rust: `session/manager_tests.rs` (exit code, 4 MiB exact SHA-256 through a slow consumer with backpressure, concurrent isolation, child-observed resize, UTF-8, Ctrl+C, process-group cleanup, detach/reattach without respawn, dead-view detach, restart, spawn-failure isolation, DSR without a view); Windows ConPTY tests on CI. Frontend: `terminal/controller.test.ts`. Manual: Linux/WebKitGTK run incl. renderer reload reattach. |
| M4 Workspace/layout core | FlexLayout workspaces, four mode presets, layout reconciliation, Rust-side hostile-layout validation, stable terminal hosts | `layout/modes.test.ts`, `App.test.tsx` (mode switches keep Terminal instances, no session commands), `workspace.rs` tests (T-CFG-01 corpus). |
| M5 Linux discovery | XDG precedence + Hidden/NoDisplay/OnlyShowIn, Flatpak/Snap tagging, user-dirs Desktop (no `~/Desktop` hard-coding), opt-in bounded custom roots, spec-exact Exec tokenizer, symlink-escape flagging, notify watcher with debounce and degraded mode | `discovery/*` tests incl. fixtures under `src-tauri/tests/fixtures/linux/`, property tests for Exec quoting and parser robustness, watcher add/modify/remove tests. |
| M6 Windows discovery | Known Folders, bounded `.lnk` reader (no `Resolve`), Needs-Review outcomes, duplicates retained, custom roots | `discovery/lnk.rs`, `discovery/windows.rs` tests with generated shortcuts (all platforms); Known Folder test on Windows CI. |
| M7 Classification + policy | Deterministic classifier with evidence, interpreter detection, review approvals bound to fingerprints, member authorization, execution policy (the only path to a process), detached external launch, URL allowlist, startup-policy executor, audit log | `classify.rs`, `policy.rs`, `registry.rs`, `external.rs`, `app_tests.rs` (custom-root review → pin → tamper → denied → re-review flow; import executes nothing). |
| M8 Persistence & recovery | Atomic checksummed snapshot generations, loss-window report, migrations/downgrade refusal, settings, non-secret import/export with linter and review staging | `persist.rs` tests (corruption, truncation, crash points, write failure), `transfer.rs` hostile-import corpus, `app_tests.rs` persistence across restart. |
| M9 UX/a11y/perf | Themes (dark/light/high contrast), reduced motion, remappable shortcuts, command palette, ARIA roles/labels, focus-trapped dialogs, failure banners/badges, perf harness | `keymap.test.ts`, `App.test.tsx`; perf baseline in `docs/research/perf/BASELINE.md`. |
| Setup wizard (ADR-009) | `./setup.sh` / `scripts/setup/{linux.sh,windows.ps1,macos.sh}`: one check, one plan, one consent, one authorization, one-shot install, verification. Linux implemented (apt verified; dnf/pacman/zypper best-effort); Windows (winget, one UAC prompt) and macOS (toolchain only) prepared | `scripts/setup/tests/linux_test.sh` (19 stub-based cases: single sudo call, positional package args, checksum refusal, cancel/decline, distro mapping, pkexec fallback); CI installs the Linux build deps through the wizard; Windows `-Check`/`-DryRun` on PowerShell 5.1 + 7 and macOS `--check` in CI; manual run on Ubuntu 24.04 (real install + real pinned-hash downloads). |
| Owner-selected features (ADR-011) | SSH connection manager (Host aliases only), SSH auto-reconnect with backoff, Ctrl+click links (OSC 8 confirmed), color tags, output export to Downloads, search across all terminals | Rust: `sshconfig.rs`, `export.rs`, `app_tests.rs` (backoff rules; real-PTY reconnect with a fake ssh), `transfer.rs` (colors); frontend: `controller.test.ts`, `App.test.tsx`; manual run on Linux/WebKitGTK 2026-10-26 (all five features). |
| M10 Packaging & release | deb + AppImage + NSIS bundle config; `release.yml` builds on tags, publishes a **draft** release with SHA-256 sums and build-provenance attestations; dependency inventory; release checklist | `docs/release/RELEASE_CHECKLIST.md`, `docs/security/DEPENDENCIES.md`. |

## Open items (honest list)

- **Human review** of M3–M10, ADR-003 and ADR-006..011.
- **Setup wizard**: Fedora, Arch and openSUSE are verified end to end in CI
  containers (install, check, full build); Windows winget installs are
  exercised in CI, but the UAC relaunch for a standard user still needs one
  manual run; macOS is preparatory.
- **Publication**: history was rewritten and gitleaks is clean (ADR-010), but
  old PR refs on GitHub still hold the original commits; publishing from a
  fresh CRMonize.com repository is recommended.
- **Windows runtime**: Known Folder discovery, ConPTY sessions and external
  launches are exercised on the Windows CI runner only. Manual WebView2 smoke
  (layout continuity, renderer reload, WSL, real `.lnk` dry run with counts
  only) has not been performed.
- **NFR budgets**: PT-1 keystroke latency, PT-3 restore time, anything on
  WebView2, and the WebView stage of PT-4 under sustained load are not yet
  measured. Measured on Linux: core-path throughput ~70 MiB/s exact, scan of
  2,000 entries ~35 ms; **idle RSS with two terminals ~397 MiB, above the
  provisional 300 MB PT-5 budget** — needs PSS measurement and a decision
  (see `docs/research/perf/BASELINE.md`).
- **Accessibility**: automated axe-style checks and NVDA/Orca passes are not
  done; ARIA roles and keyboard paths exist but are not certified.
- **Signing**: binaries are not code-signed; no release has been published.
- Manual release matrix (clean-VM install/uninstall) is pending.

## History

2026-08-26: HUMAN_ARCHITECTURE_GATE=APPROVED; HUMAN_NAMING_GATE=APPROVED;
HUMAN_M0_GATE=APPROVED. GitHub repository created.

2026-08-28: M1 COMPLETE / HUMAN APPROVED / MERGED. `HUMAN_M2_GATE=APPROVED`;
`ADR_004=ACCEPTED` (portable-pty 0.9.0 + ToolOnize mitigations).

2026-10-25: Owner waived per-milestone pauses; M3–M10 implemented on branch
`ccr-9a7d39fd-u0lmc3` and awaiting review.
