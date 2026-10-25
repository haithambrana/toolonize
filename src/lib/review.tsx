/** Member review/re-authorization dialog (FR-015 re-review flow). */
import { api, type MemberView, type RegistryEntry, type WorkspaceView } from "./ipc";
import { memberTitle } from "./actions";
import { openDialog } from "./dialogs";
import { LauncherDetails } from "../components/LauncherDetails";
import { attempt, getState } from "../state/store";

export async function reviewMember(w: WorkspaceView, m: MemberView): Promise<void> {
  const entry = getState().registry?.entries.find((e) => e.launcher_id === m.launcher_id);
  if (!entry) {
    await openDialog<void>(`${memberTitle(m)}: launcher missing`, (close) => (
      <>
        <div className="dialog-body">
          <p>
            This member refers to a launcher that is not present on this machine
            {m.portable ? ` (${m.portable.kind}: ${m.portable.value})` : ""}. It stays in the
            workspace and cannot start. Install the tool or remove the member.
          </p>
        </div>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={() => close(undefined)} data-autofocus>
            Close
          </button>
        </div>
      </>
    ));
    return;
  }
  const decision = await openDialog<"approve" | "authorize">(
    `Review ${memberTitle(m)}`,
    (close) => <ReviewBody entry={entry} member={m} close={close} />,
    { wide: true }
  );
  if (decision === "approve") {
    const ok = await attempt(
      () => api.approve(entry.launcher_id, entry.fingerprint),
      "Approval failed"
    );
    if (ok === undefined) return;
    await attempt(() => api.authorizeMember(w.id, m.id, entry.fingerprint), "Authorization failed");
  } else if (decision === "authorize") {
    await attempt(() => api.authorizeMember(w.id, m.id, entry.fingerprint), "Authorization failed");
  }
}

function ReviewBody({
  entry,
  member,
  close,
}: {
  entry: RegistryEntry;
  member: MemberView;
  close: (v: "approve" | "authorize" | undefined) => void;
}) {
  const blocked = entry.classification.blocked || entry.parse_status !== "ok";
  return (
    <>
      <div className="dialog-body">
        {member.status.reason && <p className="alert alert-warn">{member.status.reason}</p>}
        <LauncherDetails entry={entry} />
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)}>
          Cancel
        </button>
        {!blocked && (
          <button
            type="button"
            className={entry.classification.interpreter ? "btn btn-danger" : "btn btn-primary"}
            onClick={() => close(entry.review_pending ? "approve" : "authorize")}
            data-autofocus
          >
            {entry.review_pending
              ? "Approve this exact version and authorize"
              : "Authorize this version"}
          </button>
        )}
      </div>
    </>
  );
}
