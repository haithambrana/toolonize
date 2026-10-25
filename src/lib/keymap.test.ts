import { describe, expect, it } from "vitest";
import {
  actionFor,
  bindingFor,
  conflicts,
  DEFAULT_KEYMAP,
  effectiveKeymap,
  keyName,
} from "./keymap";

function ev(code: string, mods: Partial<KeyboardEvent> = {}) {
  return { code, key: "", ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods };
}

describe("keymap (FR-054, T-UI-006)", () => {
  it("defaults are unique and use modifiers", () => {
    expect(conflicts(DEFAULT_KEYMAP).size).toBe(0);
    for (const b of Object.values(DEFAULT_KEYMAP)) expect(b).toMatch(/^(Ctrl|Alt|Meta)\+/);
  });

  it("uses layout-independent key names", () => {
    expect(keyName(ev("KeyF"))).toBe("F");
    expect(keyName(ev("Digit2"))).toBe("2");
    expect(keyName(ev("BracketRight"))).toBe("]");
    expect(bindingFor(ev("KeyP", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+P");
  });

  it("maps events to actions and ignores unmodified keys", () => {
    expect(actionFor(ev("KeyP", { ctrlKey: true, shiftKey: true }), DEFAULT_KEYMAP)).toBe(
      "palette"
    );
    expect(actionFor(ev("Digit3", { ctrlKey: true, shiftKey: true }), DEFAULT_KEYMAP)).toBe(
      "mode.tabs"
    );
    expect(actionFor(ev("KeyC", { ctrlKey: true }), DEFAULT_KEYMAP)).toBeUndefined();
    expect(actionFor(ev("KeyP"), DEFAULT_KEYMAP)).toBeUndefined();
  });

  it("applies remaps and reports conflicts", () => {
    const map = effectiveKeymap({ palette: "Ctrl+Alt+K", unknown: "Ctrl+X" });
    expect(map.palette).toBe("Ctrl+Alt+K");
    expect(actionFor(ev("KeyK", { ctrlKey: true, altKey: true }), map)).toBe("palette");
    const clash = effectiveKeymap({ "mode.grid": DEFAULT_KEYMAP.palette });
    expect([...conflicts(clash).values()][0]).toEqual(["palette", "mode.grid"]);
  });
});
