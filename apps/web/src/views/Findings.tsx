import { useEffect, useMemo, useState } from "react";
import {
  SEVERITIES,
  categoryLabel,
  confidenceLabel,
  findingCounts,
  isSuppressed,
  severityLabel,
  type Confidence,
  type FindingCategory,
  type Severity,
} from "@repodna/schema";
import { thousands } from "@repodna/visualization";
import { ErrorBox, Note, PageHeader, SearchBox, Select, SeverityBadge } from "../components/common";
import { FindingItem } from "../components/FindingItem";
import { useDownload } from "../lib/download";
import {
  findingsCountsCsv,
  findingsCsv,
  findingsExportReview,
  findingsJson,
  type FindingsExportFormat,
} from "../lib/findings-export";
import { findingsMarkdown } from "../lib/findings-markdown";
import { filterFindings } from "../lib/findings-filter";
import { useRoute } from "../lib/router";
import { useDataset } from "../state";

/** Findings shown at first, and added each time more are asked for. */
export const FINDINGS_PAGE = 25;
const CONFIDENCES: readonly Confidence[] = ["high", "medium", "low", "unavailable"];

export function Findings() {
  const { dna } = useDataset();
  const route = useRoute();
  const [download, downloadFailure] = useDownload();
  const [query, setQuery] = useState(() => route.params.get("q") ?? "");
  const [severity, setSeverity] = useState<"all" | Severity>(() => {
    const value = route.params.get("severity");
    return (SEVERITIES as readonly string[]).includes(value ?? "") ? (value as Severity) : "all";
  });
  const [category, setCategory] = useState<"all" | FindingCategory>(
    () => (route.params.get("category") as FindingCategory | null) ?? "all",
  );
  const [confidence, setConfidence] = useState<"all" | Confidence>(() => {
    const value = route.params.get("confidence");
    return CONFIDENCES.includes(value as Confidence) ? (value as Confidence) : "all";
  });
  const [showSuppressed, setShowSuppressed] = useState(false);
  const [reviewFormat, setReviewFormat] = useState<FindingsExportFormat | null>(null);
  const [shown, setShown] = useState(FINDINGS_PAGE);
  const counts = findingCounts(dna);
  const categories = useMemo(
    () => [...new Set(dna.findings.map((f) => f.category))].sort(),
    [dna.findings],
  );
  const filtered = useMemo(
    () => filterFindings(dna.findings, { query, severity, category, confidence, showSuppressed }),
    [dna.findings, query, severity, category, confidence, showSuppressed],
  );
  const review = useMemo(
    () => (reviewFormat ? findingsExportReview(filtered, reviewFormat) : null),
    [filtered, reviewFormat],
  );
  // Closing a stale preview prevents exporting a selection that has changed.
  useEffect(() => setReviewFormat(null), [filtered]);
  const suppressed = dna.findings.filter(isSuppressed).length;
  // Changing a filter starts again from the first page.
  useEffect(() => setShown(FINDINGS_PAGE), [query, severity, category, confidence, showSuppressed]);
  const filtersActive =
    query !== "" ||
    severity !== "all" ||
    category !== "all" ||
    confidence !== "all" ||
    showSuppressed;
  const resetFilters = () => {
    setQuery("");
    setSeverity("all");
    setCategory("all");
    setConfidence("all");
    setShowSuppressed(false);
    setReviewFormat(null);
  };

  const saveReviewedExport = () => {
    if (!reviewFormat || filtered.length === 0) return;
    const extension =
      reviewFormat === "markdown" ? "md" : reviewFormat === "counts" ? "csv" : reviewFormat;
    const content =
      reviewFormat === "counts"
        ? findingsCountsCsv(filtered)
        : reviewFormat === "csv"
          ? findingsCsv(filtered)
          : reviewFormat === "json"
            ? findingsJson(filtered)
            : findingsMarkdown(filtered);
    const mime =
      reviewFormat === "counts" || reviewFormat === "csv"
        ? "text/csv;charset=utf-8"
        : reviewFormat === "json"
          ? "application/json;charset=utf-8"
          : "text/markdown;charset=utf-8";
    const filename =
      reviewFormat === "counts" ? "repodna-findings-counts.csv" : "repodna-findings." + extension;
    download(filename, content, mime);
    setReviewFormat(null);
  };

  return (
    <>
      <PageHeader title="Findings">
        Each finding says what was observed, how, with what evidence, and what it cannot tell.
        Severity describes how much attention something deserves, not whether the code is good.
      </PageHeader>
      <ul className="severity-summary" aria-label="Findings by severity; choose one to filter">
        {SEVERITIES.map((level) => (
          <li key={level}>
            <button
              type="button"
              className="tile"
              aria-pressed={severity === level}
              onClick={() => setSeverity(severity === level ? "all" : level)}
            >
              <span className="label">
                <SeverityBadge severity={level} />
              </span>
              <span className="value">{thousands(counts[level])}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="filters">
        <SearchBox
          value={query}
          onChange={setQuery}
          label="Search findings"
          placeholder="Search titles, rules, paths, and evidence"
        />
        <Select
          id="finding-severity"
          label="Severity"
          value={severity}
          options={[
            ["all", "All severities"] as const,
            ...SEVERITIES.map((s) => [s, severityLabel(s)] as const),
          ]}
          onChange={setSeverity}
        />
        <Select
          id="finding-category"
          label="Category"
          value={category}
          options={[
            ["all", "All categories"] as const,
            ...categories.map((c) => [c, categoryLabel(c)] as const),
          ]}
          onChange={setCategory}
        />
        <Select
          id="finding-confidence"
          label="Confidence"
          value={confidence}
          options={[
            ["all", "All confidence levels"] as const,
            ...CONFIDENCES.map((c) => [c, confidenceLabel(c)] as const),
          ]}
          onChange={setConfidence}
        />
        {suppressed > 0 ? (
          <label>
            <input
              type="checkbox"
              checked={showSuppressed}
              onChange={(event) => setShowSuppressed(event.target.checked)}
            />{" "}
            Show {suppressed} suppressed
          </label>
        ) : null}
        <button type="button" className="ghost" disabled={!filtersActive} onClick={resetFilters}>
          Reset filters
        </button>
        <button
          type="button"
          className="ghost"
          disabled={filtered.length === 0}
          onClick={() => setReviewFormat("csv")}
        >
          Export filtered CSV
        </button>
        <button
          type="button"
          className="ghost"
          disabled={filtered.length === 0}
          onClick={() => setReviewFormat("json")}
        >
          Export filtered JSON
        </button>
        <button
          type="button"
          className="ghost"
          disabled={filtered.length === 0}
          onClick={() => setReviewFormat("markdown")}
        >
          Export filtered Markdown
        </button>
        <button
          type="button"
          className="ghost"
          disabled={filtered.length === 0}
          onClick={() => setReviewFormat("counts")}
        >
          Export counts-only CSV
        </button>
        <span className="muted" aria-live="polite">
          {filtered.length > shown
            ? `Showing ${thousands(shown)} of ${thousands(filtered.length)} findings`
            : `${thousands(filtered.length)} ${filtered.length === 1 ? "finding" : "findings"}`}
        </span>
      </div>
      {reviewFormat && review ? (
        <section className="panel" role="region" aria-label="Review findings export">
          <div className="panel-header">
            <h2>
              Review{" "}
              {reviewFormat === "counts"
                ? "counts-only CSV"
                : `filtered ${reviewFormat.toUpperCase()}`}{" "}
              export
            </h2>
          </div>
          <p>
            {reviewFormat === "counts" ? (
              <>
                The aggregate will summarize {thousands(filtered.length)} filtered findings,
                including {thousands(review.suppressed)} suppressed findings.
              </>
            ) : (
              <>
                The full file will contain {thousands(filtered.length)} findings,{" "}
                {thousands(review.paths)} distinct repository paths,{" "}
                {thousands(review.evidenceItems)} evidence items, and{" "}
                {thousands(review.suppressed)} suppressed findings.
              </>
            )}
          </p>
          <Note caution>
            {reviewFormat === "counts" ? (
              <>
                This export contains fixed severity labels and aggregate counts only. It omits
                individual paths, titles, rules and evidence. Even aggregate counts may be sensitive;
                review before sharing.
              </>
            ) : (
              <>
                Repository paths, summaries, and evidence may contain sensitive information. This
                preview does not detect secrets, anonymize content, or remove private data. Review
                the saved file before sharing it.
              </>
            )}
          </Note>
          <p className="muted">
            {reviewFormat === "counts"
              ? "Complete aggregate of the currently filtered findings, without per-finding records."
              : `Local sample of the first ${review.sampled} findings (up to 4,000 characters).`}
            {review.shortened ? " The saved file contains additional content not shown here." : ""}
          </p>
          <pre
            className="note mono"
            tabIndex={0}
            aria-label={
              reviewFormat === "counts"
                ? "Counts-only CSV preview"
                : "Filtered findings export sample"
            }
          >
            {review.preview}
          </pre>
          <div className="actions">
            <button type="button" onClick={saveReviewedExport}>
              {reviewFormat === "counts"
                ? "Save counts-only CSV"
                : `Save full ${reviewFormat.toUpperCase()} export`}
            </button>{" "}
            <button type="button" className="ghost" onClick={() => setReviewFormat(null)}>
              Cancel export
            </button>
          </div>
        </section>
      ) : null}
      {downloadFailure ? <ErrorBox>{downloadFailure}</ErrorBox> : null}
      {filtered.length === 0 ? (
        <p className="muted">
          {dna.findings.length === 0
            ? "This analysis produced no findings."
            : "No findings match these filters."}
        </p>
      ) : (
        <>
          <div className="panel">
            {filtered.slice(0, shown).map((finding) => (
              <FindingItem key={finding.id} finding={finding} level={2} />
            ))}
          </div>
          {filtered.length > shown ? (
            <p className="more">
              <button type="button" onClick={() => setShown((count) => count + FINDINGS_PAGE)}>
                Show {thousands(Math.min(FINDINGS_PAGE, filtered.length - shown))} more
              </button>{" "}
              <button type="button" className="ghost" onClick={() => setShown(filtered.length)}>
                Show all {thousands(filtered.length)}
              </button>
            </p>
          ) : null}
        </>
      )}
    </>
  );
}
