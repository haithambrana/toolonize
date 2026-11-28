/**
 * Full backup and restore, duplicate and templates (ADR-014). The core
 * validates everything; restored workspaces wait for review and every
 * terminal must be authorized again.
 */
import { useEffect, useState } from "react";
import { api, type RestoreReport, type TemplateSummary, type WorkspaceView } from "./ipc";
import { openDialog, promptDialog } from "./dialogs";
import { flushAllNotes, loadNotes } from "./notes";
import { readFileText } from "./files";
import { selectWorkspace } from "./actions";
import { attempt, notify, refreshSettings, refreshWorkspaces } from "../state/store";
import { t } from "../i18n";

/** Backups larger than this are refused before reading (the core caps at 16 MiB). */
export const MAX_BACKUP_FILE = 16 * 1024 * 1024;

const nameCheck = (v: string) =>
  v.trim().length === 0
    ? "Enter a name"
    : v.trim().length > 80
      ? "At most 80 characters"
      : [...v].some((c) => c.charCodeAt(0) < 32)
        ? "Single line only"
        : null;

/** Write a backup of everything to Downloads. */
export async function backupNow(includeSettings: boolean): Promise<string | undefined> {
  // Typed-but-unsaved note text must be in the backup too.
  await flushAllNotes();
  const path = await attempt(() => api.exportBackup(includeSettings), t("Backup failed"));
  if (path) notify("info", `Backup saved to ${path}`);
  return path;
}

export function restoreSummary(r: RestoreReport): string {
  const parts = [
    `${r.workspaces} workspace(s) with ${r.members} terminal(s)`,
    r.notes ? `${r.notes} note(s)` : "",
    r.templates ? `${r.templates} template(s)` : "",
    r.connections ? `${r.connections} connection(s)` : "",
    r.settings_restored ? "settings" : "",
  ].filter(Boolean);
  return `Restored ${parts.join(", ")}. Review and authorize each workspace before it can start.`;
}

/** Restore a backup file chosen by the user. */
export async function restoreFromFile(
  file: File,
  restoreSettings: boolean
): Promise<RestoreReport | undefined> {
  if (file.size > MAX_BACKUP_FILE) {
    notify("error", t("The backup file is too large (more than 16 MiB)"));
    return undefined;
  }
  const report = await attempt(
    async () => api.restoreBackup(await readFileText(file), restoreSettings),
    t("Restore failed")
  );
  if (!report) return undefined;
  await refreshWorkspaces();
  if (report.settings_restored) await refreshSettings();
  notify(
    report.warnings.length ? "warning" : "info",
    restoreSummary(report) + (report.warnings.length ? ` ${report.warnings.join("; ")}.` : "")
  );
  return report;
}

export async function duplicateWorkspace(w: WorkspaceView): Promise<void> {
  await flushAllNotes();
  const id = await attempt(() => api.duplicateWorkspace(w.id), t("Could not duplicate"));
  if (!id) return;
  await refreshWorkspaces();
  await selectWorkspace(id);
  await loadNotes(id);
  notify("info", `Created “${w.name} (copy)”.`);
}

export async function saveAsTemplate(w: WorkspaceView): Promise<void> {
  const name = await promptDialog({
    title: t("Save as template"),
    label: t("Template name (terminals, options and layout; no folders or notes)"),
    initial: w.name,
    confirm: t("Save template"),
    validate: nameCheck,
  });
  if (!name) return;
  const id = await attempt(() => api.saveTemplate(w.id, name.trim()), t("Could not save template"));
  if (id) notify("info", `Template “${name.trim()}” saved.`);
}

function TemplateList({ close }: { close: (v: string | undefined) => void }) {
  const [list, setList] = useState<TemplateSummary[] | null>(null);
  // Confirmed inline: a nested dialog would unmount this list.
  const [confirming, setConfirming] = useState<string | null>(null);
  const load = () =>
    void attempt(() => api.listTemplates(), t("Could not load templates")).then((l) =>
      setList(l ?? [])
    );
  useEffect(load, []);
  return (
    <>
      <div className="dialog-body">
        {list === null ? (
          <p className="muted">{t("Loading…")}</p>
        ) : list.length === 0 ? (
          <p className="muted">
            {t("No templates yet. Open a workspace and choose More ▾ › Save as template….")}
          </p>
        ) : (
          <ul className="template-list" aria-label={t("Templates")}>
            {list.map((tpl) => (
              <li key={tpl.id} className="template-row">
                <span className="template-name">{tpl.name}</span>
                <span className="muted small">{t("{n} terminal(s)", { n: tpl.members })}</span>
                <button
                  type="button"
                  className="btn btn-sm btn-primary"
                  aria-label={t("Use template {name}", { name: tpl.name })}
                  onClick={() => close(tpl.id)}
                >
                  {t("Use")}
                </button>
                {confirming === tpl.id ? (
                  <>
                    <span className="small" role="alert">
                      {t("Delete “{name}”?", { name: tpl.name })}
                    </span>
                    <button
                      type="button"
                      className="btn btn-sm btn-danger"
                      onClick={async () => {
                        setConfirming(null);
                        await attempt(
                          () => api.deleteTemplate(tpl.id),
                          t("Could not delete template")
                        );
                        load();
                      }}
                    >
                      {t("Yes, delete")}
                    </button>
                    <button
                      type="button"
                      className="btn btn-sm"
                      onClick={() => setConfirming(null)}
                    >
                      {t("Keep")}
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="btn btn-sm"
                    aria-label={t("Delete template {name}", { name: tpl.name })}
                    onClick={() => setConfirming(tpl.id)}
                  >
                    {t("Delete")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)} data-autofocus>
          {t("Close")}
        </button>
      </div>
    </>
  );
}

/** Pick a template, name the new workspace, create it. */
export async function newFromTemplate(): Promise<void> {
  const templateId = await openDialog<string>("New workspace from template", (close) => (
    <TemplateList close={close} />
  ));
  if (!templateId) return;
  const name = await promptDialog({
    title: t("New workspace"),
    label: t("Name"),
    confirm: t("Create"),
    validate: nameCheck,
  });
  if (!name) return;
  const id = await attempt(
    () => api.createFromTemplate(templateId, name.trim()),
    t("Could not create workspace")
  );
  if (!id) return;
  await refreshWorkspaces();
  await selectWorkspace(id);
}
