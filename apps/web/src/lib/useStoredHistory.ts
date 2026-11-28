import { useEffect, useRef, useState } from "react";
import type { Backend } from "./backend";
import { EMPTY_HISTORY, StoredHistoryController } from "./storedHistoryController";

/** Keep history and its request deadlines scoped to the selected backend. */
export function useStoredHistory(backend: Backend | null, initialRepositoryId?: string | null) {
  const [state, setState] = useState(EMPTY_HISTORY);
  const controller = useRef<StoredHistoryController | null>(null);

  useEffect(() => {
    setState(EMPTY_HISTORY);
    if (!backend) return;
    const instance = new StoredHistoryController(backend, setState);
    controller.current = instance;
    if (initialRepositoryId) instance.select(initialRepositoryId);
    return () => {
      instance.dispose();
      if (controller.current === instance) controller.current = null;
    };
  }, [backend, initialRepositoryId]);

  return {
    ...state,
    toggle: (id: string) => controller.current?.toggle(id),
    more: () => controller.current?.more(),
    refresh: (id: string) => controller.current?.refresh(id) ?? Promise.resolve(),
    reset: () => controller.current?.reset(),
  };
}
