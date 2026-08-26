# ADR-001: Tauri 2 + Rust core + React/TypeScript frontend

Date: 2026-08-26
Status: **Accepted**

## Context

We need one cross-platform codebase (Linux + Windows V1) for a desktop
product whose frontend is a rich docking workspace UI with embedded
terminals, and whose backend must hold all privileged capability (PTY
spawning, launcher execution policy, discovery). The WebView must not
receive unrestricted native execution. Bundling a full Chromium runtime is
undesired (size/supply-chain surface).

## Decision

- Desktop shell: **Tauri 2**.
- Backend/core language: **Rust**.
- Frontend: **React + TypeScript**, Vite-style toolchain.

## Rationale (evidence)

1. **Native/system WebView instead of bundled Chromium**: Tauri relies on the
   OS WebView (WebView2 on Windows, WebKitGTK on Linux) rather than shipping
   a browser [v2.tauri.app/security, retrieved 2026-08-26]. Matches our size
   and supply-chain posture.
2. **Explicit trust boundary**: Tauri's security model formally separates the
   privileged Rust core from untrusted WebView code and routes everything
   through IPC [v2.tauri.app/security].
3. **Capability/permission model**: v2 replaces the allowlist with
   permissions + scopes + capabilities attached to windows/webviews,
   platform-targetable (linux/windows) and deny-by-default configurable
   [v2.tauri.app/blog/tauri-20; /security/capabilities; /security/
   permissions]. This is the enforcement point for SEC-002/003.
4. **IPC fit for terminals**: v2 IPC rewrite adds raw payloads and Channel
   streaming suitable for high-volume PTY output [official blog,
   2024-10-08].
5. **Packaging support** for Linux and Windows distribution channels is a
   documented Tauri capability (bundle targets; verified during M10).
6. Stable since 2024-10-08 — mature enough to bet on.

Known constraints we accept and mitigate: system-WebView variance
(WebKitGTK across distros → M1 smoke gate; min versions documented);
capability system explicitly does not protect against our own Rust bugs or
lax scopes [capabilities doc] → threat model owns those layers.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Electron | bundles Chromium; larger attack/size surface vs our posture |
| Native GTK (prototype path) | disposable PoC already showed cross-platform cost; Windows GTK story weak; contradicts one-codebase rule |
| Qt (+Rust bindings) | heavier C++ toolchain; licensing considerations; weaker web-tech layout ecosystem fit |

## Consequences

- Frontend compromise blast-radius depends entirely on our capability
  discipline (tests enforce).
- Two-language stack (TS+Rust) — accepted; boundary keeps each side simple.
- M1 must validate WebKitGTK/WebView2 baselines before deeper work.

## Links

THREAT_MODEL T-WEB-*; PRD SEC-002/003; IMPLEMENTATION_PLAN M1/M10.
