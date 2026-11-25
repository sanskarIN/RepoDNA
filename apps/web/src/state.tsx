// Application state: the backend, the analysis being explored, the analyses opened recently
// in this browser, and preferences.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { parseArtifact, type RepositoryDna } from "@repodna/schema";
import type { Theme } from "@repodna/visualization";
import { detectBackend, type Backend, type Session } from "./lib/backend";
import { loadDemo } from "./lib/demo";
import { keepOffline } from "./lib/offline";
import {
  browserStore,
  openPointer,
  rememberRecent,
  REMEMBER_KEY,
  RECENT_MAX_BYTES,
  setOpenPointer,
  setRememberRecent,
  type OpenPointer,
  type RecentEntry,
  type RecentStore,
} from "./lib/recent";
import { useTheme, type ThemePreference } from "./lib/theme";

export type Origin =
  | { kind: "stored"; repositoryId: string; scanId?: string; name: string }
  | { kind: "file"; name: string }
  | { kind: "demo"; title: string; file?: string };

export interface Dataset {
  dna: RepositoryDna;
  origin: Origin;
  warnings: string[];
}

/** The file an analysis was read from, so that it can be kept as a recent analysis. */
export interface SourceFile {
  text: string;
  size: number;
}

export interface AppState {
  ready: boolean;
  error: string | null;
  backend: Backend | null;
  session: Session | null;
  signInNeeded: boolean;
  dataset: Dataset | null;
  /** Shows an analysis; with `source`, keeps it as a recent analysis in this browser. */
  open(dataset: Dataset, source?: SourceFile): void;
  close(): void;
  /** Analyses opened from files in this browser, newest first. */
  recent: RecentEntry[];
  /** Whether recent analyses are kept in this browser. */
  remember: boolean;
  /** Whether this store can reopen a retained file after a page reload. */
  recentCanReload: boolean;
  setRemember(on: boolean): void;
  openRecent(entry: RecentEntry): Promise<void>;
  forget(id: string): void;
  forgetAll(): void;
  theme: Theme;
  themePreference: ThemePreference;
  setThemePreference(preference: ThemePreference): void;
}

const Context = createContext<AppState | null>(null);

/** What to keep in the tab's session storage so that a reload opens `dataset` again. */
function pointerFor(dataset: Dataset, kept: boolean): OpenPointer | null {
  const origin = dataset.origin;
  switch (origin.kind) {
    case "stored":
      return {
        kind: "stored",
        repositoryId: origin.repositoryId,
        scanId: origin.scanId,
        name: origin.name,
      };
    case "demo":
      return origin.file ? { kind: "demo", file: origin.file, title: origin.title } : null;
    case "file":
      return kept ? { kind: "file", id: dataset.dna.analysisMetadata.id } : null;
  }
}

/** Opens the analysis a tab had open before it was reloaded, or null when it is gone. */
async function restore(
  pointer: OpenPointer,
  backend: Backend | null,
  store: RecentStore,
): Promise<Dataset | null> {
  switch (pointer.kind) {
    case "stored": {
      if (!backend) {
        return null;
      }
      const dna = await backend.artifact(pointer.repositoryId, pointer.scanId);
      return {
        dna,
        origin: {
          kind: "stored",
          repositoryId: pointer.repositoryId,
          scanId: pointer.scanId,
          name: pointer.name,
        },
        warnings: [],
      };
    }
    case "demo": {
      const loaded = await loadDemo({ file: pointer.file, title: pointer.title, description: "" });
      return {
        dna: loaded.artifact,
        origin: { kind: "demo", title: pointer.title, file: pointer.file },
        warnings: loaded.warnings,
      };
    }
    case "file": {
      // The session pointer alone does not authorize restoring a file:
      // older buggy versions could leave orphan text after deleting metadata.
      if (!rememberRecent()) return null;
      const entry = (await store.list()).find((item) => item.id === pointer.id);
      if (!entry) return null;
      const text = await store.text(pointer.id);
      if (text === null || !rememberRecent()) return null;
      const loaded = parseArtifact(text);
      if (loaded.artifact.analysisMetadata.id !== pointer.id) return null;
      return {
        dna: loaded.artifact,
        origin: { kind: "file", name: entry.name },
        warnings: loaded.warnings,
      };
    }
  }
}

export function AppProvider({
  children,
  initial = null,
  store: givenStore,
}: {
  children: ReactNode;
  /** An analysis to open at start (tests and embedding). */
  initial?: Dataset | null;
  /** Where recent analyses are kept (tests); the browser's storage otherwise. */
  store?: RecentStore;
}) {
  const [themePreference, setThemePreference, theme] = useTheme();
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [backend, setBackend] = useState<Backend | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [signInNeeded, setSignInNeeded] = useState(false);
  const [dataset, setDataset] = useState<Dataset | null>(initial);
  const [store] = useState<RecentStore>(() => givenStore ?? browserStore());
  const recentCanReload = store.persistsAcrossReload !== false;
  const [recent, setRecent] = useState<RecentEntry[]>([]);
  const [remember, setRememberState] = useState(rememberRecent);
  // Serialize recent-history writes in this tab, including clearing history.
  // This prevents a pending file save from resurrecting data after opt-out.
  const writeQueue = useRef<Promise<void>>(Promise.resolve());
  const retentionEpoch = useRef(0);
  const selectionEpoch = useRef(0);
  const selectedFileId = useRef<string | null>(null);
  const pendingRecentId = useRef<string | null>(null);
  const pendingRecentSelection = useRef<number | null>(null);
  const refreshSequence = useRef(0);
  const enqueueWrite = useCallback((write: () => Promise<void>): Promise<void> => {
    const pending = writeQueue.current.then(write);
    // A failed write must not permanently block a later delete/clear.
    writeQueue.current = pending.catch(() => {});
    return pending;
  }, []);
  // An analysis given at start replaces the one a reloaded tab had open.
  const [openedAtStart] = useState(initial !== null);

  const refresh = useCallback(() => {
    const sequence = ++refreshSequence.current;
    void store.list().then(
      (entries) => {
        // A slower earlier read must not repopulate data after a later clear.
        if (sequence === refreshSequence.current) {
          setRecent(rememberRecent() ? entries : []);
        }
      },
      () => {
        if (sequence === refreshSequence.current) setRecent([]);
      },
    );
  }, [store]);

  useEffect(() => {
    let cancelled = false;
    const start = async () => {
      let detected: Awaited<ReturnType<typeof detectBackend>> | null = null;
      try {
        detected = await detectBackend();
        if (!cancelled) {
          setBackend(detected.backend);
          setSession(detected.session);
          setSignInNeeded(detected.signInNeeded);
          keepOffline(detected.backend !== null || detected.signInNeeded);
        }
      } catch (reason) {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      }
      // A reload keeps the analysis the tab had open.
      const pointer = openedAtStart ? null : openPointer();
      if (pointer) {
        const selection = selectionEpoch.current;
        try {
          const restored = await restore(pointer, detected?.backend ?? null, store);
          if (!cancelled && selection === selectionEpoch.current) {
            if (restored && (pointer.kind !== "file" || rememberRecent())) {
              selectedFileId.current = pointer.kind === "file" ? pointer.id : null;
              setDataset(restored);
            } else {
              setOpenPointer(null);
            }
          }
        } catch {
          if (!cancelled && selection === selectionEpoch.current) {
            setOpenPointer(null);
          }
        }
      }
      if (!cancelled) {
        setReady(true);
      }
    };
    void start();
    if (rememberRecent()) {
      refresh();
    } else {
      // Retention could have been disabled just before a tab closed, before
      // its asynchronous deletion committed. Finish the clear on startup.
      setRecent([]);
      void enqueueWrite(() => store.clear()).then(refresh, refresh);
    }
    return () => {
      cancelled = true;
      refreshSequence.current += 1;
    };
  }, [store, refresh, enqueueWrite, openedAtStart]);

  const open = useCallback(
    (next: Dataset, source?: SourceFile) => {
      const selection = ++selectionEpoch.current;
      pendingRecentId.current = null;
      pendingRecentSelection.current = null;
      selectedFileId.current = next.origin.kind === "file" ? next.dna.analysisMetadata.id : null;
      setDataset(next);
      const keep =
        remember &&
        rememberRecent() &&
        next.origin.kind === "file" &&
        source !== undefined &&
        source.size <= RECENT_MAX_BYTES;
      // File pointers must not outlive their saved text. Set one only when
      // persistence succeeds for the still-selected analysis.
      setOpenPointer(pointerFor(next, false));
      if (keep && source && next.origin.kind === "file") {
        const epoch = retentionEpoch.current;
        const metadata = next.dna.analysisMetadata;
        const entry: RecentEntry = {
          id: metadata.id,
          name: next.origin.name,
          repository: next.dna.identity.name,
          generatedAt: metadata.generatedAt,
          openedAt: new Date().toISOString(),
          size: source.size,
        };
        void enqueueWrite(async () => {
          if (epoch !== retentionEpoch.current || !rememberRecent()) return;
          await store.add(entry, source.text);
        }).then(
          () => {
            if (
              epoch === retentionEpoch.current &&
              selection === selectionEpoch.current &&
              store.persistsAcrossReload !== false &&
              rememberRecent()
            ) {
              setOpenPointer(pointerFor(next, true));
            }
            refresh();
          },
          refresh,
        );
      }
    },
    [remember, store, refresh, enqueueWrite],
  );
  const close = useCallback(() => {
    selectionEpoch.current += 1;
    selectedFileId.current = null;
    pendingRecentId.current = null;
    pendingRecentSelection.current = null;
    setDataset(null);
    setOpenPointer(null);
  }, []);

  const openRecent = useCallback(
    async (entry: RecentEntry) => {
      // A slow storage read must not reopen a file deleted in the meantime,
      // override a newer selection, or re-enable a disabled retention choice.
      const selection = ++selectionEpoch.current;
      const retention = retentionEpoch.current;
      pendingRecentId.current = entry.id;
      pendingRecentSelection.current = selection;
      try {
        const text = await store.text(entry.id);
        if (
          selection !== selectionEpoch.current ||
          retention !== retentionEpoch.current ||
          !rememberRecent()
        ) {
          return;
        }
        if (text === null) {
          await enqueueWrite(() => store.remove(entry.id));
          refresh();
          throw new Error(`${entry.name} is no longer kept in this browser.`);
        }
        const loaded = parseArtifact(text);
        if (loaded.artifact.analysisMetadata.id !== entry.id) {
          throw new Error(`${entry.name} does not match its saved analysis metadata.`);
        }
        open(
          {
            dna: loaded.artifact,
            origin: { kind: "file", name: entry.name },
            warnings: loaded.warnings,
          },
          { text, size: entry.size },
        );
      } finally {
        // Two overlapping reads may refer to the same ID. The older read
        // must never clear the newer read's pending selection marker.
        if (pendingRecentSelection.current === selection) {
          pendingRecentId.current = null;
          pendingRecentSelection.current = null;
        }
      }
    },
    [store, open, refresh, enqueueWrite],
  );
  const clearFilePointer = useCallback((id?: string) => {
    const pointer = openPointer();
    const matchesPointer = pointer?.kind === "file" && (id === undefined || pointer.id === id);
    if (matchesPointer) {
      setOpenPointer(null);
    }
    if (
      matchesPointer ||
      id === undefined ||
      selectedFileId.current === id ||
      pendingRecentId.current === id
    ) {
      selectionEpoch.current += 1;
      pendingRecentId.current = null;
      pendingRecentSelection.current = null;
    }
  }, []);
  const forget = useCallback(
    (id: string) => {
      clearFilePointer(id);
      void enqueueWrite(() => store.remove(id)).then(refresh, refresh);
    },
    [store, refresh, enqueueWrite, clearFilePointer],
  );
  const forgetAll = useCallback(() => {
    retentionEpoch.current += 1;
    clearFilePointer();
    refreshSequence.current += 1;
    setRecent([]);
    void enqueueWrite(() => store.clear()).then(refresh, refresh);
  }, [store, refresh, enqueueWrite, clearFilePointer]);
  const setRemember = useCallback(
    (on: boolean) => {
      setRememberRecent(on);
      setRememberState(on);
      if (!on) {
        retentionEpoch.current += 1;
        clearFilePointer();
        refreshSequence.current += 1;
        setRecent([]);
        void enqueueWrite(() => store.clear()).then(refresh, refresh);
      }
    },
    [store, refresh, enqueueWrite, clearFilePointer],
  );

  useEffect(() => {
    // localStorage preference changes are broadcast to other same-origin tabs.
    // Keep the UI in sync, and finish any pending file removal in this tab.
    const onStorage = (event: StorageEvent) => {
      if (event.key !== REMEMBER_KEY && event.key !== null) return;
      const enabled = rememberRecent();
      setRememberState(enabled);
      if (enabled) {
        refresh();
      } else {
        retentionEpoch.current += 1;
        clearFilePointer();
        refreshSequence.current += 1;
        setRecent([]);
        void enqueueWrite(() => store.clear()).then(refresh, refresh);
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, [store, refresh, enqueueWrite, clearFilePointer]);

  const value = useMemo<AppState>(
    () => ({
      ready,
      error,
      backend,
      session,
      signInNeeded,
      dataset,
      open,
      close,
      recent,
      remember,
      recentCanReload,
      setRemember,
      openRecent,
      forget,
      forgetAll,
      theme,
      themePreference,
      setThemePreference,
    }),
    [
      ready,
      error,
      backend,
      session,
      signInNeeded,
      dataset,
      open,
      close,
      recent,
      remember,
      recentCanReload,
      setRemember,
      openRecent,
      forget,
      forgetAll,
      theme,
      themePreference,
      setThemePreference,
    ],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useApp(): AppState {
  const value = useContext(Context);
  if (!value) {
    throw new Error("useApp must be used inside AppProvider");
  }
  return value;
}

/** The analysis being explored; views that need one render only when it exists. */
export function useDataset(): Dataset {
  const { dataset } = useApp();
  if (!dataset) {
    throw new Error("No analysis is open.");
  }
  return dataset;
}

/** A short description of where the open analysis came from. */
export function originLabel(dataset: Dataset): string {
  const date = dataset.dna.analysisMetadata.generatedAt?.slice(0, 10) ?? "";
  switch (dataset.origin.kind) {
    case "stored":
      return `Stored analysis · ${date}`;
    case "file":
      return `Snapshot from ${dataset.origin.name} · ${date}`;
    case "demo":
      return `Demo · ${date}`;
  }
}
