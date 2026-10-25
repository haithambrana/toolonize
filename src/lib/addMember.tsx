/** "Add member" picker: only runnable (reviewed) launchers are offered. */
import { useState } from "react";
import type { RegistryEntry, WorkspaceView } from "./ipc";
import { addLauncher } from "./actions";
import { openDialog } from "./dialogs";
import { entryName } from "./labels";
import { Badges, TypeChip } from "../components/LauncherDetails";
import { getState } from "../state/store";

function AddMemberBody({
  entries,
  close,
}: {
  entries: RegistryEntry[];
  close: (v: { entry: RegistryEntry; start: boolean } | undefined) => void;
}) {
  const [query, setQuery] = useState("");
  const q = query.toLowerCase();
  const shown = entries.filter(
    (e) => !q || entryName(e).toLowerCase().includes(q) || e.launcher_id.toLowerCase().includes(q)
  );
  const groups: [string, RegistryEntry[]][] = [
    ["Built-in shells & connections", shown.filter((e) => e.origin === "synthetic")],
    [
      "Terminal tools",
      shown.filter((e) => e.origin !== "synthetic" && e.effective_type !== "external_gui"),
    ],
    [
      "External applications",
      shown.filter((e) => e.origin !== "synthetic" && e.effective_type === "external_gui"),
    ],
  ];
  return (
    <>
      <div className="dialog-body">
        <input
          className="input"
          placeholder="Filter launchers"
          aria-label="Filter launchers"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          data-autofocus
        />
        <p className="muted small">
          Only reviewed, runnable launchers are listed. Adding one authorizes exactly its current
          command for this workspace; if the launcher changes later it must be re-reviewed.
        </p>
        <div className="picker">
          {groups.map(([title, list]) =>
            list.length === 0 ? null : (
              <section key={title}>
                <h4 className="subhead">{title}</h4>
                <ul className="picker-list">
                  {list.map((e) => (
                    <li key={e.launcher_id} className="picker-item">
                      <div className="picker-main">
                        <span className="picker-name">{entryName(e)}</span>
                        <TypeChip type={e.effective_type} /> <Badges entry={e} />
                        <div className="muted small">{e.origin_label}</div>
                      </div>
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() => close({ entry: e, start: false })}
                      >
                        Add
                      </button>
                      {e.effective_type !== "external_gui" && (
                        <button
                          type="button"
                          className="btn btn-sm btn-primary"
                          onClick={() => close({ entry: e, start: true })}
                        >
                          Add & start
                        </button>
                      )}
                    </li>
                  ))}
                </ul>
              </section>
            )
          )}
          {shown.length === 0 && (
            <p className="muted">
              No runnable launchers match. Check the Launchers view for items that need review.
            </p>
          )}
        </div>
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)}>
          Close
        </button>
      </div>
    </>
  );
}

export async function addMemberDialog(w: WorkspaceView): Promise<void> {
  const entries = (getState().registry?.entries ?? [])
    .filter((e) => e.executable && !e.decision.hidden)
    .sort((a, b) => entryName(a).localeCompare(entryName(b)));
  const choice = await openDialog<{ entry: RegistryEntry; start: boolean }>(
    `Add to ${w.name}`,
    (close) => <AddMemberBody entries={entries} close={close} />,
    { wide: true }
  );
  if (choice) await addLauncher(w, choice.entry, choice.start);
}
