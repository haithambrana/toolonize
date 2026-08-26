# Competitive Analysis

Status: Research (Discovery phase). Nothing described here is implemented.
Retrieval date for all sources: 2026-08-26 unless noted.

Working codename: **ToolOnize** (temporary; not the final product name).

Product concept under evaluation:

> A local-first, launcher-aware developer workspace orchestrator that discovers
> the developer's existing tools and launchers, organizes them into persistent
> workspaces, embeds terminal-based tools, and launches external GUI
> applications without forcing the user to rebuild their environment.
>
> Core proposition: **Your existing dev tools. One persistent workspace.**

---

## 1. Method

- Primary sources preferred: official project sites, official documentation,
  upstream GitHub repositories (README, docs, releases), Microsoft Learn.
- Secondary sources (comparison articles) are used only for landscape discovery
  and are marked as secondary. Claims taken from them are flagged.
- No feature was attributed to a competitor without a source. Items that could
  not be verified are marked `UNVERIFIED` and excluded from decisions.

## 2. Category map (2026)

The market splits into layers:

| Layer | Examples | Scope |
| --- | --- | --- |
| Terminal emulator | Ghostty, Alacritty, kitty | Draw a fast, correct terminal |
| Terminal + multiplexer | WezTerm, Zellij, tmux | Emulator + panes/sessions |
| Terminal platform / SSH client | Tabby, Termius | Profiles, connections, sync |
| OS-shipped terminal | Windows Terminal | Default terminal for Windows |
| OS utility suite (adjacent) | Microsoft PowerToys (Command Palette, Workspaces) | Launcher-style app launch; app-set orchestration/positioning |
| AI-native terminal / ADE | Warp, Wave Terminal | Terminal + built-in AI/agent UX |
| Agent orchestrator | Conductor, Superset, Termdock | Supervise multiple coding agents |

Sources: layer taxonomy synthesized from product READMEs cited below and from
secondary comparisons [termdock.com blog, 2026-03-20, retrieved 2026-08-26],
[moltamp.com blog, 2026-05-19, retrieved 2026-08-26], [youngju.dev deep dive,
2026-05-16, retrieved 2026-08-26]. The layer model itself is analysis, not a
claim made by any single vendor.

## 3. Product-by-product analysis

### 3.1 Wave Terminal (wavetermdev/waveterm)

- Category: AI-integrated terminal with workspace/dashboard ambitions.
- Target user: developers who want terminal, file previews, browser, and AI in
  one window.
- Sources: [github.com/wavetermdev/waveterm README], [docs.waveterm.dev],
  [waveterm.dev homepage, published 2026-04-16] — all retrieved 2026-08-26.
- Terminal capability: full terminal blocks; GPU accelerated "on most
  platforms"; works with bash/zsh/fish.
- Workspace persistence: multi-tab layout system ("workspaces"); block-based
  UI; workspaces persist within Wave's own model.
- Remote/SSH: durable SSH sessions designed to survive network interruptions
  and app restarts with automatic reconnection; parses the user's existing
  `~/.ssh/config` and `/etc/ssh/ssh_config`; WSL connections discovered from
  the Windows registry [docs/docs/connections.mdx on GitHub, retrieved
  2026-08-26]. Installs its `wsh` helper onto remotes (opt-out).
- Launcher/tool discovery: **none of the OS launcher surface**. Wave discovers
  *its own* widgets and SSH connections. It does not scan `.desktop` entries or
  Start Menu shortcuts to index the developer's installed GUI tools.
- External GUI orchestration: partial — it can open URLs/browsers inline, but
  does not inventory or launch the OS's registered applications as a feature.
- Layout: flexible block layout, splits, tabs.
- AI emphasis: heavy (Wave AI, BYOK keys, local models).
- Configuration burden: moderate; JSON config files.
- Relevance: closest conceptual neighbor. Confirms demand for
  "workspace + dashboard around terminals", but its center of gravity is
  AI/preview/browser widgets, not discovery of the developer's existing local
  tool ecosystem.

### 3.2 Tabby (Eugeny/tabby)

- Category: cross-platform terminal emulator, SSH client, serial client.
- Target user: users wanting a modern configurable terminal with profiles.
- Sources: [github.com/eugeny/tabby README, ~74k stars, MIT],
  [tabby.sh], [tabby.sh/about/features] — retrieved 2026-08-26.
- Terminal capability: VT220 + extensions, ligatures, Unicode/double-width,
  bracketed paste, split panes, PowerShell/PS Core/WSL/Git-Bash/Cygwin/CMD
  support, Clink-based completion on Windows.
- Workspace persistence: "remembers your tabs" and split panes; a community
  plugin (`tabby-workspace-manager`) adds workspace profiles — i.e.,
  workspaces exist only as third-party plugin profiles of Tabby's own shell
  list.
- Remote/SSH: integrated SSH2 client with connection manager, X11/port
  forwarding, jump hosts, agent forwarding (Pageant + native OpenSSH agent),
  encrypted container for SSH secrets.
- Launcher/tool discovery: none beyond its own profile system.
- External GUI orchestration: none (WinSCP integration is an exception case).
- Layout: nested split panes, tabs on any side, Quake-style dock.
- AI emphasis: none built-in; MCP-server plugin exists (community).
- Configuration burden: GUI settings + JS plugins; moderate.
- Relevance: strong on terminals/profiles; weak where we focus (discovery +
  external app orchestration). Its encrypted secret container is a reminder of
  scope we deliberately exclude (no password manager).

### 3.3 WezTerm

- Category: terminal emulator + multiplexer written in Rust.
- Target user: power users; Lua-configurable environments.
- Sources: [wezterm.org], [wezterm.org/features.html],
  [wezterm.org/multiplexing.html], [wezterm.org/ssh.html] — retrieved
  2026-08-26.
- Terminal capability: GPU rendering, ligatures, images (Kitty/Sixel/iTerm2
  protocols per secondary comparison), runs Linux/macOS/Windows 10/FreeBSD/
  NetBSD.
- Workspace persistence: multiplexing *domains* (local, unix socket, TLS,
  SSH). Local panes/tabs persist while the process lives; true persistence
  requires running a mux server (local domain over unix socket, or remote
  daemon). Session restore across restarts exists when domains are configured.
- Remote/SSH: embedded libssh2-based client; auto-populates SSH domains from
  `~/.ssh/config`; ad-hoc SSH sessions are non-persistent by design;
  mux-over-SSH requires wezterm on the remote host.
- Launcher/tool discovery: none. It spawns commands via config/Lua, not by
  indexing OS launchers.
- External GUI orchestration: none.
- Layout: tabs, splits, panes; no docking/persistence-of-layout-as-product
  beyond mux state.
- AI emphasis: none built-in.
- Configuration burden: high ceiling (Lua), hot reload; demanding for casual
  users (600+ line configs reported in secondary sources).
- Relevance: technically adjacent (Rust, `portable-pty` origin project — see
  TECHNOLOGY_RESEARCH.md). Not a workspace orchestrator.

### 3.4 Windows Terminal (microsoft/terminal)

- Category: OS-shipped terminal for Windows; also ships ConPTY infrastructure.
- Target user: every Windows command-line user.
- Sources: [github.com/microsoft/Terminal README], [learn.microsoft.com
  Windows Terminal docs: install, panes, dynamic-profiles] — retrieved
  2026-08-26.
- Terminal capability: modern VT support, 24-bit color, ConPTY-based hosting,
  Atlas/DirectWrite rendering.
- Workspace persistence: saves window/layout state to a degree; settings.json
  profiles; command-line arguments can restore specific tab/pane layouts
  [learn.microsoft.com/windows/terminal/command-line-arguments].
- Remote/SSH: none built in (relies on ssh.exe inside a tab). tmux control
  mode (`tmux -CC`) integration has been under development upstream (PR
  #18928, opened 2025-05-20, team feedback visible in-thread) — evidence that
  even Microsoft sees pane-level remote-session integration as valuable.
- Launcher/tool discovery: **dynamic profiles** detect WSL distributions and
  installed PowerShell versions automatically [learn.microsoft.com
  .../dynamic-profiles]. This is narrow, purpose-built detection — not general
  launcher discovery, and it stops at shells.
- External GUI orchestration: none.
- Layout: tabs, panes (split/swap/move between tabs, zoom), read-only panes.
- AI emphasis: none shipped in stable at retrieval time.
- Configuration burden: JSON settings + GUI editor; low-to-moderate.
- Relevance: validates ConPTY as the Windows substrate; validates "auto-detect
  what's installed" as a valued behavior (dynamic profiles) while leaving the
  broader discovery/orchestration problem unsolved.

### 3.5 Other relevant products (verified briefly)

- **Zellij** ([github.com/zellij-org/zellij]): self-describes as "a terminal
  workspace with batteries included" — a terminal *multiplexer* with KDL
  layouts, WASM plugins, floating/stacked panes, web client. Terminal-scoped;
  no OS launcher discovery; no external GUI apps. The word "workspace" here
  means pane layouts, validating terminology but not overlapping our scope.
- **Ghostty** (secondary sources only at retrieval): macOS/Linux, no Windows
  yet as of mid-2026 [youngju.dev 2026-05-16; termdock.com 2026-03-20 — both
  secondary]. Minimal by design; no session restore yet per [novvista.com
  hands-on, 2026-05-02, secondary]. Linux is one of our two V1 platforms, so
  Ghostty is *not* "off our platforms" — it is simply not a workspace
  competitor: a terminal emulator with no launcher discovery, no external-app
  orchestration, and no persistent-workspace model. It is retained as a
  terminal-quality benchmark for the Linux side; its absence on Windows means
  no symmetric Windows-side benchmark exists.
- **Warp**: proprietary "Agentic Development Environment" with subscription
  AI; cloud features; Windows support reported in 2026 [termdock.com,
  secondary]. Closed source limits verification; treated as AI-terminal
  competitor pressure rather than a feature reference.
- **tmux**: baseline for persistent remote sessions through the user's SSH
  environment. We explicitly interoperate rather than replace.
- **Conductor / Superset / Termdock / MOLTamp** (discovered during research):
  a new cluster of *agent orchestrators* — supervising multiple AI CLI agents,
  git-worktree isolation, drag-and-drop panes [moltamp.com 2026-05-19,
  superset.sh compare 2026-03-21, termdock.com 2026-03-20 — all secondary].
  They validate the "orchestration layer above terminals" trend but are
  agent-centric, mostly macOS-first, and none indexes the OS launcher surface
  into persistent tool workspaces.

### 3.6 Adjacent precedent: launcher-only tools

- **j4-dmenu-desktop** ([github.com/enkore/j4-dmenu-desktop]): scans
  `$XDG_DATA_HOME`/`$XDG_DATA_DIRS` `.desktop` files to provide a dmenu
  application menu, spec-conformant, with inotify-based live updates in daemon
  mode. Proves the discovery pipeline is tractable — but it is a menu, not a
  workspace, and has no terminal embedding or persistence story.

### 3.7 Adjacent competitor: Microsoft PowerToys (Workspaces; Command Palette)

All claims below are from primary Microsoft sources (Microsoft Learn),
retrieved 2026-08-26.

#### 3.7.1 Workspaces [learn.microsoft.com/en-us/windows/powertoys/workspaces;
ms.date 2025-08-20, page updated 2026-04-22 — PRIMARY]

Official documentation establishes that Workspaces:

- is "a desktop manager utility that helps you launch applications to custom
  positions and configurations with a single click";
- can save/restore *sets of applications* by capturing desktop state via an
  editor ("capture your desktop state as a new workspace");
- launches applications into custom positions/configurations (positioning via
  public APIs + the FancyZones engine; explicit limitation: no window
  snapping);
- launches a whole workspace with one action (Launch button, or a generated
  desktop shortcut pinnable to the taskbar), with a per-app launch-status
  dialog;
- supports per-application CLI arguments to configure app state on launch
  (e.g., VS Code file path; Terminal profile);
- can move/reposition *existing* application windows when "Move existing
  windows" is enabled;
- offers per-app "Launch as Admin" (UAC prompt per app);
- is Windows-only (part of the Windows PowerToys suite).

Assessment: Workspaces is a first-class adjacent competitor for the
*orchestration half* of our proposition — external GUI apps + saved layout +
startup-with-one-action on Windows. It does not discover/classify OS launcher
metadata for review, does not embed terminals or PTY-backed sessions, has no
remote-session members, no provenance/authorization model, and no Linux
support.

#### 3.7.2 Command Palette [learn.microsoft.com/en-us/windows/powertoys/
command-palette/overview; ms.date 2026-04-10, updated 2026-08-25 — PRIMARY]

Official documentation establishes that Command Palette:

- launches installed apps (type-to-launch, Enter);
- runs commands (`>` prefix) including shell-style commands;
- searches files/settings and other sources (files/folders, web, Windows
  Settings, WinGet, clipboard history, window switching);
- supports bookmarks for files/folders/URLs/shell commands, a Dock, and a
  third-party extension ecosystem.

Assessment: adjacent discovery/launch functionality. It surfaces installed
apps and commands keyboard-first, but it is a launcher, not a workspace: no
persistent developer workspaces binding tools+layout+policy, no embedded
terminal/session architecture, no conservative classification/review flow,
Windows-only.

#### 3.7.3 Combined significance

Microsoft now ships application discovery/launch (Command Palette) *and*
workspace/application orchestration (Workspaces) inside one suite — evidence
that the problem space matters to our exact target user. However they are
separate utilities within PowerToys, both Windows-only, and neither provides
our proposed architecture: embedded PTY-backed terminal members, persistent
developer workspaces mixing terminals + remote sessions + GUI apps,
conservative provenance/classification/authorization over discovered
launchers, or Linux coverage. Any differentiation claim must be stated as a
*combination* claim (see §5), not as "nobody does X".

## 4. Cross-cutting comparison

Legend: ● strong, ○ partial, – absent. Assessments summarize the cited
sources above; they are analytical judgments, not vendor claims.

| Dimension | Wave | Tabby | WezTerm | Win Terminal | Zellij | PT Workspaces/CmdPal | **ToolOnize (concept)** |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Quality terminal emulation | ● | ● | ● | ● | ● | – | ● (xterm.js) |
| Panes/tabs/docking | ○ | ○ | ○ | ○ | ○ | ○ (window positioning only) | ● docking + modes |
| Persistent workspaces | ○ | ○ | ○ (mux) | ○ | ○ layouts | ○ (saved app sets, Windows-only) | ● first-class |
| SSH interop w/ user env | ● | ● | ● | – (via tab) | ○ | – | ● delegate to ssh |
| tmux coexistence | ○ | – | ○ | ○ (in dev) | n/a | – | ● (interoperate) |
| OS launcher discovery (.desktop/.lnk) | – | – | – | ○ (shells only) | – | ○ (installed-app launch; no provenance model) | ● core feature |
| External GUI app launch | ○ (URLs) | – | – | – | – | ● (Workspaces core function) | ● core feature |
| Classification/review of launchers | – | – | – | – | – | – | ● core feature |
| Embedded PTY/session architecture | ● | ● | ● | ● | ● | – | ● core feature |
| AI emphasis | ● | – | – | – | – | – | non-goal V1 |
| Config burden | medium | medium | high | low-medium | medium | low | low-medium goal |
| Platforms | L/M/W | L/M/W | L/M/W/BSD | W | L/M/W | W | L/W V1 |

Legend note: "PT Workspaces/CmdPal" = Microsoft PowerToys Workspaces +
Command Palette assessed together per §3.7; they are separate utilities and
the combined column summarizes suite-level capability.

## 5. Differentiation hypothesis test

Hypothesis under test:

> Existing terminal products are strong at terminals, panes, SSH,
> multiplexing and increasingly AI. The proposed product should differentiate
> around **discovering and orchestrating the developer's existing local/remote
> tool ecosystem into persistent workspaces**, rather than claiming novelty
> from terminal panes alone.

Evidence-based findings:

1. **Terminal/pane/SSH strength is table stakes.** Every serious competitor
   listed above already does terminals well; several do durable SSH (Wave),
   multiplexing (WezTerm, Zellij), or OS integration (Windows Terminal).
   Claiming novelty there would be false.
2. **AI is crowding the terminal layer.** Wave and Warp embed assistants;
   agent orchestrators (Conductor, Superset, Termdock) form a new category.
   Competing on AI would put us against funded proprietary products and
   violate our no-AI/local-first V1 scope.
3. **OS-surface app discovery/launch is no longer unoccupied — Microsoft
   ships it.** PowerToys Command Palette launches installed apps and runs
   commands (Windows); PowerToys Workspaces saves/restores application sets
   with positions, CLI arguments, and one-action launch (Windows). The older
   claim that "no surveyed product treats launcher discovery as first-class"
   is therefore withdrawn. What remains defensible: no surveyed product
   indexes OS launcher surface *with provenance/classification/review*,
   across Linux + Windows, or binds that discovery into workspaces containing
   embedded PTY-backed terminal members alongside external GUI apps.
4. **Workspace persistence exists but splits along two incomplete halves.**
   Terminal-bound persistence: Wave organizes its own blocks; WezTerm needs
   mux servers; Zellij lays out panes. App-set orchestration: PowerToys
   Workspaces binds external GUI apps + window positions + CLI args +
   one-action launch — but only on Windows, only for foreign windows, with
   no embedded terminals, remote sessions, provenance model, or review flow.
   No surveyed product binds together discovered GUI apps + embedded terminal
   commands + remote sessions + saved layout + startup policy into one
   restorable unit on both our platforms.

**Verdict: hypothesis CONFIRMED, restated conservatively.**

Refinement: differentiation is the *combination* of the following; each part
alone has strong precedents and must not be claimed as novel:

1. cross-platform Linux + Windows launcher discovery;
2. embedded PTY-backed terminal members;
3. external GUI application members;
4. persistent developer workspaces;
5. conservative provenance/classification/authorization;
6. interoperability with the user's existing ssh/tmux tooling;
7. no cloud/account requirement.

Where we would be redundant: plain panes/tabs (WezTerm/Zellij), SSH client
features (Tabby/Wave), AI assistance (Wave/Warp), pure app-set launch +
positioning on Windows (PowerToys Workspaces), keyboard-first app/command
launch on Windows (PowerToys Command Palette). These are explicitly non-goals
or commodity integrations for V1.

Defensible gap (stated modestly): the **combination** above — launcher-aware
workspace orchestration spanning Linux + Windows with embedded sessions and a
trust posture — is not offered as a single product by anything surveyed as of
2026-08-26. Risk notes: (a) PowerToys is free, Microsoft-distributed, and
could extend toward terminals/discovery; (b) Wave could add `.desktop`/Start
Menu discovery relatively easily given its widget model. Our durability
depends on execution quality of the classification/authorization model and
honest cross-platform session semantics — architectural commitments, not a
market moat. No stronger moat is claimed than this evidence supports.

## 6. Open questions for validation (post-discovery)

- Confirm Wave/Tabby roadmaps have not silently added launcher discovery
  (re-check before implementation milestones).
- Track PowerToys Workspaces/Command Palette evolution (both primary-documented
  and actively updated; a terminal/discovery expansion there would erode the
  Windows-side gap) — re-check before implementation milestones.
- Verify Windows Terminal tmux control mode ship status (PR #18928 was still
  in review at retrieval) — affects our "tmux interop" messaging only.
- UWP/MSIX packaged-app enumeration on Windows (AppsFolder) needs a technical
  spike; see PLATFORM_DISCOVERY_RESEARCH.md §5.
