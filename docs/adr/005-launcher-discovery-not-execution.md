# ADR-005: Launcher discovery is not execution

Date: 2026-08-26
Status: **Accepted**

## Context

The product's differentiator is discovering the developer's existing
launchers (Linux `.desktop` entries; Windows Start Menu/Desktop `.lnk`) and
organizing them into workspaces. Discovery inherently reads attacker-
influenceable metadata (user-writable launcher directories, extracted
archives, synced folders). Naive designs that "find and offer to run" blur
into auto-execution and create a trivial code-execution vector against
developers.

Related spec facts constraining any execution path:
- Linux `Exec` is an argv template with defined quoting/field-code rules —
  not a shell line [Desktop Entry Specification, The Exec key,
  specifications.freedesktop.org, retrieved 2026-08-26].
- A Windows Shell Link (`.lnk`) stores a target path, a command-line
  argument *string*, a working directory, and other metadata — not a
  pre-parsed argv array; command-line processing belongs to the target
  application/runtime [Microsoft Learn, Shell Links; Microsoft Learn,
  CreateProcess].
- Windows `IShellLink::Resolve` can silently re-target via search heuristics
  unless flags suppress it [Microsoft Learn, IShellLink::Resolve].

## Decision

Adopt the pipeline **Discovery → Normalization → Classification →
Review/Pinning → Workspace membership → User-authorized execution**, with
these invariants:

1. Discovery performs **metadata reading only**: no spawning, no script
   resolution beyond existence checks (`TryExec` stat), no network.
2. Classification is conservative and deterministic; anything ambiguous is
   `Unknown/Needs Review` and structurally non-executable.
3. Only explicit human pinning into a workspace authorizes a launcher for
   execution; authorization is bound to launcher_id + descriptor_fingerprint
   + workspace/member scope and enforced in the Rust Execution Policy, never
   in UI state alone. Stable launcher identity (`launcher_id`) is separate
   from target-change detection (`descriptor_fingerprint`); changing primary
   IDs is never the security mechanism.
4. Target changes after approval re-trigger review (descriptor_fingerprint
   change ⇒ member flagged Changed / Re-review Required; authorization
   suspended for next start; membership visible; running sessions untouched).
5. Running sessions are never restarted/re-killed by discovery changes;
   new content applies only on explicit next start.
6. Execution preserves the native invocation semantics of the authorized
   target and never adds an implicit interpreter layer: on Linux,
   spec-conformant Exec argv-template expansion feeds exec-style APIs; on
   Windows, the resolved target is invoked with its stored command-line
   argument string passed through unmodified. If a reviewed descriptor
   explicitly targets an interpreter (`cmd.exe`, PowerShell, `pwsh`, or
   another shell), that target's own semantics apply by design and review
   must clearly surface interpreter execution; ambiguous cases remain
   Needs Review.

## Rationale

- Threat analysis: malicious discovered launchers, post-discovery target
  swap, PATH spoofing and quoting attacks are all first-class threats
  (THREAT_MODEL T-DISC-01..06). Making discovery read-only and execution
  authorization explicit collapses most of that risk surface while keeping
  the product's core value.
- Competitive evidence: no surveyed product articulates this trust posture;
  it is part of our defensible differentiation (COMPETITIVE_ANALYSIS §5).

## Consequences

- First-run UX shows a review step rather than a ready-to-run list — a
  deliberate friction/for-safety trade accepted by product principles
  (PRD §6 principle 2).
- Review queue and provenance labeling become core UX, not polish.

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| Auto-run "safe-looking" launchers | violates least privilege; unclassifiable risk |
| Execute on single click from discovery list | conflates browsing with authority grant |
| OS-launcher delegation only (spawn via xdg-open / ShellExecute on items) | still executes unreviewed targets; kept as *mechanism* for authorized external apps, gated by policy |

## Links

PRD FR-001..014, SEC-001; ARCHITECTURE §5; PLATFORM_ADAPTERS; THREAT_MODEL
§3 discovery section; TEST_STRATEGY §7 negative tests.
