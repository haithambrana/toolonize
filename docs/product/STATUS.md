# Project Status

Phase: M0 — Repository Foundation (in progress)

Architecture: HUMAN APPROVED for progression into M0.

Naming: ToolOnize — HUMAN APPROVED. ToolOnize is the approved
working/public product identity (former discovery-phase codename was
"Dev Command Center" — historical naming research preserved in
docs/research/NAMING_RESEARCH.md). Preliminary naming research is not legal
trademark clearance; formal legal/trademark review remains a later release
gate if required.

Implementation: NOT STARTED

Public repository: NOT CREATED

Tauri identifier direction: com.toolonize.desktop
CLI / binary: toolonize
Repository slug: toolonize
Config-directory direction: toolonize

## Required gates before implementation

- Market and competitor research — DRAFTED, revised after human
  architecture review incl. Microsoft PowerToys Workspaces/Command Palette
  as adjacent competitors (docs/research/COMPETITIVE_ANALYSIS.md)
- Product positioning — APPROVED (PRD §1–§5)
- Naming — APPROVED: ToolOnize (final human decision recorded in
  docs/research/NAMING_RESEARCH.md, Final Human Naming Decision)
- V1 scope — APPROVED (PRD §15)
- PRD — HUMAN APPROVED
- Architecture — HUMAN APPROVED
- Threat model — HUMAN APPROVED
- Implementation plan — HUMAN APPROVED
- Test strategy — HUMAN APPROVED
- ADRs — 001/002/005 Accepted; 003 conditional on its M4 state-preservation
  gate; 004 Proposed / Spike Required, must be resolved in M2
- Public-repository secret-safety review — rules defined (docs/security/PUBLIC_REPOSITORY_SAFETY.md); full-history scan required before any public push

2026-08-26: human architecture review corrections applied (documentation
only) — PowerToys added to competitive set; Linux Desktop-dir/custom-root
discovery sources; launcher identity split from descriptor authorization
fingerprint; lossless PTY output requirement; process-state vs view-state
separation; NFR/test ID layers; Windows merge/Resolve semantics marked
Spike/Verification Required; release-integrity wording corrected.

2026-08-26: HUMAN_ARCHITECTURE_GATE=APPROVED. PRD, architecture, threat
model, implementation plan, and test strategy are human-approved for
progression into M0. This approval does NOT claim any implementation exists.
ADR-001, ADR-002, and ADR-005 moved to Accepted; ADR-003 remains conditional
on the M4 gate; ADR-004 remains Proposed / Spike Required (M2 mandatory gate).

2026-08-26: HUMAN_NAMING_GATE=APPROVED. Final public product name is
ToolOnize (display ToolOnize, repo toolonize, binary toolonize, config dir
toolonize, Tauri identifier direction com.toolonize.desktop). All naming
rounds preserved as historical research.

Nothing in this repository is implemented. All capability statements are
researched / decided / proposed / planned — none are implemented.
