import { useMemo, useState } from "react";
import type { AnalysisComparison, ChangeKind } from "../lib/analysisComparison";
import { buildComparisonExport, serializeComparisonExport, type ComparisonSection, type ExportFormat, type ExportMode, type ExportSelection } from "../lib/comparisonExport";
import { useDownload } from "../lib/download";
import { ErrorBox, Note } from "./common";

interface Props {
  comparison: AnalysisComparison;
  section: ComparisonSection;
  kind: ChangeKind | "all";
  query: string;
}
/** Export is explicitly user initiated; counts-only is the default and does
 * not embed source-derived text, even when the user filters by a private path. */
export function ComparisonExportPanel({ comparison, section, kind, query }: Props) {
  const [mode, setMode] = useState<ExportMode>("counts");
  const [scope, setScope] = useState<ExportSelection["scope"]>("all");
  const [format, setFormat] = useState<ExportFormat>("json");
  const [confirmed, setConfirmed] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [download, downloadError] = useDownload();
  const selection = useMemo<ExportSelection>(() => ({ mode, scope, section, kind, query }), [mode, scope, section, kind, query]);
  const preview = useMemo(() => buildComparisonExport(comparison, selection, 0), [comparison, selection]);
  const save = () => {
    setError(null);
    setStatus(null);
    if (mode === "details" && !confirmed) return;
    try {
      const output = serializeComparisonExport(comparison, selection, format);
      download(output.filename, output.text, output.mime);
      setStatus(output.omittedDetails ? `Prepared ${output.exportedDetails.toLocaleString()} detail rows; ${output.omittedDetails.toLocaleString()} details omitted by the export limit. Counts include all selected changes.` : "Prepared comparison export. Your browser or desktop save dialog handles the file.");
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
  };
  return (
    <details>
      <summary>Export comparison</summary>
      <p>Save a local record of these observed changes and their coverage. The export is not a Git diff or a contribution report.</p>
      <fieldset className="choices">
        <legend>Export content</legend>
        <label><input type="radio" name="comparison-export-mode" checked={mode === "counts"} onChange={() => { setMode("counts"); setConfirmed(false); setStatus(null); }} /> Counts and coverage only (default)</label>
        <label><input type="radio" name="comparison-export-mode" checked={mode === "details"} onChange={() => { setMode("details"); setConfirmed(false); setStatus(null); }} /> Include item names, before/after values and evidence</label>
      </fieldset>
      {mode === "details" ? <>
        <Note caution>Detailed exports can contain file paths, dependency names, finding summaries and other private repository metadata. The export does not apply additional artifact privacy redactions.</Note>
        <label><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /> I understand that the detailed export includes source-derived metadata.</label>
      </> : <p className="muted">Counts-only exports omit item labels, internal IDs, paths, revisions and search terms. Counts can still reveal characteristics of a private repository.</p>}
      <div className="filters">
        <label htmlFor="comparison-export-scope">Scope</label>
        <select id="comparison-export-scope" value={scope} onChange={(event) => { setScope(event.target.value as ExportSelection["scope"]); setStatus(null); }}><option value="all">All categories</option><option value="filtered">Current category and filters</option></select>
        <label htmlFor="comparison-export-format">Format</label>
        <select id="comparison-export-format" value={format} onChange={(event) => { setFormat(event.target.value as ExportFormat); setStatus(null); }}><option value="json">JSON</option><option value="csv">CSV</option></select>
      </div>
      <p>{preview.selectedChanges.toLocaleString()} observed changes selected across {preview.groups.length} {preview.groups.length === 1 ? "category" : "categories"}.</p>
      <button type="button" disabled={mode === "details" && !confirmed} onClick={save}>Download comparison export</button>
      {status ? <p role="status">{status}</p> : null}
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      {downloadError ? <ErrorBox>{downloadError}</ErrorBox> : null}
    </details>
  );
}
