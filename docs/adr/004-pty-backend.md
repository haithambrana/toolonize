# ADR-004: PTY backend — portable-pty 0.9.0 with mitigations

Date: 2026-08-28
Status: **Accepted**

## Context

The terminal session layer needs a cross-platform pseudo-terminal (Linux
`openpty`, Windows ConPTY) that supports shells, WSL, ssh/tmux passthrough,
resize, UTF-8, full-screen TUIs, clean exit detection and cleanup.

`portable-pty` (part of the wezterm project) is the most widely used Rust
option, but version 0.9.0 has documented Windows problems:

1. ConPTY is created with `PSEUDOCONSOLE_INHERIT_CURSOR`, which makes it send
   a Device Status Report request (`ESC[6n`) at startup; an embedder that
   never answers deadlocks (wezterm/wezterm#6783).
2. Dropping the input writer early terminates the ConPTY child on Windows;
   the same two problems were reported and fixed downstream in
   vercel/turborepo#11816.
3. Forks exist specifically to patch ConPTY behaviour, but have little
   adoption.

Because the Windows behaviour depends on flags and on how the consumer uses
the library, the choice was made by experiment rather than on claims.

## Decision

ToolOnize uses **`portable-pty` 0.9.0 on Linux and Windows**, behind the
internal `PtyBackend` trait, with ToolOnize-owned mitigations. The version is
pinned exactly (`=0.9.0`); `Cargo.lock` is authoritative.

### Mandatory mitigations (each covered by regression tests)

1. DSR/CPR handling keeps an incomplete `ESC[6n` across reads and answers only
   after the complete request is seen.
2. The input writer lives as long as the session, so a Windows ConPTY child is
   never terminated by an early drop.
3. Output transport is bounded and lossless: no drop-oldest, drop-newest or
   silent truncation; a stalled consumer causes an explicit desync.
4. Resize is verified by the child, not inferred from the API call.
5. High-volume output keeps exact byte-count and SHA-256 integrity tests.
6. Ctrl+C semantics are tested on Linux and Windows.
7. UTF-8 and VT sequences pass through without lossy conversion.
8. Child exit, cleanup, orphan prevention and resource lifecycle are tested.
9. Concurrent sessions stay isolated.
10. Timeouts and transport desynchronization fail explicitly instead of
    hanging.

### Upgrades

A change from `portable-pty` 0.9.0 must pass the PTY regression tests on Linux
and Windows (DSR/CPR, writer lifetime, resize, shells, UTF-8, Ctrl+C, exact
high-volume integrity, cleanup, concurrency) before it is adopted. Dependency
tooling must not move this pin automatically.

## Evidence

A dedicated test harness compared the candidates on Linux and Windows x86_64,
including a real Tauri/WebKitGTK window (PTY → Rust → Tauri Channel → WebView
→ xterm.js):

- `portable-pty` 0.9.0 with the mitigations passed every required check on
  both platforms: child-observed resize, UTF-8, Ctrl+C, DSR, an exact
  262,144-byte SHA-256 transfer, cleanup, shell variants and five concurrent
  sessions.
- A slow-consumer transport test delivered 2,097,152 bytes with backpressure
  and zero loss under a 64 KiB queue; a drop-on-full transport lost 97% of the
  same stream.
- The same checks now run as part of the session manager tests
  (`src-tauri/src/session/manager_tests.rs`) on Linux and Windows CI.

## Alternatives considered

- **Direct native backends** (`openpty` + `fork`/`exec`; `CreatePseudoConsole`
  + `CreateProcessW`) also passed, but would make ToolOnize own Win32 handle
  lifecycle, process attribute lists, pipe ownership, command-line quoting and
  unsafe FFI invariants across Windows versions.
- **portable-pty on Linux, direct ConPTY on Windows** passed as well; it was
  not chosen because the mitigated portable backend passed with far less
  platform code to maintain.
- **Patched portable-pty forks** were rejected for their supply-chain and
  maintenance risk.

## Consequences

- Terminal sessions depend only on `PtyBackend`; the backend can be replaced
  without redesigning sessions.
- The upstream Windows risks are real and are controlled by the mitigations
  and the upgrade rule above.

## Links

ARCHITECTURE §6; THREAT_MODEL (terminal threats); `src-tauri/src/session/`.
