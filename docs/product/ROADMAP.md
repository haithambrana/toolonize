# Roadmap

Status: M0–M2 complete, human-approved, and merged. M3–M10 implemented in
one pass on 2026-10-25 after the owner waived per-milestone review pauses;
all of them await human review (see STATUS.md for evidence and open items).
Milestones are dependency-ordered without calendar dates (per engineering
constitution: no fabricated estimates).

## Phase 0 — Discovery & architecture — APPROVED
- Competitive, technology, platform-discovery research. **Done.**
- PRD, architecture, threat model, ADRs, implementation plan, test strategy.
  **Human-approved** (see docs/product/STATUS.md).
- Naming: **ToolOnize** — human-approved (former discovery-phase codename
  "Dev Command Center"; historical research preserved in
  docs/research/NAMING_RESEARCH.md).
- Preliminary naming research is not legal trademark clearance; formal
  legal/trademark review remains a later release gate if required.
- M0 repository foundation and public-repository gate: **Complete / human
  approved / merged.**

## Phase 1 — Foundations (M0–M2)
- M0 Repository/public-safety foundation (CI skeleton, security policy,
  fixture conventions): **Complete / human approved / merged.**
- M1 Cross-platform framework shell (Tauri window on Linux+Windows, IPC
  sanity, empty React UI shell): **Complete / human approved / merged.**
- M2 PTY technical spike **+ critical integration risk gate** (Linux +
  Windows): PTY backend behavior; PTY→Rust→Tauri→WebView→xterm.js
  throughput; lossless byte integrity/backpressure on the full path; xterm
  instance lifecycle; FlexLayout live-terminal move/maximize/restore smoke;
  renderer reload semantics — both OSes. **Complete / human approved;
  ADR-004 accepted; merged.**
- Gate: spike report accepted AND all six critical risks validated on both
  platforms; framework renders on both OSes. **Passed.**
- Selected backend: `portable-pty` 0.9.0 + ToolOnize-owned mitigations on
  Linux and Windows. Direct native implementations remain spike-verified
  fallback/reference paths only.

## Phase 2 — Terminal core (M3) — IMPLEMENTED, pending review
Session manager, terminal lifecycle independent of layout (process state ≠
view attachment), lossless output pump, copy/paste/search/resize,
exit/reconnect semantics with honest plain-SSH behavior.

## Phase 3 — Workspace core (M4) — IMPLEMENTED, pending review
- flexlayout integration with a stable terminal-host architecture (ADR-003
  gate passed in automated tests); workspace model; Grid/Focus/Tabs/
  Master+Stack modes; layout persistence round-trip.

## Phase 4 — Discovery (M5–M6) — IMPLEMENTED, pending review
- M5 Linux adapter (.desktop pipeline incl. user Desktop dir via
  xdg-user-dirs + opt-in custom roots, Flatpak/Snap, change watching).
- M6 Windows adapter (Known Folders, stored-metadata `.lnk` reader —
  ADR-007; duplicates retained with provenance; packaged apps deferred).
  Windows runtime verification beyond CI is still owed.

## Phase 5 — Trust layer (M7) — IMPLEMENTED, pending review
- Classification engine, review queue UX, execution policy + authorization,
  external app launching, audit log.

## Phase 6 — Durability (M8) — IMPLEMENTED, pending review
- Atomic checksummed snapshots (ADR-006), schema versioning, import/export
  (non-secret) with review staging, startup policies.

## Phase 7 — Productization (M9–M10) — IMPLEMENTED, pending review
- M9 themes incl. high contrast, reduced motion, keymap + command palette,
  failure-state UX, perf harness with a partial Linux baseline.
- M10 deb/AppImage/NSIS packaging, draft-release pipeline with SHA-256 sums
  and build-provenance attestations (no signing), release checklist,
  dependency inventory. No release published.

## Post-V1 candidates (explicitly not committed)
Packaged-app discovery (if spike proves it), PATH opt-in scan, plugin API,
updater with signatures, macOS evaluation.

Detailed work units, tests, and acceptance criteria per milestone:
IMPLEMENTATION_PLAN.md.
