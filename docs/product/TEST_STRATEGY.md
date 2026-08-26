# Test Strategy

Status: DRAFT — designed before implementation (constitution rule 10).
Nothing is implemented; this document defines the testing contract that
milestones must satisfy.

Principles:
- Tests are specified with requirements, not retrofitted.
- No test may depend on a developer's real SSH config, real servers, personal
  launchers, credentials, hostnames, usernames, IPs, or machine state. All
  fixtures use fictional data (`example.com`, `alice@example`,
  `Org.Example.Tool.desktop`, `Example Tool.lnk`, ...).
- Platform-specific tests run only on matching CI runners or documented
  manual matrices.
- Every requirement id in PRD.md maps to at least one test below.

---

## 1. Test layers

| Layer | Scope | Tooling direction |
| --- | --- | --- |
| L1 Rust unit | parsers, classifiers, policy, storage, exec expansion | built-in `cargo test` + `proptest` |
| L2 Frontend unit/component | stores, TerminalView behaviors w/ mocked backend, layout transforms | Vitest + Testing Library (proposed) |
| L3 Integration (in-process) | session manager ↔ PTY backend ↔ registry on real OS | cargo test with feature-flagged backends |
| L4 E2E (app-level) | window opens, IPC surface negative tests, workspace journeys | Playwright/Tauri WebDriver (decided in M1) |
| L5 Contract suites | PTY behavior matrix; discovery pipeline contracts | scenario scripts + assertions |
| L6 Manual release matrix | install/run smoke on clean VMs, a11y keyboard journeys | checklists in docs/release/ |

## 2. Rust unit tests (L1)

Modules and required cases:

- **Keyfile/Desktop Entry parser** (M5): valid minimal entry; localized keys;
  unknown keys/groups tolerated; duplicate groups → error→Needs Review;
  invalid UTF-8; oversized file (limit); CRLF handling; BOM.
- **Exec tokenizer/expander**: spec quoting rules — double-quote escaping of
  `` ` ``,`$`,`\`,`"`; reserved-char detection; field-code expansion `%f %F
  %u %U`; `%%` literal; deprecated codes stripped; >1 file-code → reject;
  codes inside quotes → undefined per spec → classify Needs Review; no shell
  semantics ever (assert metacharacters remain literal argv).
  Property-based: round-trip generate argv → encode per spec → parse → equal
  argv (within spec allowances).
- **Classifier** (M7): golden tables per platform fixture sets; every output
  ∈ {EmbeddedTerminal, LocalCommand, RemoteSSH, ExternalGUI, Unknown};
  Unknown is default on any ambiguity (property: mutated/garbage inputs never
  yield executable classifications).
- **Execution policy**: authorization checks bound to launcher_id +
  descriptor_fingerprint + workspace/member scope; unpinned start attempt →
  denied+audited; descriptor-change flow: security-relevant change keeps
  launcher_id stable, marks member Changed/Re-review Required, suspends
  prior authorization, leaves running sessions untouched, denies next start
  until re-review (T-POLICY-001..004); argv construction from descriptors
  only (no string concat of user input into shells).
- **Storage layer** (M8): atomic write success/failure injection; journal
  replay order; corruption fuzzing (bit flips, truncation) → recover-or-
  detect, never silently accept; migration chain fixtures v(n)→v(n+1);
  export linter catches secret-shaped strings (key patterns, `BEGIN ...
  PRIVATE KEY`, token-like literals from fixture corpus).
- **Import validation**: hostile fixtures (path traversal `../`, absolute
  machine paths, embedded scripts as names/descriptors, huge files, deep
  JSON nesting) rejected/staged safely (T-PERSIST-003).
- **Startup policy / external launch**: Ask/Restore/Restore+terminals/+ext
  simulation incl. per-member failure isolation (T-POLICY-006); detached
  external launch reports started/failed and never manages the foreign
  window (T-POLICY-007).
- **Crash/recovery drills**: journal replay, kill -9 mid-write loop,
  renderer crash mid-layout-edit → last consistent state restored
  (T-PERSIST-004; FS-03/FS-07).

## 3. Frontend unit/component tests (L2)

- Workspace store: add/remove/reorder members; mode transforms produce
  expected FlexLayout JSON for Grid/Focus/Tabs/Master+Stack (T-LAYOUT-001).
- Mode transforms assert process-state neutrality: transforming modes
  emits view/layout changes only, never session events (T-LAYOUT-003).
- Maximize/restore + Focus→Restore return the same Terminal instance and
  session id with scrollback intact (T-LAYOUT-004).
- Layout JSON schema guard: unknown node types, oversized payloads, wrong
  version → rejected before touching model (T-LAYOUT-002).
- TerminalView (mocked session bus): renders output chunks; sends resize on
  container resize; copy/paste actions; search open/navigate; exit banner on
  Exited event; reconnect action emits request only for remote-type members
  (T-UI-001..006).
- View/process separation: attach/detach/hide and all mode transforms emit
  view-state changes only; ProcessSessionState transitions are never emitted
  by layout operations (T-STATE-001..004, backed by L3 assertions).
- Review queue component: pin/hide/alias flows emit correct intents; no
  direct execution affordance exists for Unknown items (negative DOM assert)
  (T-POLICY-005).
- i18n-ready labels/theme tokens applied.

## 4. Integration tests (L3)

- Session manager lifecycle on real OS: spawn true shell (`bash -c` /
  `PowerShell -NoProfile -Command` / `cmd /c`), feed input, read echoed
  output, resize (assert `stty size` / `mode con` reflects), exit code
  propagation, kill tree cleanup (no orphan children — process-table assert),
  restart semantics (T-PTY-011, T-STATE-005).
- Process/view separation on real OS: hide/detach/reattach a live terminal
  and drag it between containers — PTY id, process handle, and scrollback
  unchanged; no respawn (T-STATE-001..004).
- Registry ↔ watcher: tmpdir-based XDG dir / temp Programs-like dir;
  add/modify/remove entries → registry diff events; watcher kill → degraded
  mode flag (FS-01) (T-DISC-LNX-010, T-DISC-WIN-008).
- Identity vs fingerprint: modify `Exec` of a registered fixture entry →
  launcher_id unchanged, descriptor_fingerprint changed, member flagged
  Changed, authorization suspended (T-POLICY-002).
- Workspace save/load across simulated restart (new app instance reads same
  state dir) (T-PERSIST-001).

## 5. PTY contract tests (L5)

Derived from M2 spike matrix; automated subset runs in CI per platform,
full matrix documented manually. Stable IDs:

| ID | Contract | Assertion sketch |
| --- | --- | --- |
| T-PTY-001 | Spawn shells | bash/sh, PowerShell, cmd, WSL (win), ssh to loopback fixture server (container), tmux present in PATH fixture |
| T-PTY-002 | Resize | set 80x24→120x40; query inside; equality within 1 cell |
| T-PTY-003 | UTF-8 | emoji/CJK/accented round-trip through echo path |
| T-PTY-004 | Cursor handshake | ConPTY DSR answered; no startup hang (timeout-bounded) |
| T-PTY-005 | Ctrl+C | SIGINT/VK interrupt observed by foreground process |
| T-PTY-006 | High volume — lossless integrity | deterministic pattern generator ≥10 MB/s ×30 s through the FULL path (PTY→Rust reader→Tauri channel→WebView→xterm.js); byte stream verified lossless (pattern/checksum); UI responsive; memory growth bounded; queue depths recorded (NFR-002/PT-4) |
| T-PTY-007 | Backpressure/desync | reader-side water marks engage under overload; no silent byte drop; hard-limit breach → explicit desynchronization/error state surfaced |
| T-PTY-008 | TUI | vim/htop-class app enters/exits alt screen cleanly after resize |
| T-PTY-009 | Full-screen agent CLI | scripted pty-driven pseudo-agent rendering full-screen refresh cycles |
| T-PTY-010 | Process exit | exit code surfaced; state transition exact (process_state only) |
| T-PTY-011 | Reconnect/restart | restart action respawns fresh; remote member reconnect uses user-config passthrough flag only (no creds); plain-SSH reconnect asserts NEW remote session semantics documented in UI copy |
| T-PTY-012 | Hidden console (Win) | no new top-level console window (Win32 enum assert) |
| T-PTY-013 | Cleanup | handle/fd counts stable across 100 spawn/close cycles |

Clipboard boundary: paste into PTY goes through app clipboard integration
only; bracketed-paste warnings verified at L2/L4.

## 6. Discovery fixtures (fictional data only)

`tests/fixtures/linux/desktop-entries/` (registered XDG sources): valid app;
Terminal=true dev tool; TryExec missing target; NoDisplay helper; Hidden
tombstone overriding system dup; OnlyShowIn filtered; Flatpak-style export
entry; Snap-style entry; malformed corpus (hostile quoting, giant values,
control chars, symlinked file inside root).

`tests/fixtures/linux/desktop-dir/` (Desktop-folder source, FR-007): valid
`.desktop` launcher with fictional Exec (`Org.Example.DeployHelper.desktop`
pointing at `deploy-helper.sh` under a fictional path); Terminal=true
example; malformed/hostile entry (expected Needs Review); fixture manifest
records that entries here have **no Desktop File ID** and identity =
source-root + relative path. A companion `user-dirs.dirs` fixture exercises
XDG_DESKTOP_DIR resolution incl. a relocated/localized Desktop path
(`$HOME/Bureau`) — asserting the adapter never assumes `~/Desktop`.

`tests/fixtures/linux/custom-root/` (opt-in custom roots, FR-008): nested
valid entries within depth/count limits; over-limit tree (bounded scan
assert); symlink pointing outside root (flagged, not followed); `.desktop`
with hostile quoting.

`tests/fixtures/windows/shortcuts/` (generated by setup script at test time,
never committed binaries if avoidable — generation script committed instead):
valid .lnk to fictional exe; moved-target lnk; dead lnk; lnk→lnk chain; lnk
to folder; duplicate per-user/common pair (**both retained with provenance**
until the M6 dedup policy is verified; post-spike assertions updated to the
recorded policy only); oversized/icon-weird lnk.

Fixture manifest documents each file's expected classification → drives
golden tests.

Discovery test-ID assignments (definitions = the fixture/case groups above
plus L3 watcher/integration tests):

- T-DISC-LNX-001 registered-source scan & precedence; 002 normalized record
  shape; 003 Hidden tombstone; 004 NoDisplay/OnlyShowIn handling; 005
  Flatpak/Snap origin tagging; 006 classification goldens (shells/
  Terminal=true/GUI); 007 malformed corpus → Needs Review; 008 negative:
  no execution/no network during scan; 009 Exec tokenizer property suite;
  010 watcher add/modify/remove + degraded mode (FS-01); 011 negative: PATH
  not enumerated by default.
- T-DISC-LNX-DES-001 XDG_DESKTOP_DIR resolution from user-dirs.dirs fixture
  (incl. relocated `$HOME/Bureau` case; no hard-coded `~/Desktop`); 002
  Desktop-dir entries discovered with `desktop_dir` provenance; 003 no
  Desktop File ID assumed — identity is source-root + relative path; 004
  hostile desktop-dir entry → Needs Review, start denied before review.
- T-DISC-LNX-CR-001 opt-in only (no scan before user adds root); 002 bounded
  scan (depth/count caps; over-limit tree truncated+flagged); 003 read-only +
  symlink-outside-root flagged; 004 custom-root records provenance-labeled
  (`custom_root`) and gated by review/pin.
- T-DISC-WIN-001 Known-Folder roots resolved at runtime (never hard-coded);
  002 normalized record from .lnk fields; 003 moved/dead target → Needs
  Review per post-spike policy; 004 chain/folder targets → Needs Review;
  005 classification goldens; 006 duplicate per-user/common pair retained
  with provenance until M6 dedup policy verified (post-spike assertions
  updated to recorded policy only); 007 oversized/icon-weird lnk bounds;
  008 watcher events + degraded mode (FS-01); 009 Windows custom roots
  mirror FR-008 contract.

## 7. Security tests

- IPC surface (T-SEC-001): enumerate registered commands; assert capability
  files allow exactly the intended set (snapshot test); frontend negative
  invoke tests fail closed (L4).
- Injection corpus (T-SEC-002), distinguishing platform invocation models
  (never a universal Windows argv parser):
  - Linux Desktop Entry Exec parsing: spec tokenizer/expander; strings with
    shell metacharacters expand to discrete literal arguments (no shell ever,
    unless the executable itself is a shell) — asserted end-to-end via a
    probe program showing the argv received.
  - Windows non-shell target: fictional probe executable; assert ToolOnize adds no
    extra interpreter layer and the stored `.lnk` argument string is passed
    through unmodified (probe observes exactly authorized target + stored
    parameter string).
  - Windows shell/interpreter target (fixture descriptor explicitly targeting
    an interpreter such as `cmd.exe`/PowerShell/`pwsh`): review UI clearly
    identifies interpreter execution; execution uses exactly the authorized
    target + stored parameters; ToolOnize never rewrites or interpolates parameters;
    expected interpreter semantics may execute shell syntax (by design, under
    explicit authorization).
  - Unknown/ambiguous interpreter invocation ⇒ Needs Review; start attempt
    denied before human review.
- Path attacks (T-SEC-003): symlink escape attempts out of watched roots;
  UNC path flags; PATH-order spoof simulation (fixture PATH with shadowing
  fake tool) — discovery marks origin explicitly; execution resolves via
  absolute descriptor where spec provides it.
- Desktop-dir / custom-root hostility (T-SEC-004): planted hostile
  `.desktop` in fixture Desktop dir and custom root → Unknown/Needs Review,
  distinct provenance labels, no ID assumed, bounds respected; start attempt
  before review denied+audited.
- Descriptor-tamper flow (T-SEC-005): swap pinned fixture target post-pin →
  fingerprint change detected, Changed/Re-review Required, authorization
  suspended, running session untouched, next start denied until re-review.
- WebView compromise drill (T-SEC-006) (design review + code assertion): no
  command exists that takes raw arbitrary command lines from the frontend
  (static inventory test over command signatures).
- Log redaction (T-SEC-007): run flows with secret-shaped env vars set to
  dummy values; grep logs/state dirs for them (must be absent).

## 8. Crash-recovery & migration tests

Covered under L1/L3 above plus manual drills: kill -9 during save (loop
harness); power-loss simulation via VM snapshot revert during write window;
renderer crash mid-layout-edit → relaunch restores last consistent state
(FS-03/FS-07).

## 9. Repository safety tests (CI-enforced)

Secret pattern scan (gitleaks or equivalent config) over full history;
forbidden-pattern grep list: private key headers, `AKIA`-style tokens,
`.ssh/` references outside docs prose, real-looking IPv4s beyond doc RFC5737
examples (192.0.2.0/24 etc.), hostname patterns like `*.local`, `devbox`,
personal usernames. Docs-only exceptions require explicit allowlist entries
reviewed by humans.

## 10. Cross-platform CI matrix

| Runner | Jobs |
| --- | --- |
| ubuntu-latest | L1, L2, L3(linux), L5(linux subset), lint, secret scan, docs link check |
| windows-latest | L1 (platform-neutral), L3(windows), L5(windows subset incl. ConPTY contracts), packaging dry-run later |
| optional arm64 linux | smoke parity (post-V1 consideration) |

Manual-only (documented checklist): WSL distro behaviors, tmux attach on
real network, clean-VM installs, a11y screen-reader passes (NVDA + Orca).


## 11. Traceability matrix (Requirement ID → Milestone → Test ID(s))

Every V1 FR, NFR, SEC and ACC requirement has at least one mapping. A
scripted docs consistency check (planned tooling, docs-defined here) will
fail CI if a requirement ID appears in the PRD without a row below or a
test ID without a definition.

| Requirement(s) | Milestone | Test/gate IDs |
| --- | --- | --- |
| FR-001 | M5 | T-DISC-LNX-001..009 |
| FR-002 | M6 | T-DISC-WIN-001..007 |
| FR-003 | M5, M6 | T-DISC-LNX-002, T-DISC-LNX-005; T-DISC-WIN-002 |
| FR-004 | M5–M7 | T-DISC-LNX-008; T-POLICY-001; T-SEC-004 |
| FR-005 | M5, M6 | T-DISC-LNX-010; T-DISC-WIN-008 |
| FR-006 | M5 (negative) | T-DISC-LNX-011 |
| FR-007 | M5 | T-DISC-LNX-DES-001..004 |
| FR-008 | M5, M6 | T-DISC-LNX-CR-001..004; T-DISC-WIN-009 |
| FR-010, FR-011, FR-012, FR-013, FR-014 | M7 | T-DISC-LNX-006, T-DISC-LNX-007; T-DISC-WIN-005; T-POLICY-005 |
| FR-015 | M7, M8 | T-POLICY-001..004; T-SEC-005 |
| FR-020, FR-021, FR-022, FR-023 | M4, M8 | T-LAYOUT-001..004; T-PERSIST-001 |
| FR-024 | M7 | T-POLICY-006 |
| FR-025 | M8 | T-PERSIST-002, T-PERSIST-003 |
| FR-030 | M2, M3 | T-PTY-001; T-PTY-009 |
| FR-031 | M3, M4 | T-STATE-001..004; T-LAYOUT-003, T-LAYOUT-004; T-PTY-006 |
| FR-032 | M2, M3 | T-PTY-002 |
| FR-033 | M3 | T-UI-003, T-UI-004 |
| FR-034 | M3 | T-UI-005 |
| FR-035 | M3 | T-PTY-010 |
| FR-036 | M3 | T-PTY-011 |
| FR-037 | M3, M8 | T-STATE-005; T-PERSIST-004 |
| FR-038 | M5–M7 | T-POLICY-002, T-POLICY-003; T-SEC-005 |
| FR-040, FR-041 | M7 | T-POLICY-007 |
| FR-050, FR-051, FR-052, FR-053, FR-054 | M8, M9 | T-PERSIST-001..004; T-UI-006 |
| NFR-001 | M2, M3, M9 | PT-1/PT-2/PT-3 harness records; T-PTY-006 latency columns |
| NFR-002 | M2, M3 | T-PTY-006, T-PTY-007 |
| NFR-003 | M3, M9 | T-PTY-006 memory columns; T-PTY-013; PT-5 records |
| NFR-004 | M3–M8 | FS-01..08 scenario tests (T-PERSIST-004 suite + manual drills) |
| NFR-005 | M8 | T-PERSIST-001, T-PERSIST-004; corruption/migration fuzz tests (§2 storage layer) |
| NFR-006 | M1–M10 | parity ledger review gate; per-platform CI matrix (§10); independent WebKitGTK/WebView2 reporting rule in perf records |
| NFR-007 | M9 | offline journey checklist (manual matrix; no-network test run) |
| NFR-008 | M0, M10 | dependency inventory + audit gates; adapter-boundary code-review checklist |
| SEC-001 | M7 | T-POLICY-001; T-SEC-004 |
| SEC-002, SEC-003 | M1, M7 | T-SEC-001; T-SEC-006 |
| SEC-004 | M8 | T-SEC-007; export linter tests (§2 storage) |
| SEC-005 | M5 | quoting property tests (§2 Exec tokenizer); T-SEC-002 |
| SEC-006 | M3 | T-SEC-007 |
| SEC-007 | M7 | external-opener allowlist negative tests (extends T-SEC-001 corpus) |
| SEC-008 | M10 | release-integrity checklist gate (checksums present; attestation evaluation recorded; native signing only per feasibility decision) |
| ACC-001, ACC-002, ACC-003, ACC-004, ACC-005 | M9 | a11y automated suites + keyboard journeys + NVDA/Orca manual pass (§10 manual matrix) |

Test-ID registry: T-DISC-LNX-001..011, T-DISC-LNX-DES-001..004,
T-DISC-LNX-CR-001..004, T-DISC-WIN-001..009, T-POLICY-001..007, T-PTY-001..
013, T-UI-001..006, T-LAYOUT-001..004, T-STATE-001..005, T-PERSIST-001..004,
T-SEC-001..007. Definitions live in the sections above; any new requirement
ID must extend this table in the same change.

Test-ID counting hygiene (M0 requirement for planned tooling): this registry
uses compact range notation (`T-PTY-001..013`), so one written token can
represent several distinct IDs. The future docs consistency checker MUST
either parse/expand range notation correctly or maintain a canonical fully
expanded test-ID registry; it MUST report the canonical unique test count;
requirement coverage MUST be checked against the expanded registry. A naive
grep/token count of explicitly written IDs is NOT authoritative and MUST NOT
be treated as such.
