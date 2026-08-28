# PTY Spike Report - M2

Date: 2026-08-28

Branch: `m2-pty-spike`

Human decision: `HUMAN_M2_GATE=APPROVED`

ADR: `ADR_004=ACCEPTED`

Merge: `PR_2_MERGE=PENDING_FINAL_CI_AND_HUMAN_MERGE`

This report separates local execution, hosted Windows execution, and hosted
real-WebView execution. A successful job, a Linux report, a transport model,
or a cached report is never substituted for platform-specific evidence.

## Candidates

| Candidate | Spike implementation | Material concern |
|-----------|----------------------|------------------|
| `portable-pty` 0.9.0 + mitigation | `native_pty_system`, bounded reader pump, DSR response | wezterm/wezterm#6783 and vercel/turborepo#11816 document Windows DSR and stdin-lifecycle regressions |
| Direct native | `libc::openpty` on Linux; windows-rs `CreatePseudoConsole` on Windows | More owned unsafe FFI and lifecycle code; hosted Windows execution now passes |
| Patched forks | Research only | Low adoption and additional supply-chain risk |

ADR-004 selects `portable-pty` 0.9.0 with ToolOnize-owned mitigations on Linux
and Windows. The harness keeps the choice behind `PtyBackend`; the direct
native implementations remain isolated spike fallback/reference paths, not V1
production backends.

## Harness

The throwaway crate is `tools/spike-pty/` (`publish = false`). It exercises
spawn, shell variants, invalid executables, child-observed resize, UTF-8,
Ctrl+C termination, DSR behavior, exact high-volume output, cleanup, TUI and
agent-style output, hidden-console structure, clipboard input, and concurrent
sessions.

The portable backend isolates its blocking reader behind a single reader pump,
retains incomplete DSR requests across reads, and controls input-writer
lifetime. These are required integration behaviors, not claims that the
upstream Windows risks have disappeared.
The direct Windows backend polls its synchronous ConPTY output pipe with
`PeekNamedPipe`. Harness reads return `WouldBlock` after a bounded interval,
allowing scenario deadlines to fire without an unsafe `Send` override. The
direct Windows implementation:

- passes the `HPCON` value required by `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`;
- uses aligned process-attribute storage and checks every fallible Win32 call;
- quotes the Windows command line and explicitly invalidates inherited stdio;
- owns process/thread handles with `OwnedHandle`, waits for child termination,
  closes both communication endpoints, and then closes `HPCON` exactly once;
- responds to DSR even when `ESC[6n` is split across reads;
- calls `ResizePseudoConsole` and fails on error;
- does not use an unsafe `Send` implementation.

## Linux Backend Evidence

Command:

```bash
cargo run --manifest-path tools/spike-pty/Cargo.toml --bin spike-pty
```

Source: `docs/research/spike-m2/report.json`. The report has explicit
`platform: "linux"` and `architecture: "x86_64"` metadata.

```text
Total: 31, PASS: 29, FAIL: 0, BLOCKED: 0, NOT_VERIFIED: 2
```

The two `NOT_VERIFIED` records are the Windows-only hidden-console row, one per
backend. Every runnable required Linux row passes for both
`portable-pty-0.9.0` and `direct-unix-openpty`.

Key executed results:

| Contract | Portable | Direct Unix |
|----------|----------|-------------|
| Child-observed resize | `SIZE=40x120` | `SIZE=40x120` |
| UTF-8 emoji/CJK/accented/combining | PASS | PASS |
| Ctrl+C terminates child | PASS | PASS |
| DSR timeout | PASS | PASS |
| Cleanup, 20 cycles | fd `4 -> 4` | fd `4 -> 4` |
| Concurrent sessions | 5 isolated PASS | 5 isolated PASS |

### Exact High Volume

The logical payload is exactly `262144` `A` bytes bounded by
`PAYLOAD_START`/`DONE_MARKER`. Linux compares the raw bytes without
normalization. Windows emits the payload in 64-byte rows to avoid ConPTY's
automatic-wrap redraw. The Windows verifier removes only predefined ConPTY
terminal protocol bytes (C0 line/cursor controls, CSI, OSC, and the exact
space/backspace cursor artifact), rejects every unexpected printable byte,
then requires the exact byte count and SHA-256. There is no percentage
threshold.

```text
PAYLOAD_BYTES=262144
EXPECTED_SHA256=97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9
DELIVERED_BYTES=262144
DELIVERED_SHA256=97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9
EXACT_MATCH=true
```

Both Linux backends and both hosted Windows backends produce this result.

### Hosted Linux

PR run [`33131442504`](https://github.com/haithambrana/toolonize/actions/runs/33131442504),
Linux job `98721662032`, passed at commit `3563af5`. It independently recorded
the same 31-row Linux summary and exact SHA-256 for both backends.

## Backpressure Evidence

The experiment uses independent producer and deliberately slow consumer
threads coordinated by a condition variable. The producer waits after the
high-water mark until the queue reaches the low-water mark.

```text
CAPACITY=65536
HIGH_WATER=49152
LOW_WATER=16384
PRODUCED=2097152
DELIVERED=2097152
DROPPED=0
MAX_QUEUE_DEPTH=49152
BACKPRESSURE_EVENTS=63
HARD_LIMIT_BREACHES=0
LOSSLESS=true
```

The contrast transport produced `2097152`, delivered `65536`, and silently
dropped `2031616` bytes. Unit tests require backpressure greater than zero,
queue depth within capacity, no loss, and explicit desynchronization on a hard
limit breach.

## Real WebView Evidence

The dedicated config `src-tauri/tauri.spike.conf.json` starts Vite with
`VITE_M2_SPIKE=1` and loads `?spikeAuto=1`. The local real-display command was:

```bash
npm run tauri -- dev --no-watch --features spike --config src-tauri/tauri.spike.conf.json
```

The actual Tauri window launched on Linux and emitted:

```json
{"payloadBytes":262144,"deliveredPayloadBytes":262144,"expectedSha256":"97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9","deliveredSha256":"97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9","exactByteIntegrity":true,"xtermWriteCompleted":true,"inputReturn":true,"realResize":true,"processExitCode":0}
```

This is a real `Terminal` opened in WebKitGTK. Each `term.write` callback is
awaited before completion. Raw Channel bytes are concatenated and compared
exactly, SHA-256 is calculated in the WebView, resize is reported by the PTY
child, and input traverses WebView -> Rust -> PTY -> Rust -> WebView. Rust
validates the browser report, prints `M2_REAL_WEBVIEW_REPORT=...`, and exits
nonzero on mismatch. There is no simulated fallback and CI does not swallow
timeouts.

PR run `33131442504`, Linux job `98721662032`, independently reproduced this
under `xvfb-run` and emitted exactly one matching `M2_REAL_WEBVIEW_REPORT`.
The job also passed the spike-feature Tauri build check.

## Windows Status

`cargo check --target x86_64-pc-windows-msvc --all-targets` passes locally, but
the runtime claim comes only from hosted Windows execution.

PR run [`33131442504`](https://github.com/haithambrana/toolonize/actions/runs/33131442504),
Windows job `98721661871`, passed at commit `3563af5`:

```text
Total: 31, PASS: 31, FAIL: 0, BLOCKED: 0, NOT_VERIFIED: 0
non-PASS records: 0
has_portable: True, has_direct_windows: True, has_unix: False
```

| Contract | Portable 0.9.0 + mitigation | Direct Windows ConPTY |
|----------|-----------------------------|------------------------|
| Child-observed resize | `SIZE=40x120` | `SIZE=40x120` |
| UTF-8 emoji/CJK/accented/combining | PASS | PASS |
| Ctrl+C termination | PASS | PASS |
| DSR startup/response | PASS | PASS |
| Exact 256 KiB SHA-256 | `262144`, exact match | `262144`, exact match |
| Cleanup, 20 cycles | handles `126 -> 126` | handles `127 -> 127` |
| Concurrent sessions | 5 isolated PASS | 5 isolated PASS |
| Shells | PowerShell/cmd/pwsh PASS; WSL unavailable | PowerShell/cmd/pwsh PASS; WSL unavailable |
| Hidden console | native ConPTY path | pseudoconsole attribute; no `CREATE_NEW_CONSOLE` |

The independent push run
[`33131439161`](https://github.com/haithambrana/toolonize/actions/runs/33131439161),
Windows job `98721650821`, reproduced the same all-PASS result. Failed PR run
`33130352404` at commit `68a375c` exposed a portable ConPTY DSR request split
across reads. Commit `49a1599` retains the partial request and responds only
when it is complete; repair PR run `33130859724` and final pre-decision run
`33131442504` passed. Post-decision push run `33132617007` then exposed two
runner-speed assumptions in the portable Windows scenarios: a fixed
three-second shell-output window and Ctrl+C sent before PowerShell explicitly
reported readiness. The harness now uses a bounded five-second Windows startup
window and waits for `CTRLC_READY` before sending Ctrl+C. All failed and
timed-out runs remain repair history and are not counted as passing evidence.

## Human Architecture Decision

The Human Product/Technical Lead selected `portable-pty` 0.9.0 with explicit
ToolOnize integration mitigations as the V1 production backend on Linux and
Windows. `HUMAN_M2_GATE=APPROVED` and `ADR_004=ACCEPTED`.

Both portable-pty with mitigations and the direct native implementations pass
every MUST row. Before human review, this report recommended portable-pty on
Linux plus direct ConPTY on Windows to avoid the documented portable-pty
Windows DSR and input-writer lifecycle risks. The human reviewer considered
that recommendation and selected the single portable backend because it also
passed the complete matrix and avoids ToolOnize ownership of Win32 handle
lifecycle, `CreatePseudoConsole`, `CreateProcessW`, process attribute lists,
pipe ownership, command-line quoting, polling, cleanup, unsafe FFI invariants,
and Windows-version behavior.

This selection does not erase the upstream risks. M3 production integration
must preserve regression coverage for:

1. split DSR/CPR startup requests, retaining incomplete `ESC[6n` sequences;
2. input-writer lifetime without premature ConPTY child termination;
3. bounded lossless output with no silent dropping or truncation;
4. child-observed resize;
5. exact high-volume byte-count and SHA-256 integrity;
6. Ctrl+C semantics;
7. lossless UTF-8 and VT sequence preservation;
8. child exit, cleanup, and process/resource lifecycle;
9. concurrent-session isolation; and
10. explicit timeout/desynchronization failure instead of hangs.

`Cargo.lock` remains authoritative. Any portable-pty version upgrade must pass
the relevant M2 matrix on Linux and Windows before adoption, including all
Windows DSR/CPR, input lifetime, resize, shells, UTF-8, Ctrl+C, exact-volume,
cleanup, and concurrency regressions.

The verified `direct-unix-openpty` and `direct-windows-ConPTY` paths remain
fallback/reference spike implementations only. M3 must not wire them into
normal execution or add runtime/user backend selection. Patched portable-pty
forks remain not selected. Future promotion of direct Windows ConPTY requires
production evidence of a portable-pty blocker and a reviewed ADR amendment.

## Current Gate

| Evidence | State |
|----------|-------|
| Linux portable backend | PASS locally and in job `98721662032` |
| Linux direct backend | PASS locally and in job `98721662032` |
| Bounded slow-consumer backpressure | PASS locally and in hosted CI |
| Real Tauri/WebKitGTK/xterm.js pipeline | PASS locally and in job `98721662032` |
| Windows portable backend runtime | PASS in job `98721661871` |
| Windows direct ConPTY spike runtime | PASS in job `98721661871` |
| Full app CI | PASS in run `33131442503` |
| Repository safety | PASS in run `33131442542` |
| V1 production backend | portable-pty 0.9.0 + ToolOnize mitigations on Linux and Windows |
| Direct native implementations | SPIKE-VERIFIED FALLBACK / REFERENCE ONLY |
| ADR-004 human selection | ACCEPTED |
| PR #2 merge | PENDING FINAL CI AND HUMAN MERGE |

The M2 technical evidence and human architecture decision are complete. PR #2
remains draft until final post-decision CI is reviewed and a human authorizes
merge. M3 has not started.

M2_PTY_SPIKE_GATE=APPROVED
