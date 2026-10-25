/** Renders the top dialog from the dialog service with focus trapping,
 * Escape to cancel and restored focus (ACC-001/003). */
import { useEffect, useRef, useSyncExternalStore } from "react";
import { closeTop, dialogQueue, subscribeDialogs, type DialogRequest } from "../lib/dialogs";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function Modal({ request }: { request: DialogRequest }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = ref.current;
    const target =
      node?.querySelector<HTMLElement>("[data-autofocus]") ??
      node?.querySelector<HTMLElement>(FOCUSABLE);
    target?.focus();
    return () => previous?.focus?.();
  }, []);
  return (
    <div
      className="dialog-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && closeTop(undefined)}
    >
      <div
        ref={ref}
        className={request.wide ? "dialog dialog-wide" : "dialog"}
        role="dialog"
        aria-modal="true"
        aria-labelledby={`dialog-title-${request.id}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            closeTop(undefined);
          }
          if (e.key === "Tab" && ref.current) {
            const items = [...ref.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
            if (items.length === 0) return;
            const first = items[0];
            const last = items[items.length - 1];
            if (e.shiftKey && document.activeElement === first) {
              e.preventDefault();
              last.focus();
            } else if (!e.shiftKey && document.activeElement === last) {
              e.preventDefault();
              first.focus();
            }
          }
        }}
      >
        <h2 id={`dialog-title-${request.id}`} className="dialog-title">
          {request.title}
        </h2>
        {request.render(closeTop)}
      </div>
    </div>
  );
}

export function DialogHost() {
  const items = useSyncExternalStore(subscribeDialogs, dialogQueue);
  const top = items[items.length - 1];
  return top ? <Modal key={top.id} request={top} /> : null;
}
