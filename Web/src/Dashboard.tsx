import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useLocation } from "wouter";
import {
  ArrowRight,
  CircleStop,
  Layers3,
  ScanLine,
  ShieldCheck,
} from "lucide-react";
import { useAuth } from "./auth";
import { useWorkspace } from "./data";
import { batchLabel, dateLabel, isTerminal, type Job } from "./models";
import { Empty, ErrorNote, Loading, StateBadge } from "./ui";
import { ScanPanel } from "./ScanPanel";

export function JobRow({ job }: { job: Job }) {
  return (
    <Link href={`/batches/${job.id}`} className="job-row">
      <span className="job-icon">
        <Layers3 size={20} />
      </span>
      <span className="job-info">
        <strong>{batchLabel(job)}</strong>
        <small>
          {dateLabel(job.createdAt)} · {job.progress.imagesSaved} images
        </small>
      </span>
      <StateBadge state={job.state} />
      <ArrowRight size={16} className="muted" />
    </Link>
  );
}
export function CancelJob({ id }: { id: string }) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const cancel = useMutation({
    mutationFn: () => api.send(`jobs/${id}/cancel`, "POST"),
    onSuccess: () => {
      void queries.invalidateQueries({ queryKey: ["jobs"] });
      void queries.invalidateQueries({ queryKey: ["batch", id] });
      void queries.invalidateQueries({ queryKey: ["status"] });
    },
  });
  return (
    <>
      <button
        className="button danger"
        disabled={cancel.isPending}
        onClick={() => cancel.mutate()}
      >
        <CircleStop size={17} />
        {cancel.isPending ? "Cancelling…" : "Cancel job"}
      </button>
      <ErrorNote error={cancel.error} />
    </>
  );
}
export function Dashboard() {
  const { jobs, status, devices } = useWorkspace();
  const [, navigate] = useLocation();
  const active = jobs.data?.jobs.filter((job) => !isTerminal(job.state)) || [];
  return (
    <>
      <header className="page-heading">
        <div>
          <p className="eyebrow">YOUR SCANNING WORKSPACE</p>
          <h1>
            {status.isError
              ? "Let’s reconnect."
              : status.data?.activeJobID
                ? "Capture in progress."
                : "Ready for the next batch."}
          </h1>
          <p>Scan, review and find every image. All in one place.</p>
        </div>
        <span className="date-pill">
          {new Intl.DateTimeFormat(undefined, {
            weekday: "short",
            day: "numeric",
            month: "short",
          }).format(new Date())}
        </span>
      </header>
      <div className="summary-grid">
        <div>
          <ScanLine size={20} />
          <span>
            <small>SCANNERS CONNECTED</small>
            <strong>
              {devices.isError || !devices.data
                ? "—"
                : devices.data.devices.length}
            </strong>
          </span>
        </div>
        <div>
          <Layers3 size={20} />
          <span>
            <small>JOBS IN QUEUE</small>
            <strong>
              {status.isError || !status.data ? "—" : status.data.queuedJobs}
            </strong>
          </span>
        </div>
        <div>
          <ShieldCheck size={20} />
          <span>
            <small>ORIGINALS</small>
            <strong className="summary-word">Preserved</strong>
          </span>
        </div>
      </div>
      <div className="dashboard-grid">
        <ScanPanel onStarted={(id) => navigate(`/batches/${id}`)} />
        <div className="dashboard-right">
          <section className="status-panel">
            <div className="section-heading">
              <p className="eyebrow">SCANNER SERVICE</p>
              <span
                className={`status-light ${status.isError ? "offline" : ""}`}
              />
            </div>
            <h2>
              {!status.data
                ? "Connecting…"
                : status.isError
                  ? "Connection lost"
                  : status.data.activeJobID
                    ? "Working on your batch"
                    : status.data.paused
                      ? "Paused"
                      : "Standing by"}
            </h2>
            <p>
              {devices.data?.devices.map((d) => d.name).join(", ") ||
                "Connect a scanner to get started."}
            </p>
            <div className="storage-line">
              <span>SAVE DESTINATION</span>
              <strong>
                {status.data?.storageRoot || "Waiting for service"}
              </strong>
            </div>
          </section>
          {!!active.length && (
            <section className="panel">
              <div className="section-heading">
                <h2>In progress</h2>
                <span className="badge">{active.length}</span>
              </div>
              {active.map((job) => (
                <div key={job.id} className="active-job">
                  <JobRow job={job} />
                  <CancelJob id={job.id} />
                </div>
              ))}
            </section>
          )}
          <section className="panel recent-panel">
            <div className="section-heading">
              <h2>Recent batches</h2>
              <Link href="/library" className="text-button">
                View library <ArrowRight size={14} />
              </Link>
            </div>
            <ErrorNote
              error={
                jobs.error ||
                status.error ||
                status.data?.persistenceError?.message
              }
            />
            {jobs.isPending ? (
              <Loading />
            ) : jobs.data?.jobs.length ? (
              jobs.data.jobs
                .slice(0, 5)
                .map((job) => <JobRow key={job.id} job={job} />)
            ) : (
              <Empty title="Your first batch starts here">
                Scanned images will appear in your library.
              </Empty>
            )}
          </section>
          <div className="workflow-note">
            <span>01</span>
            <p>
              <strong>A place for every scan.</strong>
              <br />
              Permanent batch numbers match your boxes. Dates stay in the scan
              details.
            </p>
          </div>
        </div>
      </div>
    </>
  );
}
