# ADR-004: PTY backend — Proposed — Spike Complete / Human Decision Required

Date: 2026-08-26 (spike executed 2026-08-27)
Status: **PROPOSED — SPIKE COMPLETE / HUMAN DECISION REQUIRED** (fresh Windows and xvfb evidence pass; backend selection remains human-gated)

> **Human gate:** Fresh `windows-latest` runtime evidence and the Linux real-WebView/xterm pipeline are green at commit `49a1599`. This ADR remains proposed until a human reviews the evidence, selects a backend shape, and explicitly flips it to `Accepted`.

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

## Spike results (2026-08-27/28, Linux and Windows x86_64)

**Harness:** `tools/spike-pty/` (31 records, 29 PASS, 0 FAIL, 2 Windows-only NOT_VERIFIED on Linux). Report JSON `docs/research/spike-m2/report.json`, human report `docs/research/PTY_SPIKE_REPORT.md`.

**Linux (local):** Both candidates pass every MUST row.
- `portable-pty 0.9.0 + mitigations`: resize is child-observed, UTF-8 and Ctrl+C pass, and high-volume output is exactly 262144 bytes with SHA-256 `97a2fc...00c9`.
- `direct-unix-openpty` (`libc::openpty` + `fork`/`exec`): the same required rows and exact SHA-256 check pass.

**Transport:** Independent producer/slow-consumer threads produce and deliver 2097152 bytes with 63 producer waits, max queue depth 49152 under a 65536-byte capacity, and zero hard breaches. The contrast transport drops 2031616 bytes.

**Real WebView:** A Tauri/WebKitGTK window executed PTY -> Rust -> Tauri Channel -> WebView -> xterm.js with 262144 exact payload bytes, matching SHA-256, awaited xterm writes, input return, child-observed resize, and child exit code 0. PR run `33130859724`, Linux job `98719792866`, reproduced it under fail-closed `xvfb-run`.

**Windows:** PR run `33130859724`, Windows job `98719793033`, recorded 31 PASS, 0 FAIL, 0 BLOCKED, and 0 NOT_VERIFIED after the portable and direct backends were made resilient to DSR requests split across reads. Both `portable-pty-0.9.0` + mitigation and direct ConPTY pass child-observed resize, UTF-8, Ctrl+C, DSR, exact 262144-byte SHA-256, cleanup, shell variants, hidden-console evidence, and five concurrent sessions. Push run `33130857270`, Windows job `98719783997`, independently reproduced the result.

**Proposed recommendation:** portable-pty on Linux plus direct ConPTY on Windows, subject to human review. Both a fully portable backend and the hybrid pass every MUST row; the hybrid avoids the documented portable-pty Windows lifecycle regressions at the cost of owning the Win32 adapter. This is evidence for the decision, not acceptance of it.

## Consequences

- M3 cannot start until this ADR reaches `Accepted` via the M2 gate (IMPLEMENTATION_PLAN rollback condition). The technical gate is ready for human review but does not itself flip the status.
- Short-term cost: spike harness code (`tools/spike-pty`, `src-tauri/src/commands/spike.rs` behind feature `spike`, `src/spike/TerminalSpike.tsx`); long-term benefit: terminal correctness is now evidence-gated.
- **Next step for human:** Review `docs/research/PTY_SPIKE_REPORT.md`, `docs/research/spike-m2/report.json`, and run `33130859724`; then either request changes or flip this ADR to `Accepted` with the chosen backend (portable, direct, or hybrid) before M3.

## Links

TECHNOLOGY_RESEARCH §4; IMPLEMENTATION_PLAN M2/M3; TEST_STRATEGY §5;
THREAT_MODEL risk R1; PTY_SPIKE_REPORT.md; spike-m2/report.json
