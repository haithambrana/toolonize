# ADR-004: PTY backend — Proposed / Spike Required

Date: 2026-08-26
Status: **Proposed / Spike Required** (must be resolved by milestone M2 gate
before production terminal work proceeds)

## Context

The terminal session layer needs a cross-platform PTY abstraction (Linux
openpty; Windows ConPTY) supporting shells, WSL, ssh/tmux passthrough,
resize, UTF-8, TUIs/full-screen agent CLIs, clean exit detection and
cleanup. `portable-pty` was the leading candidate, but research surfaced
material Windows concerns that forbid selecting it (or any library) on
claims alone.

## Verified evidence of concern

1. `portable-pty` 0.9.0 (published 2025-02-11, lives inside the wezterm
   monorepo — no standalone repo) [crates.io, retrieved 2026-08-26].
2. Open upstream issue wezterm/wezterm#6783 (filed 2025-03-11, activity into
   2026-07): 0.9.0 "doesn't work on Windows" for minimal consumers; root-
   cause discussion identifies ConPTY created with
   `PSEUDOCONSOLE_INHERIT_CURSOR`, which makes ConPTY emit a Device Status
   Report request (`ESC[6n`) during startup; embedders that never respond
   deadlock. Maintainer notes limited Windows testing availability.
   [GitHub issue thread, retrieved 2026-08-26]
3. Independent downstream confirmation: vercel/turborepo PR #11816 (merged
   2026-02-12) documents (a) the same DSR/CPR startup hang after upgrading
   portable-pty 0.8.1→0.9.0 and (b) a second regression where unconditional
   stdin drop terminates ConPTY children on Windows; fix responds to the DSR
   and gates the stdin drop behind non-Windows. Adds regression tests.
   [GitHub PR, retrieved 2026-08-26]
4. Fork ecosystem exists specifically to patch ConPTY behavior:
   psmux/portable-pty-patched adds `PSEUDOCONSOLE_PASSTHROUGH_MODE`,
   `WIN32_INPUT_MODE`, `RESIZE_QUIRK` with Win11 22H2+ runtime detection,
   noting upstream doesn't pass modern flags; `xpty` fork targets async +
   better Windows control but has negligible adoption. [repos/crates.io,
   retrieved 2026-08-26]

Conclusion: the candidate space is real but unsettled; Windows behavior
differs by flag configuration and consumer discipline. This is precisely the
class of decision our constitution requires be settled by experiment.

## Decision

**Do not lock a PTY backend now.** Instead:

1. The architecture defines an internal `PtyBackend` trait (spawn / read /
   write / resize / kill / wait) so backend choice is isolated behind one
   boundary.
2. Milestone M2 executes a throwaway-spike comparison of:
   - **A**: `portable-pty` 0.9.0 + documented mitigations (respond-to-DSR
     handshake; guarded stdin drop per turborepo findings);
   - **B**: patched fork (psmux flags), assessed also for supply-chain risk;
   - **C**: thin in-house implementation over `libc`(openpty) /
     `windows` crate `CreatePseudoConsole`.
3. The spike must validate, per platform: Linux PTY; Windows ConPTY;
   PowerShell; cmd; WSL; resize; UTF-8; cursor behavior incl. DSR/CPR
   handshake; Ctrl+C; clipboard boundary behavior; high-volume output;
   TUIs; OpenCode-like full-screen apps; process exit; reconnect/restart;
   hidden console-window suppression; resource cleanup (no orphans/handle
   leaks).
4. Selection rule: choose the option passing every MUST row, weighing
   maintenance reality (upstream responsiveness evidence above) over
   feature claims. Result recorded by flipping this ADR to Accepted with
   the results table linked under docs/research/spike-m2/.

## Alternatives (post-spike shapes)

- Adopt A/B/C outright; or hybrid (e.g., C on Windows, A on Linux) if
  evidence supports asymmetry — allowed because the trait isolates it.

## Consequences

- M3 cannot start until this ADR reaches Accepted via the M2 gate
  (IMPLEMENTATION_PLAN rollback condition).
- Short-term cost: spike harness code; long-term benefit: terminal
  correctness is the product's foundation and is now evidence-gated.

## Links

TECHNOLOGY_RESEARCH §4; IMPLEMENTATION_PLAN M2/M3; TEST_STRATEGY §5;
THREAT_MODEL risk R1.
