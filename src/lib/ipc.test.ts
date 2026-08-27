import { describe, it, expect, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    app_name: "ToolOnize",
    app_version: "0.1.0",
    target_os: "linux",
    target_arch: "x86_64",
    status: "ok",
  }),
}));

import { ping } from "./ipc";
import { invoke } from "@tauri-apps/api/core";

describe("ipc wrapper", () => {
  it("calls tauri invoke with ping and returns typed response", async () => {
    const res = await ping();
    expect(invoke).toHaveBeenCalledWith("ping");
    expect(res.app_name).toBe("ToolOnize");
    expect(res.status).toBe("ok");
    // ensure no sensitive fields leaked
    expect((res as unknown as Record<string, unknown>).hostname).toBeUndefined();
    expect((res as unknown as Record<string, unknown>).username).toBeUndefined();
  });
});
