/**
 * Full parsed metadata of a launcher (FR-012): everything a reviewer needs
 * to decide, including provenance, exact argv, evidence and issues.
 */
import type { LauncherType, RegistryEntry } from "../lib/ipc";
import { TYPE_LABELS, entryName } from "../lib/labels";

export function TypeChip({ type }: { type: LauncherType }) {
  return <span className={`chip chip-type-${type}`}>{TYPE_LABELS[type]}</span>;
}

export function Badges({ entry }: { entry: RegistryEntry }) {
  const c = entry.classification;
  return (
    <span className="badges">
      {entry.review_pending && <span className="chip chip-warn">Review</span>}
      {c.blocked && <span className="chip chip-danger">Blocked</span>}
      {c.interpreter && (
        <span className="chip chip-danger" title="Runs code through a shell or interpreter">
          Interpreter
        </span>
      )}
      {entry.parse_status === "unavailable" && <span className="chip chip-muted">Unavailable</span>}
      {entry.change === "new" && <span className="chip chip-info">New</span>}
      {entry.change === "changed" && <span className="chip chip-warn">Changed</span>}
      {entry.descriptor.path_resolved && (
        <span className="chip chip-muted" title="Program resolved via PATH at launch">
          PATH
        </span>
      )}
      {entry.decision.hidden && <span className="chip chip-muted">Hidden</span>}
    </span>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="kv-row">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

export function LauncherDetails({ entry }: { entry: RegistryEntry }) {
  const d = entry.descriptor;
  const c = entry.classification;
  return (
    <div className="launcher-details">
      <dl className="kv">
        <Row label="Name">{entryName(entry)}</Row>
        <Row label="Kind">
          <TypeChip type={entry.effective_type} />{" "}
          {entry.effective_type !== c.proposed && (
            <span className="muted small">
              proposed: {TYPE_LABELS[c.proposed]} ({c.confidence} confidence)
            </span>
          )}
        </Row>
        <Row label="Origin">
          {entry.origin_label}
          {entry.origin !== "synthetic" && (
            <div className="mono small muted break">{entry.origin_root}</div>
          )}
        </Row>
        {d.source_path && (
          <Row label="Source file">
            <span className="mono small break">{d.source_path}</span>
          </Row>
        )}
        <Row label="Program">
          {d.program ? (
            <code className="break">{d.program}</code>
          ) : (
            <span className="muted">none</span>
          )}
          {d.path_resolved && (
            <div className="muted small">Resolved through PATH when it starts.</div>
          )}
        </Row>
        {d.args.length > 0 && (
          <Row label="Arguments">
            <ol className="argv">
              {d.args.map((a, i) => (
                <li key={i}>
                  <code className="break">{a === "" ? '""' : a}</code>
                </li>
              ))}
            </ol>
            <div className="muted small">
              Each item is passed as one literal argument; no shell is involved.
            </div>
          </Row>
        )}
        {d.raw_args && (
          <Row label="Argument string">
            <code className="break">{d.raw_args}</code>
            <div className="muted small">
              Passed to the target unmodified, exactly as stored in the shortcut.
            </div>
          </Row>
        )}
        {d.exec_template && (
          <Row label="Exec">
            <code className="break">{d.exec_template}</code>
          </Row>
        )}
        {d.working_dir && (
          <Row label="Working dir">
            <code className="break">{d.working_dir}</code>
          </Row>
        )}
        <Row label="Fingerprint">
          <code title={entry.fingerprint}>{entry.fingerprint.slice(0, 16)}…</code>
        </Row>
        {entry.comment && <Row label="Comment">{entry.comment}</Row>}
      </dl>
      {c.interpreter && (
        <p className="alert alert-danger">
          This launcher runs code through a shell or interpreter. Whatever the command text says
          will execute with your permissions.
        </p>
      )}
      <h4 className="subhead">Why ToolOnize classified it this way</h4>
      <ul className="evidence">
        {c.evidence.map((e, i) => (
          <li key={i}>{e}</li>
        ))}
      </ul>
    </div>
  );
}
