import { useEffect, useState, type ReactNode } from "react";
import {
  describeEvidence,
  severityLabel,
  type Evidence,
  type Severity,
  type StoryStatement,
} from "@repodna/schema";
import { NAV } from "../lib/nav";
import { useRoute } from "../lib/router";
import { useApp } from "../state";
import { PanelTitle } from "./panelTitle";

export function PageHeader({
  title,
  children,
  meta,
}: {
  title: string;
  children?: ReactNode;
  /** A line about where the data comes from, under the description. */
  meta?: ReactNode;
}) {
  return (
    <header className="page-header">
      <h1>{title}</h1>
      {children ? <p>{children}</p> : null}
      {meta ? <p className="muted meta">{meta}</p> : null}
      <OnThisPage />
    </header>
  );
}

/** Views of an analysis with at least this many panels list them under the title. */
const ON_THIS_PAGE_MIN = 4;

/** The title of each panel of the view, as it is shown now. */
function panelHeadings(main: HTMLElement): HTMLElement[] {
  return [...main.querySelectorAll<HTMLElement>("section.panel > .panel-header h2")];
}

/** Moves to a panel and gives its title the focus, so that reading continues from there. */
function jumpTo(heading: HTMLElement) {
  heading.closest("section")?.scrollIntoView({ block: "start" });
  heading.tabIndex = -1;
  heading.focus({ preventScroll: true });
}

/**
 * Links to the panels of a long view. The hash holds the route, so they move the page
 * instead of changing the address.
 */
export function OnThisPage() {
  const route = useRoute();
  const isAnalysisView = NAV.some((item) => item.path === route.path && item.needsData);
  const [headings, setHeadings] = useState<HTMLElement[]>([]);
  useEffect(() => {
    const main = document.getElementById("main");
    if (!main) {
      return;
    }
    let frame = 0;
    const collect = () => {
      frame = 0;
      const found = panelHeadings(main);
      setHeadings((previous) =>
        previous.length === found.length && previous.every((heading, i) => heading === found[i])
          ? previous
          : found,
      );
    };
    collect();
    // Panels can appear after the view, once data has loaded or a filter changed.
    const observer = new MutationObserver(() => {
      if (frame === 0) {
        frame = window.requestAnimationFrame(collect);
      }
    });
    observer.observe(main, { childList: true, subtree: true });
    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(frame);
    };
  }, []);
  if (!isAnalysisView || headings.length < ON_THIS_PAGE_MIN) {
    return null;
  }
  return (
    <nav className="on-this-page" aria-label="On this page">
      <ul>
        {headings.map((heading, index) => (
          <li key={index}>
            <button type="button" className="chip-button" onClick={() => jumpTo(heading)}>
              {heading.textContent}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}

/**
 * A titled card. With `table`, a toggle switches between the chart and its table twin,
 * so every value is reachable without the chart.
 */
export function Panel({
  title,
  description,
  children,
  table,
  actions,
  id,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
  table?: ReactNode;
  actions?: ReactNode;
  id?: string;
}) {
  const [showTable, setShowTable] = useState(false);
  const headingId = id ? `${id}-title` : undefined;
  return (
    <section className="panel" id={id} aria-labelledby={headingId}>
      <div className="panel-header">
        <div>
          <h2 id={headingId}>{title}</h2>
          {description ? <p>{description}</p> : null}
        </div>
        <div className="actions">
          {actions}
          {table ? (
            <button
              type="button"
              className="view-toggle"
              aria-pressed={showTable}
              onClick={() => setShowTable((value) => !value)}
            >
              {showTable ? "Show chart" : "Show table"}
            </button>
          ) : null}
        </div>
      </div>
      <PanelTitle.Provider value={title}>
        {showTable && table ? table : children}
      </PanelTitle.Provider>
    </section>
  );
}

/** A key number; with `to`, a link to the view that tells more about it. */
export function Tile({
  label,
  value,
  note,
  to,
}: {
  label: string;
  value: ReactNode;
  note?: ReactNode;
  to?: string;
}) {
  const content = (
    <>
      <div className="label">{label}</div>
      <div className="value" title={typeof value === "string" ? value : undefined}>
        {value}
      </div>
      {note ? <div className="hint">{note}</div> : null}
    </>
  );
  return to ? (
    <a className="tile tile-link" href={to}>
      {content}
    </a>
  ) : (
    <div className="tile">{content}</div>
  );
}

const SEVERITY_COLOR: Record<Severity, string> = {
  critical: "var(--critical)",
  warning: "var(--warning)",
  attention: "var(--attention)",
  info: "var(--info)",
};

/** A severity label with its color dot: meaning never depends on color alone. */
export function SeverityBadge({ severity }: { severity: Severity }) {
  return (
    <span className="severity" style={{ ["--sev" as string]: SEVERITY_COLOR[severity] }}>
      {severityLabel(severity)}
    </span>
  );
}

export function Chip({ children, title }: { children: ReactNode; title?: string }) {
  return (
    <span className="chip" title={title}>
      {children}
    </span>
  );
}

export function Note({ children, caution }: { children: ReactNode; caution?: boolean }) {
  return <p className={caution ? "note caution" : "note"}>{children}</p>;
}

/** Commands to run, with a button that copies them. */
export function CommandBox({
  lines,
  label = "Copy the commands",
}: {
  lines: readonly string[];
  label?: string;
}) {
  const [copied, setCopied] = useState<"yes" | "failed" | null>(null);
  const text = lines.join("\n");
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied("yes");
    } catch {
      // No clipboard access, as on a page served over plain HTTP: the text can still be selected.
      setCopied("failed");
    }
    window.setTimeout(() => setCopied(null), 2000);
  };
  return (
    <div className="commands">
      {/* Long commands scroll sideways on narrow screens, so the box takes keyboard focus. */}
      <pre className="note mono" tabIndex={0} aria-label="Commands">
        {text}
      </pre>
      <button type="button" className="ghost copy" aria-label={label} onClick={() => void copy()}>
        {copied === "yes" ? "Copied" : copied === "failed" ? "Copy failed" : "Copy"}
      </button>
      <span role="status" className="visually-hidden">
        {copied === "yes" ? "Copied to the clipboard." : ""}
      </span>
    </div>
  );
}

export function ErrorBox({ children }: { children: ReactNode }) {
  return (
    <div className="error-box" role="alert">
      {children}
    </div>
  );
}

/** Shown instead of a chart or list when there is nothing to show, with the reason. */
export function Empty({ children }: { children: ReactNode }) {
  return <p className="muted">{children}</p>;
}

/** Says why a section was not analyzed, from the section's status and notes. */
export function SectionStatus({ status, notes }: { status: string; notes?: string[] }) {
  if (status === "analyzed") {
    return notes && notes.length > 0 ? (
      <ul className="evidence">
        {notes.map((note) => (
          <li key={note}>{note}</li>
        ))}
      </ul>
    ) : null;
  }
  const label =
    status === "partial"
      ? "Only part of this section could be analyzed."
      : status === "skipped"
        ? "This section was not analyzed with the selected profile."
        : "This section could not be analyzed.";
  return (
    <Note caution>
      {label}
      {notes && notes.length > 0 ? ` ${notes.join(" ")}` : ""}
    </Note>
  );
}

/**
 * A link to a web page outside RepoDNA. In the desktop app it opens in the system browser;
 * in a browser it opens in a new tab without access to this page.
 */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  const { backend } = useApp();
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(event) => {
        if (backend?.openExternal) {
          event.preventDefault();
          void backend.openExternal(href);
        }
      }}
    >
      {children}
    </a>
  );
}

/** A search box that Ctrl/Cmd+F focuses. */
export function SearchBox({
  value,
  onChange,
  label,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
  placeholder?: string;
}) {
  return (
    <>
      <label className="visually-hidden" htmlFor="view-search">
        {label}
      </label>
      <input
        id="view-search"
        type="search"
        data-view-search
        value={value}
        placeholder={placeholder ?? label}
        onChange={(event) => onChange(event.target.value)}
        style={{ flex: "1 1 260px" }}
      />
    </>
  );
}

/** A labeled select for filter rows. */
export function Select<T extends string>({
  id,
  label,
  value,
  options,
  onChange,
}: {
  id: string;
  label: string;
  value: T;
  options: readonly (readonly [T, string])[];
  onChange: (value: T) => void;
}) {
  return (
    <>
      <label className="visually-hidden" htmlFor={id}>
        {label}
      </label>
      <select id={id} value={value} onChange={(event) => onChange(event.target.value as T)}>
        {options.map(([option, text]) => (
          <option key={option} value={option}>
            {text}
          </option>
        ))}
      </select>
    </>
  );
}

/** Evidence items as a list of plain-text descriptions. */
export function EvidenceList({ evidence }: { evidence: readonly Evidence[] }) {
  if (evidence.length === 0) {
    return null;
  }
  return (
    <ul className="evidence">
      {evidence.map((item, index) => (
        <li key={index} className="path">
          {describeEvidence(item)}
        </li>
      ))}
    </ul>
  );
}

/** Statements labeled as facts or interpretations, with their evidence on request. */
export function Statements({ statements }: { statements: readonly StoryStatement[] }) {
  return (
    <ul className="statements">
      {statements.map((statement, index) => (
        <li key={index}>
          <Chip>{statement.kind === "fact" ? "Fact" : "Interpretation"}</Chip> {statement.text}
          {statement.evidence.length > 0 ? (
            <details>
              <summary>Evidence</summary>
              <EvidenceList evidence={statement.evidence} />
            </details>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/** Text that a privacy preset may have removed. */
export function Omittable({ text, mono }: { text: string; mono?: boolean }) {
  if (!text) {
    return <span className="muted">(omitted for privacy)</span>;
  }
  return mono ? <span className="mono">{text}</span> : <>{text}</>;
}
