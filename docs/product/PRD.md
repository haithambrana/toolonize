# Product Requirements Document — ToolOnize

Status: DRAFT — pending human review gate.
Phase: Discovery. Nothing in this document is implemented.
Product name: **ToolOnize** (human-approved; formerly temporary codename "Dev Command Center" during discovery — see §17 Naming and docs/research/NAMING_RESEARCH.md).

Related: ARCHITECTURE.md, THREAT_MODEL.md, IMPLEMENTATION_PLAN.md,
TEST_STRATEGY.md, ROADMAP.md, docs/research/*.

---

## 1. Problem statement

Developers accumulate a personal ecosystem of tools across machines and
projects: terminal-based tools (shells, build watchers, test runners, log
tails, AI CLI agents), remote sessions (SSH into servers, WSL distros, tmux
sessions), and GUI applications (browsers, database clients, containers UIs,
API tools). Every workspace switch means re-assembling this by hand: opening
terminals, re-typing commands, hunting Start Menu/.desktop entries, restoring
pane layouts that no terminal product persists.

Existing products solve fragments: terminals do panes; multiplexers persist
terminal sessions; launchers find apps; none of them *discover the developer's
existing environment and bind it into durable workspaces*. The result is
repeated manual setup, context loss between sessions and machines, and
inconsistent trust practices (users paste launch commands without review).

**ToolOnize** is a local-first desktop application for
Linux and Windows that discovers existing launchers, classifies them
conservatively, organizes them into persistent workspaces with professional
docking layouts, embeds their terminal-based tools, and launches their GUI
tools — with explicit authorization before anything executes.

> Core proposition: **Your existing dev tools. One persistent workspace.**

## 2. Goals and success criteria (V1)

- A developer can, on a fresh machine, go from "installed my tools" to "one
  click restores my project workspace" without writing config files.
- Zero account, zero cloud, zero telemetry.
- Discovery never executes anything automatically.
- Workspaces restore layout + membership + startup policy after restart;
  externally persistent sessions reconnect through the user's own
  environment: tmux/screen provide remote persistence; plain SSH reconnect
  starts a new remote session unless the remote command itself persists
  state.

Success criteria are defined as measurable acceptance criteria per requirement
(§8–§16) and the V1 Definition of Done (§18).

## 3. Non-goals (V1 MUST NOT become)

An IDE; a code editor; a Git client; a browser engine; an SSH client
replacement (we delegate to the user's ssh); a tmux replacement (we
interoperate); an AI assistant; a cloud-sync product; a collaboration
platform; an account service; a password manager; a file manager; a general
app-launcher replacement for the OS (we orchestrate developer workspaces, not
the desktop); embedding or positioning arbitrary foreign application windows
(external GUI members launch externally per FR-040/041).

Post-V1 consideration (explicitly out of V1 scope): a **Custom Command
Editor** / arbitrary ad-hoc command creation. In V1, execution occurs only
through reviewed, authorized descriptors (or trusted core-generated synthetic
system launchers) — never through raw frontend-supplied command text.

## 4. User personas

### P1 — "Multi-project polyglot" Dana (primary)
Senior developer on Linux, 6–10 active projects, uses tmux on three servers,
runs dev servers, test watchers, log tails and CLI coding agents side by side,
plus GUI tools (browser profiles, DB client, container desktop). Pain:
rebuilding the same 8-pane arrangement daily; losing which tool lived where.

### P2 — "Windows enterprise" Alex (primary)
Backend engineer on Windows 11; PowerShell + WSL Ubuntu; SSHes to Linux CI
and prod bastions via ~/.ssh/config; uses Windows Terminal today but rebuilds
tab sets per project; cannot install cloud-synced tools due to company policy;
needs local-only tooling.

### P3 — "Consultant switching contexts" Sam (secondary)
Moves between client environments weekly (some remote). Needs disposable,
import/exportable (non-secret) workspace definitions and strict separation so
client A's launchers never auto-run inside client B's session.

### P4 — "Open-source evaluator" Robin (secondary)
Discovers the repo on GitHub; runs it in a VM first; reads docs to judge
engineering quality. Requirements: honest claims, safe defaults, no telemetry
phones home, clean uninstall story (delete app dir = gone).

## 5. Jobs to be done

- JT-1: When I start or switch to a project, I want my whole tool set
  assembled into one persistent developer workspace, so I don't spend 15
  minutes re-creating it.

Workspace-membership semantics behind JT-1 (consistent across all product
language): Terminal/PTY-backed members run embedded inside ToolOnize
panes. External GUI Application members belong to the workspace orchestration
model but launch as normal external applications (FR-040/041). V1 does not
embed arbitrary foreign GUI windows and does not position or manage them
after launch. "One persistent developer workspace" is a workspace-orchestration
promise, not a claim that every GUI application is embedded inside the ToolOnize
window.
- JT-2: When I install a new tool, I want it to show up as something I can add
  to a workspace, so I don't have to hand-write launcher metadata.
- JT-3: When I close and reopen the app (or reboot), I want my layouts back
  and my tmux/SSH sessions reattachable, so I keep flow across restarts.
- JT-4: When something unknown or suspicious appears in discovery, I want it
  flagged for review rather than executed, so I stay in control of what runs.
- JT-5: When I share a workspace definition with a teammate, I want to export
  non-secret configuration only, so no credentials ever leave my machine.

## 6. Product principles

1. **Local-first.** All state on disk under the user's control; offline is
   the normal case, not degraded mode.
2. **Discovery ≠ execution.** Nothing found is run until the user explicitly
   authorizes it (per-item pin/authorize).
3. **Conservative by default.** Ambiguity resolves to "Needs Review", never to
   "auto-run".
4. **Honest session semantics.** We never claim local processes survive us;
   we make tmux/SSH persistence explicit and visible.
5. **Trust boundary discipline.** The WebView never receives unrestricted
   native execution; policy lives in the Rust layer.
6. **One codebase, two platforms.** Platform differences live behind adapters;
   behavior parity is a stated goal with documented exceptions.

## 7. User journeys (abridged)

### J1 — First run (Dana, Linux)
Launch → discovery scans XDG data dirs incl. Flatpak/Snap exports → Review
screen lists found launchers grouped: Embedded Terminal candidates / Local
Commands / External GUI Apps / **Needs Review** → Dana pins ~12 items into a
new "Project Phoenix" workspace, arranges Grid layout over her terminal panes
(dev server, test watcher, agent CLI), and adds her DB GUI as an external-app
member (it launches as a normal external application when policy allows — it
does not occupy a ToolOnize pane) → saves. Nothing ran during discovery.

### J2 — Daily start (Alex, Windows)
Opens app → workspace "API Platform" restores: PowerShell pane, WSL pane,
SSH pane to staging (reconnects via user's ssh config; if network absent,
pane shows explicit reconnect affordance), Edge pinned to internal dashboard
(launched on startup only because startup policy = "launch external apps") →
Focus mode on the failing service's pane → Ctrl+Shift+F search in scrollback.

### J3 — Change over time (Sam)
Client adds an internal tool → rescan detects new .desktop entry → appears
under New/Changed in registry → Sam reviews, pins to client-A workspace.
A script referenced by an existing launcher changes while its terminal runs:
session keeps running untouched (policy); on next explicit restart the new
script version applies.

### J4 — Failure (Robin)
Kills the app mid-session → relaunch → crash-safe journal restores last saved
workspace state; one pane whose local process died shows "process exited"
with restart action; nothing auto-executes on restore beyond the workspace's
declared startup policy.

## 8. Functional requirements

### 8.1 Launcher discovery

- **FR-001**: The system SHALL discover launchers on Linux from XDG
  `applications` data dirs (`$XDG_DATA_HOME/applications`,
  `$XDG_DATA_DIRS/**/applications`) honoring user-over-system precedence,
  `Hidden` tombstones, `NoDisplay`, `OnlyShowIn`/`NotShowIn`, and include
  Flatpak/Snap export locations when present.
- **FR-002**: The system SHALL discover launchers on Windows from Known
  Folders resolved at runtime (`FOLDERID_Programs`,
  `FOLDERID_CommonPrograms`, `FOLDERID_Desktop`, `FOLDERID_PublicDesktop`),
  enumerating `.lnk` files and resolving them conservatively (no UI, no
  heuristic retargeting during discovery).
- **FR-003**: Discovery SHALL produce normalized Launcher records containing
  at minimum: stable launcher_id (logical identity per DOMAIN_MODEL rules),
  display name, source platform, origin kind/path, raw target descriptor
  (Exec template / link target + command-line argument string + workdir),
  descriptor_fingerprint input fields, icon reference, visibility flags, and
  parse status.
- **FR-004**: Discovery SHALL NOT execute any discovered target, spawn shells,
  resolve scripts, or perform network operations.
- **FR-005**: The system SHALL support change detection (file watching or
  equivalent rescan semantics) so added/removed/modified launchers appear in
  the registry without app restart; watch failures degrade to explicit manual
  rescan with UI indication.
- **FR-006**: PATH-wide executable enumeration SHALL NOT be part of default
  discovery (opt-in later only with evidence).
- **FR-007**: On Linux, the system SHALL additionally discover `.desktop`
  entries placed in the user's Desktop directory, resolved through the XDG
  user-dirs mechanism (`XDG_DESKTOP_DIR` / `user-dirs.dirs`; the path MUST
  NOT be hard-coded). These entries form a distinct origin kind separate
  from registered XDG application entries; they do not necessarily possess a
  Desktop File ID and SHALL receive conservative identity (source-root +
  relative path) and provenance semantics.
- **FR-008**: Users MAY add launcher roots explicitly (opt-in only).
  Discovery over custom roots SHALL be metadata-only, read-only, bounded to
  the declared subtree (no recursive whole-disk search), change-watchable,
  and SHALL label every record's provenance; all custom-root records are
  subject to normal review/authorization before any execution.

### 8.2 Classification and review

- **FR-010**: Each launcher SHALL be classified into exactly one of:
  Embedded Terminal candidate, Local Command, Remote/SSH Command, External
  GUI Application, Unknown/Needs Review.
- **FR-011**: Classification SHALL be conservative: ambiguous, malformed, or
  potentially unsafe entries classify as Unknown/Needs Review and can never
  be executed until a human reviews them.
- **FR-012**: The UI SHALL provide a review surface listing unclassified/
  changed launchers with full parsed metadata before pinning.
- **FR-013**: Only launchers explicitly pinned/authorized into a workspace
  are executable; unpinned discoveries remain inert data.
- **FR-014**: Terminal-capable classification SHALL recognize common shell
  invocation patterns (bash/sh, PowerShell, cmd, WSL distributions, ssh,
  tmux attach patterns) using evidence from Exec/target fields, and label
  uncertain ones for review rather than guessing.
- **FR-015**: Launcher identity and target authorization SHALL be separate
  concepts. Each launcher has a stable `launcher_id` (logical identity,
  stable across ordinary metadata/target edits where platform identity is
  unchanged) and a `descriptor_fingerprint` covering security-relevant
  executable metadata (executable/target, argv/Exec template, working
  directory, relevant launch mode, source identity). Execution authorization
  is bound to launcher_id + descriptor_fingerprint + workspace/member
  authorization scope. When a security-relevant descriptor change occurs:
  membership remains visible and preserved; running sessions remain
  untouched; the member becomes Changed / Re-review Required; prior
  execution authorization is invalid for the next start; no silent execution
  uses the changed descriptor.

### 8.3 Workspaces

- **FR-020**: A workspace SHALL contain: member launcher references, saved
  layout state, per-member view preferences, startup policy, name, icon,
  timestamps.
- **FR-021**: Members MAY be of types: embedded-terminal, local-command,
  remote-command, external-app. Mixed-type membership SHALL be supported.
- **FR-022**: Workspaces SHALL support four layout modes: Grid, Focus, Tabs,
  Master + Stack, implemented as presets/constraints over the docking model.
- **FR-023**: Layout state (tabsets, positions, sizes, maximization, active
  tab) SHALL round-trip save→restore without loss of membership.
- **FR-024**: Startup policy per workspace: Ask / Restore layout only /
  Restore + start terminal members / Restore + also launch external apps.
  Default: Ask.
- **FR-025**: Import/export of a workspace SHALL emit non-secret JSON
  (membership by portable descriptors, layout, policy) and SHALL refuse to
  include machine-specific absolute paths unless the user opts into
  "machine-bound export"; imported workspaces land in Needs Review state.

### 8.4 Terminals and sessions

- **FR-030**: Embedded terminals SHALL support bash/sh (Linux); PowerShell,
  cmd (Windows); WSL distros where installed; user's ssh; tmux attach via the
  user's environment; interactive TUI applications including full-screen CLI
  agents.
- **FR-031**: Terminal lifecycle SHALL be independent of layout via two
  orthogonal state machines: process/session state (New, Starting, Running,
  Exited, Failed, Stopping/Closed; remote adds Disconnected/Reconnecting)
  and view attachment state (Detached/Attached/Hidden). Resize, drag, move
  between tabsets, maximize, restore, tab switch, and mode switching mutate
  ONLY view attachment/layout state and MUST NOT terminate or reset the
  underlying PTY session or lose scrollback.
- **FR-032**: PTY resize SHALL propagate (rows/cols) on container geometry
  changes.
- **FR-033**: Copy/paste and text selection SHALL work; paste of multi-line
  content SHALL warn by default (bracketed-paste aware).
- **FR-034**: In-terminal search over scrollback SHALL be provided.
- **FR-035**: Process exit SHALL be surfaced (exit code/status banner) with
  explicit user-driven restart; automatic restart exists only as opt-in per
  member.
- **FR-036**: Reconnect semantics: remote-command members expose explicit
  Reconnect; underlying transport is the user's ssh/tmux; the app never
  fabricates credentials. Honest continuity semantics: a Rust-owned local
  PTY process can survive a renderer/WebView reload while the app process
  stays alive; if the whole application exits, its local processes (incl.
  local `ssh`) normally exit; reconnecting to plain SSH normally starts a
  NEW remote session — the app MUST NOT imply session continuity unless the
  remote command itself provides it (tmux/screen are what persist).
- **FR-037**: Closing the app terminates local child processes by default; a
  workspace-level option may mark a member as "detach expected" (tmux/screen
  style) documenting that persistence comes from the external multiplexer,
  not from us.
- **FR-038**: Underlying script/content changes after discovery MUST NOT kill
  or restart running sessions; new content applies on next explicit start.

### 8.5 External applications

- **FR-040**: Authorized External GUI Application members SHALL launch via
  the OS-registered mechanism (Exec argv / ShellExecute-style invocation of
  the resolved target), detached from our process lifetime.
- **FR-041**: Launch results (started / failed + reason) SHALL be reported;
  we do not manage the launched app's windows.

### 8.6 Registry, persistence, settings

- **FR-050**: The Launcher Registry SHALL persist discovered+user decisions
  (pinned, hidden, aliases) keyed by stable launcher_id tolerant of path
  changes. Authorization records SHALL bind launcher_id +
  descriptor_fingerprint + workspace/member scope so target changes
  re-trigger review without mutating identity (FR-015).
- **FR-051**: App state (workspaces, layouts, settings) SHALL persist locally
  and restore after unclean exit via atomic writes + recovery journal.
- **FR-052**: State schema SHALL carry a version for migration; migrations
  tested per TEST_STRATEGY.md.
- **FR-053**: Settings SHALL include theme, keymap profile, default shell
  preference per platform, and privacy switches (all off-by-default extras
  stay off).
- **FR-054**: Keyboard shortcuts for core actions (workspace switch, mode
  toggle, search, split actions, review queue) SHALL exist and be remappable.

## 9. Behavioral requirements by platform

### 9.1 Linux behavior
- FR-001 pipeline; Terminal=true entries classified as Embedded Terminal
  candidates with the note that the actual terminal emulator is chosen per
  Desktop Entry spec delegation; TryExec-missing entries marked unavailable
  (visible, not executable); DBusActivatable=true entries require Exec for
  our launch path — else Needs Review.
- FR-007 pipeline: Desktop-dir entries appear under a distinct
  "Desktop folder" provenance label; lacking a guaranteed Desktop File ID,
  their identity is source-root + relative path; default classification
  Unknown/Needs Review. FR-008: custom roots are opt-in, bounded, read-only,
  watchable, provenance-labeled.

### 9.2 Windows behavior
- FR-002 pipeline; duplicate per-user/common shortcut merge semantics are
  Spike/Verification Required (M6): until verified against primary Microsoft
  documentation, both records are retained with provenance and nothing is
  silently discarded;
  `.lnk` chains and folder targets → Needs Review; packaged (UWP/MSIX) apps
  pending spike (see PLATFORM_DISCOVERY_RESEARCH §5); console-window
  suppression for spawned PTY children (no flashing consoles); UTF-8 handling
  per PTY spike outcomes.

## 10. Failure states (must be designed, tested)

| ID | Failure | Required behavior |
| --- | --- | --- |
| FS-01 | Watch service dies | UI badge "live updates off", manual rescan works |
| FS-02 | PTY backend spawn fails | Member shows error + retry; other members unaffected |
| FS-03 | Workspace file corrupt | Load previous journal snapshot; report loss window |
| FS-04 | SSH unreachable on restore | Pane shows reconnect affordance; no silent retry storm |
| FS-05 | External app launch denied/fails | Toast with reason; workspace unaffected |
| FS-06 | Import file malicious/malformed | Reject safely; parse inside validation staging (a review boundary, not an execution sandbox); nothing executes |
| FS-07 | Renderer (WebView) crash | Rust core survives sessions; UI reloads and reattaches |
| FS-08 | Disk full during save | Atomic-write failure surfaces; no partial state |

## 11. Offline behavior

Full functionality offline except: none of V1 features require network.
SSH/tmux usage requires the user's own network; absence is FS-04. No update
checks without consent (updates documented, opt-in check).

## 12. Non-functional requirements (provisional engineering budgets)

These are requirement-level NFRs with stable IDs. The numeric values are
**provisional engineering budgets**: hypotheses to be baselined in M2/M3 and
ratified (or revised with evidence) by M9 — they are not claims. PT-*
targets are measurable subordinate targets under the NFR layer, never a
substitute for it.

- **NFR-001 Performance/latency.** Interactive operations feel responsive.
  - PT-1 Keystroke→render: measurement points are defined as (a) keystroke
    captured at the WebView input handler, (b) PTY echo bytes delivered back
    through IPC, (c) xterm.js write accepted, (d) frame presented.
    Report p50/p95 per platform; provisional budget ≤ 1 frame @ 60 Hz for
    the echo path until M2/M3 baseline data exists ("indistinguishable from
    a reference terminal" is explicitly NOT the acceptance form).
  - PT-2 Discovery scan: fixture-defined run (e.g., ≥ 400 synthetic entries
    across XDG-like roots) measured warm and cold separately.
  - PT-3 Workspace restore (10 members): first paint of panes < 1 s
    provisional; terminals connect async.
- **NFR-002 Throughput/backpressure.** Sustained high-volume terminal output
  is delivered losslessly with bounded resource use.
  - PT-4 High-volume output (e.g., deterministic pattern generator at
    10 MB/s sustained for 30 s): UI remains responsive AND byte integrity is
    verified end-to-end (pattern/checksum match over the full byte stream —
    no silent dropping permitted; VT streams are stateful).
- **NFR-003 Resource limits.** Bounded memory/handles.
  - PT-5 Idle memory: provisional budget < 300 MB RSS, defined against a
    reference workspace shape — 12 members (6 running local terminals with
    default 10k-line scrollback cap, 3 remote members disconnected, 3
    external-app placeholders), one workspace active — measured separately
    per platform (WebKitGTK vs WebView2 reported independently).
  - Scrollback caps configurable with documented defaults; handle/fd counts
    stable across spawn/close cycles.
- **NFR-004 Reliability/failure isolation.** Failure-state table FS-01..08
  behaviors hold; one failing member/source/pump must not degrade others
  (per-session isolation; adapter scan degradation).
- **NFR-005 Crash-safe persistence.** Atomic writes + recovery journal;
  kill -9 mid-write recovers to a consistent state with an explicit loss
  window report (FS-03); schema migrations tested.
- **NFR-006 Cross-platform parity.** Behavior parity per the adapters parity
  ledger with documented exceptions; WebKitGTK and WebView2 results are
  always reported independently, never averaged into one number.
- **NFR-007 Offline/local-first behavior.** Full V1 functionality offline;
  no feature requires network or an account; absence of network affects only
  the user's own remote sessions (FS-04).
- **NFR-008 Maintainability/dependency boundaries.** Platform differences
  confined below the adapter trait line; minimal dependency set with
  inventory + audit gates at M10; no silent dependency additions
  (constitution rule 6).

M2/M3 establish baseline measurements; M9 ratifies budgets with recorded
evidence (docs/research/perf/). No benchmark value may be fabricated before
then.

## 13. Security requirements

See THREAT_MODEL.md for full analysis. Normative subset:

- **SEC-001**: No automatic execution of discovered content; execution
  requires explicit user authorization bound to workspace membership.
- **SEC-002**: All WebView→Rust IPC passes validated, typed command
  arguments; capabilities follow least privilege per window; custom commands
  define explicit permission scopes (Tauri capability model).
- **SEC-003**: Execution policy (what may run, argv construction) lives
  exclusively in the Rust layer; the frontend receives handles/results, not
  raw spawn power.
- **SEC-004**: No plaintext credential storage; no secret material imported
  or exported by the app; SSH auth delegates to the user's agent/config.
- **SEC-005**: Launcher parsing must be robust against hostile inputs
  (unbounded sizes, quoting attacks, unicode tricks); property tests cover
  quoting/escaping per spec.
- **SEC-006**: Logs and crash reports redact paths flagged sensitive by the
  user and never contain env secrets; default logging level records no
  command output.
- **SEC-007**: Opening external URLs uses the OS opener with scheme
  allowlist (http/https); no arbitrary scheme dispatch from web content.
- **SEC-008**: Release integrity (layered, honestly stated): (1) SHA-256
  checksums published for every release artifact; (2) GitHub build
  provenance / artifact attestations (Sigstore-based) evaluated for public
  GitHub releases as a no-secret-in-repo provenance mechanism; (3)
  platform-native code signing — Windows Authenticode/MSIX signing, Linux
  package/artifact signing if adopted — is decided by a future signing
  feasibility gate and MUST NOT be promised before it is provisioned. An
  updater (post-V1) verifies whatever integrity mechanisms exist at that
  time before applying.

## 14. Accessibility requirements

- **ACC-001**: Full keyboard operability of chrome (layout navigation,
  review queue, menus) building on FlexLayout ARIA roles/keyboard support.
- **ACC-002**: Visible focus indicators; respect OS high-contrast themes.
- **ACC-003**: Screen-reader labels for controls; terminal area exposes
  standard xterm.js accessibility considerations (documented limits).
- **ACC-004**: Reduced-motion setting honored in animations.
- **ACC-005**: Minimum contrast for themes shipped in-repo (WCAG AA for UI
  chrome text).

## 15. V1 scope summary

Included: Linux+Windows; launcher auto-discovery per FR-001/2; Desktop-dir
launchers (FR-007) and opt-in custom roots (FR-008); conservative
classification/review; change detection; embedded terminals (bash/sh,
PowerShell, cmd, WSL, ssh, tmux interop); external GUI launching; workspace
save/restore; docking layouts + 4 modes; copy/paste/search; local commands;
reconnect semantics; startup policies; import/export non-secret config;
keyboard shortcuts; themes; accessibility baseline; crash-safe persistence;
tests; CI; packaging; GitHub Releases.

Excluded: everything in §3 Non-goals; macOS; packaged-app discovery (pending
spike outcome); PATH-wide scan (default); sync/collaboration/accounts;
telemetry; plugin system (post-V1 consideration).

## 16. Acceptance criteria framework

Each FR/NFR/SEC/ACC requirement maps to tests defined in TEST_STRATEGY.md;
a milestone is done when its acceptance criteria pass on both platforms in
CI or documented manual matrix. Traceability: the Requirement ID → Milestone
→ Test ID(s) matrix is maintained in TEST_STRATEGY §11 and must leave no V1
requirement unmapped; a scripted docs consistency check (planned tooling)
will detect unmapped requirement IDs before implementation milestones close.

## 17. Naming (criteria only — no rename performed)

Current name is a codename. The future naming-collision gate requires:
(1) trademark search clearance in relevant classes (software), (2) GitHub
org/repo name availability, (3) no conflicting established OSS project of the
same category, (4) domain availability desirable but optional, (5) name does
not imply AI/cloud features contradicting product principles, (6) legal
review for public release. This gate precedes public repo creation per
AGENTS.md rule 9.

## 18. V1 Definition of Done

All of the following, verifiable:
1. All FR/NFR/SEC/ACC requirements implemented with passing tests on Linux
   and Windows CI.
2. PTY spike concluded with ADR-004 moved to Accepted and documented results.
3. Threat model reviewed; all HIGH risks mitigated or explicitly accepted in
   writing.
4. Packaging produces installable artifacts for both platforms via CI;
   smoke-tested manually per release checklist.
5. Public-safety review (PUBLIC_REPOSITORY_SAFETY.md checklist) passes;
   naming-collision gate passed; repository published under final name.
6. Documentation reflects reality (no aspirational claims marked as done).
