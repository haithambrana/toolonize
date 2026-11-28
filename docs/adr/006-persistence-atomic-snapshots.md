# ADR-006: Persistence uses atomic checksummed snapshot generations

Date: 2026-10-25
Status: **Accepted**

## Context

ToolOnize needs crash-safe local state with an explicit loss window, schema
versioning with migrations, and no partial state on write failure. The
options were atomic writes plus a recovery journal, or atomic snapshots
alone.

ToolOnize's persisted state is small (workspaces, layout JSON snapshots,
registry decisions; typically tens of KiB) and mutated by discrete user
actions, not by high-frequency streams. Layout saves are debounced in the UI.

## Decision

Every committed mutation writes a **complete snapshot**; there is no
append-only journal.

- Envelope: `{format, schema_version, saved_at, checksum, payload}` where
  `checksum` is SHA-256 over the canonical payload JSON.
- Write: temp file → `fsync` → rotate generations (`state.json` →
  `state.json.1` → `state.json.2`) → atomic rename → directory `fsync`.
- Load: newest generation whose format, checksum and schema verify. If that
  is not the latest file, the UI reports the restored snapshot time (loss
  window) and the bad file is copied aside as `state.json.corrupt-<ms>`.
- A failed write returns an error and the in-memory state is not changed:
  commands are transactional (mutate a copy → save → commit).
- Newer-schema files are refused (no silent downgrade); older schemas run
  through registered migrations.
- Settings use the same writer in the config directory.

## Consequences

- Loss window for committed mutations is zero; a crash mid-write leaves the
  previous generation intact (tested: temp-file crash, crash between rotation
  and commit, bit flips, truncation, write failure).
- No journal replay code to get wrong. The cost is rewriting the full file
  per mutation, which is negligible at this state size.
- Schema version 1 is the first released schema, so there are no historical
  migration fixtures yet; the migration chain and downgrade refusal are
  exercised with synthetic versions in tests.

## Links

`src-tauri/src/persist.rs`, `src-tauri/src/app/mod.rs` (`Core::mutate`);
THREAT_MODEL T-CFG-*.
