/**
 * In-app updates (ADR-019): the core checks GitHub Releases and installs a
 * signed update; this module schedules checks and asks before installing.
 */
import { api } from "./ipc";
import { confirmDialog } from "./dialogs";
import { t } from "../i18n";
import { errorMessage } from "./ipc";
import { getState, notify, setState } from "../state/store";

/** First automatic check after start, then every few hours. */
export const FIRST_CHECK_MS = 15_000;
export const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

function patch(p: Partial<ReturnType<typeof getState>["update"]>) {
  setState((s) => ({ update: { ...s.update, ...p } }));
}

/** Ask the core; `manual` reports "up to date" and errors to the user. */
export async function checkForUpdates(manual: boolean): Promise<void> {
  if (getState().update.checking || getState().update.installing) return;
  patch({ checking: true, error: null });
  try {
    const status = await api.updateCheck();
    patch({ status, checking: false });
    if (manual && status.configured && !status.available) {
      notify("info", t("ToolOnize {version} is the latest version.", { version: status.current }));
    }
    if (!manual && status.available && status.version) {
      notify("info", t("ToolOnize {version} is available.", { version: status.version }), 12000);
    }
  } catch (error) {
    const message = errorMessage(error);
    patch({ checking: false, error: message });
    if (manual) notify("error", message);
  }
}

/** Confirm, then download, verify, install and restart. */
export async function installUpdate(): Promise<void> {
  const status = getState().update.status;
  if (!status?.available || !status.version) return;
  const ok = await confirmDialog({
    title: t("Update to ToolOnize {version}?", { version: status.version }),
    body: (
      <>
        <p>
          {t(
            "ToolOnize downloads the update, checks its signature, installs it and restarts. Your terminals come back after the restart, in the same folders."
          )}
        </p>
        {status.notes && <pre className="update-notes">{status.notes}</pre>}
      </>
    ),
    confirm: t("Update and restart"),
  });
  if (!ok) return;
  patch({ installing: true, progress: null, error: null });
  try {
    await api.updateInstall();
  } catch (error) {
    const message = errorMessage(error);
    patch({ installing: false, error: message });
    notify("error", message);
  }
}

let timers: ReturnType<typeof setTimeout>[] = [];

/** Automatic checks while the setting is on. Returns a cleanup. */
export function scheduleUpdateChecks(): () => void {
  const tick = () => {
    if (getState().settings?.auto_update_check) void checkForUpdates(false);
  };
  timers.push(setTimeout(tick, FIRST_CHECK_MS));
  timers.push(setInterval(tick, CHECK_EVERY_MS) as unknown as ReturnType<typeof setTimeout>);
  return () => {
    for (const id of timers) clearTimeout(id);
    timers = [];
  };
}

export function formatProgress(p: { downloaded: number; total: number | null } | null): string {
  if (!p) return t("Starting the download…");
  const mib = (n: number) => (n / (1024 * 1024)).toFixed(1);
  return p.total
    ? t("Downloading: {done} of {total} MiB", { done: mib(p.downloaded), total: mib(p.total) })
    : t("Downloading: {done} MiB", { done: mib(p.downloaded) });
}
