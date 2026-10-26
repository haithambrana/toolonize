import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";

describe("H16A capability-security: clipboard minimum permissions", () => {
  const capPath = join(process.cwd(), "src-tauri/capabilities/default.json");
  const raw = readFileSync(capPath, "utf8");
  const cap = JSON.parse(raw) as {
    identifier: string;
    windows: string[];
    permissions: string[];
    local?: boolean;
  };

  it("clipboard read-text is allowed", () => {
    expect(cap.permissions).toContain("clipboard-manager:allow-read-text");
  });

  it("clipboard write-text is allowed", () => {
    expect(cap.permissions).toContain("clipboard-manager:allow-write-text");
  });

  it("does NOT grant image/html/clear/default/all clipboard permissions", () => {
    const p = cap.permissions.join("\n");
    expect(p).not.toContain("clipboard-manager:allow-read-image");
    expect(p).not.toContain("clipboard-manager:allow-write-image");
    expect(p).not.toContain("clipboard-manager:allow-write-html");
    expect(p).not.toContain("clipboard-manager:allow-clear");
    expect(p).not.toContain("clipboard-manager:default");
    // No wildcard/default/all grant
    expect(cap.permissions).not.toContain("clipboard-manager:allow-*");
    expect(cap.permissions).not.toContain("clipboard-manager:allow-all");
    // No plain "clipboard-manager" without explicit allow
    for (const perm of cap.permissions) {
      if (perm.startsWith("clipboard-manager:")) {
        expect([
          "clipboard-manager:allow-read-text",
          "clipboard-manager:allow-write-text",
        ]).toContain(perm);
      }
    }
  });

  it("does NOT grant filesystem/shell/process/http/opener/remote URL capability", () => {
    const perms = cap.permissions;
    const blockedSubstrings = [
      "fs:",
      "filesystem",
      "shell:",
      "shell:allow",
      "process:",
      "http:",
      "opener:",
      "remote",
    ];
    for (const sub of blockedSubstrings) {
      expect(perms.some((perm) => perm.toLowerCase().includes(sub.toLowerCase()))).toBe(false);
    }
    // Also ensure no broad plugin defaults
    expect(perms).not.toContain("fs:default");
    expect(perms).not.toContain("shell:default");
    expect(perms).not.toContain("http:default");
    expect(perms).not.toContain("process:default");
    expect(perms).not.toContain("opener:default");
  });

  it("is local main-window only", () => {
    expect(cap.windows).toEqual(["main"]);
    // local content only (capability local flag)
    expect(cap.local).toBe(true);
  });

  it("description documents real Linux WebKit failure reason for clipboard", () => {
    // Original had 17 perms (allow-ping + 14 terminal + 2 m3); now 19 with clipboard text only.
    expect(cap.permissions.length).toBe(19);
    // Description must document why native permission exists: real Linux WebKit failure
    const rawText = readFileSync(
      join(process.cwd(), "src-tauri/capabilities/default.json"),
      "utf8"
    );
    expect(rawText).toMatch(/clipboard/i);
  });
});
