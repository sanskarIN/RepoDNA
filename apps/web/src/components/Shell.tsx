import { useEffect, useState, type ReactNode } from "react";
import { useApp, originLabel } from "../state";
import { NAV } from "../lib/nav";
import { href, useRoute } from "../lib/router";
import { CommandPalette } from "./CommandPalette";
import { ShortcutHelp } from "./ShortcutHelp";

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.isContentEditable ||
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT"
  );
}

export function Shell({ children }: { children: ReactNode }) {
  const { dataset, themePreference, setThemePreference, theme } = useApp();
  const route = useRoute();
  const [palette, setPalette] = useState<"all" | "open" | null>(null);
  const [help, setHelp] = useState(false);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;
      if (mod && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPalette("all");
      } else if (mod && event.key.toLowerCase() === "p" && dataset) {
        event.preventDefault();
        setPalette("open");
      } else if (mod && event.key.toLowerCase() === "f") {
        const search = document.querySelector<HTMLInputElement>("[data-view-search]");
        if (search) {
          event.preventDefault();
          search.focus();
          search.select();
        }
      } else if (event.key === "?" && !isTyping(event.target)) {
        event.preventDefault();
        setHelp(true);
      } else if (event.key === "Escape") {
        setPalette(null);
        setHelp(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dataset]);

  const groups = ["Explore", "Health", "Share", "App"] as const;
  return (
    <div className="app">
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <aside className="sidebar" aria-label="Navigation">
        <a className="brand" href={href(dataset ? "/overview" : "/")}>
          <img src="./favicon.svg" alt="" />
          RepoDNA
        </a>
        {dataset ? (
          <div className="dataset-label">
            <strong>{dataset.dna.identity.name}</strong>
            <span>{originLabel(dataset)}</span>
          </div>
        ) : null}
        <nav className="nav">
          {groups.map((group) => (
            <div key={group}>
              <div className="nav-heading">{group}</div>
              {NAV.filter((item) => item.group === group).map((item) => {
                const disabled = item.needsData && !dataset;
                return (
                  <a
                    key={item.path}
                    href={href(item.path)}
                    className={disabled ? "disabled" : undefined}
                    aria-disabled={disabled || undefined}
                    tabIndex={disabled ? -1 : undefined}
                    aria-current={route.path === item.path ? "page" : undefined}
                  >
                    {item.label}
                  </a>
                );
              })}
            </div>
          ))}
        </nav>
        <div className="sidebar-footer">
          <p>
            Local-first · no telemetry
            <br />
            Made by the Sanskar
          </p>
        </div>
      </aside>
      <div className="main">
        <div className="topbar">
          <button
            type="button"
            onClick={() => setPalette("all")}
            aria-keyshortcuts="Control+K Meta+K"
          >
            Search and commands <kbd>Ctrl K</kbd>
          </button>
          <div className="spacer" />
          <button
            type="button"
            className="ghost"
            onClick={() => setHelp(true)}
            aria-keyshortcuts="?"
          >
            Shortcuts <kbd>?</kbd>
          </button>
          <label className="visually-hidden" htmlFor="theme-select">
            Theme
          </label>
          <select
            id="theme-select"
            value={themePreference}
            onChange={(event) => setThemePreference(event.target.value as typeof themePreference)}
            title={`Theme (currently ${theme})`}
          >
            <option value="system">System theme</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
        <main id="main" className="content" tabIndex={-1}>
          {children}
        </main>
      </div>
      {palette ? <CommandPalette mode={palette} onClose={() => setPalette(null)} /> : null}
      {help ? <ShortcutHelp onClose={() => setHelp(false)} /> : null}
    </div>
  );
}
