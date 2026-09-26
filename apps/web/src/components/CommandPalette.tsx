import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../state";
import { navigate } from "../lib/router";
import { NAV } from "../lib/nav";

interface Item {
  id: string;
  label: string;
  kind: string;
  run: () => void;
}

const MAX_RESULTS = 60;

/** Search every view, file, module, dependency, and finding (Ctrl+K), or only files and
 *  modules (quick open, Ctrl+P). */
export function CommandPalette({ mode, onClose }: { mode: "all" | "open"; onClose: () => void }) {
  const { dataset, setThemePreference, theme, close } = useApp();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const opener = useRef<Element | null>(document.activeElement);

  useEffect(() => {
    input.current?.focus();
    const previous = opener.current;
    return () => {
      if (previous instanceof HTMLElement) {
        previous.focus();
      }
    };
  }, []);

  const items = useMemo<Item[]>(() => {
    const list: Item[] = [];
    if (mode === "all") {
      for (const nav of NAV) {
        if (!nav.needsData || dataset) {
          list.push({
            id: `nav:${nav.path}`,
            label: nav.label,
            kind: "View",
            run: () => navigate(nav.path),
          });
        }
      }
      list.push({
        id: "theme",
        label: theme === "dark" ? "Switch to the light theme" : "Switch to the dark theme",
        kind: "Command",
        run: () => setThemePreference(theme === "dark" ? "light" : "dark"),
      });
      if (dataset) {
        list.push({
          id: "close",
          label: "Close this analysis",
          kind: "Command",
          run: () => {
            close();
            navigate("/");
          },
        });
      }
    }
    if (dataset) {
      const dna = dataset.dna;
      for (const module of dna.architecture.modules) {
        list.push({
          id: `module:${module.id}`,
          label: module.path || module.name,
          kind: "Module",
          run: () => navigate("/architecture", { module: module.id }),
        });
      }
      for (const file of dna.structure.files) {
        list.push({
          id: `file:${file.path}`,
          label: file.path,
          kind: "File",
          run: () => navigate("/files", { q: file.path }),
        });
      }
      if (mode === "all") {
        for (const dependency of dna.dependencies.dependencies) {
          list.push({
            id: `dep:${dependency.ecosystem}:${dependency.name}`,
            label: dependency.name,
            kind: `${dependency.ecosystem} package`,
            run: () => navigate("/dependencies", { q: dependency.name }),
          });
        }
        for (const finding of dna.findings) {
          list.push({
            id: `finding:${finding.id}`,
            label: finding.title,
            kind: "Finding",
            run: () => navigate("/findings", { q: finding.title }),
          });
        }
      }
    }
    return list;
  }, [mode, dataset, theme, setThemePreference, close]);

  const results = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    const matching =
      terms.length === 0
        ? items
        : items.filter((item) => {
            const text = `${item.label} ${item.kind}`.toLowerCase();
            return terms.every((term) => text.includes(term));
          });
    return matching.slice(0, MAX_RESULTS);
  }, [items, query]);

  const choose = (item: Item | undefined) => {
    if (item) {
      onClose();
      item.run();
    }
  };

  return (
    <div
      className="overlay"
      onPointerDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label={mode === "open" ? "Quick open" : "Search and commands"}
      >
        <input
          ref={input}
          type="search"
          role="combobox"
          aria-label={mode === "open" ? "Open a file or module" : "Search and run commands"}
          aria-expanded="true"
          aria-controls="palette-results"
          aria-activedescendant={results[active] ? `palette-${active}` : undefined}
          placeholder={
            mode === "open"
              ? "Open a file or module…"
              : "Search views, files, modules, packages, findings…"
          }
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setActive((index) => Math.min(results.length - 1, index + 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setActive((index) => Math.max(0, index - 1));
            } else if (event.key === "Enter") {
              event.preventDefault();
              choose(results[active]);
            } else if (event.key === "Escape") {
              onClose();
            }
          }}
        />
        <ul id="palette-results" role="listbox">
          {results.map((item, index) => (
            <li
              key={item.id}
              id={`palette-${index}`}
              role="option"
              aria-selected={index === active}
              onPointerEnter={() => setActive(index)}
              onClick={() => choose(item)}
            >
              <span className="path">{item.label}</span>
              <span className="kind">{item.kind}</span>
            </li>
          ))}
          {results.length === 0 ? <li className="muted">No matches.</li> : null}
        </ul>
      </div>
    </div>
  );
}
