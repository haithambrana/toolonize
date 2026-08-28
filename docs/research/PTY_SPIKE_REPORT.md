# PTY Spike Report - M2

Date: 2026-08-28

Branch: `m2-pty-spike`

Human decision: `HUMAN_M2_GATE=CHANGES_REQUIRED`

ADR: `ADR_004=PROPOSED_NOT_ACCEPTED`

Merge: `PR_2_MERGE=BLOCKED`

This report separates local execution, hosted Windows execution, and hosted
real-WebView execution. A successful job, a Linux report, a transport model,
or a cached report is never substituted for platform-specific evidence.

## Candidates

| Candidate | Spike implementation | Material concern |
|-----------|----------------------|------------------|
| `portable-pty` 0.9.0 + mitigation | `native_pty_system`, bounded reader pump, DSR response | wezterm/wezterm#6783 and vercel/turborepo#11816 document Windows DSR and stdin-lifecycle regressions |
| Direct native | `libc::openpty` on Linux; windows-rs `CreatePseudoConsole` on Windows | More owned unsafe FFI and lifecycle code; hosted Windows execution now passes |
| Patched forks | Research only | Low adoption and additional supply-chain risk |

The harness keeps the choice behind `PtyBackend`; ADR-004 remains proposed
until the human reviewer accepts one candidate or a hybrid.

## Harness

The throwaway crate is `tools/spike-pty/` (`publish = false`). It exercises
spawn, shell variants, invalid executables, child-observed resize, UTF-8,
Ctrl+C termination, DSR behavior, exact high-volume output, cleanup, TUI and
agent-style output, hidden-console structure, clipboard input, and concurrent
sessions.

The portable backend isolates its blocking reader behind a single reader pump.
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

PR run [`33129730084`](https://github.com/haithambrana/toolonize/actions/runs/33129730084),
Linux job `98716178189`, passed at commit `f2d518f`. It independently recorded
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

PR run `33129730084`, Linux job `98716178189`, independently reproduced this
under `xvfb-run` and emitted exactly one matching `M2_REAL_WEBVIEW_REPORT`.
The job also passed the spike-feature Tauri build check.

## Windows Status

`cargo check --target x86_64-pc-windows-msvc --all-targets` passes locally, but
the runtime claim comes only from hosted Windows execution.

PR run [`33129730084`](https://github.com/haithambrana/toolonize/actions/runs/33129730084),
Windows job `98716178092`, passed at commit `f2d518f`:

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
[`33129727977`](https://github.com/haithambrana/toolonize/actions/runs/33129727977),
Windows job `98716170413`, reproduced the same all-PASS result. Earlier failed
and timed-out runs are retained as repair history and are not counted as
passing evidence.

## Recommendation for Human Review

Both candidates satisfy every MUST row on Linux and Windows. The proposed
selection for human review is a hybrid: `portable-pty` on Linux and direct
ConPTY on Windows. This preserves the mature Linux abstraction while avoiding
the documented portable-pty Windows DSR/stdin lifecycle regressions; direct
Windows also showed lower high-volume elapsed time in the canonical run. The
tradeoff is ownership of a small Win32 lifecycle adapter. A single portable
backend remains a viable lower-maintenance alternative because its mitigated
Windows path also passed every row. This recommendation is not an accepted
architecture decision.

## Current Gate

| Evidence | State |
|----------|-------|
| Linux portable backend | PASS locally and in job `98716178189` |
| Linux direct backend | PASS locally and in job `98716178189` |
| Bounded slow-consumer backpressure | PASS locally and in hosted CI |
| Real Tauri/WebKitGTK/xterm.js pipeline | PASS locally and in job `98716178189` |
| Windows portable backend runtime | PASS in job `98716178092` |
| Windows direct ConPTY runtime | PASS in job `98716178092` |
| Full app CI | PASS in run `33129730089` |
| Repository safety | PASS in run `33129730118` |
| ADR-004 human selection | NOT ACCEPTED |
| PR #2 merge | BLOCKED |

The executed technical evidence is complete for human review. Patched forks
remain not recommended based on current supply-chain evidence. Only the human
reviewer may select the backend, accept ADR-004, change the M2 human gate, or
authorize merge.

M2_PTY_SPIKE_GATE=READY_FOR_HUMAN_REVIEW
