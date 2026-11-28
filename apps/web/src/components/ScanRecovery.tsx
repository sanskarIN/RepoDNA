import { useState } from "react";
import type { JobState } from "../lib/backend";

interface Props {
  load: () => Promise<JobState[]>;
  onChoose: (id: string) => void;
  disabled: boolean;
}

type Listing =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; jobs: JobState[] }
  | { kind: "error"; message: string };

const STATUS_LABELS = {
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
} as const;

/** Recover a job after leaving the analysis page. Results are deliberately
 * fetched on demand and refreshed by ID before the scan controller adopts it.
 */
export function ScanRecovery({ load, onChoose, disabled }: Props) {
  const [listing, setListing] = useState<Listing>({ kind: "idle" });

  const refresh = async () => {
    setListing({ kind: "loading" });
    try {
      setListing({ kind: "ready", jobs: await load() });
    } catch (error) {
      setListing({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  };

  return (
    <div className="panel" aria-label="Recent native analyses">
      <button
        type="button"
        disabled={disabled || listing.kind === "loading"}
        onClick={() => void refresh()}
      >
        {listing.kind === "loading" ? "Finding analyses…" : "Find recent analyses"}
      </button>
      <p className="muted">
        Reconnect to jobs still known by this native process. Restarting it clears the job list,
        but saved analysis results remain in the repository library.
      </p>
      {listing.kind === "error" ? <p role="alert">{listing.message}</p> : null}
      {listing.kind === "ready" ? (
        listing.jobs.length ? (
          <ul className="evidence">
            {listing.jobs.map((job) => (
              <li key={job.id}>
                <span>
                  {job.input} · {STATUS_LABELS[job.status]}
                </span>{" "}
                <button type="button" disabled={disabled} onClick={() => onChoose(job.id)}>
                  {job.status === "running" ? "Monitor analysis" : "View analysis status"}
                  <span className="visually-hidden"> {job.id}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p role="status">No recent native analysis jobs.</p>
        )
      ) : null}
    </div>
  );
}
