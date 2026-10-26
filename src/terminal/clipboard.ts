/**
 * Minimal clipboard adapter — production delegates to the official Tauri 2
 * clipboard manager plugin; tests mock this module.
 *
 * Privacy: clipboard contents are never logged, persisted, or included in
 * errors/telemetry. Only read as direct result of explicit toolbar Paste action.
 * No polling, no startup/reload reads, no Rust log.
 *
 * Reason for native permission:
 * navigator.clipboard.readText() proved unreliable in the real Linux WebKit/
 * Tauri runtime during Human M3 desktop testing (Paste read failed).
 * The official plugin is now empirically required.
 */

import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function readClipboardText(): Promise<string> {
  return readText();
}

export async function writeClipboardText(text: string): Promise<void> {
  return writeText(text);
}
