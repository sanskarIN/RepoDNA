import { useEffect, useMemo, useState } from "react";
import {
  SEVERITIES,
  categoryLabel,
  describeEvidence,
  findingCounts,
  isSuppressed,
  severityLabel,
  severityRank,
  type Finding,
  type FindingCategory,
  type Severity,
} from "@repodna/schema";
import { thousands } from "@repodna/visualization";
import { PageHeader, SearchBox, Select, SeverityBadge } from "../components/common";
import { FindingItem } from "../components/FindingItem";
import { useRoute } from "../lib/router";
import { useDataset } from "../state";

function searchable(finding: Finding): string {
  return [
    finding.title,
    finding.summary,
    finding.rule,
    finding.category,
    ...(finding.paths ?? []),
    ...finding.evidence.map(describeEvidence),
  ]
    .join(" ")
    .toLowerCase();
}

/** Findings shown at first, and added each time more are asked for. */
export const FINDINGS_PAGE = 25;

export function Findings() {
  const { dna } = useDataset();
  const route = useRoute();
  const [query, setQuery] = useState(() => route.params.get("q") ?? "");
  const [severity, setSeverity] = useState<"all" | Severity>(() => {
    const value = route.params.get("severity");
    return (SEVERITIES as readonly string[]).includes(value ?? "") ? (value as Severity) : "all";
  });
  const [category, setCategory] = useState<"all" | FindingCategory>(
    () => (route.params.get("category") as FindingCategory | null) ?? "all",
  );
  const [showSuppressed, setShowSuppressed] = useState(false);
  const [shown, setShown] = useState(FINDINGS_PAGE);
  const counts = findingCounts(dna);
  const categories = useMemo(
    () => [...new Set(dna.findings.map((f) => f.category))].sort(),
    [dna.findings],
  );
  const texts = useMemo(
    () => new Map(dna.findings.map((f) => [f.id, searchable(f)])),
    [dna.findings],
  );
  const filtered = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    return (
      dna.findings
        .filter(
          (f) =>
            (showSuppressed || !isSuppressed(f)) &&
            (severity === "all" || f.severity === severity) &&
            (category === "all" || f.category === category) &&
            terms.every((term) => (texts.get(f.id) ?? "").includes(term)),
        )
        // Findings are stored in display order (by severity, category, and rule, each rule in
        // its analyzer's order), so a stable sort by severity keeps that order.
        .sort((a, b) => severityRank(b.severity) - severityRank(a.severity))
    );
  }, [dna.findings, query, severity, category, showSuppressed, texts]);
  const suppressed = dna.findings.filter(isSuppressed).length;
  // Changing a filter starts again from the first page.
  useEffect(() => setShown(FINDINGS_PAGE), [query, severity, category, showSuppressed]);

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
        <span className="muted" aria-live="polite">
          {filtered.length > shown
            ? `Showing ${thousands(shown)} of ${thousands(filtered.length)} findings`
            : `${thousands(filtered.length)} ${filtered.length === 1 ? "finding" : "findings"}`}
        </span>
      </div>
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
