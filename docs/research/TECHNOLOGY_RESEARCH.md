# Technology Research

Status: Research (Discovery phase). No dependency has been adopted; nothing is
implemented. Version numbers are observations at retrieval time and must be
re-verified when implementation begins.

Retrieval date for all sources: 2026-08-26 unless noted.

Scope: the four technology decisions that shape architecture —
1. Desktop shell: Tauri 2 + Rust + React + TypeScript (Vite-style frontend).
2. Terminal UI: xterm.js.
3. Layout/docking: flexlayout-react (leading choice).
4. PTY backend: unresolved — spike required (see ADR-004).

---

## 1. Tauri 2 (desktop shell)

### Findings

- **Stable status.** Tauri 2.0 reached stable on 2024-10-08 [official blog,
  v2.tauri.app/blog/tauri-20, retrieved 2026-08-26]. The framework moved most
  OS functionality into official plugins so the core can stay small and
  stable.
- **Security model matches our trust-boundary requirement.** Tauri's security
  documentation explicitly defines a trust boundary between the Rust core
  ("full access to system resources") and WebView frontend code ("only access
  to exposed resources via the well-defined IPC layer") [v2.tauri.app/security,
  retrieved 2026-08-26].
- **Capabilities / permissions / scopes.** The v2 access-control system
  replaces the v1 allowlist with: permissions ("on-off toggles for commands"),
  scopes ("parameter validation"), capabilities ("attaching permissions to
  windows/webviews") [v2.tauri.app/blog/tauri-20; v2.tauri.app/security/
  permissions; v2.tauri.app/security/capabilities, retrieved 2026-08-26].
  Capabilities support per-platform targeting (`linux`, `windows`, ...), which
  maps directly onto our adapter strategy.
- **Documented limits of the capability system.** Official docs state it does
  NOT protect against malicious/insecure Rust code, overly lax scopes, missing
  scope checks in command implementations, or compromised WebViews/0-days
  [v2.tauri.app/security/capabilities, "What does it not protect against",
  retrieved 2026-08-26]. Our threat model must own these layers ourselves —
  especially scope checks in *our* commands.
- **IPC.** The v2 IPC rewrite supports raw payloads and a `Channel` type for
  streaming data frontend↔backend [official blog, retrieved 2026-08-26] —
  relevant for high-volume terminal output streams.
- **System WebView approach.** Tauri relies on the OS WebView rather than
  bundling Chromium [v2.tauri.app/security, "Tauri's approach is to rely on
  the operating system WebView", retrieved 2026-08-26]: WebView2 on Windows,
  WebKitGTK on Linux (prerequisite details on the official prerequisites page;
  exact minimum versions UNVERIFIED at retrieval — verify during M1).

### Implications for us

- The WebView is treated as untrusted by default; all privileged operations
  (PTY spawn, launcher execution, discovery) live in Rust commands behind
  narrowly-scoped capabilities. This is exactly the architecture mandated by
  our constitution.
- We must define custom permissions/scopes for every app command we register;
  default "all commands allowed for all windows" must be overridden
  [capabilities doc notes this default].
- Risk accepted: Linux rendering depends on WebKitGTK across distros — needs
  an early smoke test in M1 (window creation + xterm.js render) before deeper
  investment.

## 2. xterm.js (terminal UI)

### Findings

- **Package & version.** Published as `@xterm/xterm` under the xtermjs GitHub
  organization; npm showed **6.0.0** published ~7 months before retrieval
  (≈ early 2026) [@xterm/xterm on npm; github.com/xtermjs/xterm.js, retrieved
  2026-08-26].
- **Addons (team-maintained, verified names)** [@xterm/xterm README/npm]:
  - `@xterm/addon-fit` — fit terminal to container (drives PTY resize);
  - `@xterm/addon-search` — search functionality;
  - `@xterm/addon-serialize` — serialize buffer to VT sequences or HTML;
  - `@xterm/addon-webgl` — WebGL2 renderer (with documented context-loss
    handling); `@xterm/addon-canvas` alternative renderer;
  - `@xterm/addon-clipboard` — clipboard access integration;
  - `@xterm/addon-image`, `@xterm/addon-unicode11` (Unicode 11 widths),
    `@xterm/addon-web-links`, `@xterm/addon-ligatures`,
    `@xterm/addon-attach` (websocket attach — we will not use it; we attach
    over Tauri IPC).
- **State preservation across re-parenting.** Upstream documents
  `xterm-headless` + serialize addon as the pattern for keeping terminal state
  where the process runs and restoring state upon reconnection [npm README,
  retrieved 2026-08-26]. For layout moves (drag between tabsets) we must keep
  the DOM element mounted (move the node) rather than unmount/remount; this is
  consistent with FlexLayout's state-preservation claim (§3). Exact behavior
  must be validated in M3/M4 integration tests.
- **Capability coverage vs V1 requirements**: keyboard handling, mouse-aware
  TUIs, Unicode (plus unicode11 widths), selection/copy/paste, search, links —
  covered by core + addons above. xterm.js is a terminal *emulator frontend*
  only: it provides no PTY, no session persistence, no process management.
  Those are ours.

### Implications for us

- Terminal lifecycle design rule (carried into ARCHITECTURE.md): one long-lived
  `Terminal` instance per session; the layout layer moves its container node;
  never dispose/recreate on layout changes.
- WebGL renderer with canvas fallback (WebKitGTK WebGL maturity varies);
  context-loss handling per upstream guidance.

## 3. flexlayout-react (layout)

### Findings

- **Package, license, activity.** `flexlayout-react` by Caplin Systems Ltd;
  MIT license; current line 0.10.x (npm listed 0.10.5 updated 2026-08-12);
  ~98k weekly downloads; repo caplin/FlexLayout (~1.3k stars), actively
  updated through 2026 [npm registry page; github.com/caplin/FlexLayout,
  retrieved 2026-08-26]. ESM-only package; sole runtime dependency is React.
- **Model concepts.** JSON-serializable layout tree: `Model.fromJson(json)` /
  `model.toJson()`; nodes are `row` → `tabset` → `tab`; up to four border
  tabsets; sub-layouts for popouts/floating panels; tabs host arbitrary
  components via a factory function [README, retrieved 2026-08-26].
- **Feature coverage vs our requirements**: splitters/resize, drag/dock tabs
  and whole tabsets, pinnable tabs, maximize tabset, overflow menus, popout
  windows, theming, TypeScript declarations, ARIA roles + keyboard operation
  (documented accessibility section), Playwright tests in-repo.
- **Critical feature for us:** README explicitly lists *"Preservation of
  component state when tabs are moved"* [github.com/caplin/FlexLayout README,
  retrieved 2026-08-26]. This aligns with our hard requirement that moving a
  terminal between tabsets must not destroy the PTY-backed view. Caveat
  added by architecture review: the upstream claim concerns React component
  state; it is not, by itself, evidence that our xterm DOM/session lifecycle
  (instance identity, scrollback integrity, resize propagation, no
  dispose-on-move) behaves correctly. Must be proven with a spike test in
  M4 against the full assertion list in ARCHITECTURE §6 / ADR-003; a stable
  host/portal pattern is the documented contingency if direct mounting
  fails.
- **Layout modes mapping (design, not implementation):**
  Grid = row/tabset tree; Tabs = single tabset; Focus = maximized tabset;
  Master + Stack = border tabset + main area. All four map naturally onto the
  FlexLayout model.

### Alternatives considered (for ADR-003)

| Option | License (verified) | Status at retrieval | Notes |
| --- | --- | --- | --- |
| **flexlayout-react** 0.10.5 | MIT | updated 2026-08-12 | JSON model, state preservation claim, ARIA/keyboard a11y, React-only dep |
| dockview / dockview-react 8.2.0 | MIT (except `dockview-enterprise`, proprietary) | very active (updated 2026-08-19), ~215k weekly downloads | React/Vue/Angular bindings, React 16.8–19 peer; strong grid/group API |
| react-mosaic | (not re-verified) | — | Binary-split panes only; no tabsets; weaker fit for Master+Stack |

Both leading options are viable; flexlayout is the lead because of the
explicit component-state-preservation guarantee, JSON model round-trip for
persistence, and built-in accessibility work. dockview is the documented
fallback if M4 validation finds blocking defects. Full comparison in ADR-003.

## 4. PTY backend candidates (summary; full analysis in ADR-004)

Verified facts driving the "Proposed / Spike Required" status:

- `portable-pty` 0.9.0 published 2025-02-11 from inside the wezterm monorepo
  (no standalone repo), MIT, ~12.6M total downloads, ~500 dependents
  [crates.io/crates/portable-pty, retrieved 2026-08-26].
- Open upstream issue wezterm/wezterm#6783 (filed 2025-03-11, activity through
  2026-07): portable-pty 0.9.0 breaks simple Windows consumers; root cause
  identified in-thread — ConPTY created with `PSEUDOCONSOLE_INHERIT_CURSOR`
  sends a Device Status Report request (`ESC[6n`) that deadlocks embedders
  which do not answer it; maintainer acknowledges limited Windows testing
  capacity [GitHub issue, retrieved 2026-08-26].
- Independent downstream confirmations: Turborepo PR #11816 (merged
  2026-02-12) documents the same ConPTY hang plus a second regression
  (unconditional stdin drop kills ConPTY children on Windows) and ships tests
  for both [github.com/vercel/turborepo/pull/11816, retrieved 2026-08-26].
- Ecosystem forks exist precisely because of the above:
  `portable-pty-psmux` / psmux/portable-pty-patched adds ConPTY
  `PASSTHROUGH_MODE`, `WIN32_INPUT_MODE`, `RESIZE_QUIRK` flags with Win11
  22H2+ detection [repo README + crates.io, retrieved 2026-08-26];
  `xpty` is another portable-pty fork targeting async + better Windows
  control, minimal adoption [crates.io, retrieved 2026-08-26].

Conclusion: no candidate can be selected on claims alone. Architecture must
own a thin internal `PtyBackend` trait; the M2 spike compares (a) portable-pty
0.9.0 + mitigations, (b) patched forks, (c) direct ConPTY/openpty via
`windows`/`libc` crates against the full checklist in ADR-004.

## 5. Source index

Primary: v2.tauri.app (blog, concept, security, permissions, capabilities);
github.com/xtermjs/xterm.js + npm @xterm/xterm; github.com/caplin/FlexLayout +
npm flexlayout-react; crates.io (portable-pty, portable-pty-psmux, xpty);
github.com/wezterm/wezterm issue #6783; github.com/vercel/turborepo PR #11816;
github.com/psmux/portable-pty-patched; Microsoft Learn (ConPTY pages referenced
via issues; KNOWNFOLDERID/IShellLink cited in PLATFORM_DISCOVERY_RESEARCH.md).

Secondary (landscape only): termdock.com, moltamp.com, superset.sh,
novvista.com, youngju.dev blogs (2026). Used only where flagged.
