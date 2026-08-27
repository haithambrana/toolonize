# PTY Spike Report - M2

Date: 2026-08-28

Branch: `m2-pty-spike`

Human decision: `HUMAN_M2_GATE=CHANGES_REQUIRED`

ADR: `ADR_004=PROPOSED_NOT_ACCEPTED`

Merge: `PR_2_MERGE=BLOCKED`

This report separates executed evidence from compilation and pending CI. A
successful job, a Linux report, a transport model, or a cached report is never
used as Windows or real-WebView evidence.

## Candidates

| Candidate | Spike implementation | Material concern |
|-----------|----------------------|------------------|
| `portable-pty` 0.9.0 + mitigation | `native_pty_system`, bounded reader pump, DSR response | wezterm/wezterm#6783 and vercel/turborepo#11816 document Windows DSR and stdin-lifecycle regressions |
| Direct native | `libc::openpty` on Linux; windows-rs `CreatePseudoConsole` on Windows | More owned unsafe FFI and lifecycle code; requires direct Windows execution evidence |
| Patched forks | Research only | Low adoption and additional supply-chain risk |

The harness keeps the choice behind `PtyBackend`; ADR-004 remains proposed
until the human reviewer accepts one candidate or a hybrid.

## Harness

The throwaway crate is `tools/spike-pty/` (`publish = false`). It exercises
spawn, shell variants, invalid executables, child-observed resize, UTF-8,
Ctrl+C termination, DSR behavior, exact high-volume output, cleanup, TUI and
agent-style output, hidden-console structure, clipboard input, and concurrent
sessions.

Blocking PTY readers are isolated behind a single reader pump. Harness reads
return `WouldBlock` after 25 ms, allowing scenario deadlines to fire without
spawning one abandoned thread per read. The direct Windows implementation:

- passes the address of the `HPCON` value to `UpdateProcThreadAttribute`;
- uses aligned process-attribute storage and checks every fallible Win32 call;
- quotes the Windows command line;
- owns process/thread handles with `OwnedHandle` and closes `HPCON` once;
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

The child emits exactly `262144` `A` bytes followed immediately by
`DONE_MARKER`. No percentage threshold or CR stripping is used.

```text
PAYLOAD_BYTES=262144
EXPECTED_SHA256=97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9
DELIVERED_BYTES=262144
DELIVERED_SHA256=97a2fc5541dcc9c06b99b2a84c34961fa0c3af20dba3968df2f96a56c6bc00c9
EXACT_MATCH=true
```

Both Linux backends produce this result.

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

Linux CI must independently reproduce this under `xvfb-run`; local execution
alone does not satisfy that CI evidence row.

## Windows Status

`cargo check --target x86_64-pc-windows-msvc --all-targets` passes locally.
That is compilation evidence only.

The last published Windows jobs for commit `b577492` were cancelled at the
15-minute job limit while the old blocking harness was running. They did not
produce valid Windows runtime evidence. The repair adds nonblocking readers, a
180-second process-tree timeout, explicit report platform metadata, and strict
per-backend PASS validation. A new `windows-latest` run is still required.

Required Windows evidence:

- report metadata says `platform == "windows"`;
- both `portable-pty-0.9.0` and `direct-windows-ConPTY` execute;
- PowerShell, `cmd.exe`, and `pwsh.exe` pass;
- WSL records `WSL=PASS` or `WSL=NOT_AVAILABLE_IN_CI`;
- child-observed real resize, exact UTF-8, Ctrl+C termination, exact SHA-256
  high volume, cleanup handle count, and concurrent sessions pass;
- hidden-console evidence records the ConPTY creation path and absence of
  `CREATE_NEW_CONSOLE` for the direct backend.

## Current Gate

| Evidence | State |
|----------|-------|
| Linux portable backend | PASS locally; fresh CI pending |
| Linux direct backend | PASS locally; fresh CI pending |
| Bounded slow-consumer backpressure | PASS locally; fresh CI pending |
| Real Tauri/WebKitGTK/xterm.js pipeline | PASS locally; fresh xvfb CI pending |
| Windows portable backend runtime | BLOCKED pending fresh CI |
| Windows direct ConPTY runtime | BLOCKED pending fresh CI |
| ADR-004 human selection | NOT ACCEPTED |
| PR #2 merge | BLOCKED |

The backend recommendation remains undecided pending Windows runtime results.
Patched forks are not recommended based on current supply-chain evidence. A
direct-Windows/portable-Linux hybrid and direct native on both platforms both
remain candidate outcomes for human review.

M2_PTY_SPIKE_GATE=BLOCKED
