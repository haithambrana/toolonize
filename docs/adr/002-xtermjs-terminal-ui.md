# ADR-002: xterm.js as terminal UI

Date: 2026-08-26
Status: **Accepted**

## Context

V1 requires a browser-rendered terminal supporting: normal keyboard
behavior, mouse-aware TUIs, Unicode (incl. wide chars), selection/copy/
paste, search, resize-driven reflow, and full-screen CLI agents — inside a
system WebView, over our own IPC transport (not websockets).

## Decision

Use **xterm.js** (`@xterm/xterm`) plus team-maintained addons:
`addon-fit` (resize), `addon-search`, `addon-serialize` (state export),
`addon-webgl` with canvas fallback (rendering), `addon-clipboard`
integration where needed, `addon-unicode11`.

## Rationale (evidence)

1. Maintained upstream org xtermjs/xterm.js; current stable line 6.0.0 on
   npm at retrieval (early-2026 publish) [@xterm/xterm npm page, retrieved
   2026-08-26]; release cadence active (releases list synchronized-output
   DEC 2026 mode, ligature detail work, etc.).
2. Capability coverage matches every V1 terminal requirement (PRD FR-030..
   034): core emulator + fit/search/serialize/webgl/unicode11 addons are
   team-maintained [README/npm].
3. Ecosystem maturity: xterm.js is the de-facto standard embedded terminal
   in web-technology products (widely embedded across the industry; addon
   API designed for embedding).
4. State-preservation pattern exists upstream (xterm-headless + serialize)
   for reconnect-style state restore [npm README] — complements our
   DOM-re-parenting strategy for layout moves.

## Constraints & mitigations

- xterm.js is an emulator *frontend* only: no PTY/process/persistence — by
  design those live in our Rust Session Manager (ARCHITECTURE §6).
- WebGL context loss in WebViews: use documented `onContextLoss` handling
  and canvas fallback [addon-webgl README].
- Layout-move continuity requires keeping the instance mounted: TerminalView
  moves its container node; FlexLayout's component-state preservation claim
  (ADR-003) is validated in M4 with a regression test (FR-031).

## Alternatives considered

| Option | Why not |
| --- | --- |
| Native terminal widget per platform | violates one-codebase rule; duplicates emulator logic twice |
| Custom canvas terminal renderer | enormous cost, no upside over mature lib |
| HTerm-like minimal custom | same cost argument |

## Consequences

Terminal UX parity with major products achievable; scrollback caps and
serialize usage defined in TEST_STRATEGY §5 contract suite.

## Links

TECHNOLOGY_RESEARCH §2; ARCHITECTURE §6; IMPLEMENTATION_PLAN M3/M4 gates.
