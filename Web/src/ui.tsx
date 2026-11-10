import { AlertCircle, LoaderCircle, ScanLine } from "lucide-react";
import {
  cloneElement,
  isValidElement,
  useId,
  type ReactElement,
  type ReactNode,
} from "react";
import type { JobState } from "./models";

export function Brand() {
  return (
    <div className="brand">
      <span className="brand-symbol">
        <ScanLine size={24} />
      </span>
      <span>
        Scanner<span className="brand-sub">CAPTURE WORKSPACE</span>
      </span>
    </div>
  );
}
export function ErrorNote({ error }: { error: unknown }) {
  return error ? (
    <div className="error-note" role="alert">
      <AlertCircle size={18} />
      <span>{error instanceof Error ? error.message : String(error)}</span>
    </div>
  ) : null;
}
export function Loading({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="loading" role="status">
      <LoaderCircle size={18} className="spin" />
      {label}
    </div>
  );
}
export function StateBadge({ state }: { state: JobState }) {
  return (
    <span className={`badge state-${state}`}>
      <span className="status-dot" />
      {state === "scanning" ? "Receiving images" : state}
    </span>
  );
}
export function Empty({
  title,
  children,
}: {
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="empty">
      <ScanLine size={32} />
      <h3>{title}</h3>
      <p>{children}</p>
    </div>
  );
}
export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <label className="field" htmlFor={id}>
      <span id={`${id}-label`}>{label}</span>
      {isValidElement(children)
        ? cloneElement(children as ReactElement<Record<string, unknown>>, {
            id,
            "aria-labelledby": `${id}-label`,
          })
        : children}
    </label>
  );
}
