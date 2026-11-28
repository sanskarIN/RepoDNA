import { useCallback, useEffect, useRef, useState } from "react";
import type { Backend, ScanSummary } from "./backend";

interface HistoryState {
  id: string | null;
  scans: ScanSummary[] | null;
  cursor: string | null;
  loading: boolean;
  error: string | null;
}

const EMPTY: HistoryState = {
  id: null,
  scans: null,
  cursor: null,
  loading: false,
  error: null,
};

/** Keeps pagination scoped to the selected repository and backend. Old
 * responses cannot overwrite a new selection or a refreshed history. */
export function useStoredHistory(backend: Backend | null) {
  const [state, setState] = useState<HistoryState>(EMPTY);
  const snapshot = useRef(state);
  const generation = useRef(0);
  const publish = useCallback((next: HistoryState) => {
    snapshot.current = next;
    setState(next);
  }, []);

  const reset = useCallback(() => {
    generation.current += 1;
    publish(EMPTY);
  }, [publish]);

  useEffect(() => {
    reset();
    return () => {
      generation.current += 1;
    };
  }, [backend, reset]);

  const load = async (id: string, append: boolean) => {
    if (!backend) return;
    const previous = snapshot.current;
    if (append && (previous.id !== id || previous.loading || !previous.cursor)) return;
    const cursor = append ? previous.cursor : null;
    const token = ++generation.current;
    const initial: HistoryState = {
      id,
      scans: append ? previous.scans : null,
      cursor,
      loading: true,
      error: null,
    };
    publish(initial);
    try {
      const page = backend.history
        ? await backend.history(id, cursor ?? undefined)
        : { scans: (await backend.repository(id)).scans, nextCursor: null };
      if (generation.current !== token) return;
      const seen = new Set((initial.scans ?? []).map((scan) => scan.id));
      const added = page.scans.filter((scan) => {
        if (seen.has(scan.id)) return false;
        seen.add(scan.id);
        return true;
      });
      if (page.nextCursor && (page.nextCursor === cursor || added.length === 0)) {
        throw new Error("History pagination did not advance. Refresh the history to retry.");
      }
      publish({
        id,
        scans: [...(initial.scans ?? []), ...added],
        cursor: page.nextCursor,
        loading: false,
        error: null,
      });
    } catch (reason) {
      if (generation.current !== token) return;
      publish({
        ...initial,
        loading: false,
        error: reason instanceof Error ? reason.message : String(reason),
      });
    }
  };

  const toggle = (id: string) => {
    if (snapshot.current.id === id) reset();
    else void load(id, false);
  };

  const more = () => {
    if (snapshot.current.id) void load(snapshot.current.id, true);
  };

  const refresh = async (id: string) => {
    if (snapshot.current.id === id) await load(id, false);
  };

  return { ...state, toggle, more, refresh, reset };
}
