import { useEffect, useRef, useState } from "react";
import type { Backend, JobState, RemoteRevision } from "./backend";

const POLL_DELAY = 400;
const MAX_FAILURES = 3;

interface ActiveScan {
  id: string | null;
  timer: number | null;
  polling: boolean;
  failures: number;
  cancelling: boolean;
}

type OpenResult = (repositoryId: string, scanId?: string) => void | Promise<void>;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function clearTimer(scan: ActiveScan) {
  if (scan.timer !== null) window.clearTimeout(scan.timer);
  scan.timer = null;
}

/** A single-flight native scan with bounded polling retries and manual recovery.
 * Leaving the view stops its polling, not the analysis in the native process.
 */
export function useScanJob(
  backend: Pick<Backend, "startScan" | "job" | "cancelJob"> | null,
  onDone: OpenResult,
) {
  const [job, setJob] = useState<JobState | null>(null);
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [paused, setPaused] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pollError, setPollError] = useState<string | null>(null);
  const active = useRef<ActiveScan | null>(null);
  const mounted = useRef(false);
  const openResult = useRef(onDone);

  useEffect(() => {
    openResult.current = onDone;
  }, [onDone]);

  useEffect(() => {
    mounted.current = true;
    setJob(null);
    setStarting(false);
    setCancelling(false);
    setPaused(false);
    setError(null);
    setPollError(null);
    return () => {
      mounted.current = false;
      if (active.current) clearTimer(active.current);
      active.current = null;
    };
  }, [backend]);

  const current = (scan: ActiveScan) => mounted.current && active.current === scan;

  const accept = (scan: ActiveScan, next: JobState) => {
    if (!current(scan)) return;
    setJob(next);
    setStarting(false);
    setPollError(null);
    setPaused(false);
    scan.failures = 0;
    if (next.status === "running") {
      schedule(scan, POLL_DELAY);
      return;
    }
    clearTimer(scan);
    active.current = null;
    setCancelling(false);
    if (next.status === "completed") {
      if (next.repositoryId) {
        const repositoryId = next.repositoryId;
        const open = async () => {
          try {
            await openResult.current(repositoryId, next.scanId ?? undefined);
          } catch (reason) {
            if (mounted.current && active.current === null) setError(message(reason));
          }
        };
        void open();
      } else {
        setError(
          "Analysis finished but no saved result is available. Check its warnings or local storage.",
        );
      }
    } else if (next.status === "failed") {
      setError(next.error ?? "The analysis failed.");
    }
  };

  function schedule(scan: ActiveScan, delay: number) {
    if (!current(scan) || !scan.id || scan.timer !== null || scan.polling || !backend) return;
    const id = scan.id;
    scan.timer = window.setTimeout(async () => {
      scan.timer = null;
      if (!current(scan)) return;
      scan.polling = true;
      try {
        const next = await backend.job(id);
        if (!current(scan)) return;
        scan.polling = false;
        if (next.id !== id) throw new Error("The backend returned a different analysis job.");
        accept(scan, next);
      } catch (reason) {
        if (!current(scan)) return;
        scan.polling = false;
        scan.failures += 1;
        setPollError(message(reason));
        if (scan.failures >= MAX_FAILURES) {
          setPaused(true);
        } else {
          schedule(scan, POLL_DELAY * 2 ** scan.failures);
        }
      }
    }, delay);
  }

  const launch = async (operation: () => Promise<JobState>, expectedId?: string) => {
    // State updates are asynchronous: the ref guards a second submission even
    // before React has rendered the disabled button.
    if (!backend || !mounted.current || active.current) return;
    const scan: ActiveScan = {
      id: null,
      timer: null,
      polling: false,
      failures: 0,
      cancelling: false,
    };
    active.current = scan;
    setJob(null);
    setError(null);
    setPollError(null);
    setStarting(true);
    setPaused(false);
    try {
      const next = await operation();
      if (!current(scan)) return;
      if (expectedId !== undefined && expectedId !== next.id) {
        throw new Error("The backend returned a different analysis job.");
      }
      scan.id = next.id;
      accept(scan, next);
    } catch (reason) {
      if (!current(scan)) return;
      active.current = null;
      setStarting(false);
      setError(message(reason));
    }
  };

  const start = (input: string, profile: string, revision?: RemoteRevision) =>
    backend ? launch(() => backend.startScan(input, profile, revision)) : Promise.resolve();

  const attach = (id: string) => (backend ? launch(() => backend.job(id), id) : Promise.resolve());

  const resume = () => {
    const scan = active.current;
    if (!scan || !scan.id || !current(scan)) return;
    scan.failures = 0;
    setPaused(false);
    setPollError(null);
    schedule(scan, 0);
  };

  const stop = async () => {
    const scan = active.current;
    if (!backend || !scan || !scan.id || scan.cancelling || !current(scan)) return;
    scan.cancelling = true;
    setCancelling(true);
    setError(null);
    try {
      await backend.cancelJob(scan.id);
      if (!current(scan)) return;
      // The cancellation request is not confirmation that the job stopped.
      // Keep observing the same ID until its actual terminal status arrives.
      resume();
    } catch (reason) {
      if (!current(scan)) return;
      scan.cancelling = false;
      setCancelling(false);
      setError(message(reason));
    }
  };

  return {
    job,
    starting,
    busy: starting || job?.status === "running",
    cancelling,
    paused,
    error,
    pollError,
    start,
    attach,
    resume,
    stop,
  };
}
