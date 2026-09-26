import { useState, type ReactNode } from "react";
import {
  describeEvidence,
  severityLabel,
  type Evidence,
  type Severity,
  type StoryStatement,
} from "@repodna/schema";
import { useApp } from "../state";

export function PageHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <header className="page-header">
      <h1>{title}</h1>
      {children ? <p>{children}</p> : null}
    </header>
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
      {showTable && table ? table : children}
    </section>
  );
}

export function Tile({
  label,
  value,
  note,
}: {
  label: string;
  value: ReactNode;
  note?: ReactNode;
}) {
  return (
    <div className="tile">
      <div className="label">{label}</div>
      <div className="value" title={typeof value === "string" ? value : undefined}>
        {value}
      </div>
      {note ? <div className="hint">{note}</div> : null}
    </div>
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
