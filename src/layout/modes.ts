/**
 * Layout mode presets over the FlexLayout JSON model (FR-022).
 *
 * Pure functions: they only produce layout JSON. They never touch sessions,
 * so switching modes cannot start, stop, or respawn anything (T-LAYOUT-003).
 */
import type { LayoutMode } from "../lib/ipc";

export interface LayoutMember {
  id: string;
  title: string;
}

/* Minimal structural types for the subset of FlexLayout JSON we generate. */
export interface TabJson {
  type: "tab";
  id: string;
  name: string;
  component: "member";
  config: { memberId: string };
  enableClose?: boolean;
  enablePopout?: boolean;
}
export interface TabSetJson {
  type: "tabset";
  id?: string;
  weight?: number;
  selected?: number;
  active?: boolean;
  maximized?: boolean;
  children: (TabJson | Record<string, unknown>)[];
  [key: string]: unknown;
}
export interface RowJson {
  type: "row";
  id?: string;
  weight?: number;
  children: (RowJson | TabSetJson)[];
  [key: string]: unknown;
}
export interface ModelJson {
  global?: Record<string, unknown>;
  borders?: unknown[];
  layout: RowJson;
  [key: string]: unknown;
}

export const GLOBAL = {
  tabEnableClose: false,
  tabEnablePopout: false,
  tabEnableRename: true,
  tabSetEnableMaximize: true,
  tabSetEnableTabStrip: true,
  tabSetEnableClose: false,
  enableEdgeDock: true,
};

export function tabId(memberId: string): string {
  return `tab-${memberId}`;
}

export function tab(member: LayoutMember): TabJson {
  return {
    type: "tab",
    id: tabId(member.id),
    name: member.title,
    component: "member",
    config: { memberId: member.id },
    enableClose: false,
    enablePopout: false,
  };
}

function tabset(members: LayoutMember[], id: string, extra: Partial<TabSetJson> = {}): TabSetJson {
  return { type: "tabset", id, weight: 50, children: members.map(tab), ...extra };
}

function model(layout: RowJson): ModelJson {
  return { global: { ...GLOBAL }, borders: [], layout };
}

function grid(members: LayoutMember[], maximized?: string): ModelJson {
  if (members.length === 0) {
    return model({ type: "row", id: "root", children: [tabset([], "ts-empty")] });
  }
  const cols = Math.ceil(Math.sqrt(members.length));
  const rows = Math.ceil(members.length / cols);
  const columns: RowJson[] = [];
  for (let c = 0; c < cols; c++) {
    const cells: TabSetJson[] = [];
    for (let r = 0; r < rows; r++) {
      const m = members[r * cols + c];
      if (!m) continue;
      cells.push(
        tabset([m], `ts-${m.id}`, maximized === m.id ? { maximized: true, active: true } : {})
      );
    }
    if (cells.length > 0)
      columns.push({ type: "row", id: `col-${c}`, weight: 50, children: cells });
  }
  return model({ type: "row", id: "root", children: columns });
}

function tabs(members: LayoutMember[], focused?: string): ModelJson {
  const selected = Math.max(
    0,
    members.findIndex((m) => m.id === focused)
  );
  return model({
    type: "row",
    id: "root",
    children: [tabset(members, "ts-all", { selected, active: true, weight: 100 })],
  });
}

function masterStack(members: LayoutMember[], focused?: string): ModelJson {
  const master = members.find((m) => m.id === focused) ?? members[0];
  if (!master) return grid([]);
  const rest = members.filter((m) => m.id !== master.id);
  const children: (RowJson | TabSetJson)[] = [
    tabset([master], "ts-master", { weight: rest.length ? 62 : 100, active: true }),
  ];
  if (rest.length) {
    children.push({
      type: "row",
      id: "stack",
      weight: 38,
      children: rest.map((m) => tabset([m], `ts-${m.id}`)),
    });
  }
  return model({ type: "row", id: "root", children });
}

/** Layout JSON for `mode` over `members` (in workspace order). */
export function layoutFor(
  mode: LayoutMode,
  members: LayoutMember[],
  focused?: string | null
): ModelJson {
  const f = focused ?? undefined;
  switch (mode) {
    case "grid":
      return grid(members);
    case "focus":
      return grid(members, f ?? members[0]?.id);
    case "tabs":
      return tabs(members, f);
    case "master_stack":
      return masterStack(members, f);
  }
}

/** Member ids shown by a layout, in document order. */
export function membersIn(json: unknown): string[] {
  const out: string[] = [];
  const visit = (node: unknown) => {
    if (!node || typeof node !== "object") return;
    const n = node as Record<string, unknown>;
    if (n.type === "tab") {
      const id = (n.config as { memberId?: unknown } | undefined)?.memberId;
      if (typeof id === "string") out.push(id);
    }
    if (Array.isArray(n.children)) n.children.forEach(visit);
  };
  const root = json as { layout?: unknown; borders?: unknown[] } | null;
  if (root?.layout) visit(root.layout);
  root?.borders?.forEach(visit);
  return out;
}

function firstTabset(node: RowJson | TabSetJson): TabSetJson | undefined {
  if (node.type === "tabset") return node;
  for (const child of node.children) {
    const found = firstTabset(child);
    if (found) return found;
  }
  return undefined;
}

/**
 * Bring a saved layout in line with the current member list: drop tabs of
 * removed members, append tabs for new ones, refresh titles. Returns null
 * when the saved layout cannot be used (then a preset is generated).
 */
export function reconcile(json: unknown, members: LayoutMember[]): ModelJson | null {
  if (!json || typeof json !== "object" || !(json as ModelJson).layout) return null;
  const copy = JSON.parse(JSON.stringify(json)) as ModelJson;
  const wanted = new Map(members.map((m) => [m.id, m]));
  const present = new Set<string>();
  const prune = (node: RowJson | TabSetJson): void => {
    if (node.type === "tabset") {
      node.children = node.children.filter((child) => {
        const c = child as Partial<TabJson>;
        if (c.type !== "tab") return true;
        const id = c.config?.memberId;
        if (typeof id !== "string" || !wanted.has(id) || present.has(id)) return false;
        present.add(id);
        c.name = wanted.get(id)!.title;
        return true;
      });
      if (typeof node.selected === "number" && node.selected >= node.children.length) {
        node.selected = Math.max(0, node.children.length - 1);
      }
      return;
    }
    node.children.forEach(prune);
  };
  prune(copy.layout);
  const missing = members.filter((m) => !present.has(m.id));
  if (missing.length) {
    const target = firstTabset(copy.layout);
    if (!target) return null;
    target.children.push(...missing.map(tab));
  }
  return copy;
}

/** Ordered list of members for next/previous navigation. */
export function cycle(ids: string[], current: string | null, step: 1 | -1): string | null {
  if (ids.length === 0) return null;
  const index = current ? ids.indexOf(current) : -1;
  const next = (index + step + ids.length) % ids.length;
  return ids[index === -1 ? 0 : next];
}
