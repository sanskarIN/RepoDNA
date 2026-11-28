import { useLayoutEffect, useRef, useState } from "react";
import {
  ReportOperation,
  type OperationOptions,
  type ReportOperationState,
  type ReportTask,
} from "./reportOperation";

/** Tie an operation to the exact artifact/selection identity rather than a
 * display name or analysis ID that could be reused by a different document. */
export function useReportOperation<T>(source: unknown, discard?: (value: T) => void) {
  const [snapshot, setSnapshot] = useState<{ source: unknown; state: ReportOperationState<T> }>({
    source,
    state: { status: "idle" },
  });
  const controller = useRef<{ source: unknown; instance: ReportOperation<T> } | null>(null);
  const disposer = useRef(discard);
  disposer.current = discard;
  useLayoutEffect(() => {
    const instance = new ReportOperation<T>(
      (state) => setSnapshot({ source, state }),
      undefined,
      (value) => disposer.current?.(value),
    );
    controller.current = { source, instance };
    setSnapshot({ source, state: { status: "idle" } });
    return () => {
      instance.dispose();
      if (controller.current?.instance === instance) controller.current = null;
    };
  }, [source]);
  const current = () => {
    const active = controller.current;
    return active && active.source === source ? active.instance : null;
  };
  const state: ReportOperationState<T> = snapshot.source === source ? snapshot.state : { status: "idle" };
  return {
    state,
    start: (label: string, make: () => ReportTask<T>, options?: OperationOptions, onReady?: (value: T) => void) =>
      current()?.start(label, make, options, onReady) ?? false,
    cancel: () => current()?.cancel() ?? false,
    reset: () => current()?.reset(),
  };
}
