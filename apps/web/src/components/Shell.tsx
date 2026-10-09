import { useEffect, useState, type ReactNode } from "react";
import { isSuppressed } from "@repodna/schema";
import { thousands } from "@repodna/visualization";
import { useApp, originLabel } from "../state";
import { WEBSITE } from "../lib/links";
import { NAV, neighbors } from "../lib/nav";
import { href, navigate, parseHash, useRoute } from "../lib/router";
import { CommandPalette } from "./CommandPalette";
import { ExternalLink } from "./common";
import { FileDrop } from "./FileDrop";
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

/** How far down the page the Back to top button appears, in screen heights. */
const BACK_TO_TOP_AFTER = 1.5;

function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/** A button to the top of a long page, shown once the page has been scrolled well down. */
function BackToTop() {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const update = () => setShown(window.scrollY > window.innerHeight * BACK_TO_TOP_AFTER);
    update();
    window.addEventListener("scroll", update, { passive: true });
    return () => window.removeEventListener("scroll", update);
  }, []);
  if (!shown) {
    return null;
  }
  return (
    <button
      type="button"
      className="back-to-top"
      onClick={() => {
        window.scrollTo({ top: 0, behavior: prefersReducedMotion() ? "auto" : "smooth" });
        document.getElementById("main")?.focus({ preventScroll: true });
      }}
    >
      ↑ Back to top
    </button>
  );
}

export function Shell({ children }: { children: ReactNode }) {
  const { dataset, themePreference, setThemePreference, theme } = useApp();
  const route = useRoute();
  const [palette, setPalette] = useState<"all" | "open" | null>(null);
  const [help, setHelp] = useState(false);
  // The navigation is folded away on narrow screens until the Menu button opens it.
  const [menuOpen, setMenuOpen] = useState(false);

  // Choosing a page closes the menu.
  useEffect(() => {
    setMenuOpen(false);
  }, [route.path]);

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
      } else if (
        (event.key === "[" || event.key === "]") &&
        !mod &&
        !event.altKey &&
        dataset &&
        !isTyping(event.target)
      ) {
        const { previous, next } = neighbors(parseHash(window.location.hash).path);
        const target = event.key === "[" ? previous : next;
        if (target) {
          event.preventDefault();
          navigate(target.path);
        }
      } else if (event.key === "Escape") {
        setPalette(null);
        setHelp(false);
        setMenuOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dataset]);

  const groups = ["Explore", "Health", "Share", "App"] as const;
  const findings = dataset ? dataset.dna.findings.filter((f) => !isSuppressed(f)).length : 0;
  return (
    <div className="app">
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <aside className="sidebar" aria-label="Navigation">
        <div className="sidebar-top">
          <a className="brand" href={href(dataset ? "/overview" : "/")}>
            <img src="./favicon.svg" alt="" />
            RepoDNA
          </a>
          <button
            type="button"
            className="ghost menu-toggle"
            aria-expanded={menuOpen}
            aria-controls="sidebar-menu"
            onClick={() => setMenuOpen((open) => !open)}
          >
            {menuOpen ? "Close menu" : "Menu"}
          </button>
        </div>
        {dataset ? (
          <div className="dataset-label">
            <strong>{dataset.dna.identity.name}</strong>
            <span>{originLabel(dataset)}</span>
          </div>
        ) : null}
        <div id="sidebar-menu" className="sidebar-menu" data-open={menuOpen || undefined}>
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
                      {item.path === "/findings" && dataset ? (
                        <span className="nav-count">
                          <span className="visually-hidden">: </span>
                          {thousands(findings)}
                        </span>
                      ) : null}
                    </a>
                  );
                })}
              </div>
            ))}
          </nav>
          <div className="sidebar-footer">
            <nav className="footer-links" aria-label="Legal">
              {NAV.filter((item) => item.group === "Legal").map((item) => (
                <a
                  key={item.path}
                  href={href(item.path)}
                  aria-current={route.path === item.path ? "page" : undefined}
                >
                  {item.label}
                </a>
              ))}
            </nav>
            <p>
              Local-first · no telemetry
              <br />
              <ExternalLink href={WEBSITE}>Made by the Sanskar</ExternalLink>
            </p>
          </div>
        </div>
      </aside>
      <div className="main">
        <header className="topbar">
          <button
            type="button"
            onClick={() => setPalette("all")}
            aria-keyshortcuts="Control+K Meta+K"
          >
            Search<span className="wide-only"> and commands</span> <kbd>Ctrl K</kbd>
          </button>
          <div className="spacer" />
          <button
            type="button"
            className="ghost shortcuts-button"
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
        </header>
        <main id="main" className="content" tabIndex={-1}>
          {children}
        </main>
      </div>
      <BackToTop />
      <FileDrop />
      {palette ? <CommandPalette mode={palette} onClose={() => setPalette(null)} /> : null}
      {help ? <ShortcutHelp onClose={() => setHelp(false)} /> : null}
    </div>
  );
}
