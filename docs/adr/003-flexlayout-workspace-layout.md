# ADR-003: flexlayout-react for workspace docking layout

Date: 2026-08-26
Status: **Accepted with the stable-host architecture — M4 gate passed in
automated tests; pending human review** (per-milestone review pause waived by
the owner on 2026-10-25, see STATUS.md). Fallback dockview not needed.

## Context

The product requires professional docking: drag/dock tabs, tabsets, split
resizing, maximize, borders, popouts; four user-facing modes (Grid, Focus,
Tabs, Master+Stack); JSON round-trip persistence; keyboard/ARIA
accessibility; and — critically — moving a live terminal between tabsets
must not remount/destroy it (PRD FR-031).

## Decision

Adopt **flexlayout-react** (Caplin) as the docking/layout library, with an
explicit go/no-go validation in milestone M4. If the state-preservation gate
fails for reasons we cannot work around, fall back to **dockview-react**
(documented re-ADR).

## Rationale (evidence)

1. Feature fit verified from upstream README [caplin/FlexLayout,
   retrieved 2026-08-26]: splitters, tab drag/ordering/docking, pinnable
   tabs, tabset dragging, maximize, border tabsets, popouts/sub-layouts,
   theming, TypeScript types, ARIA roles + keyboard operation, Playwright
   tests.
2. **"Preservation of component state when tabs are moved"** is an explicit
   README feature claim — directly targeting our hardest requirement.
3. Persistence model matches ours: `Model.fromJson` / `toJson()` JSON tree
   (rows/tabsets/tabs + borders + subLayouts) → our workspace snapshot
   format with schema validation on load.
4. Health: MIT license; npm 0.10.5 updated 2026-08-12; ~98k weekly
   downloads; sole runtime dependency is React [npm page].
5. Mode mapping is natural: Grid=row/tabset tree, Tabs=single tabset,
   Focus=maximized tabset, Master+Stack=border tabset + main area.

## Alternatives considered

| Option | License | Status (2026-08-26) | Assessment |
| --- | --- | --- | --- |
| **dockview(-react)** 8.x | MIT core (`dockview-enterprise` proprietary) | very active; ~215k weekly downloads; React 16.8–19 peers | Strong alternative; groups/grid API; chosen fallback |
| rc-dock | MIT | not re-evaluated in depth | dockview's modern successor lineage; less documentation depth found |
| react-mosaic | MIT-ish | binary-split only | lacks tabsets ⇒ poor Master+Stack fit |
| Custom pane engine | — | — | rejected: months of edge-case work (drag, a11y, popouts) with no V1 differentiation |

## Conditions & risks

- The upstream README claim covers *React component state* preservation when
  tabs move. That alone is NOT sufficient evidence that our xterm
  DOM/session lifecycle is correct. Gate test (M4) must therefore prove,
  on both platforms: xterm `Terminal` instance identity stable; PTY session
  id stable; no `dispose()` on layout movement; scrollback intact across
  ordinary layout changes; resize propagation correct through every
  transform; Focus → Restore returns the exact running terminal; mode
  transformations do not respawn processes — including during sustained
  high-volume output (`yes`-class flood). If direct mounting cannot satisfy
  these, a stable host/portal architecture (long-lived terminal host per
  session; layout nodes as attachment targets; React portal / persistent
  host pattern permitted) must be evaluated before the dockview fallback.
  Failure of both triggers dockview evaluation spike (≤ timeboxed) then
  re-ADR.
- ESM-only packaging — fine under Vite toolchain.
- Layout JSON treated as hostile input on load (THREAT_MODEL T-CFG-01).

## Consequences

We do not build/maintain a recursive pane engine; UI effort concentrates on
workspace semantics instead.

## M4 gate outcome (2026-10-25)

The stable host/portal pattern from the Conditions section was adopted from
the start instead of relying on FlexLayout's direct-mount behavior:

- One `TerminalController` per PTY session owns the xterm.js `Terminal` and
  its host element (`src/terminal/controller.ts`). FlexLayout tab components
  only *attach* that element to their container and *park* it in an
  off-screen container when FlexLayout unmounts them. `dispose()` is called
  only when a session is closed or its member removed.
- Mode presets are pure functions over the layout JSON
  (`src/layout/modes.ts`); they cannot reach sessions.
- The Rust session's process state is a separate state machine; view
  transitions are reported as `attach`/`hide`/`detach` only.

Evidence:

- `src/terminal/controller.test.ts`: Terminal identity stable across
  mount/unmount/remount, no dispose, no session start/stop/restart calls.
- `src/App.test.tsx` ("mode switches never recreate terminals"): rendering
  the real FlexLayout `Layout`, switching Tabs → Master+Stack → Focus → Grid
  keeps the same two Terminal instances, never disposes them, and issues no
  session commands.
- `src-tauri/src/session/manager_tests.rs`
  (`detach_reattach_is_view_only_t_state_002`): detach/hide/reattach keeps
  pid and generation and loses no bytes.
- Manual run (Linux, WebKitGTK under Xvfb): a live shell kept its scrollback
  through Grid → Master+Stack → Tabs and through a renderer reload.

Not yet evidenced: the same manual run on WebView2 (Windows).

## Links

TECHNOLOGY_RESEARCH §3; ARCHITECTURE §7; IMPLEMENTATION_PLAN M4;
TEST_STRATEGY §3/§8.
