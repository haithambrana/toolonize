# Platform Discovery Research

Status: Research (Discovery phase). Nothing implemented.
Retrieval date for all sources: 2026-08-26 unless noted.

Purpose: establish the OS-level facts that launcher discovery must be built
on, with specification-grade citations. This document constrains the Linux
Adapter and Windows Adapter designs (see ARCHITECTURE.md and
PLATFORM_ADAPTERS.md).

Core principle (product-level): **discovery is not execution.** Everything in
this document describes reading metadata only.

---

## 1. Linux: Freedesktop Desktop Entry Specification

Primary source: Desktop Entry Specification, specifications.freedesktop.org
`/desktop-entry/latest/` (sections: Recognized desktop entry keys; The Exec
key; Additional applications actions) [retrieved 2026-08-26].

### 1.1 File locations and precedence

- Application entries live under the `applications/` subdirectory of XDG data
  dirs: `$XDG_DATA_HOME/applications` (default `~/.local/share/applications`)
  and each dir in `$XDG_DATA_DIRS/applications` (default
  `/usr/local/share:/usr/share`) [XDG Base Directory Specification,
  specifications.freedesktop.org/basedir-spec/latest; defaults corroborated by
  distro profile scripts observed in research].
- Precedence: user dirs shadow system dirs by desktop-file id; a user file
  with `Hidden=true` acts as a tombstone meaning "treat the upper-level entry
  as deleted" (spec language: "strictly equivalent to the .desktop file not
  existing at all, as far as that user is concerned").
- Desktop-file ids use `/`→`-` encoding for subdirectories (e.g.
  `org.example.Tool.desktop`, `com.vendor.Suite-tool.desktop`).

### 1.2 Keys we must model

| Key | Spec semantics (summarized) |
| --- | --- |
| `Type` | `Application`, `Link`, or `Directory`; only `Application` is executable-launch relevant |
| `Name` / localized `Name[locale]` | Display name |
| `Exec` | Command line to execute; required unless `DBusActivatable=true` |
| `TryExec` | Path to executable used to detect installation; relative names looked up in `$PATH`; if missing/not executable "the entry may be ignored" |
| `Path` | Working directory to run in |
| `Terminal` | If true, the *user's* terminal emulator program runs the command — delegation is done by the launching environment, not defined by this spec |
| `NoDisplay` | "exists, but don't display it in menus" — still usable for MIME association etc. |
| `Hidden` | Tombstone / deleted marker (see §1.1); different meaning from NoDisplay |
| `OnlyShowIn` / `NotShowIn` | Desktop-environment visibility filters |
| `DBusActivatable` | When true, launch via D-Bus activation and ignore `Exec` per spec guidance |

### 1.3 Exec syntax — what a conformant parser must handle

Verified against "The Exec key" section [latest + 1.0 versions]:

- An executable may be a full path or bare name (PATH lookup by the launching
  environment). The executable token may not contain `=`.
- Arguments are space-separated; an argument containing any reserved character
  must be quoted. Reserved characters: space, tab, newline, `"`, `'`, `\`,
  `>`, `<`, `~`, `|`, `&`, `;`, `$`, `*`, `?`, `#`, `(`, `)`, `` ` ``.
- Quoting is double-quote only; inside quotes, `"`, `` ` ``, `$`, `\` are
  escaped with backslash. Implementations **must undo quoting** before field
  expansion and before exec — i.e., `Exec` is an argv template, NOT a shell
  line. No shell expansion, no pipes/redirection semantics.
- Field codes: `%f`, `%F`, `%u`, `%U` (+ legacy `%i %c %k`). Literal `%` is
  `%%`. Deprecated codes are removed and ignored. At most one of
  `%f/%u/%F/%U` per line; unused ones must be stripped. Expansion happens
  exactly once; expanded values are not re-scanned for field codes. Field
  codes must not appear inside quoted arguments (undefined result).
  `%F`/`%U`/`%i` must appear as standalone arguments.

Design consequence: we need a small, spec-conformant Exec tokenizer +
field-code expander with property-based tests (quoting round-trips), NOT a
shell interpreter. Any input that does not parse cleanly classifies as
Unknown/Needs Review rather than being guessed at.

### 1.4 Flatpak and Snap sources

- Flatpak (official docs, docs.flatpak.org/en/latest/conventions.html):
  exported desktop files land in
  `$HOME/.local/share/flatpak/exports/share/applications` (user installs) or
  `/var/lib/flatpak/exports/share/applications` (system installs), named by
  application ID. Because these live under XDG data dirs when
  `/etc/profile.d/flatpak.sh` has run, they are usually discovered by simply
  honoring `XDG_DATA_DIRS`; we should also scan the export paths explicitly so
  discovery works even when the env var is incomplete (common in non-login
  contexts).
- Snap: entries exposed under `/var/lib/snapd/desktop/applications`
  [corroborated by multiple community sources incl. AskUbuntu #1013391 and
  launcher projects' docs — primary snapd documentation citation pending;
  treat exact path as high-confidence but re-verify during M5 with a fixture
  test].
- Flatpak/Snap entries can be labeled with their source in our Launcher
  Registry (`origin` metadata) for review UX; launching remains delegated to
  the standard mechanism (argv from Exec, or `flatpak run`/`snap run` only if
  the Exec already encodes it — we do not invent alternate invocation).

### 1.5 Parsing safety

- Keyfile-style parsing must tolerate: comments, localized keys, unknown
  groups/keys (spec allows extension keys like `X-*`), duplicate groups, and
  invalid UTF-8 → classify as Needs Review, never crash, never execute.
- Validation precedent exists upstream: `desktop-file-validate` tooling is the
  reference validator used by distros (referenced by launcher projects; see
  e.g. deskentry README usage) — useful as a test oracle for fixtures.
- A `.desktop` file is attacker-influenced input whenever it comes from a
  non-system location; THREAT_MODEL.md covers malicious metadata (long
  strings, control chars, huge files, symlinked targets).

### 1.6 User Desktop directory launchers (xdg-user-dirs) — separate source

Gap identified by architecture review: the sources above cover registered XDG
application directories only. Real users also keep `.desktop` launchers
directly on their *Desktop*.

- The user's Desktop directory MUST be resolved through the freedesktop
  **xdg-user-dirs** mechanism — `XDG_DESKTOP_DIR` from
  `$XDG_CONFIG_HOME/user-dirs.dirs` (written by `xdg-user-dirs-update`;
  format `XDG_DESKTOP_DIR="$HOME/..."`, homedir-relative or absolute)
  [freedesktop.org, xdg-user-dirs project page + user-dirs.dirs(5) format;
  retrieved 2026-08-26]. The path may be localized or relocated (e.g.,
  `$HOME/Bureau`) and MUST NEVER be hard-coded as `~/Desktop`.
- Prefer reading `user-dirs.dirs` directly over shelling out to the
  `xdg-user-dir` helper: the helper passes its argument through `eval`
  without sanity checks [primary project page describes the tool; the eval
  behavior is corroborated by secondary sources citing upstream issue
  tracker — re-verify against primary docs during M5], so our adapter parses
  the config file with the same bounded parser discipline as any other
  config input. If the variable is absent, treat Desktop-dir discovery as
  unavailable for this user (skip + record) rather than assuming a default
  path.
- Entries found there are a **separate origin kind** from registered XDG
  application entries (`desktop_dir`, not `xdg_user`/`xdg_system`). They are
  outside the menu-registration system: they do not participate in
  desktop-file-ID precedence, `Hidden` tombstoning, or `OnlyShowIn`
  filtering.
- **No guaranteed Desktop File ID.** A `.desktop` file outside XDG
  application dirs does not necessarily have a Desktop File ID (IDs are
  defined relative to the `applications/` subdirectories of XDG data dirs).
  Identity for these entries is therefore modeled explicitly as
  *source-root identity + relative path* (see DOMAIN_MODEL.md launcher_id),
  with conservative provenance semantics: user-writable location,
  attacker-influenceable content, default classification Unknown/Needs
  Review until reviewed.

### 1.7 Opt-in user-added launcher roots (V1 capability)

Design requirement added by architecture review, applying to both platforms:

- Users MAY declare additional launcher roots (folders containing `.desktop`
  files on Linux, `.lnk` files on Windows). Opt-in only; nothing is scanned
  automatically.
- Metadata discovery only: read-only, no execution, no script resolution
  beyond existence checks — identical rules to built-in sources.
- Bounded: explicit depth limit, entry-count cap, and per-file size cap;
  **no recursive whole-disk search** — a custom root is exactly the declared
  folder subtree, nothing more.
- Change-watchable via the same FsWatch contract as first-class roots.
- Everything from custom roots carries an explicit `custom_root` origin kind
  and flows through normal normalization → classification → review → pinning
  before anything can execute.
- PATH-wide executable enumeration remains excluded from automatic/default
  discovery (unchanged; FR-006).

## 2. Windows: Known Folders, Shell Links, packaged apps

Primary sources: Microsoft Learn — KNOWNFOLDERID page; SHGetKnownFolderPath
API page; Shell Links overview; IShellLink::Resolve API page; "How to Add
Shortcuts to the Start Menu" how-to [all retrieved 2026-08-26].

### 2.1 Known Folders (never hard-code English paths)

- Discovery MUST resolve folders via `SHGetKnownFolderPath` with
  `KNOWNFOLDERID`s, not hard-coded paths. Relevant IDs verified on the
  KNOWNFOLDERID reference:
  - `FOLDERID_Programs` (PERUSER): default
    `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
    ({A77F5D77-2E2B-44C3-A6A2-ABA601054A51});
  - `FOLDERID_CommonPrograms` (COMMON): default
    `%ALLUSERSPROFILE%\Microsoft\Windows\Start Menu\Programs`
    ({0139D44E-6AFE-49F2-8690-3DAFCAE6FFB8});
  - `FOLDERID_StartMenu` / `FOLDERID_CommonStartMenu`: menu roots;
  - `FOLDERID_Desktop` (per-user) and `FOLDERID_PublicDesktop` (public
    Desktop): per-user vs all-users desktop shortcuts.
- Per-user vs common duplicate handling: **Spike/Verification Required.**
  The earlier draft asserted "same-relative-name per-user shortcuts shadow
  common shortcuts" as a merge rule; primary Microsoft documentation proving
  exact Start Menu duplicate/merge semantics has not been located at
  retrieval. Until M6 verifies this against authoritative sources, discovery
  retains **both records with their provenance** and never silently discards
  a shortcut on an unverified precedence assumption. A deterministic
  deduplication policy is recorded only after that spike (M6).

### 2.2 `.lnk` / Shell Link resolution

- Shortcuts are COM objects: create/read via `IShellLink` +
  `IPersistFile::Load`, then read target path, arguments, working directory,
  icon location, description [Shell Links overview]. CoInitialize required.
- Resolution behavior matters for both correctness and security: if the
  original target moved, `IShellLink::Resolve` uses the Distributed Link
  Tracking service and then search heuristics — including finding "an object
  with the same attributes and file creation time" under a *different name*,
  and recursive local-volume searches [IShellLink::Resolve docs]. Flags
  `SLR_NO_UI`, `SLR_NOTRACK`, `SLR_NOSEARCH`, `SLR_KNOWNFOLDER` suppress parts
  of this; `SLR_NOUPDATE` (do not update link data) and `SLR_NOLINKINFO`
  (disable distributed-link-tracking info) must also be considered and the
  choice documented.
- Design objective refined by architecture review: **metadata inspection
  without hidden retargeting or mutation.** The M6 spike compares two modes:
  (a) reading stored link metadata *without* calling `Resolve` where that is
  sufficient for display/classification; (b) calling `IShellLink::Resolve`
  only when necessary and only with conservative flags — at minimum
  `SLR_NO_UI | SLR_NOSEARCH | SLR_NOTRACK`, plus an explicit documented
  decision on `SLR_NOUPDATE`/`SLR_NOLINKINFO`. What the user sees must be the
  link as stored/resolvable without heuristic guessing; anything ambiguous is
  flagged Needs Review. The final policy is recorded **only after** the
  Windows spike (M6).
- A `.lnk` may point to another `.lnk`, to a folder, or to nothing; arguments
  and working dir are separate fields; icon may be extracted from a different
  file. All of this feeds classification.

### 2.3 Packaged (UWP/MSIX) applications

- Start Menu also surfaces packaged apps which have no classic `.lnk`;
  enumeration normally goes through the shell Apps Folder /
  `PackageManager` APIs. **UNVERIFIED at retrieval**: exact API surface
  (shell:AppsFolder enumeration vs Windows.Management.Deployment) needs a
  dedicated spike item in M6 before we commit to supporting packaged apps in
  V1. Default plan: V1 discovers classic shortcuts first; packaged-app
  support is stretch scope gated by that spike.

### 2.4 Other sources

- `App Paths` registry key (`...\CurrentVersion\App Paths`) exists as a
  launch-resolution mechanism [referenced across Microsoft docs; exact page
  UNVERIFIED at retrieval] — out of default V1 scope; record only.
- PATH-wide executable enumeration: rejected as default V1 discovery
  (signal-to-noise unacceptable — every CLI utility would appear as a
  "launcher"); may exist later as opt-in "advanced scan".

## 3. Change detection / rescan semantics

### 3.1 Linux

- inotify (man7.org, inotify(7)): watches are per-directory; recursive
  watching requires walking and watching each subdirectory; events of interest
  for `applications/` trees: `IN_CLOSE_WRITE`, `IN_MOVED_TO`, `IN_MOVED_FROM`,
  `IN_DELETE`, `IN_CREATE`. Watch queues can overflow (`IN_Q_OVERFLOW`) →
  design must fall back to full rescan on overflow.
- Flatpak/Snap export dirs, the resolved user Desktop dir (xdg-user-dirs),
  and opt-in custom roots get the same treatment when present.

### 3.2 Windows

- `ReadDirectoryChangesW` provides recursive change notification on a
  directory tree [Microsoft Learn]; buffer sizing and overflow handling
  require care; fall back to periodic rescan. (Exact doc URL recorded in
  source index; API choice is standard practice.)
- Rescan results flow through the same normalization/classification pipeline;
  a changed launcher file updates registry metadata but NEVER touches running
  sessions (constitution: no auto-restart on underlying script change).

## 4. Cross-platform pipeline implications (summary)

| Concern | Linux | Windows |
| --- | --- | --- |
| Source of truth | XDG data dirs + Flatpak/Snap exports + user Desktop dir (xdg-user-dirs) + opt-in custom roots | Known Folders (Programs, Desktop, common variants) + opt-in custom roots |
| Unit of discovery | `.desktop` keyfile | `.lnk` shell link |
| Target resolution | Exec argv template + TryExec check | stored link metadata; Resolve only per M6-spike policy (conservative flags) |
| Hidden/deleted semantics | Hidden tombstone / NoDisplay (registered dirs) | duplicate merge semantics pending M6 verification |
| Identity semantics | desktop-file ID where registered; source-root + relative path for Desktop/custom roots | shortcut origin identity per DOMAIN_MODEL launcher_id rules |
| Change detection | inotify per-dir watches | ReadDirectoryChangesW |
| Ambiguity handling | classify Unknown → review | classify Unknown → review |

## 5. Open verification items (tracked into implementation plan)

1. snapd desktop-files primary documentation + fixture path check (M5/M6).
2. UWP/MSIX AppsFolder enumeration API decision spike (M6).
3. Start Menu per-user/common duplicate merge semantics — primary Microsoft
   evidence or spike verification; deterministic dedup policy recorded
   afterwards (M6).
4. `.lnk` discovery policy: stored-metadata-only vs conservative-Resolve
   mode comparison, incl. documented decision on `SLR_NOUPDATE`/
   `SLR_NOLINKINFO` (M6).
5. xdg-user-dirs `user-dirs.dirs` format edge cases (missing variable,
   quoted/absolute paths, localization) fixture check against the primary
   project docs (M5).
6. App Paths registry relevance assessment (deferred).
7. WebKitGTK/WebView2 minimum versions for Tauri 2 on target distros (M1).
