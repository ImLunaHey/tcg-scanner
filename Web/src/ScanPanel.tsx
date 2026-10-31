import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUpRight, ScanLine, RefreshCw } from "lucide-react";
import { useAuth } from "./auth";
import { ApiError } from "./api";
import { useWorkspace } from "./data";
import {
  controls,
  uuid,
  type Batch,
  type DeviceStatus,
  type Format,
  type Inspection,
  type ScanSubmission,
} from "./models";
import { ErrorNote, Field, Loading } from "./ui";

const pendingKey = "scanner.pendingScan.v1";
function loadPending(): ScanSubmission | null {
  try {
    const p = JSON.parse(sessionStorage.getItem(pendingKey) || "null");
    return p?.requestID && p?.deviceID ? p : null;
  } catch {
    return null;
  }
}
export function ScanPanel({ onStarted }: { onStarted: (id: string) => void }) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const { devices, profiles, status } = useWorkspace();
  const [chosenDevice, setChosenDevice] = useState("");
  const deviceID = chosenDevice || devices.data?.devices[0]?.id || "";
  const [profileID, setProfileID] = useState("");
  const [chosenDPI, setDPI] = useState(300);
  const [format, setFormat] = useState<Format>("png");
  const [duplex, setDuplex] = useState(false);
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState(loadPending);
  const sending = useRef(false);
  const hardwareBusy = !!status.data?.activeJobID || !!status.data?.queuedJobs;
  const inspection = useQuery({
    queryKey: ["capabilities", deviceID],
    queryFn: ({ signal }) =>
      api.get<Inspection>(
        `devices/${encodeURIComponent(deviceID)}/capabilities`,
        signal,
      ),
    enabled: !!deviceID && !!status.data && !hardwareBusy,
    staleTime: 60_000,
    retry: false,
  });
  const available = controls(inspection.data);
  const profile = profiles.data?.profiles.find((p) => p.id === profileID);
  const dpi =
    profile?.dpi ??
    (available.resolutions.includes(chosenDPI)
      ? chosenDPI
      : (available.resolutions[0] ?? chosenDPI));
  const useDuplex = profile?.duplex ?? (duplex && available.duplex);
  const selectedFormat = profile?.format ?? format;
  const configured =
    !!inspection.data &&
    !inspection.isError &&
    available.resolutions.includes(dpi) &&
    (!useDuplex || available.duplex);
  const online =
    !!status.data &&
    !status.isError &&
    !devices.isError &&
    !status.data.paused &&
    !status.data.stopping &&
    !status.data.persistenceError;
  function persist(value: ScanSubmission | null) {
    setPending(value);
    try {
      if (value) sessionStorage.setItem(pendingKey, JSON.stringify(value));
      else sessionStorage.removeItem(pendingKey);
    } catch {
      /* Still retain in memory. */
    }
  }
  const start = useMutation({
    mutationFn: async (submission: ScanSubmission) => {
      if (sending.current)
        throw new Error("A request is already being submitted.");
      sending.current = true;
      try {
        return await api.send<Batch>("jobs", "POST", submission);
      } finally {
        sending.current = false;
      }
    },
    onSuccess: (batch) => {
      persist(null);
      setReady(false);
      void queries.invalidateQueries({ queryKey: ["jobs"] });
      void queries.invalidateQueries({ queryKey: ["status"] });
      onStarted(batch.id);
    },
    onError: (error) => {
      // A timeout/5xx may have created a job. Keep the exact request for an idempotent retry.
      if (
        error instanceof ApiError &&
        [400, 403, 404, 409, 422, 429].includes(error.status)
      )
        persist(null);
    },
  });
  const defaults = useMutation({
    mutationFn: () =>
      api.send<DeviceStatus>(
        `devices/${encodeURIComponent(deviceID)}/settings`,
        "PATCH",
        { dpi, duplex: useDuplex, format: selectedFormat },
      ),
  });
  function begin() {
    if (
      !configured ||
      !ready ||
      !online ||
      hardwareBusy ||
      sending.current ||
      pending
    )
      return;
    const submission = {
      requestID: uuid(),
      deviceID,
      dpi,
      duplex: useDuplex,
      format: selectedFormat,
      ...(profileID ? { profileID } : {}),
    };
    persist(submission);
    start.mutate(submission);
  }
  return (
    <section className="panel scan-panel">
      <div className="section-heading">
        <div>
          <p className="eyebrow">CAPTURE</p>
          <h2>Start a new batch</h2>
        </div>
        <ScanLine size={23} />
      </div>
      <fieldset
        disabled={hardwareBusy || start.isPending || !!pending}
        className="form-stack"
      >
        <Field label="Scanner">
          <select
            value={deviceID}
            onChange={(e) => {
              setChosenDevice(e.target.value);
              setReady(false);
            }}
          >
            {!devices.data?.devices.length && (
              <option value="">No scanner connected</option>
            )}
            {devices.data?.devices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Scan profile">
          <select
            value={profileID}
            onChange={(e) => {
              setProfileID(e.target.value);
              setReady(false);
            }}
          >
            <option value="">Custom settings</option>
            {profiles.data?.profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </Field>
        <div className="form-row">
          <Field label="Resolution">
            <select
              value={dpi}
              disabled={!!profile || !available.resolutions.length}
              onChange={(e) => setDPI(Number(e.target.value))}
            >
              {!available.resolutions.includes(dpi) && (
                <option value={dpi}>{dpi} DPI — not confirmed</option>
              )}
              {available.resolutions.map((dpi) => (
                <option key={dpi} value={dpi}>
                  {dpi} DPI
                </option>
              ))}
            </select>
          </Field>
          <Field label="File format">
            <select
              disabled={!!profile}
              value={selectedFormat}
              onChange={(e) => setFormat(e.target.value as Format)}
            >
              <option value="png">PNG · lossless</option>
              <option value="tiff">TIFF · lossless</option>
            </select>
          </Field>
        </div>
        <label className="toggle-row">
          <span>
            <strong>Scan both sides</strong>
            <small>
              {available.duplex
                ? "Duplex supported by this scanner"
                : "Duplex is unavailable or not yet confirmed"}
            </small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={useDuplex}
            disabled={!!profile || !available.duplex}
            onChange={(e) => setDuplex(e.target.checked)}
          />
        </label>
        <p className="muted small">
          Current capture settings: A4 · RGB colour · 8-bit. Card feeding and
          front/back pairing still need validation.
        </p>
        <label className="acknowledge">
          <input
            type="checkbox"
            checked={ready}
            onChange={(e) => setReady(e.target.checked)}
          />
          <span>
            I have checked the feeder setup and loaded ordinary paper for this
            scan.
          </span>
        </label>
      </fieldset>
      <ErrorNote
        error={
          devices.error ||
          status.error ||
          inspection.error ||
          start.error ||
          defaults.error
        }
      />
      {inspection.isFetching && (
        <Loading label="Reading scanner capabilities…" />
      )}
      {profile && !configured && !inspection.isFetching && (
        <p className="notice">
          This profile is not supported by the current capability report. Choose
          custom settings or edit the profile.
        </p>
      )}
      {pending ? (
        <div className="notice">
          <strong>Confirm the previous request</strong>
          <p>
            A response may have been lost. Retrying uses the same request ID, so
            it cannot create a duplicate batch.
          </p>
          <button
            className="button primary"
            disabled={start.isPending || !online}
            onClick={() => start.mutate(pending)}
          >
            <RefreshCw size={16} />
            Retry same request
          </button>
        </div>
      ) : (
        <button
          className="button primary full"
          disabled={
            !configured || !ready || !online || hardwareBusy || start.isPending
          }
          onClick={begin}
        >
          <ScanLine size={18} />
          {hardwareBusy
            ? "Scanner is busy"
            : start.isPending
              ? "Starting…"
              : "Start scan"}
          <ArrowUpRight size={18} />
        </button>
      )}
      <div className="panel-footer">
        <button
          className="text-button"
          disabled={!deviceID || hardwareBusy || inspection.isFetching}
          onClick={() => {
            setReady(false);
            void inspection.refetch();
          }}
        >
          Refresh capabilities
        </button>
        <button
          className="text-button"
          disabled={
            !configured || hardwareBusy || defaults.isPending || !!pending
          }
          onClick={() => defaults.mutate()}
        >
          Save as defaults
        </button>
      </div>
      {defaults.isSuccess && (
        <p className="success-text" role="status">
          Defaults saved for future jobs.
        </p>
      )}
    </section>
  );
}
