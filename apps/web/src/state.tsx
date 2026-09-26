// Application state: the backend, the analysis being explored, and preferences.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { RepositoryDna } from "@repodna/schema";
import type { Theme } from "@repodna/visualization";
import { detectBackend, type Backend, type Session } from "./lib/backend";
import { useTheme, type ThemePreference } from "./lib/theme";

export type Origin =
  | { kind: "stored"; repositoryId: string; scanId?: string; name: string }
  | { kind: "file"; name: string }
  | { kind: "demo"; title: string };

export interface Dataset {
  dna: RepositoryDna;
  origin: Origin;
  warnings: string[];
}

export interface AppState {
  ready: boolean;
  error: string | null;
  backend: Backend | null;
  session: Session | null;
  signInNeeded: boolean;
  dataset: Dataset | null;
  open(dataset: Dataset): void;
  close(): void;
  theme: Theme;
  themePreference: ThemePreference;
  setThemePreference(preference: ThemePreference): void;
}

const Context = createContext<AppState | null>(null);

export function AppProvider({
  children,
  initial = null,
}: {
  children: ReactNode;
  /** An analysis to open at start (tests and embedding). */
  initial?: Dataset | null;
}) {
  const [themePreference, setThemePreference, theme] = useTheme();
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [backend, setBackend] = useState<Backend | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [signInNeeded, setSignInNeeded] = useState(false);
  const [dataset, setDataset] = useState<Dataset | null>(initial);

  useEffect(() => {
    let cancelled = false;
    detectBackend()
      .then((detected) => {
        if (!cancelled) {
          setBackend(detected.backend);
          setSession(detected.session);
          setSignInNeeded(detected.signInNeeded);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setReady(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const open = useCallback((next: Dataset) => setDataset(next), []);
  const close = useCallback(() => setDataset(null), []);

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
