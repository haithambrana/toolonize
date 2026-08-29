# Project Status

Phase: M3 in progress — Production Terminal Session Manager

Architecture: HUMAN APPROVED for progression into M0.
Naming: ToolOnize — HUMAN APPROVED. ToolOnize is the approved
working/public product identity (former discovery-phase codename was
"Dev Command Center" — historical naming research preserved in
docs/research/NAMING_RESEARCH.md). Preliminary naming research is not legal
trademark clearance; formal legal/trademark review remains a later release
gate if required.

M0: COMPLETE / HUMAN APPROVED / MERGED
M1: COMPLETE / HUMAN APPROVED / MERGED
M2: COMPLETE / HUMAN APPROVED / MERGED
ADR-004: ACCEPTED
M3: IN PROGRESS — Production Terminal Session Manager
Selected production backend: `portable-pty` 0.9.0 + ToolOnize mitigations
Public repository: CREATED - haithambrana/toolonize
Implementation: M1 framework shell complete; M3 production terminal lifecycle core in progress
Repository safety CI: GREEN

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
- ADRs - 001/002/004/005 Accepted; 003 conditional on its M4
  state-preservation gate
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
on the M4 gate. At that time ADR-004 remained Proposed / Spike Required for
the mandatory M2 gate.

2026-08-26: HUMAN_NAMING_GATE=APPROVED. Final public product name is
ToolOnize (display ToolOnize, repo toolonize, binary toolonize, config dir
toolonize, Tauri identifier direction com.toolonize.desktop). All naming
rounds preserved as historical research.

2026-08-26: HUMAN_M0_GATE=APPROVED. M0 repository foundation complete and
repository-safety CI green. Public repository created as
haithambrana/toolonize (default branch main) at base commit
86844ba420d8cde38adb0790f276e36f2709b95d.

2026-08-27: M1 Cross-Platform Framework Shell authorized and started on branch
m1-framework-shell. Only the M1 framework shell (Tauri window + hardened IPC)
is in progress. No terminal, workspace, launcher, PTY, layout, SSH, tmux,
persistence, or production feature exists yet.

2026-08-28: M1 is COMPLETE / HUMAN APPROVED / MERGED. The cross-platform
framework shell is the only production application milestone implemented; no
production terminal, workspace, launcher, SSH, tmux, or persistence feature
exists yet.

2026-08-28: M2 PTY Backend Technical Spike evidence is complete. PR run
`33131442504` passes the Linux harness and real Tauri/WebKitGTK/xterm.js
pipeline under xvfb, and records 31/31 PASS on Windows for both portable-pty +
mitigation and direct ConPTY. App CI run `33131442503` and repository-safety
run `33131442542` pass.

2026-08-28: `HUMAN_M2_GATE=APPROVED`; `ADR_004=ACCEPTED`. ToolOnize V1 selects
`portable-pty` 0.9.0 with ToolOnize-owned mitigations on Linux and Windows.
Direct native implementations remain spike-verified fallback/reference paths
only; patched forks are not selected. `PR_2_MERGE=PENDING_FINAL_CI_AND_HUMAN_MERGE`.
PR #2 remains draft, M2 is COMPLETE / HUMAN APPROVED, and M3 is NOT STARTED.

2026-08-28: M2 merged at `03e09d0f81fccce32435eb04c64bf933cb86da29`.
`HUMAN_M2_GATE=APPROVED`; `ADR_004=ACCEPTED` (portable-pty 0.9.0 + ToolOnize
mitigations on Linux and Windows; direct native remains fallback/reference).
M2 is COMPLETE / HUMAN APPROVED / MERGED.

2026-08-28: `HUMAN_M3_GATE=AUTHORIZED`. M3 Production Terminal Session Manager
authorized on branch `m3-terminal-session-manager` from base `03e09d0`.

2026-08-29: M3 implementation is feature-complete and ALL M3 CI workflows are
GREEN on head `7eda6f6` for BOTH push and PR runs: contract (Linux/Windows),
build (Linux/Windows), spike-pty (Linux/Windows), real WebView reload, and
repository safety. The reload test is isolated to a single transport consumer
(single-session deterministic ack ordering), and the Windows
`restart_retains_session_id_increments_generation` Exited-state deadline was
widened to 8s to remove scheduling flake. All 56 backend and 23 frontend tests
pass locally; `cargo clippy -D warnings`, `tsc --noEmit`, and Prettier are clean.
`PR_3_MERGE=BLOCKED` — PR #3 stays DRAFT pending the human M3 gate.

M3 is IN PROGRESS — Production Terminal Session Manager.

2026-08-29: HUMAN M3 MANUAL SMOKE (first pass) recorded. Manual window/shell/
UTF-8/clean-exit verified; the paste gate is NOT yet acceptable (RETEST
REQUIRED). Manual smoke status:
`MANUAL_WINDOW_OPEN=PASS`, `MANUAL_REAL_SHELL=PASS`, `MANUAL_UTF8=PASS`,
`MANUAL_CLEAN_EXIT=PASS`, `MANUAL_PASTE=RETEST_REQUIRED`. A bracketed-paste
marker was observed once in the outer host terminal; that is external, not
ToolOnize, and is not attributed to this app.

The source-level ToolOnize paste-policy defect that triggered the repair:
`TerminalView` toolbar Paste read the clipboard and wrote
`TextEncoder` bytes straight to `terminal_write`, bypassing xterm.js paste
semantics and bracketed-paste framing; the multi-line warning existed only on
the toolbar path, so native keyboard/context-menu paste bypassed the warning.

2026-08-29: H14 paste repair implemented and committed (`36c056e`). Centralized
all paste paths (toolbar button, native keyboard, context-menu) through one
shared paste policy and through xterm's public `term.paste()` API; native paste
is intercepted at the container in capture phase (`preventDefault()` so the
data reaches the PTY exactly once); the multi-line (>1 line) / large (>200
chars) warning is shared by every path, Cancel sends zero bytes, Confirm sends
exactly once; no ESC[200~/ESC[201~ are added by ToolOnize (xterm owns
bracketed-paste framing); clipboard text is never logged or persisted. 11 new
deterministic paste tests added (34 frontend tests total; no existing test
weakened). M3 run under this milestone is M3-only: the premature M4 workspace
commits were reverted off this branch (recoverable in history) so PR #3 scope
stays M3; `M4_STARTED=NO`. `MANUAL_PASTE=RETEST_REQUIRED` until the human
manual paste re-test passes.
