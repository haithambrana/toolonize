/**
 * Launcher Registry + review queue (FR-012, T-POLICY-005).
 *
 * Shows everything discovery found, with provenance and full parsed
 * metadata. There is deliberately no "run" control here: launchers only
 * become runnable as workspace members, and only after review when their
 * classification requires it.
 */
import { useEffect, useMemo, useState } from "react";
import { api, type RegistryEntry, type SyntheticSpec } from "../lib/ipc";
import { addLauncher } from "../lib/actions";
import { Badges, LauncherDetails, TypeChip } from "../components/LauncherDetails";
import { entryName } from "../lib/labels";
import { confirmDialog, openDialog, promptDialog } from "../lib/dialogs";
import { attempt, getState, notify, useAppState } from "../state/store";

type Filter = "review" | "all" | "changes" | "terminal" | "gui" | "hidden" | "unavailable";

const FILTERS: { id: Filter; label: string; test: (e: RegistryEntry) => boolean }[] = [
  {
    id: "review",
    label: "Needs review",
    test: (e) =>
      !e.decision.hidden &&
      (e.review_pending || (e.classification.blocked && e.origin !== "synthetic")),
  },
  { id: "all", label: "All", test: (e) => !e.decision.hidden && !e.no_display },
  { id: "changes", label: "New & changed", test: (e) => e.change !== "unchanged" },
  {
    id: "terminal",
    label: "Terminal",
    test: (e) =>
      !e.decision.hidden &&
      ["embedded_terminal", "local_command", "remote_ssh"].includes(e.classification.proposed),
  },
  {
    id: "gui",
    label: "Apps",
    test: (e) => !e.decision.hidden && e.classification.proposed === "external_gui",
  },
  { id: "unavailable", label: "Unavailable", test: (e) => e.parse_status === "unavailable" },
  {
    id: "hidden",
    label: "Hidden",
    test: (e) => e.decision.hidden || e.no_display || e.not_shown_here,
  },
];

async function addToWorkspace(entry: RegistryEntry) {
  const s = getState();
  if (s.workspaces.length === 0) {
    notify("info", "Create a workspace first.");
    return;
  }
  const id =
    s.workspaces.length === 1
      ? s.workspaces[0].id
      : await openDialog<string>(`Add ${entryName(entry)} to…`, (close) => (
          <>
            <div className="dialog-body">
              <ul className="picker-list">
                {s.workspaces.map((w, i) => (
                  <li key={w.id} className="picker-item">
                    <span className="picker-name">{w.name}</span>
                    <button
                      type="button"
                      className="btn btn-sm btn-primary"
                      onClick={() => close(w.id)}
                      data-autofocus={i === 0 || undefined}
                    >
                      Add here
                    </button>
                  </li>
                ))}
              </ul>
            </div>
            <div className="dialog-actions">
              <button type="button" className="btn" onClick={() => close(undefined)}>
                Cancel
              </button>
            </div>
          </>
        ));
  const w = getState().workspaces.find((x) => x.id === id);
  if (w) {
    await addLauncher(w, entry, false);
    notify("info", `Added ${entryName(entry)} to ${w.name}`);
  }
}

function Detail({ entry }: { entry: RegistryEntry }) {
  const blocked = entry.classification.blocked || entry.parse_status !== "ok";
  useEffect(() => {
    if (entry.change !== "unchanged")
      void api.acknowledge([entry.launcher_id]).catch(() => undefined);
  }, [entry.launcher_id, entry.change]);
  return (
    <aside className="detail" aria-label={`Details for ${entryName(entry)}`}>
      <h2 className="detail-title">{entryName(entry)}</h2>
      <Badges entry={entry} />
      {entry.issues.length > 0 && (
        <div className="alert alert-warn">
          <strong>Problems found</strong>
          <ul>
            {entry.issues.map((i, n) => (
              <li key={n}>{i}</li>
            ))}
          </ul>
        </div>
      )}
      <LauncherDetails entry={entry} />
      <div className="detail-actions">
        {entry.review_pending && !blocked && (
          <button
            type="button"
            className={entry.classification.interpreter ? "btn btn-danger" : "btn btn-primary"}
            onClick={async () => {
              const ok = await confirmDialog({
                title: "Approve this launcher?",
                body: (
                  <p>
                    Approving lets <strong>exactly this version</strong> of the command run when you
                    add it to a workspace and start it. Any later change to it requires a new
                    review.
                  </p>
                ),
                confirm: "Approve",
                danger: entry.classification.interpreter,
              });
              if (ok)
                await attempt(
                  () => api.approve(entry.launcher_id, entry.fingerprint),
                  "Approval failed"
                );
            }}
          >
            Approve this version
          </button>
        )}
        {entry.decision.approved_fingerprint && (
          <button
            type="button"
            className="btn"
            onClick={() => void attempt(() => api.revoke(entry.launcher_id))}
          >
            Revoke approval
          </button>
        )}
        {entry.executable && (
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void addToWorkspace(entry)}
          >
            Add to workspace…
          </button>
        )}
        <button
          type="button"
          className="btn"
          onClick={async () => {
            const alias = await promptDialog({
              title: "Rename launcher",
              label: "Display name (empty to reset)",
              initial: entry.decision.alias ?? "",
              validate: (v) => (v.length > 120 ? "Too long" : null),
            });
            if (alias !== undefined)
              await attempt(() => api.setAlias(entry.launcher_id, alias || null));
          }}
        >
          Rename
        </button>
        <button
          type="button"
          className="btn"
          onClick={() =>
            void attempt(() => api.setHidden(entry.launcher_id, !entry.decision.hidden))
          }
        >
          {entry.decision.hidden ? "Unhide" : "Hide"}
        </button>
        {entry.origin === "synthetic" &&
          !entry.launcher_id.startsWith("sys:shell:") &&
          !entry.launcher_id.startsWith("sys:wsl:") && (
            <button
              type="button"
              className="btn"
              onClick={() => void attempt(() => api.removeSynthetic(entry.launcher_id))}
            >
              Remove connection
            </button>
          )}
      </div>
      {blocked && entry.origin !== "synthetic" && (
        <p className="muted small">
          This entry is malformed, unavailable or unsupported, so it cannot be approved. Fix the
          launcher file and ToolOnize will pick up the change.
        </p>
      )}
    </aside>
  );
}

function ConnectionForm() {
  const [kind, setKind] = useState<SyntheticSpec["kind"]>("ssh");
  const [host, setHost] = useState("");
  const [session, setSession] = useState("main");
  const submit = async () => {
    const spec: SyntheticSpec =
      kind === "ssh"
        ? { kind, host }
        : kind === "ssh_tmux"
          ? { kind, host, session }
          : { kind, session };
    const id = await attempt(() => api.addSynthetic(spec), "Could not add connection");
    if (id) {
      notify("info", "Connection added. Add it to a workspace to use it.");
      setHost("");
    }
  };
  return (
    <form
      className="conn-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <select
        className="input input-sm"
        value={kind}
        onChange={(e) => setKind(e.target.value as SyntheticSpec["kind"])}
        aria-label="Connection kind"
      >
        <option value="ssh">SSH host</option>
        <option value="ssh_tmux">SSH host + tmux session</option>
        <option value="tmux">Local tmux session</option>
      </select>
      {kind !== "tmux" && (
        <input
          className="input input-sm"
          placeholder="host alias from your ssh config"
          aria-label="SSH host"
          value={host}
          onChange={(e) => setHost(e.target.value)}
        />
      )}
      {kind !== "ssh" && (
        <input
          className="input input-sm"
          placeholder="session"
          aria-label="tmux session"
          value={session}
          onChange={(e) => setSession(e.target.value)}
        />
      )}
      <button type="submit" className="btn btn-sm">
        Add
      </button>
      <p className="muted small full">
        ToolOnize runs your own <code>ssh</code> with your config and agent; it never reads keys or
        stores credentials. Plain SSH reconnects start a new remote shell; tmux sessions persist.
      </p>
    </form>
  );
}

function Sources() {
  const registry = useAppState((s) => s.registry);
  const [root, setRoot] = useState("");
  if (!registry) return null;
  return (
    <section className="sources" aria-label="Launcher sources">
      <h3 className="subhead">Sources</h3>
      <table className="table">
        <thead>
          <tr>
            <th>Origin</th>
            <th>Folder</th>
            <th>Entries</th>
            <th>Notes</th>
          </tr>
        </thead>
        <tbody>
          {registry.reports.map((r) => (
            <tr key={r.root}>
              <td>{r.origin.replace(/_/g, " ")}</td>
              <td className="mono small break">{r.root}</td>
              <td>{r.entries}</td>
              <td className="small">
                {r.truncated && <span className="chip chip-warn">limit reached</span>}{" "}
                {r.errors.join("; ")}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <h3 className="subhead">Custom roots (opt-in)</h3>
      <p className="muted small">
        Folders you add are scanned read-only for launchers, bounded in depth and size, and every
        entry found there needs review before it can run.
      </p>
      <ul className="plain-list">
        {registry.custom_roots.map((r) => (
          <li key={r} className="row-gap">
            <code className="break">{r}</code>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => void attempt(() => api.removeCustomRoot(r))}
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
      <form
        className="row-gap"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!root.trim()) return;
          const ok = await attempt(() => api.addCustomRoot(root.trim()), "Could not add folder");
          if (ok !== undefined) setRoot("");
        }}
      >
        <input
          className="input input-sm grow"
          placeholder="Absolute folder path"
          aria-label="Custom root folder"
          value={root}
          onChange={(e) => setRoot(e.target.value)}
        />
        <button type="submit" className="btn btn-sm">
          Add folder
        </button>
      </form>
      <h3 className="subhead">Connections</h3>
      <ConnectionForm />
      {registry.removed.length > 0 && (
        <>
          <h3 className="subhead">Recently removed</h3>
          <ul className="plain-list small">
            {registry.removed.slice(-20).map((r) => (
              <li key={r.launcher_id}>
                {r.display_name}{" "}
                <span className="muted">· {new Date(r.removed_at).toLocaleString()}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

export function LaunchersScreen() {
  const registry = useAppState((s) => s.registry);
  const [chosen, setFilter] = useState<Filter | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const entries = useMemo(() => registry?.entries ?? [], [registry]);
  const counts = useMemo(
    () =>
      Object.fromEntries(FILTERS.map((f) => [f.id, entries.filter(f.test).length])) as Record<
        Filter,
        number
      >,
    [entries]
  );
  // Open on the review queue when it has work, otherwise on everything.
  const filter: Filter = chosen ?? (counts.review > 0 ? "review" : "all");
  const shown = useMemo(() => {
    const f = FILTERS.find((x) => x.id === filter)!;
    const q = query.toLowerCase();
    return entries
      .filter(f.test)
      .filter(
        (e) =>
          !q ||
          entryName(e).toLowerCase().includes(q) ||
          (e.descriptor.program ?? "").toLowerCase().includes(q)
      )
      .sort((a, b) => entryName(a).localeCompare(entryName(b)));
  }, [entries, filter, query]);
  const current = shown.find((e) => e.launcher_id === selected) ?? shown[0];

  return (
    <section className="launchers" aria-label="Launchers">
      <header className="ws-header">
        <h1 className="ws-title">Launchers</h1>
        <span className="muted small">
          {entries.length} found · nothing was executed during discovery
          {registry?.last_scan_ms
            ? ` · scanned ${new Date(registry.last_scan_ms).toLocaleTimeString()}`
            : ""}
        </span>
        <div className="ws-actions">
          <button
            type="button"
            className="btn btn-sm"
            disabled={scanning}
            onClick={async () => {
              setScanning(true);
              await attempt(() => api.rescan(), "Rescan failed");
              setScanning(false);
            }}
          >
            {scanning ? "Scanning…" : "Rescan"}
          </button>
        </div>
      </header>
      <div className="filters" role="tablist" aria-label="Filter launchers">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            type="button"
            role="tab"
            aria-selected={filter === f.id}
            className={filter === f.id ? "seg seg-active" : "seg"}
            onClick={() => setFilter(f.id)}
          >
            {f.label} <span className="count">{counts[f.id]}</span>
          </button>
        ))}
        <input
          className="input input-sm"
          placeholder="Search"
          aria-label="Search launchers"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className="split">
        <ul className="launcher-list" role="listbox" aria-label="Launchers">
          {shown.map((e) => (
            <li
              key={e.launcher_id}
              role="option"
              aria-selected={current?.launcher_id === e.launcher_id}
              tabIndex={0}
              className={
                current?.launcher_id === e.launcher_id
                  ? "launcher-row launcher-row-active"
                  : "launcher-row"
              }
              onClick={() => setSelected(e.launcher_id)}
              onKeyDown={(ev) => {
                if (ev.key === "Enter" || ev.key === " ") {
                  ev.preventDefault();
                  setSelected(e.launcher_id);
                }
                if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
                  ev.preventDefault();
                  const sib = ev.currentTarget[
                    ev.key === "ArrowDown" ? "nextElementSibling" : "previousElementSibling"
                  ] as HTMLElement | null;
                  sib?.focus();
                  sib?.click();
                }
              }}
            >
              <span className="launcher-name">{entryName(e)}</span>
              <TypeChip type={e.review_pending ? e.classification.proposed : e.effective_type} />
              <Badges entry={e} />
              <span className="muted small launcher-origin">{e.origin_label}</span>
            </li>
          ))}
          {shown.length === 0 && <li className="muted empty-list">Nothing here.</li>}
        </ul>
        {current ? (
          <Detail key={current.launcher_id} entry={current} />
        ) : (
          <aside className="detail muted">Select a launcher.</aside>
        )}
      </div>
      <Sources />
    </section>
  );
}
