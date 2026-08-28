# ADR-004: PTY backend - portable-pty 0.9.0 with mitigations

Date: 2026-08-26 (spike executed 2026-08-27/28; accepted 2026-08-28)
Status: ACCEPTED

> **Human decision:** `HUMAN_M2_GATE=APPROVED`; `ADR_004=ACCEPTED`.
> ToolOnize V1 uses `portable-pty` 0.9.0 with explicit ToolOnize-owned
> integration mitigations on Linux and Windows.

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

ToolOnize V1 uses `portable-pty` 0.9.0 with ToolOnize-owned integration
mitigations on both supported platforms:

- Linux: `portable-pty` 0.9.0.
- Windows: `portable-pty` 0.9.0 over ConPTY.

The internal `PtyBackend` abstraction remains the production boundary so the
backend can be replaced without redesigning terminal sessions. The verified
`direct-unix-openpty` and `direct-windows-ConPTY` implementations remain
isolated, non-production spike fallback/reference paths. M3 must not wire them
into normal execution, expose runtime backend selection, or add a user backend
preference. Patched portable-pty forks are not selected.

`Cargo.lock` is authoritative for reproducible builds. Acceptance of this ADR
does not authorize an automatic portable-pty upgrade.

## Mandatory integration mitigations

M3 production integration must preserve regression coverage for all of these
constraints:

1. DSR/CPR startup handling must retain an incomplete `ESC[6n` across reads
   and respond only after the complete request is observed.
2. Input-writer lifetime must be controlled so premature writer drop does not
   terminate a Windows ConPTY child.
3. Output transport must be bounded and lossless: no drop-oldest, drop-newest,
   or silent truncation.
4. Resize must be verified by the child, not inferred from the API call.
5. High-volume output must retain exact byte-count and SHA-256 integrity
   regression coverage.
6. Ctrl+C semantics must remain covered on Linux and Windows.
7. UTF-8 and VT sequences must be preserved without lossy conversion.
8. Child exit, cleanup, orphan prevention, and resource lifecycle must remain
   covered.
9. Concurrent sessions must remain isolated.
10. Timeouts and transport desynchronization must fail explicitly rather than
    hang.

## Dependency upgrade gate

Any change from `portable-pty` 0.9.0 must pass the relevant M2 regression
matrix on both Linux and Windows before adoption. Windows coverage must include
DSR/CPR, input-writer lifetime, resize, shells, UTF-8, Ctrl+C, exact
high-volume integrity, cleanup, and concurrency. An upgrade requires reviewed
evidence; dependency tooling must not advance this baseline automatically.

## Alternatives considered

- A direct native backend on both platforms passed the spike but was not
  selected because it requires ToolOnize to own more platform-specific code.
- A hybrid of portable-pty on Linux and direct ConPTY on Windows passed every
  MUST row and was the pre-decision technical recommendation, but was not
  selected for V1.
- Patched portable-pty forks were not selected because their low adoption adds
  supply-chain and maintenance risk.

## Spike results (2026-08-27/28, Linux and Windows x86_64)

**Harness:** `tools/spike-pty/` (31 records, 29 PASS, 0 FAIL, 2 Windows-only NOT_VERIFIED on Linux). Report JSON `docs/research/spike-m2/report.json`, human report `docs/research/PTY_SPIKE_REPORT.md`.

**Linux (local):** Both candidates pass every MUST row.
- `portable-pty 0.9.0 + mitigations`: resize is child-observed, UTF-8 and Ctrl+C pass, and high-volume output is exactly 262144 bytes with SHA-256 `97a2fc...00c9`.
- `direct-unix-openpty` (`libc::openpty` + `fork`/`exec`): the same required rows and exact SHA-256 check pass.

**Transport:** Independent producer/slow-consumer threads produce and deliver 2097152 bytes with 63 producer waits, max queue depth 49152 under a 65536-byte capacity, and zero hard breaches. The contrast transport drops 2031616 bytes.

**Real WebView:** A Tauri/WebKitGTK window executed PTY -> Rust -> Tauri Channel -> WebView -> xterm.js with 262144 exact payload bytes, matching SHA-256, awaited xterm writes, input return, child-observed resize, and child exit code 0. PR run `33130859724`, Linux job `98719792866`, reproduced it under fail-closed `xvfb-run`.

**Windows:** PR run `33130859724`, Windows job `98719793033`, recorded 31 PASS, 0 FAIL, 0 BLOCKED, and 0 NOT_VERIFIED after the portable and direct backends were made resilient to DSR requests split across reads. Both `portable-pty-0.9.0` + mitigation and direct ConPTY pass child-observed resize, UTF-8, Ctrl+C, DSR, exact 262144-byte SHA-256, cleanup, shell variants, hidden-console evidence, and five concurrent sessions. Push run `33130857270`, Windows job `98719783997`, independently reproduced the result.

**Decision interpretation:** Both a fully portable backend and the hybrid pass every MUST row. The pre-decision report recommended portable-pty on Linux plus direct ConPTY on Windows to avoid the documented portable-pty Windows lifecycle risks. The human reviewer considered that recommendation and selected the fully portable shape because the mitigated portable backend also passed, while direct ConPTY would make ToolOnize own Win32 handle lifecycle, `CreatePseudoConsole`, `CreateProcessW`, process attribute lists, pipe ownership, command-line quoting, polling, cleanup, unsafe FFI invariants, and Windows-version behavior. The upstream portable-pty risks remain real and are controlled by the mandatory mitigations and upgrade gate above.

## Consequences

- M2 and ADR-004 are human-approved. M3 remains not started and is outside this
  decision-recording change.
- Production terminal work must use portable-pty behind `PtyBackend` and carry
  the mandatory mitigations into implementation and acceptance criteria.
- The direct implementations remain available only as isolated spike evidence
  and fallback/reference code. Promotion requires production evidence of a
  portable-pty blocker and a reviewed ADR amendment.
- The spike harness and fail-closed hosted matrix remain regression assets for
  dependency upgrades and backend-risk changes.
- PR #2 remains draft pending final post-decision CI review and human merge.

## Links

TECHNOLOGY_RESEARCH §4; IMPLEMENTATION_PLAN M2/M3; TEST_STRATEGY §5;
THREAT_MODEL risk R1; PTY_SPIKE_REPORT.md; spike-m2/report.json
