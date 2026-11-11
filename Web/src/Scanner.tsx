import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Download, RefreshCw } from "lucide-react";
import { useAuth } from "./auth";
import { saveJSON } from "./api";
import { useWorkspace } from "./data";
import type { DeviceStatus } from "./models";
import { ErrorNote, Field, Loading } from "./ui";

export function Scanner() {
  const { api } = useAuth();
  const { devices, status } = useWorkspace();
  const [selected, setSelected] = useState("");
  const id = selected || devices.data?.devices[0]?.id || "";
  const busy = !!status.data?.activeJobID || !!status.data?.queuedJobs;
  const device = useQuery({
    queryKey: ["device", id],
    queryFn: ({ signal }) =>
      api.get<DeviceStatus>(`devices/${encodeURIComponent(id)}`, signal),
    enabled: !!id && !!status.data && !busy,
    staleTime: 60_000,
    retry: false,
  });
  const report = device.data?.inspection;
  return (
    <>
      <header className="page-heading">
        <div>
          <p className="eyebrow">HARDWARE & CAPABILITIES</p>
          <h1>Scanner details</h1>
          <p>
            What the installed driver reports, with evidence and limitations.
          </p>
        </div>
      </header>
      <section className="panel">
        <div className="toolbar">
          <Field label="Scanner">
            <select
              value={id}
              disabled={busy}
              onChange={(e) => setSelected(e.target.value)}
            >
              {!id && <option value="">No scanner connected</option>}
              {devices.data?.devices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </Field>
          <button
            className="button secondary"
            disabled={!id || busy || device.isFetching}
            onClick={() => void device.refetch()}
          >
            <RefreshCw size={16} />
            Inspect readiness
          </button>
          {report && (
            <button
              className="button secondary"
              onClick={() => saveJSON(report, "scanner-capabilities.json")}
            >
              <Download size={16} />
              Download report
            </button>
          )}
        </div>
        <ErrorNote error={devices.error || device.error} />
        {device.isFetching && <Loading label="Inspecting driver settings…" />}
        {busy && (
          <p className="notice">
            The scanner is busy. Capability inspection is paused until the
            current jobs finish.
          </p>
        )}
        {device.data && (
          <p className="readiness">
            Readiness at last inspection:{" "}
            <strong>
              {device.data.ready === undefined
                ? "Unknown"
                : device.data.ready
                  ? "Ready"
                  : "Not ready"}
            </strong>
          </p>
        )}
        {device.data?.warnings.map((message, i) => (
          <p key={i} className="small muted">
            {message}
          </p>
        ))}
      </section>
      {report && (
        <>
          {report.units.map((unit, index) => (
            <section className="panel capability-panel" key={index}>
              <div className="section-heading">
                <h2>
                  {unit.type === 3
                    ? "Automatic document feeder"
                    : unit.className || `Functional unit ${unit.type}`}
                </h2>
                <span className="badge">Driver report</span>
              </div>
              <p className="small muted">
                DPI, duplex and output format can be configured from the
                dashboard. Other fields below are inspection-only in the current
                engine.
              </p>
              <div className="capability-list">
                {Object.entries(unit.settings)
                  .sort(([a], [b]) => a.localeCompare(b))
                  .map(([name, setting]) => (
                    <details key={name}>
                      <summary>
                        <span>{name.replace(/([A-Z])/g, " $1")}</span>
                        <span className="cap-value">
                          {JSON.stringify(setting.current)}
                        </span>
                      </summary>
                      <dl>
                        <dt>Evidence</dt>
                        <dd>{setting.evidence}</dd>
                        <dt>Allowed values</dt>
                        <dd>{JSON.stringify(setting.allowed) ?? "Unknown"}</dd>
                        <dt>Default</dt>
                        <dd>
                          {JSON.stringify(setting.defaultValue) ?? "Unknown"}
                        </dd>
                        <dt>Driver access</dt>
                        <dd>
                          {setting.readOnly
                            ? "Read only"
                            : "Setter exposed; not necessarily tested"}
                        </dd>
                      </dl>
                    </details>
                  ))}
              </div>
              {[...unit.vendorFeatures, ...unit.templates].map((feature, i) => (
                <details className="vendor-feature" key={i}>
                  <summary>
                    {feature.label || feature.internalName || "Vendor feature"}
                  </summary>
                  <p>{feature.tooltip}</p>
                  <pre>{JSON.stringify(feature.fields, null, 2)}</pre>
                  <p className="small muted">
                    {feature.evidence} · Inspection only
                  </p>
                </details>
              ))}
            </section>
          ))}
          <section className="panel">
            <h2>Report notes</h2>
            {report.errors.map((error, i) => (
              <ErrorNote key={i} error={error.message} />
            ))}
            {[...report.warnings, ...report.limitations].map((message, i) => (
              <p className="small muted" key={i}>
                {message}
              </p>
            ))}
            <p className="small muted">
              Unknown capabilities: {report.unknownCapabilities.join(", ")}
            </p>
          </section>
        </>
      )}
    </>
  );
}
