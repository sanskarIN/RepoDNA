// Light, dark, or following the system. The choice is remembered per browser.

import { useCallback, useEffect, useState } from "react";
import type { Theme } from "@repodna/visualization";

export type ThemePreference = "system" | "light" | "dark";

const KEY = "repodna-theme";

function stored(): ThemePreference {
  try {
    const value = window.localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

function systemTheme(): Theme {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function useTheme(): [ThemePreference, (preference: ThemePreference) => void, Theme] {
  const [preference, setPreference] = useState<ThemePreference>(stored);
  const [system, setSystem] = useState<Theme>(systemTheme);

  useEffect(() => {
    const query = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!query) {
      return;
    }
    const update = () => setSystem(query.matches ? "dark" : "light");
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  const resolved: Theme = preference === "system" ? system : preference;

  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
  }, [resolved]);

  const choose = useCallback((next: ThemePreference) => {
    setPreference(next);
    try {
      if (next === "system") {
        window.localStorage.removeItem(KEY);
      } else {
        window.localStorage.setItem(KEY, next);
      }
    } catch {
      // The choice still applies for this session.
    }
  }, []);

  return [preference, choose, resolved];
}
