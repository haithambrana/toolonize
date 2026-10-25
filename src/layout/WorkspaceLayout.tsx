/**
 * Docking layout for one workspace (flexlayout-react, ADR-003).
 *
 * The FlexLayout model is a projection; the Rust workspace engine holds the
 * saved snapshot. Tab components only attach long-lived terminal hosts, so
 * moving, maximizing, or switching tabs never recreates a terminal or
 * touches its PTY session (FR-031).
 */
import { useEffect, useRef } from "react";
import { Layout, Model, type IJsonModel, type TabNode } from "flexlayout-react";
import "flexlayout-react/style/combined.css";
import { api, type WorkspaceView } from "../lib/ipc";
import { layoutMembers } from "../lib/actions";
import { setState, useAppState } from "../state/store";
import { layoutFor, reconcile, type ModelJson } from "./modes";
import { stableStringify } from "./stable";
import { MemberPane } from "./MemberPane";

const SAVE_DELAY_MS = 400;

function buildModel(w: WorkspaceView): { model: Model } {
  const members = layoutMembers(w);
  const json =
    reconcile(w.layout.json, members) ?? layoutFor(w.layout.mode, members, w.layout.focused_member);
  let model: Model;
  try {
    model = Model.fromJson(json as IJsonModel);
  } catch {
    model = Model.fromJson(
      layoutFor(w.layout.mode, members, w.layout.focused_member) as IJsonModel
    );
  }
  return { model };
}

function selectedMember(model: Model): string | null {
  const node = model.getActiveTabset()?.getSelectedNode() as TabNode | undefined;
  const config = node?.getConfig?.() as { memberId?: string } | undefined;
  return config?.memberId ?? null;
}

export function WorkspaceLayout({ workspace }: { workspace: WorkspaceView }) {
  const theme = useAppState((s) => s.resolvedTheme);
  const memberKey = workspace.members
    .map((m) => `${m.id}:${m.title ?? ""}:${m.status.launcher_name ?? ""}`)
    .join("|");
  const savedKey = stableStringify(workspace.layout);
  const modelRef = useRef<Model | null>(null);
  const applied = useRef({ id: "", members: "", layout: "" });
  const lastSaved = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Rebuild only for changes that did not come from this view's own save:
  // another workspace, member changes, a mode switch, or an import.
  const a = applied.current;
  const external = savedKey !== a.layout && savedKey !== lastSaved.current;
  if (!modelRef.current || a.id !== workspace.id || a.members !== memberKey || external) {
    modelRef.current = buildModel(workspace).model;
  }
  applied.current = { id: workspace.id, members: memberKey, layout: savedKey };
  const model = modelRef.current;

  useEffect(() => () => clearTimeout(timer.current), []);

  const onModelChange = (m: Model) => {
    const focused = selectedMember(m);
    if (focused) setState({ focused });
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const json = m.toJson() as unknown as ModelJson;
      let mode = workspace.layout.mode;
      if (mode === "focus" && !m.getMaximizedTabset()) mode = "grid";
      const snapshot = { mode, json, focused_member: focused ?? workspace.layout.focused_member };
      lastSaved.current = stableStringify(snapshot);
      void api.saveLayout(workspace.id, mode, json, snapshot.focused_member).catch(() => undefined);
    }, SAVE_DELAY_MS);
  };

  const factory = (node: TabNode) => {
    if (node.getComponent() !== "member") return <div className="member-card muted">Empty</div>;
    const memberId = (node.getConfig() as { memberId: string }).memberId;
    return <MemberPane workspaceId={workspace.id} memberId={memberId} />;
  };

  return (
    <div
      className={`layout-host flexlayout__theme_${theme === "light" ? "light" : "dark"}`}
      data-testid="workspace-layout"
    >
      <Layout
        model={model}
        factory={factory}
        onModelChange={onModelChange}
        supportsPopout={false}
        realtimeResize
      />
    </div>
  );
}
