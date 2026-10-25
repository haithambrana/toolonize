# ADR-007: Windows shortcut discovery reads stored metadata only

Date: 2026-10-25
Status: **Implemented — pending human review and Windows runtime
verification** (M6; per-milestone review pause waived by the owner on
2026-10-25, see STATUS.md)

## Context

PLATFORM_ADAPTERS §3 and THREAT_MODEL T-WIN-01 required an M6 decision
between (a) reading stored `.lnk` metadata without `IShellLink::Resolve`
and (b) calling `Resolve` only when necessary with conservative flags. They
also required decisions on duplicate per-user/all-users shortcuts and on
packaged (UWP/MSIX) app discovery.

`Resolve` can search the file system, consult link tracking, show UI, and
rewrite the link file. Discovery must never do any of that.

## Decision

1. **Stored metadata only (option a).** ToolOnize parses the Shell Link
   binary format itself ([MS-SHLLINK]): header, LinkInfo (local base path +
   common path suffix, or network share), StringData (name, relative path,
   working directory, argument string, icon location) and the
   EnvironmentVariableDataBlock. `IShellLink::Resolve` is never called, so
   the SLR_* flag questions do not arise. The reader is bounded (1 MiB) and
   fuzzed; it runs and is tested on every platform with shortcuts generated
   at test time.
2. **Outcomes.** Local file target → `shell_link` descriptor. Moved/dead
   target → Unavailable (visible, not executable). Folder target, `.lnk`
   chains, shell-namespace-only targets (no file system path) and UNC/network
   targets → Needs Review and blocked; UNC paths are never touched.
3. **Arguments.** The stored argument string is kept verbatim. External
   launches pass it unmodified with `raw_arg` (no interpreter layer added).
   Embedded (PTY) launches must hand argv to the PTY layer, so the string is
   split with the documented `CommandLineToArgvW` rules; programs using the
   standard Windows parser receive identical arguments. Interpreter targets
   (`cmd.exe /c`, PowerShell `-Command/-File/-EncodedCommand`, script files)
   are flagged and always require review.
4. **Duplicates.** Per-user and all-users copies of a shortcut are retained
   as separate records with provenance (distinct `launcher_id`s by origin).
   No precedence rule is applied.
5. **Packaged apps.** Not enumerated in V1. AppsFolder enumeration needs
   shell-namespace APIs and activation semantics that differ from the
   argv/`ShellExecute` model; recommendation: post-V1 spike if users ask.
6. **Roots.** `FOLDERID_Programs`, `FOLDERID_CommonPrograms`,
   `FOLDERID_Desktop`, `FOLDERID_PublicDesktop` via `SHGetKnownFolderPath`
   at runtime; changes via `ReadDirectoryChangesW` (through `notify`).

## Verification status

Pure parsing, classification and policy are covered by tests on all
platforms. Known Folder resolution and the ConPTY session tests run on the
Windows CI runner. A manual read-only dry run on a real Windows machine
(counts only) is still owed (STATUS.md).

## Links

`src-tauri/src/discovery/lnk.rs`, `src-tauri/src/discovery/windows.rs`,
`src-tauri/src/policy.rs` (`split_windows_args`), `src-tauri/src/external.rs`;
PRD FR-002, T-DISC-WIN-001..009, T-SEC-002b/c.
