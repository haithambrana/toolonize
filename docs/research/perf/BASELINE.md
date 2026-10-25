# Performance baseline (M9) — Linux only, partial

Measured 2026-10-25 in a Linux x86_64 cloud container (Ubuntu 24.04 userland,
Xvfb display, WebKitGTK 4.1, software rendering). This is **not** the
CI-reference hardware profile required by M9 and **no Windows/WebView2
numbers exist yet**. Values are measurements, not claims; NFR budgets are
not ratified by this document.

## PT-4 (core path) — `cargo run --release --example perf`

PTY child (python3 pattern generator) → portable-pty reader → session output
log → pump → in-process consumer that acknowledges every frame. Byte count and
SHA-256 verified end-to-end.

| Bytes | Seconds | MiB/s | Frames | Max frame | Exact SHA-256 |
| --- | --- | --- | --- | --- | --- |
| 67,108,864 | 0.963 | 66.5 | 10,213 | 256 KiB | yes |
| 300,000,000 | 4.022 | 71.1 | 50,132 | 256 KiB | yes |

The provisional PT-4 target (10 MB/s for 30 s) is exceeded ~7x on the core
path. The WebView stage (Tauri Channel → xterm.js write) is not included here;
the M2 spike measured that path for 256 KiB, and the reader's backpressure
under a slow consumer is covered by `high_volume_is_lossless_with_backpressure`
(4 MiB, slow consumer, exact SHA-256, reader paused > 0 times).

## PT-2 discovery scan

Synthetic `.desktop` entries in one XDG data dir; five consecutive scans.

| Entries | First scan | Warm median |
| --- | --- | --- |
| 400 | 6.4 ms | 6.0 ms |
| 2,000 | 37.4 ms | 33.3 ms |

## PT-5 memory (manual)

Release binary, one workspace with two running bash terminals, idle, RSS
summed over the app process and its WebKit helper processes (`ps`; shared
pages are counted more than once, so this overstates real usage):

| Process | RSS |
| --- | --- |
| toolonize (core + GTK/WebKit UI process) | 162 MiB |
| WebKitWebProcess | 187 MiB |
| WebKitNetworkProcess | 48 MiB |
| **Total** | **~397 MiB** (+ shells) |

**Finding:** this already exceeds the provisional PT-5 budget (< 300 MB for a
12-member reference workspace). Proportional set size (PSS) measurement and
the reference workspace shape are needed before deciding whether to revise
the budget or reduce memory (e.g. scrollback/replay defaults, WebKit process
settings). Tracked as an open M9 item in STATUS.md.

## Not measured yet

- PT-1 keystroke → render latency (needs instrumented WebView build).
- PT-3 workspace restore (10 members) first paint.
- Anything on Windows / WebView2.
