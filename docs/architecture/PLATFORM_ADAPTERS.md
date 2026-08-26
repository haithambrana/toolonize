# Platform Adapters

Status: DRAFT — pending human review. Contracts and platform facts; no code.

The Platform Adapter API isolates every OS difference behind three traits so
the core engine stays platform-agnostic (constitution rule 4). Platform
facts cited here are evidenced in PLATFORM_DISCOVERY_RESEARCH.md.

---

## 1. Trait contracts (conceptual Rust)

```rust
/// Enumerate launchers available on this OS. Read-only.
trait LauncherSource {
    /// Full scan; returns raw, uninterpreted entries.
    fn scan(&self) -> Vec<RawLauncher>;
    /// Human-readable origin label for review UI.
    fn origin(&self) -> OriginKind;
}

/// Filesystem change events for launcher locations.
trait FsWatch {
    fn watch(&self, roots: Vec<PathBuf>, sink: Sender<WatchEvent>) -> WatchHandle;
    // Implementations MUST define overflow semantics -> RescanRequired event.
}

/// Resolve default shells / terminal-capable programs for embedding.
trait ShellResolver {
    fn default_shell(&self) -> ShellSpec;
    fn candidates(&self) -> Vec<ShellSpec>; // incl. WSL distros on Windows
}
```

Rules:
- Adapters return *raw* descriptors; normalization/classification happen in
  shared cross-platform code (one parser per format: keyfile, lnk).
- Adapters never execute; never follow symlinks outside declared roots;
  enforce size/count limits.
- All paths entering the registry are recorded with their origin kind.

## 2. Linux Adapter

**LauncherSource** (three source kinds; each record carries its origin kind)
- *Registered application entries* — Roots: `$XDG_DATA_HOME/applications`
  (default `~/.local/share/applications`) then `$XDG_DATA_DIRS/**/applications`
  in precedence order [XDG Base Directory spec]; plus Flatpak export dirs
  (`~/.local/share/flatpak/exports/share/applications`,
  `/var/lib/flatpak/exports/share/applications`) and Snap
  (`/var/lib/snapd/desktop/applications`, pending primary-doc re-verification)
  when present. Precedence: user shadows system by desktop-file id;
  `Hidden=true` user file tombstones system entry; `NoDisplay` filters from
  lists but keeps record; `OnlyShowIn`/`NotShowIn` evaluated against detected
  desktop (unknown desktop → conservative include + flag). Identity:
  desktop-file id + source/precedence identity.
- *User Desktop directory* — separate origin kind `desktop_dir`. Root resolved
  exclusively via the xdg-user-dirs mechanism (`XDG_DESKTOP_DIR` in
  `$XDG_CONFIG_HOME/user-dirs.dirs`; parse the file directly — never invoke
  the eval-using `xdg-user-dir` helper with unvetted input; **never hard-code
  `~/Desktop`**). Entries here do not participate in desktop-file-ID
  precedence/tombstoning and are not guaranteed a Desktop File ID; launcher_id
  = source-root identity + relative path (explicitly documented stable source
  identity); provenance defaults conservative (user-writable location).
- *Opt-in custom roots* — user-declared folders, origin kind `custom_root`.
  Opt-in only; metadata discovery only; read-only; bounded (declared subtree
  only — depth/count/size caps, no whole-disk search); change-watchable via
  FsWatch; every record provenance-labeled and subject to normal review/
  authorization. PATH-wide enumeration remains excluded.
- Parse: robust keyfile reader (localized keys, unknown keys tolerated,
  limits). Exec handled by the shared spec-conformant tokenizer — adapter
  supplies the string only.
- TryExec: existence/executability check only (no execution).
- Terminal=true → EmbeddedTerminalCandidate with delegation note.

**FsWatch**: inotify; one watch per directory under each root (recursive via
walk); events IN_CLOSE_WRITE/IN_MOVED_TO/IN_MOVED_FROM/IN_DELETE/IN_CREATE;
IN_Q_OVERFLOW ⇒ emit RescanRequired (full rescan fallback); debounce 200 ms.
Flatpak/Snap dirs, Desktop dir, and custom roots watched when they exist;
creation of those dirs itself must be noticed (watch nearest existing
ancestor).

**ShellResolver**: bash/sh defaults; honor `$SHELL` if executable.

## 3. Windows Adapter

**LauncherSource**
- Roots via SHGetKnownFolderPath: FOLDERID_Programs (per-user),
  FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_PublicDesktop.
- Duplicate per-user/common shortcut handling: exact merge/shadow semantics
  are **unverified — Spike Required (M6)** against primary Microsoft
  documentation. Until verified, discovery retains both records with
  provenance; no shortcut is silently discarded on an assumed precedence
  rule. Deterministic dedup policy is documented after M6.
- `.lnk` handling: IShellLink+IPersistFile load; GetPath/GetArguments/
  GetWorkingDirectory/GetIconLocation/GetDescription. Discovery policy is
  decided by the M6 spike comparing: (a) reading stored link metadata
  without `Resolve` where sufficient; (b) calling Resolve only when
  necessary, with at minimum SLR_NO_UI | SLR_NOSEARCH | SLR_NOTRACK and an
  explicit documented decision on SLR_NOUPDATE | SLR_NOLINKINFO. Objective:
  metadata inspection without hidden retargeting or mutation; discovery
  must never silently re-target or show UI. Outcomes:
  - resolved to file → descriptor(kind=shell_link);
  - dead/moved-unresolvable → needs_review(reason);
  - target is directory or another .lnk → needs_review(chain) (V1 does not
    auto-expand chains beyond one level; expansion is a review-time detail);
  - UNC/network targets → flagged, never auto-resolved.
- Icon extraction bounded (size cap) and optional (feature-flagged off under
  hardening mode).
- Packaged (UWP/MSIX) apps: out of V1 default scope pending M6 spike
  (AppsFolder enumeration feasibility); spike output recorded as ADR note.
- Custom roots: same opt-in contract as Linux (folders containing `.lnk`),
  origin kind `custom_root`, bounded + read-only + provenance-labeled.

**FsWatch**: ReadDirectoryChangesW with FILE_NOTIFY_CHANGE_FILE_NAME|
DIR_NAME|LAST_WRITE on each root (native recursion); buffer-size overflow ⇒
RescanRequired; debounce similar to Linux.

**ShellResolver**: PowerShell (pwsh if present, else powershell), cmd,
WSL distributions enumerated via registered-distro mechanism (spike-confirmed
approach in M2/M6), ssh passthrough (we invoke the user's ssh client inside
a PTY).

## 4. Spawn-time platform rules (shared with PTY layer)

| Concern | Linux | Windows |
| --- | --- | --- |
| Hidden console | n/a | CREATE_NO_WINDOW-family flag chosen in M2 spike |
| Working dir | Path= from entry, validated exists & allowed | link workdir, validated |
| Env additions | minimal, documented per member | same |
| Invocation semantics | spec expander output used verbatim as discrete argv via exec-style APIs | resolved target invoked with its stored command-line argument string passed through unmodified — no extra interpreter layer introduced by ToolOnize |
| Detached GUI launch | double-fork/setsid pattern per std/process semantics | ShellExecute-style detached start |

Invocation rule (normative): **ToolOnize never adds implicit shell interpretation.
It preserves the native invocation semantics of the authorized target.** A
Windows Shell Link stores a target path, a command-line argument *string*,
a working directory, and other metadata — not a pre-parsed argv array; the
target application processes that string according to its own runtime
semantics. If the reviewed descriptor explicitly targets an interpreter
(`cmd.exe`, PowerShell, `pwsh`, or another shell), shell/interpreter syntax is
intentionally interpreted by that target: this is a higher-power launcher,
must be clearly surfaced as interpreter execution in the review UI, and is
never rewritten or interpolated by ToolOnize. Unknown/ambiguous interpreter
invocations classify Needs Review.

## 5. Parity & exceptions ledger

| Capability | Linux | Windows | Exception policy |
| --- | --- | --- | --- |
| Discovery sources | XDG+Flatpak/Snap + Desktop dir + custom roots | Known Folders+.lnk + custom roots | documented per §2–3 |
| Default shells | bash/sh | PowerShell/cmd/WSL | parity of UX, not of binaries |
| Change watching | inotify | ReadDirectoryChangesW | identical event contract |
| Duplicate shortcut dedup | n/a (ID precedence is spec-defined) | pending M6 spike — both records kept w/ provenance until verified | explicit exception until decided |
| Packaged apps | n/a (Flatpak/Snap covered) | pending spike | explicit exception until decided |
| Console flash | n/a | suppressed via spawn flags | hard requirement |

Any future platform behavior change lands here first, then code — the ledger
is the review artifact for "platform parity" claims.
