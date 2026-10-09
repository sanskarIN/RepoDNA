// Application state: the backend, the analysis being explored, the analyses opened recently
// in this browser, and preferences.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { parseArtifact, type RepositoryDna } from "@repodna/schema";
import type { Theme } from "@repodna/visualization";
import { detectBackend, type Backend, type Session } from "./lib/backend";
import { loadDemo } from "./lib/demo";
import {
  browserStore,
  openPointer,
  rememberRecent,
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
      const text = await store.text(pointer.id);
      if (text === null) {
        return null;
      }
      const entry = (await store.list()).find((item) => item.id === pointer.id);
      const loaded = parseArtifact(text);
      return {
        dna: loaded.artifact,
        origin: { kind: "file", name: entry?.name ?? "analysis file" },
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
  const [recent, setRecent] = useState<RecentEntry[]>([]);
  const [remember, setRememberState] = useState(rememberRecent);
  // An analysis given at start replaces the one a reloaded tab had open.
  const [openedAtStart] = useState(initial !== null);

  const refresh = useCallback(() => {
    store.list().then(setRecent, () => setRecent([]));
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
        }
      } catch (reason) {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      }
      // A reload keeps the analysis the tab had open.
      const pointer = openedAtStart ? null : openPointer();
      if (pointer) {
        try {
          const restored = await restore(pointer, detected?.backend ?? null, store);
          if (!cancelled) {
            if (restored) {
              setDataset(restored);
            } else {
              setOpenPointer(null);
            }
          }
        } catch {
          setOpenPointer(null);
        }
      }
      if (!cancelled) {
        setReady(true);
      }
    };
    void start();
    refresh();
    return () => {
      cancelled = true;
    };
  }, [store, refresh, openedAtStart]);

  const open = useCallback(
    (next: Dataset, source?: SourceFile) => {
      setDataset(next);
      const keep =
        remember &&
        next.origin.kind === "file" &&
        source !== undefined &&
        source.size <= RECENT_MAX_BYTES;
      setOpenPointer(pointerFor(next, keep));
      if (keep && source && next.origin.kind === "file") {
        const metadata = next.dna.analysisMetadata;
        store
          .add(
            {
              id: metadata.id,
              name: next.origin.name,
              repository: next.dna.identity.name,
              generatedAt: metadata.generatedAt,
              openedAt: new Date().toISOString(),
              size: source.size,
            },
            source.text,
          )
          .then(refresh, refresh);
      }
    },
    [remember, store, refresh],
  );
  const close = useCallback(() => {
    setDataset(null);
    setOpenPointer(null);
  }, []);

  const openRecent = useCallback(
    async (entry: RecentEntry) => {
      const text = await store.text(entry.id);
      if (text === null) {
        await store.remove(entry.id);
        refresh();
        throw new Error(`${entry.name} is no longer kept in this browser.`);
      }
      const loaded = parseArtifact(text);
      open(
        {
          dna: loaded.artifact,
          origin: { kind: "file", name: entry.name },
          warnings: loaded.warnings,
        },
        { text, size: entry.size },
      );
    },
    [store, open, refresh],
  );
  const forget = useCallback(
    (id: string) => {
      store.remove(id).then(refresh, refresh);
    },
    [store, refresh],
  );
  const forgetAll = useCallback(() => {
    store.clear().then(refresh, refresh);
  }, [store, refresh]);
  const setRemember = useCallback(
    (on: boolean) => {
      setRememberRecent(on);
      setRememberState(on);
      if (!on) {
        store.clear().then(refresh, refresh);
      }
    },
    [store, refresh],
  );

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
