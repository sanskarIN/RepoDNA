import { useEffect, useRef, useState } from "react";
import type { RemoteRevision } from "./backend";
import {
  EMPTY_SCAN_JOB,
  ScanJobController,
  type OpenScanResult,
  type ScanBackend,
} from "./scanJobController";

/** Leaving the view stops observation, not the analysis in the native process. */
export function useScanJob(backend: ScanBackend | null, onDone: OpenScanResult) {
  const [state, setState] = useState(EMPTY_SCAN_JOB);
  const controller = useRef<ScanJobController | null>(null);
  const openResult = useRef(onDone);

  useEffect(() => {
    openResult.current = onDone;
  }, [onDone]);

  useEffect(() => {
    setState(EMPTY_SCAN_JOB);
    if (!backend) return;
    const instance = new ScanJobController(
      backend,
      (id, scan) => openResult.current(id, scan),
      setState,
    );
    controller.current = instance;
    return () => {
      instance.dispose();
      if (controller.current === instance) controller.current = null;
    };
  }, [backend]);

  return {
    ...state,
    start: (input: string, profile: string, revision?: RemoteRevision) =>
      controller.current?.start(input, profile, revision) ?? Promise.resolve(),
    attach: (id: string) => controller.current?.attach(id) ?? Promise.resolve(),
    resume: () => controller.current?.resume(),
    stop: () => controller.current?.stop() ?? Promise.resolve(),
  };
}
