# Roadmap

Status: APPROVED through architecture + naming gates; M0 repository foundation
in progress. Milestones are dependency-ordered without calendar dates (per
engineering constitution: no fabricated estimates).

## Phase 0 — Discovery & architecture — APPROVED
- Competitive, technology, platform-discovery research. **Done.**
- PRD, architecture, threat model, ADRs, implementation plan, test strategy.
  **Human-approved** (see docs/product/STATUS.md).
- Naming: **ToolOnize** — human-approved (former discovery-phase codename
  "Dev Command Center"; historical research preserved in
  docs/research/NAMING_RESEARCH.md).
- Preliminary naming research is not legal trademark clearance; formal
  legal/trademark review remains a later release gate if required.
- After M0 human approval the first local commit may occur; public GitHub
  repository creation remains a separate human-controlled step. M1 has not
  started.

## Phase 1 — Foundations (M0–M2)
- M0 Repository/public-safety foundation (CI skeleton, security policy,
  fixture conventions).
- M1 Cross-platform framework shell (Tauri window on Linux+Windows, IPC
  sanity, empty React UI shell).
- M2 PTY technical spike **+ critical integration risk gate** (Linux +
  Windows): PTY backend behavior; PTY→Rust→Tauri→WebView→xterm.js
  throughput; lossless byte integrity/backpressure on the full path; xterm
  instance lifecycle; FlexLayout live-terminal move/maximize/restore smoke;
  renderer reload semantics — both OSes. Resolves ADR-004.
- Gate: spike report accepted AND all six critical risks validated on both
  platforms; framework renders on both OSes.

## Phase 2 — Terminal core (M3)
Locked until the M2 critical-risk gate passes. Session manager, terminal
lifecycle independent of layout (process state ≠ view attachment), lossless
output pump, copy/paste/search/resize, exit/reconnect semantics with honest
plain-SSH behavior.

## Phase 3 — Workspace core (M4)
- flexlayout integration with state-preservation proof; workspace model;
  Grid/Focus/Tabs/Master+Stack modes; layout persistence round-trip.

## Phase 4 — Discovery (M5–M6)
- M5 Linux adapter (.desktop pipeline incl. user Desktop dir via
  xdg-user-dirs + opt-in custom roots, Flatpak/Snap, inotify rescan).
- M6 Windows adapter (Known Folders, .lnk resolution policy spike
  [stored-metadata vs conservative Resolve], duplicate-merge verification,
  change watching, packaged-app spike decision).

## Phase 5 — Trust layer (M7)
- Classification engine, review queue UX, execution policy + authorization,
  external app launching.

## Phase 6 — Durability (M8)
- Persistence, crash recovery journal, schema migrations, import/export
  (non-secret), startup policies.

## Phase 7 — Productization (M9–M10)
- M9 Accessibility, theming, keyboard map polish, NFR budget ratification
  with measured evidence (PT targets as subordinate metrics).
- M10 Packaging (deb/AppImage/MSIX-or-NSIS decision documented), CI release
  pipeline, GitHub Releases (checksums; provenance attestations evaluated;
  native signing per feasibility gate), smoke-test checklist, public-safety
  final pass.

## Post-V1 candidates (explicitly not committed)
Packaged-app discovery (if spike proves it), PATH opt-in scan, plugin API,
updater with signatures, macOS evaluation.

Detailed work units, tests, and acceptance criteria per milestone:
IMPLEMENTATION_PLAN.md.
