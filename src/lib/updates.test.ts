import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { calls, handlers, resetFakeCore, settings } from "../test/fakeCore";
import { getState, resetStore, setState } from "../state/store";
import { CHECK_EVERY_MS, FIRST_CHECK_MS, formatProgress, scheduleUpdateChecks } from "./updates";

describe("automatic update checks", () => {
  beforeEach(() => {
    resetFakeCore();
    resetStore();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  const checks = () => calls.filter((c) => c.cmd === "update_check").length;

  it("checks shortly after start and then every 6 hours while enabled", async () => {
    setState({ settings: { ...settings, auto_update_check: true } });
    const stop = scheduleUpdateChecks();
    await vi.advanceTimersByTimeAsync(FIRST_CHECK_MS - 1);
    expect(checks()).toBe(0);
    await vi.advanceTimersByTimeAsync(1);
    expect(checks()).toBe(1);
    await vi.advanceTimersByTimeAsync(CHECK_EVERY_MS);
    expect(checks()).toBe(2);
    stop();
    await vi.advanceTimersByTimeAsync(CHECK_EVERY_MS * 2);
    expect(checks()).toBe(2);
  });

  it("does nothing when the setting is off, and a failed check stays quiet", async () => {
    setState({ settings: { ...settings, auto_update_check: false } });
    const stop = scheduleUpdateChecks();
    await vi.advanceTimersByTimeAsync(FIRST_CHECK_MS + CHECK_EVERY_MS);
    expect(checks()).toBe(0);
    stop();
    handlers.set("update_check", () => {
      throw "could not check for updates: offline";
    });
    setState({ settings: { ...settings, auto_update_check: true } });
    const stop2 = scheduleUpdateChecks();
    await vi.advanceTimersByTimeAsync(FIRST_CHECK_MS);
    expect(getState().update.error).toMatch(/offline/);
    expect(getState().notices).toEqual([]);
    stop2();
  });

  it("formats download progress", () => {
    expect(formatProgress(null)).toBe("Starting the download…");
    expect(formatProgress({ downloaded: 3 * 1024 * 1024, total: null })).toBe(
      "Downloading: 3.0 MiB"
    );
  });
});
