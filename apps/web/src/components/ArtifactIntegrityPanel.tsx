import { useEffect, useRef, useState } from "react";
import type { RepositoryDna } from "@repodna/schema";
import type { ArtifactIntegrityReport, ShapeIssue } from "@repodna/schema/integrity";
import { useDownload } from "../lib/download";
import { ErrorBox, Note, Panel } from "./common";

type Result =
  | { kind: "idle" }
  | { kind: "running"; source: string }
  | { kind: "cancelled" }
  | { kind: "error"; message: string }
  | { kind: "ready"; source: string; report: ArtifactIntegrityReport };

type Selection = { kind: "file"; file: File } | { kind: "current"; artifact: RepositoryDna };
type Severity = ShapeIssue["severity"] | "all";
const PAGE_SIZE = 20;

const STATUS: Record<ArtifactIntegrityReport["status"], string> = {
  valid: "Matches the checked schema shape",
  invalid: "Schema mismatches found",
  incomplete: "Conformance not established",
};
const COMPATIBILITY: Record<ArtifactIntegrityReport["compatibility"], string> = {
  "same-version": "The artifact claims the same schema version.",
  "different-minor": "The artifact claims a different minor version. This check cannot establish conformance to that version.",
  "different-major": "The artifact claims a different major version. It may be incompatible with this reader.",
  unknown: "The artifact does not identify a recognized schema version.",
};

/** Optional local integrity diagnostics, not an alternative artifact loader. */
export function ArtifactIntegrityPanel({ current }: { current?: RepositoryDna }) {
  const [result, setResult] = useState<Result>({ kind: "idle" });
  const [query, setQuery] = useState("");
  const [severity, setSeverity] = useState<Severity>("all");
  const [page, setPage] = useState(0);
  const [download, downloadError] = useDownload();
  const active = useRef<AbortController | null>(null);
  const chooser = useRef<HTMLInputElement>(null);
  useEffect(() => () => {
    active.current?.abort();
    active.current = null;
  }, []);

  const cancel = () => {
    const previous = active.current;
    active.current = null;
    previous?.abort();
    setResult({ kind: "cancelled" });
  };

  const inspect = async (selection: Selection) => {
    const previous = active.current;
    const controller = new AbortController();
    active.current = controller;
    previous?.abort();
    const source = selection.kind === "file" ? selection.file.name : "Currently open analysis";
    setResult({ kind: "running", source });
    setQuery("");
    setSeverity("all");
    setPage(0);
    try {
      // The validator and its schema are loaded only when explicitly requested.
      const integrity = await import("@repodna/schema/integrity");
      if (active.current !== controller) return;
      const options = { signal: controller.signal };
      const report = selection.kind === "file"
        ? await integrity.inspectArtifactFile(selection.file, options)
        : await integrity.inspectArtifact(selection.artifact, options);
      if (active.current !== controller) return;
      active.current = null;
      setResult({ kind: "ready", source, report });
    } catch (reason) {
      if (active.current !== controller) return;
      active.current = null;
      setResult({ kind: "error", message: reason instanceof Error ? reason.message : String(reason) });
    }
  };

  const exportReport = async (report: ArtifactIntegrityReport) => {
    try {
      const { integrityReportJson } = await import("@repodna/schema/integrity");
      download("repodna-integrity-report.json", integrityReportJson(report), "application/json");
    } catch {
      setResult({ kind: "error", message: "Could not prepare the integrity report." });
    }
  };

  const ready = result.kind === "ready" ? result : null;
  const report = ready?.report;
  const needle = query.trim().toLowerCase();
  const filtered = (report?.issues ?? []).filter((issue) =>
    (severity === "all" || issue.severity === severity) &&
    (!needle || [issue.path, issue.code, issue.message].some((value) => value.toLowerCase().includes(needle))),
  );
  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const visiblePage = Math.min(page, totalPages - 1);
  const visible = filtered.slice(visiblePage * PAGE_SIZE, (visiblePage + 1) * PAGE_SIZE);
  return (
    <Panel id="artifact-integrity" title="Analysis file integrity" description="Check a RepoDNA document against the bundled schema. This does not upload the file, change the artifact, or prove that its measurements are correct.">
      <div className="actions">
        <button type="button" onClick={() => chooser.current?.click()}>Choose an analysis file…</button>
        <input ref={chooser} type="file" accept=".repodna,.json,application/json" className="visually-hidden" aria-label="Analysis file to validate" onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (file) void inspect({ kind: "file", file });
        }} />
        {current ? <button type="button" onClick={() => void inspect({ kind: "current", artifact: current })}>Check the open analysis</button> : null}
        {result.kind === "running" ? <button type="button" onClick={cancel}>Cancel integrity check</button> : null}
      </div>
      <p className="muted">File checks support up to 64 MiB. The ordinary analysis loader has a separate limit.</p>
      {result.kind === "running" ? <p role="status">Checking {result.source}…</p> : null}
      {result.kind === "cancelled" ? <p role="status">Integrity check cancelled.</p> : null}
      {result.kind === "error" ? <ErrorBox>{result.message}</ErrorBox> : null}
      {downloadError ? <ErrorBox>{downloadError}</ErrorBox> : null}
      {ready && report ? (
        <>
          <h3>{STATUS[report.status]}</h3>
          <dl className="facts wide">
            <dt>Document</dt><dd className="path">{ready.source}</dd>
            <dt>Claimed schema version</dt><dd>{report.artifactVersion ?? "Unknown"}</dd>
            <dt>Checked against</dt><dd>{report.checkedAgainst}</dd>
            <dt>Validation operations</dt><dd>{report.checked.toLocaleString()}</dd>
            <dt>Errors</dt><dd>{report.errors.toLocaleString()}</dd>
            <dt>Warnings</dt><dd>{report.warnings.toLocaleString()}</dd>
          </dl>
          <p>{COMPATIBILITY[report.compatibility]}</p>
          {!report.complete ? <Note caution>The checker reached a work or nesting limit. Some fields were not checked.</Note> : null}
          {report.status === "valid" ? <Note>Schema conformance is not proof of authenticity, source-code correctness or accurate analysis.</Note> : null}
          {report.omittedIssues ? <Note caution>{report.omittedIssues.toLocaleString()} additional diagnostics were counted but not retained in the report.</Note> : null}
          {report.issues.length ? (
            <>
              <div className="filters">
                <label htmlFor="integrity-severity">Diagnostic type</label>
                <select id="integrity-severity" value={severity} onChange={(event) => { setSeverity(event.target.value as Severity); setPage(0); }}>
                  <option value="all">All diagnostics</option><option value="error">Errors</option><option value="warning">Warnings</option>
                </select>
                <label htmlFor="integrity-filter">Filter diagnostics</label>
                <input id="integrity-filter" value={query} onChange={(event) => { setQuery(event.target.value); setPage(0); }} />
              </div>
              <p role="status">{filtered.length} of {report.issues.length} retained diagnostics.</p>
              <div className="table-scroll">
                <table>
                  <thead><tr><th scope="col">Field path</th><th scope="col">Type</th><th scope="col">Diagnostic</th></tr></thead>
                  <tbody>
                    {visible.map((issue, index) => <tr key={`${visiblePage * PAGE_SIZE + index}:${issue.code}`}>
                      <th scope="row" className="path">{issue.path || "(document root)"}</th>
                      <td>{issue.severity} · {issue.code}</td><td>{issue.message}</td>
                    </tr>)}
                  </tbody>
                </table>
              </div>
              {filtered.length === 0 ? <p className="muted">No retained diagnostics match this filter.</p> : null}
              {totalPages > 1 ? <div className="actions"><button type="button" disabled={visiblePage === 0} onClick={() => setPage(visiblePage - 1)}>Previous diagnostics</button><span>Page {visiblePage + 1} of {totalPages}</span><button type="button" disabled={visiblePage + 1 >= totalPages} onClick={() => setPage(visiblePage + 1)}>Next diagnostics</button></div> : null}
            </>
          ) : null}
          {report.consistency ? (
            <details open={report.consistency.observations > 0}>
              <summary>Cross-field consistency: {report.consistency.observations} observations</summary>
              <p className="muted">These checks compare reported relationships, not the underlying source code. Observations are not proof of corruption.</p>
              {report.consistency.skipped.length ? <ul className="evidence">{report.consistency.skipped.map((reason) => <li key={reason}>{reason}</li>)}</ul> : null}
              {report.consistency.issues.length ? <div className="table-scroll"><table><thead><tr><th scope="col">Field</th><th scope="col">Observation</th></tr></thead><tbody>{report.consistency.issues.map((issue, index) => <tr key={`${index}:${issue.code}`}><th scope="row" className="path">{issue.path}</th><td>{issue.message}</td></tr>)}</tbody></table></div> : <p>No retained consistency observations.</p>}
              {report.consistency.omittedIssues ? <Note caution>{report.consistency.omittedIssues} additional observations are not displayed.</Note> : null}
            </details>
          ) : <p className="muted">Cross-field checks require a complete match to the bundled schema.</p>}
          <p><button type="button" onClick={() => void exportReport(report)}>Download integrity report</button></p>
          <p className="muted">The report excludes instance values, but field paths may contain names from your document. Review it before sharing.</p>
        </>
      ) : null}
    </Panel>
  );
}
