# Implementation Plan

Status: DRAFT — pending human review. No code exists yet.
Milestones are dependency-aware vertical slices with explicit gates. No
calendar dates or hour estimates are stated (constitution).

Conventions:
- Every milestone lists: objective, prerequisites, work units (conceptual
  modules), tests, security gate, acceptance criteria, rollback/stop
  condition, and explicit deferrals.
- "Both OSes" means Linux + Windows CI or documented manual matrix until CI
  runners exist (M0 establishes CI).
- Traceability: FR/SEC/ACC ids reference PRD.md.

---

## M0 — Repository & public-safety foundation

**Objective:** make the repository itself a safe, reviewable engineering
artifact before any product code.

Prerequisites: discovery gate approved.

Work units:
- LICENSE selection (MIT proposed; final choice is a human gate),
  CONTRIBUTING.md, SECURITY.md (vuln disclosure policy), CODE_OF_CONDUCT.md.
- `.gitignore` audit (exists; extend as needed), secret-scanning config
  (e.g., gitleaks allowlist) — configuration only, no tool installation in
  this phase.
- Docs tree finalized; ADR index README; fixture-data conventions doc
  (fictional data only).
- CI skeleton definition (workflow YAML may be authored but disabled/dry-run
  until M1 needs it).

Expected files/modules (conceptual): root meta-docs; `docs/`; `.github/`
workflows skeleton; `tools/` placeholder for future dev scripts (no app
code).

Tests: repository lint (markdown link check); secret-scan dry run on the
docs-only tree; record the test-ID counting-hygiene requirement for the
future docs consistency checker (parse/expand compact test-ID ranges or keep
a canonical explicit registry; report canonical unique count; naive grep
counts non-authoritative — TEST_STRATEGY §11).

Security gate: PUBLIC_REPOSITORY_SAFETY.md checklist v1 passes on current
tree.

Acceptance criteria:
1. All governance docs present and internally consistent with AGENTS.md.
2. Secret scan clean; no machine-specific data (verified by grep patterns in
   TEST_STRATEGY §9).
3. ADR index lists 001–005 with correct statuses.
4. Docs-consistency-check requirements include test-ID counting hygiene:
   range notation is expanded correctly OR a canonical explicit test-ID
   registry is maintained; the checker reports the canonical unique test
   count; requirement coverage is checked against the expanded registry;
   naive grep counts are documented as non-authoritative.

Rollback/stop condition: any contradiction between governance docs and
AGENTS.md → stop, report to human authority.

Deferred: everything else.

## M1 — Cross-platform framework shell

**Objective:** prove Tauri 2 + React + TS shell builds and runs on both OSes,
with a minimal hardened IPC command, before deeper investment.

Prerequisites: M0.

Work units:
- Tauri 2 project scaffold (`src-tauri`), Vite-style React+TS frontend
  scaffold; strict TS config; ESLint/Prettier.
- Capability files: deny-by-default posture; one custom command
  `app::ping` returning static data through validated IPC.
- Window creation smoke page rendering system info from Rust via typed IPC.
- CI matrix: linux (WebKitGTK deps), windows (MSVC) build + unit test jobs.

Expected modules: minimal `src-tauri/src/main.rs`, `commands/ping.rs`,
capabilities/*.toml|json; frontend `src/app.tsx`.

Tests: Rust unit test for command validation; frontend render smoke
(Playwright against dev server optional here); CI green both OSes.

Security gate: capabilities file reviewed — only `app::ping` allowed;
documented checklist item added to THREAT_MODEL mitigations (IPC surface).

Acceptance criteria:
1. App window opens on both OSes in CI artifacts (manual smoke recorded).
2. Frontend cannot invoke anything except whitelisted commands (negative
   test attempting unknown invoke fails closed).
3. Build reproducible from clean checkout per documented steps.

Rollback/stop: WebKitGTK/WebView2 blockers unresolvable → stop, document,
escalate (affects ADR-001).

Deferred: theming, layout, terminals, packaging.

## M2 — PTY technical spike & critical integration risk gate (gate milestone)

**Objective:** empirically select the PTY approach and explicitly validate
the end-to-end terminal integration risks before any deep product
development; produce evidence for moving ADR-004 to Accepted. This is a
*spike*: throwaway harness code behind `tools/spike-pty/` (and a minimal
`tools/spike-terminal-ui/` harness for the WebView path), explicitly not
product code paths. Production M3 terminal lifecycle code is locked only
after this gate passes.

Prerequisites: M1.

Critical integration risks that MUST be validated here (architecture-review
requirement):
1. PTY backend behavior (per-platform matrix below);
2. PTY → Rust reader → Tauri transport/channel → WebView → xterm.js
   throughput under sustained high-volume output;
3. terminal byte integrity/backpressure across the complete path (lossless
   verification — no silent dropping);
4. xterm instance lifecycle across attach/detach/reload;
5. FlexLayout live-terminal move/maximize/restore smoke (throwaway harness;
   full proof remains an M4 gate);
6. renderer/WebView reload semantics on both Linux and Windows.

Spike matrix (minimum, per PRD/ADR-004):
Linux PTY; Windows ConPTY; PowerShell; cmd; WSL; resize behavior; UTF-8;
cursor behavior (incl. ConPTY DSR/CPR handshake); Ctrl+C; clipboard boundary
behavior; high-volume output (PT-4 stress); TUIs (vim/htop class);
OpenCode-like full-screen agent CLIs; process exit detection; reconnect/
restart; hidden console-window behavior on Windows; resource cleanup
(orphan/handle leaks).

High-volume measurements (mandatory): throughput; latency (p50/p95);
memory growth over time; queue depth at each stage (reader, channel,
WebView); UI responsiveness; **byte integrity** (deterministic pattern
verified end-to-end). Results determine practical chunk sizes, batch sizes,
and queue/water-mark limits for M3's lossless pump design.

Candidates under test:
(a) portable-pty 0.9.0 + documented mitigations (respond to DSR; guard stdin
drop per turborepo#11816 findings);
(b) patched fork (psmux flags) — assessed for supply-chain acceptability;
(c) thin own abstraction over libc/openpty + windows-rs CreatePseudoConsole.

Method: identical scripted scenarios per candidate; results table + videos/
logs committed under `docs/research/spike-m2/` (no secrets).

Tests: spike harness asserts observable outcomes (exit codes, resize
dimensions reported by `stty size`/`mode con`, UTF-8 round-trip, no zombie
processes after kill).

Security gate: spike runs with least privilege; no network; temp dirs
cleaned; findings reviewed into ADR-004.

Acceptance criteria:
1. Decision table filled for all candidates × matrix rows.
2. Chosen approach meets every MUST row or documents accepted gaps.
3. Full-path measurements recorded (throughput, p50/p95 latency, memory
   growth, queue depth, UI responsiveness, byte integrity) for both
   platforms; lossless byte integrity demonstrated on the complete path.
4. xterm lifecycle + FlexLayout move smoke + reload semantics validated on
   both OSes (throwaway harness), or blocking defects documented with a
   mitigation plan.
5. ADR-004 updated to Accepted with citations; TEST_STRATEGY gains PTY
   contract-test suite derived from the matrix.

Rollback/stop: if all candidates fail ≥2 MUST rows, or the full-path
integration risks (items 2–6 above) cannot be resolved or mitigated →
escalate architecture review before proceeding (terminal-centric product
cannot proceed on broken PTY or corrupting transport).

Deferred: production-grade session manager (M3).

## M3 — Terminal lifecycle core

**Objective:** production Terminal Session Manager: sessions independent of
UI, full terminal UX baseline.

Prerequisites: M2 decision accepted.

Work units:
- `PtyBackend` trait + chosen impl(s) behind feature flags; spawn/input/
  resize/output/close semantics; output pump with **lossless** backpressure:
  bounded batching/coalescing, high/low water marks, backpressure toward the
  PTY reader/child via normal OS flow-control consequences, per-session
  isolation, explicit desynchronization/error state on hard-limit breach —
  no silent byte loss (NFR-002; limits informed by M2 measurements).
- Session registry (Rust side): handles, two orthogonal state machines —
  ProcessSessionState (New/Starting/Running/Exited/Failed/Stopping/Closed;
  remote adds Disconnected/Reconnecting) and a separate view attachment
  state owned by the UI layer — event stream over Tauri Channel.
- Frontend TerminalView: xterm.js instance management, fit addon, search,
  clipboard integration, bracketed-paste warning, exit banner, restart &
  reconnect actions.
- Shell resolution per platform (bash/sh | PowerShell | cmd | WSL distro |
  ssh user-config passthrough | tmux attach string), with honest reconnect
  semantics (plain SSH reconnect = new remote session unless the remote
  command persists state).

Tests: PTY contract suite (from M2 matrix, automated subset); session state
machine unit tests (process-state transitions only — layout/view events must
not mutate them); byte-integrity assertions under sustained output; UI
component tests (mocked backend): copy/paste, search, resize events;
crash-of-renderer drill (FS-07) manual script.

Security gate: input path validates byte streams only; no command
interpolation APIs exposed; console-window suppression verified on Windows;
scrollback cap enforced.

Acceptance criteria:
1. FR-030..038 demonstrable in a bare test workspace UI (no flexlayout yet).
2. Kill renderer → sessions survive; reload reattaches (FR-037 boundary
   documented).
3. ProcessSessionState is provably unaffected by attach/detach/hide
   transitions (T-STATE suite green).
4. Sustained high-volume output shows zero byte loss end-to-end
   (T-PTY integrity tests green on both OSes).
5. Contract suite green on both OSes.

Rollback/stop: state-preservation across view remount unreliable → revisit
xterm embedding strategy before M4.

Deferred: layout modes, persistence.

## M4 — Workspace/layout core

**Objective:** docking workspaces with guaranteed terminal continuity.

Prerequisites: M3.

Work units:
- flexlayout-react integration; factory mapping tab→component kind.
- State-preservation proof (gate; go/no-go vs dockview fallback per ADR-003).
  FlexLayout's upstream React-state claim is NOT accepted as sufficient —
  the gate must demonstrate, on both platforms: xterm `Terminal` instance
  identity stable; PTY session id stable; no `dispose()` on layout movement;
  scrollback intact across ordinary layout changes; resize propagation
  correct through every transform; Focus → Restore returns the exact running
  terminal; mode transformations never respawn processes. If direct mounting
  cannot satisfy these, evaluate a stable host/portal architecture
  (long-lived terminal host per session; layout nodes as attachment targets;
  React portal / persistent host pattern permitted) before falling back to
  dockview. ADR-003 remains conditional until this test passes.
- Workspace model (Rust-owned source of truth mirrored to frontend store):
  members, layout JSON (FlexLayout toJson/fromJson), mode presets Grid /
  Focus / Tabs / Master+Stack.
- Mode switching transforms (layout ↔ constraints) touching only view
  attachment/layout state, never ProcessSessionState.
- Layout persistence hooks (in-memory at this stage; disk in M8).

Tests: layout round-trip property tests; move-live-terminal integration test
(assert PTY id unchanged, Terminal instance identity unchanged, scrollback
byte-exact intact); maximize/restore and tab-switch variants of the same
assertions (T-LAYOUT suite); mode-switch golden states + process-state
unchanged assertions; keyboard navigation smoke (a11y groundwork).

Security gate: layout JSON schema validation on load (size limits, unknown-
node policy = reject) — imported layouts are hostile input (THREAT_MODEL
T-CFG-01).

Acceptance criteria:
1. FR-020..023 demonstrable; FR-031 regression test in CI (move during
   `yes`-class sustained output loses nothing and respawns nothing).
2. Four modes switch correctly with mixed member types (terminals +
   placeholders for GUI apps), with ProcessSessionState provably untouched.
3. Fallback decision (dockview) or portal-host decision formally recorded if
   triggered.

Rollback/stop: neither library preserves state adequately → custom pane
manager escalation (architecture review gate).

Deferred: discovery-backed member pickers.

## M5 — Linux launcher discovery adapter

**Objective:** spec-conformant .desktop pipeline feeding the Launcher
Registry, covering registered XDG sources, the user's Desktop directory, and
opt-in custom roots.

Prerequisites: M4 (registry UI surfaces exist minimally).

Work units:
- XDG dir resolution (base-dir spec defaults), precedence merge, Hidden/
  NoDisplay/OnlyShowIn handling.
- Keyfile parser (robust; unknown-key tolerant; fuzz-tested).
- Exec tokenizer + field-code expander per spec (no shell interpretation);
  TryExec availability check (stat + access, no execution).
- Desktop-dir source (FR-007): resolve the user's Desktop via xdg-user-dirs
  (`XDG_DESKTOP_DIR` from `$XDG_CONFIG_HOME/user-dirs.dirs`; parse the file
  directly — never invoke `xdg-user-dir` with unvetted input; never hard-code
  `~/Desktop`); distinct `desktop_dir` origin kind; identity = source-root +
  relative path (no guaranteed Desktop File ID); conservative provenance.
- Opt-in custom roots (FR-008): user-declared folders, `custom_root` origin
  kind; read-only metadata-only scans bounded to declared subtrees (depth/
  count/size caps; no whole-disk search); watchable; provenance-labeled;
  normal review/authorization applies. PATH-wide enumeration remains excluded.
- Flatpak/Snap export scanning + origin tagging.
- inotify watcher service (per-dir watches, overflow → rescan) with
  debounce; registry diffing incl. descriptor_fingerprint recomputation.

Expected modules: `adapters/linux/{discover,keyfile,exec,watch,userdirs}.rs`;
fixtures under `tests/fixtures/linux/desktop-entries/` plus
`tests/fixtures/linux/desktop-dir/` and `tests/fixtures/linux/custom-root/`
(all fictional).

Tests: fixture-driven parse/classify table tests; quoting property tests
(proptest) vs spec rules; malformed-entry corpus (oversized, bad UTF-8,
hostile quoting) → Needs Review, never panic; watcher integration test
(tmpdir add/modify/remove); desktop-dir fixtures incl. entries without any
registered ID and localized/relocated Desktop paths; custom-root boundary
tests (depth/count limits, traversal/symlink escape attempts → flagged).

Security gate: parser resource limits (max file size, max entries); no exec
paths anywhere in module; symlink policy implemented (do not follow outside
declared roots).

Acceptance criteria:
1. FR-001/003/004/005/007/008 pass on Linux CI with fixtures.
2. Real-system dry-run checklist executed manually once (read-only) and
   documented — counts only, no personal data committed.
3. Classification smoke: known-pattern shells/Terminal=true/GUI apps land in
   expected buckets on fixture set.

Rollback/stop: spec ambiguities causing unsafe parsing → constrain scope to
validated subset, mark rest Needs Review by default.

Deferred: DBusActivatable launching (classify-only in V1 if Exec missing).

## M6 — Windows launcher discovery adapter

**Objective:** Known-Folders + .lnk pipeline with conservative resolution.

Prerequisites: M5 (shared normalization/classification contracts exist).

Work units:
- SHGetKnownFolderPath integration (windows crate) for Programs/Common
  Programs/Desktop/Public Desktop.
- Duplicate-handling spike: verify exact per-user/common merge semantics
  against primary Microsoft documentation; until verified, retain both
  records with provenance — never silently discard a shortcut on an
  unverified precedence assumption. Deterministic dedup policy recorded
  after the spike.
- `.lnk` discovery policy spike: compare (a) reading stored link metadata
  without `Resolve` where sufficient vs (b) calling `IShellLink::Resolve`
  only when necessary with conservative flags — at minimum SLR_NO_UI |
  SLR_NOSEARCH | SLR_NOTRACK, plus an explicit documented decision on
  SLR_NOUPDATE and SLR_NOLINKINFO. Design objective: metadata inspection
  without hidden retargeting or mutation; final policy recorded only after
  this spike. IShellLink/IPersistFile reader captures target/args/workdir/
  icon; chain/folder/dead targets → Needs Review.
- ReadDirectoryChangesW watcher with overflow fallback.
- Custom roots support mirroring FR-008 (`custom_root` origin kind).
- Packaged-apps spike (AppsFolder enumeration feasibility) → written
  recommendation; default remains out-of-scope unless trivially safe.
- Console-window suppression constants shared with M2 backend decisions.

Fixtures: synthetic `.lnk` files generated by test setup script (never from
real machines); fictional targets; duplicate per-user/common fixture pair
retained-with-provenance expectations until dedup policy is decided.

Tests: fixture-driven resolution table (valid, moved target, dead target,
chain, folder); duplicate-pair provenance tests (both records present,
origin-labeled; dedup behavior asserted only against the post-spike policy);
watcher test; negative: oversized lnk, weird encodings.

Security gate: resolution flags enforced in code review checklist;
icon extraction bounded; no auto-follow of UNC/network paths (flag Needs
Review instead).

Acceptance criteria:
1. FR-002/003/005 pass on Windows CI with generated fixtures.
2. Spike recommendation for packaged apps recorded (ADR note or research
   addendum).
3. Manual read-only real-machine dry-run documented (counts only).

Rollback/stop: Resolve heuristics uncontrollable in practice → degrade to
raw-link display + mandatory manual review for all `.lnk` (still safe).

Deferred: App Paths registry sources.

## M7 — Classification + execution policy

**Objective:** turn discovered data into trustworthy action.

Prerequisites: M5+M6 pipelines emit normalized records.

Work units:
- Classifier engine (rules + heuristics, deterministic): Embedded Terminal /
  Local Command / Remote-SSH / External GUI / Unknown. Evidence-based rules
  documented inline (e.g., ssh/tmux/wsl patterns per PRD FR-014).
- Review queue UX (list, diffs on change, pin/hide/alias flows).
- Execution policy module (Rust): argv construction from descriptors,
  authorization check bound to launcher_id + descriptor_fingerprint +
  workspace/member scope (FR-015), launch of external apps detached; audit
  log (local, redacted).
- Descriptor-change flow: fingerprint mismatch on a pinned member →
  Changed/Re-review Required state, authorization suspended, membership and
  running sessions untouched.
- Startup-policy executor (Ask/Restore/Restore+terminals/+external).

Tests: classifier golden tests per platform fixtures; policy negative tests
(unpinned member cannot start — attempt logged); fingerprint-binding tests:
(a) Exec edit with unchanged desktop-file id keeps launcher_id stable AND
invalidates authorization until re-review; (b) start attempt after descriptor
change without re-review is denied+audited; injection-attempt corpus per
TEST_STRATEGY T-SEC-002, distinguishing invocation models: (a) Linux Exec
strings containing shell metacharacters → discrete literal arguments per
spec, never a shell; (b) Windows non-shell target → fictional probe verifies
ToolOnize adds no extra interpreter layer and stored parameters pass through
unmodified; (c) Windows explicit interpreter target → review UI clearly
identifies interpreter execution and execution uses exactly the authorized
target + stored parameters (never rewritten); (d) unknown/ambiguous
interpreter invocation → Needs Review; startup-policy simulation.

Security gate: SEC-001..005 verification pass; capability list re-audited
(new commands added this milestone get scoped permissions).

Acceptance criteria:
1. FR-010..015, FR-040/041, FR-024 pass.
2. Unknown launcher execution attempts structurally impossible (code-path
   proof + test).
3. Changed-descriptor members cannot start without re-review (structural
   test), while their membership and any running session remain untouched.
4. Review queue usable end-to-end on both platforms.

Rollback/stop: any path where frontend can trigger spawn outside policy →
milestone blocked until redesigned (hard stop).

Deferred: heuristic ML classification (never in V1).

## M8 — Persistence & recovery

**Objective:** durable, crash-safe state with honest migration story.

Prerequisites: M4 layout model stable; M7 membership model stable.

Work units:
- Storage layer: atomic writes (temp+rename), recovery journal, schema
  version + migrations; storage location per platform conventions
  (XDG state/config dirs | %APPDATA% equivalent via Known Folder).
- Workspace save/restore incl. layout JSON; settings; registry decisions.
- Import/export (FR-025): non-secret export format with portable
  descriptors; import validation staging (review boundary — not an
  execution sandbox) → Needs Review area.
- Crash-recovery drills wired into test suite where automatable.

Tests: corruption/fuzz tests on state files; migration old→new fixtures;
journal replay tests; FS-01..08 scenario tests; export/import round-trip
property tests incl. hostile inputs.

Security gate: exported files scanned for secret-shaped content before
write (best-effort linter + docs); import never executes; path traversal
checks on import descriptors.

Acceptance criteria:
1. FR-051/052/053 pass; kill -9 mid-write recovers cleanly (automated).
2. Import of malicious fixture executes nothing and reports reasons.
3. Migration from each historical schema version fixture succeeds.

Rollback/stop: journal design unable to meet FS-03 within complexity budget
→ simplify to "last atomic snapshot" guarantee and document loss window.

Deferred: encrypted state at rest (documented decision; threat model
accepts local-attacker-with-user-rights as out of scope V1).

## M9 — UX, accessibility, performance polish

**Objective:** product feel + measurable NFR compliance.

Prerequisites: features complete through M8.

Work units:
- Keyboard map implementation + conflicts doc; command palette (local).
- Themes (light/dark/high-contrast), reduced motion, focus-visible pass.
- Screen-reader labels/ARIA audit; FlexLayout keyboard nav enablement.
- Performance tuning vs NFR-001..008 / PT-1..PT-5 with measurement harness
   results recorded in docs (as measurements, not marketing claims);
   WebKitGTK and WebView2 results reported independently.
- Failure-state UX polish per FS table (banners, badges, empty states).

Tests: axe-style automated a11y checks on chrome; keyboard-only journey
scripts (manual checklist automated where feasible); perf harness outputs
archived under docs/research/perf/.

Security gate: none new; regression pass of prior gates.

Acceptance criteria:
1. ACC-001..005 verifiable; keyboard-only first-run journey completes.
2. NFR budgets ratified or revised with measured evidence on CI-reference
   hardware profile, documented per platform (docs/research/perf/).
3. Zero P1 a11y blockers open.

Rollback/stop: NFR-001 echo-latency budget unreachable due to WebView limits
→ document ceiling, reassess renderer addon choices (canvas/webgl) before
M10.

Deferred: internationalization (structure ready, translations post-V1).

## M10 — Packaging, CI, release readiness

**Objective:** installable, updatable-later, publicly publishable product.

Prerequisites: M9 exit.

Work units:
- Packaging: Linux — deb + AppImage (candidates; final selection documented
  w/ rationale); Windows — NSIS or MSIX (decision documented). Icons,
  metadata, license bundling, third-party license aggregation.
- Release CI: tagged builds → artifacts → draft GitHub Releases; SHA-256
  checksums published; GitHub build provenance / artifact attestations
  (Sigstore-based) evaluated as the no-secret-in-repo provenance mechanism;
  platform-native code signing (Windows Authenticode/MSIX; Linux package/
  artifact signing if adopted) is decided by a separate signing-feasibility
  gate — V1 documentation must not claim signed binaries unless signing is
  actually provisioned. Any signing keys are human-held, never in repo.
- Smoke-test checklist per release (manual matrix documented).
- Public-safety final pass: naming-collision gate, secret sweep, docs truth
  pass ("planned"→"implemented" status flips only where true).

Tests: packaged-artifact install/run smoke on clean VM images (manual
matrix); uninstall cleanliness check (delete = gone, except user-chosen
state dir documented).

Security gate: supply-chain review — dependency inventory + license
compatibility table; cargo/npm audit policies defined (advisory gating is a
human decision).

Acceptance criteria:
1. Fresh-VM install → first-run journey (J1) succeeds on both OSes.
2. Release checklist signed off; artifacts published as drafts for human
   release approval.
3. AGENTS.md rule 9 gates (naming + secret-safety) evidenced as passed.

Rollback/stop: signing/packaging blocker → ship zip artifacts + documented
manual install as interim, keep Releases gated.

Deferred: auto-updater (post-V1, signature-verified design required).
