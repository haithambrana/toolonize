/**
 * Promise-based modal dialog service (confirm / prompt / custom content) with
 * focus trapping, Escape to cancel and restored focus (ACC-001/003).
 */
import { useState, type ReactNode } from "react";

export interface DialogRequest {
  id: number;
  title: string;
  render: (close: (value: unknown) => void) => ReactNode;
  resolve: (value: unknown) => void;
  wide?: boolean;
}

let queue: DialogRequest[] = [];
const listeners = new Set<() => void>();
let nextId = 0;

function emit() {
  for (const l of listeners) l();
}

export function openDialog<T>(
  title: string,
  render: (close: (value: T | undefined) => void) => ReactNode,
  options: { wide?: boolean } = {}
): Promise<T | undefined> {
  return new Promise((resolve) => {
    queue = [
      ...queue,
      {
        id: ++nextId,
        title,
        render: render as DialogRequest["render"],
        resolve: resolve as (v: unknown) => void,
        wide: options.wide,
      },
    ];
    emit();
  });
}

export function closeTop(value: unknown) {
  const top = queue[queue.length - 1];
  if (!top) return;
  queue = queue.slice(0, -1);
  emit();
  top.resolve(value);
}

export async function confirmDialog(options: {
  title: string;
  body: ReactNode;
  confirm: string;
  danger?: boolean;
  cancel?: string;
}): Promise<boolean> {
  const result = await openDialog<boolean>(options.title, (close) => (
    <>
      <div className="dialog-body">{options.body}</div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(false)}>
          {options.cancel ?? "Cancel"}
        </button>
        <button
          type="button"
          className={options.danger ? "btn btn-danger" : "btn btn-primary"}
          onClick={() => close(true)}
          data-autofocus
        >
          {options.confirm}
        </button>
      </div>
    </>
  ));
  return result === true;
}

function PromptBody({
  label,
  initial,
  confirm,
  close,
  validate,
}: {
  label: string;
  initial: string;
  confirm: string;
  close: (v: string | undefined) => void;
  validate?: (v: string) => string | null;
}) {
  const [value, setValue] = useState(initial);
  const error = validate ? validate(value) : value.trim() ? null : "Required";
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!error) close(value.trim());
      }}
    >
      <div className="dialog-body">
        <label className="field">
          <span className="field-label">{label}</span>
          <input
            className="input"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            data-autofocus
            aria-invalid={error ? true : undefined}
          />
        </label>
        {error && value && <p className="field-error">{error}</p>}
      </div>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={() => close(undefined)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!!error}>
          {confirm}
        </button>
      </div>
    </form>
  );
}

export function promptDialog(options: {
  title: string;
  label: string;
  initial?: string;
  confirm?: string;
  validate?: (v: string) => string | null;
}): Promise<string | undefined> {
  return openDialog<string>(options.title, (close) => (
    <PromptBody
      label={options.label}
      initial={options.initial ?? ""}
      confirm={options.confirm ?? "Save"}
      close={close}
      validate={options.validate}
    />
  ));
}

/** Test helper. */
export function resetDialogs() {
  queue = [];
  emit();
}

export function subscribeDialogs(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function dialogQueue(): DialogRequest[] {
  return queue;
}
