import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { calls, core, handlers, resetFakeCore } from "../test/fakeCore";
import { getState, resetStore } from "../state/store";
import {
  editNote,
  flushAllNotes,
  hasNote,
  loadNotes,
  noteKey,
  noteText,
  resetNotes,
  SAVE_DELAY_MS,
} from "./notes";

const saves = () => calls.filter((c) => c.cmd === "note_save").map((c) => c.args);

describe("sticky note autosave", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetFakeCore();
    resetStore();
    resetNotes();
  });
  afterEach(() => vi.useRealTimers());

  it("saves once after the user pauses, with the latest text", async () => {
    await loadNotes("ws1");
    editNote("ws1", "m1", "t");
    editNote("ws1", "m1", "ta");
    editNote("ws1", "m1", "tail -f app.log");
    expect(getState().noteStatus[noteKey("ws1", "m1")]).toBe("saving");
    expect(noteText("ws1", "m1")).toBe("tail -f app.log");
    expect(saves()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(saves()).toEqual([{ workspaceId: "ws1", memberId: "m1", text: "tail -f app.log" }]);
    expect(getState().noteStatus[noteKey("ws1", "m1")]).toBe("saved");
    expect(core.notes.ws1.members.m1).toBe("tail -f app.log");
    expect(hasNote("ws1", "m1")).toBe(true);
  });

  it("keeps the text and retries with backoff when a save fails", async () => {
    await loadNotes("ws1");
    let fail = 2;
    const save = handlers.get("note_save")!;
    handlers.set("note_save", (a) => {
      if (fail-- > 0) throw new Error("disk full");
      return save(a);
    });
    editNote("ws1", null, "release on Thursday");
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(getState().noteStatus[noteKey("ws1", null)]).toBe("error");
    expect(getState().notices.at(-1)?.message).toMatch(/Note not saved: disk full/);
    await vi.advanceTimersByTimeAsync(1000); // second attempt fails
    expect(getState().noteStatus[noteKey("ws1", null)]).toBe("error");
    await vi.advanceTimersByTimeAsync(2000); // third attempt succeeds
    expect(getState().noteStatus[noteKey("ws1", null)]).toBe("saved");
    expect(core.notes.ws1.project).toBe("release on Thursday");
    expect(saves()).toHaveLength(3);
    // Only the first failure is announced, not every retry.
    expect(getState().notices.filter((n) => /Note not saved/.test(n.message))).toHaveLength(1);
  });

  it("does not retry what the core will never accept", async () => {
    await loadNotes("ws1");
    handlers.set("note_save", () => {
      throw new Error("a note can hold at most 256 KiB");
    });
    editNote("ws1", null, "x");
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS + 20000);
    expect(saves()).toHaveLength(1);
    expect(noteText("ws1", null)).toBe("x"); // still visible, nothing lost on screen
  });

  it("flushes pending notes immediately (window hidden / closing)", async () => {
    await loadNotes("ws1");
    editNote("ws1", "m2", "unsaved line");
    await flushAllNotes();
    expect(saves()).toHaveLength(1);
    expect(core.notes.ws1.members.m2).toBe("unsaved line");
  });

  it("text typed while loading is not overwritten by the loaded notes", async () => {
    core.notes.ws1 = { project: "old", members: {} };
    editNote("ws1", null, "new typing");
    await loadNotes("ws1");
    expect(noteText("ws1", null)).toBe("new typing");
  });
});
