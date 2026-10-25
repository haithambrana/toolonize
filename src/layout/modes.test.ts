import { describe, expect, it } from "vitest";
import {
  cycle,
  layoutFor,
  membersIn,
  reconcile,
  tabId,
  type RowJson,
  type TabSetJson,
} from "./modes";
import { stableStringify } from "./stable";
import { calls } from "../test/fakeCore";

const members = ["a", "b", "c", "d", "e"].map((id) => ({ id, title: `Member ${id}` }));

function tabsets(node: RowJson | TabSetJson): TabSetJson[] {
  if (node.type === "tabset") return [node];
  return node.children.flatMap(tabsets);
}

describe("layout modes (T-LAYOUT-001)", () => {
  it("grid places every member exactly once in a near-square grid", () => {
    const json = layoutFor("grid", members);
    expect(membersIn(json)).toEqual(["a", "d", "b", "e", "c"]);
    expect(json.layout.children).toHaveLength(3); // ceil(sqrt(5)) columns
    expect(tabsets(json.layout)).toHaveLength(5);
  });

  it("focus maximizes the focused member's tabset", () => {
    const json = layoutFor("focus", members, "c");
    const max = tabsets(json.layout).filter((t) => t.maximized);
    expect(max).toHaveLength(1);
    expect(membersIn({ layout: { type: "row", children: max } })).toEqual(["c"]);
  });

  it("tabs puts all members in one tabset with the focused one selected", () => {
    const json = layoutFor("tabs", members, "d");
    const sets = tabsets(json.layout);
    expect(sets).toHaveLength(1);
    expect(sets[0].selected).toBe(3);
    expect(membersIn(json)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("master+stack puts the focused member large and stacks the rest", () => {
    const json = layoutFor("master_stack", members, "b");
    const [master, stack] = json.layout.children as [TabSetJson, RowJson];
    expect(membersIn({ layout: { type: "row", children: [master] } })).toEqual(["b"]);
    expect(master.weight).toBeGreaterThan(stack.weight!);
    expect(membersIn({ layout: stack })).toEqual(["a", "c", "d", "e"]);
  });

  it("every mode yields the same member set and stable tab ids", () => {
    for (const mode of ["grid", "focus", "tabs", "master_stack"] as const) {
      const json = layoutFor(mode, members, "a");
      expect([...membersIn(json)].sort()).toEqual(["a", "b", "c", "d", "e"]);
      expect(JSON.stringify(json)).toContain(tabId("a"));
    }
  });

  it("mode transforms are pure: no IPC, no session effects (T-LAYOUT-003)", () => {
    const before = calls.length;
    for (const mode of ["grid", "focus", "tabs", "master_stack"] as const)
      layoutFor(mode, members, "e");
    expect(calls.length).toBe(before);
  });

  it("handles zero and one member", () => {
    expect(membersIn(layoutFor("grid", []))).toEqual([]);
    expect(membersIn(layoutFor("master_stack", [members[0]]))).toEqual(["a"]);
  });
});

describe("reconcile", () => {
  it("drops removed members, adds new ones, refreshes titles, keeps arrangement", () => {
    const saved = layoutFor("master_stack", members.slice(0, 3), "a");
    const next = [
      { id: "a", title: "Renamed A" },
      { id: "c", title: "Member c" },
      { id: "z", title: "New" },
    ];
    const out = reconcile(saved, next)!;
    expect([...membersIn(out)].sort()).toEqual(["a", "c", "z"]);
    expect(JSON.stringify(out)).toContain("Renamed A");
    // Master position preserved for "a".
    expect(membersIn({ layout: { type: "row", children: [out.layout.children[0]] } })).toEqual([
      "a",
      "z",
    ]);
  });

  it("rejects unusable snapshots and duplicate tabs", () => {
    expect(reconcile(null, members)).toBeNull();
    expect(reconcile({ nope: 1 }, members)).toBeNull();
    const dup = layoutFor("tabs", members.slice(0, 2));
    (dup.layout.children[0] as TabSetJson).children.push(
      (dup.layout.children[0] as TabSetJson).children[0]
    );
    expect(membersIn(reconcile(dup, members.slice(0, 2)))).toEqual(["a", "b"]);
  });
});

describe("helpers", () => {
  it("cycles through ids", () => {
    expect(cycle(["a", "b", "c"], "c", 1)).toBe("a");
    expect(cycle(["a", "b", "c"], "a", -1)).toBe("c");
    expect(cycle(["a"], null, 1)).toBe("a");
    expect(cycle([], null, 1)).toBeNull();
  });

  it("stable stringify ignores key order (Rust round trip)", () => {
    expect(stableStringify({ b: 1, a: { d: [1, { y: 2, x: 1 }], c: null } })).toBe(
      stableStringify({ a: { c: null, d: [1, { x: 1, y: 2 }] }, b: 1 })
    );
  });
});
