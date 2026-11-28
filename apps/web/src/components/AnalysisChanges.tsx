import { useMemo, useState } from "react";
import type { RepositoryDna } from "@repodna/schema";
import {
  CHANGE_LABELS,
  compareAnalyses,
  filterAnalysisChanges,
  type ChangeKind,
} from "../lib/analysisComparison";
import { Note, Panel } from "./common";
import { ComparisonExportPanel } from "./ComparisonExportPanel";
import { COMPARISON_SECTIONS, type ComparisonSection } from "../lib/comparisonExport";

type Section = ComparisonSection;
const SECTIONS = COMPARISON_SECTIONS;
const PAGE_SIZE = 40;

/** A paginated view of the observed differences in the two saved artifacts.
 * Only the current page is rendered, even for large repository inventories. */
export function AnalysisChanges({ current, baseline }: { current: RepositoryDna; baseline: RepositoryDna }) {
  const result = useMemo(() => compareAnalyses(current, baseline), [current, baseline]);
  const [section, setSection] = useState<Section>("files");
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<ChangeKind | "all">("all");
  const [requestedPage, setRequestedPage] = useState(0);
  const group = result[section];
  const counts = useMemo(() => {
    const map = new Map<ChangeKind, number>();
    for (const change of group.changes) map.set(change.kind, (map.get(change.kind) ?? 0) + 1);
    return map;
  }, [group]);
  const filtered = useMemo(() => filterAnalysisChanges(group.changes, query, kind), [group, query, kind]);
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const page = Math.min(requestedPage, pages - 1);
  const visible = filtered.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const resetFilters = (next: Section) => {
    setSection(next);
    setKind("all");
    setQuery("");
    setRequestedPage(0);
  };
  return (
    <Panel
      title="Observed analysis changes"
      description="Current analysis compared with the selected baseline. These are differences between measurements, not a Git ancestry or source-code diff."
    >
      <div className="filters">
        <label htmlFor="comparison-change-section">Change category</label>
        <select id="comparison-change-section" value={section} onChange={(event) => resetFilters(event.target.value as Section)}>
          {SECTIONS.map(([id, label]) => (
            <option key={id} value={id}>{label} ({result[id].changes.length})</option>
          ))}
        </select>
        <label htmlFor="comparison-change-kind">Change type</label>
        <select id="comparison-change-kind" value={kind} onChange={(event) => {
          setKind(event.target.value as ChangeKind | "all");
          setRequestedPage(0);
        }}>
          <option value="all">All observed changes</option>
          {(Object.keys(CHANGE_LABELS) as ChangeKind[]).filter((key) => counts.has(key)).map((key) => (
            <option key={key} value={key}>{CHANGE_LABELS[key]} ({counts.get(key)})</option>
          ))}
        </select>
        <label htmlFor="comparison-change-query">Filter changes</label>
        <input id="comparison-change-query" value={query} placeholder="Name, path, or change" onChange={(event) => {
          setQuery(event.target.value);
          setRequestedPage(0);
        }} />
      </div>
      <p className="muted">
        Inventory coverage: current {group.coverage.current} · baseline {group.coverage.baseline}.
      </p>
      {group.coverage.notes.map((note) => <p className="muted" key={note}>{note}</p>)}
      {group.ambiguousKeys > 0 ? (
        <Note caution>{group.ambiguousKeys} ambiguous duplicate identifiers were excluded rather than paired arbitrarily.</Note>
      ) : null}
      {group.unverified > 0 ? (
        <p className="muted">{group.unverified} matching file paths have insufficient recorded content hashes to establish content equality.</p>
      ) : null}
      <p role="status">
        {filtered.length} of {group.changes.length} observed changes.
        {filtered.length > 0 ? ` Showing ${page * PAGE_SIZE + 1}–${Math.min(filtered.length, (page + 1) * PAGE_SIZE)}.` : ""}
      </p>
      {visible.length > 0 ? (
        <div className="table-scroll">
          <table>
            <thead>
              <tr><th scope="col">Item</th><th scope="col">Change</th><th scope="col">Baseline</th><th scope="col">Current</th><th scope="col">Evidence</th></tr>
            </thead>
            <tbody>
              {visible.map((change) => (
                <tr key={change.key}>
                  <th scope="row" className={section === "files" ? "path" : undefined}>{change.label}</th>
                  <td>{CHANGE_LABELS[change.kind]}</td>
                  <td>{change.before}</td>
                  <td>{change.after}</td>
                  <td>{change.details.length ? change.details.join("; ") : "Inventory membership"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="muted">{group.changes.length === 0 ? "No differences were established from the available measurements." : "No changes match the selected filter."}</p>
      )}
      {pages > 1 ? (
        <div className="actions" aria-label="Comparison change pages">
          <button type="button" disabled={page === 0} onClick={() => setRequestedPage(page - 1)}>Previous changes</button>
          <span>Page {page + 1} of {pages}</span>
          <button type="button" disabled={page + 1 >= pages} onClick={() => setRequestedPage(page + 1)}>Next changes</button>
        </div>
      ) : null}
      <ComparisonExportPanel comparison={result} section={section} kind={kind} query={query} />
    </Panel>
  );
}
