import { useMemo, useState } from "react";
import type { ScanSummary } from "../lib/backend";
import { useDownload } from "../lib/download";
import {
  buildHistoryTrend, gapLabel, HISTORY_METRICS, historyTrendCsv, metricDefinition,
  trendChart, trendOptions, trendPage,
  type HistoryMetric, type TrendFilter, type TrendSort, type TrendSummary,
} from "../lib/historyTrend";
import { ErrorBox, Note } from "./common";

interface Props {
  scans: readonly ScanSummary[];
  partial: boolean;
  onSelect: (scan: ScanSummary) => void;
}
const TABLE_SIZE = 20;
function number(value: number | null): string {
  return value === null ? "Not available" : value.toLocaleString();
}
function delta(value: number | null): string {
  if (value === null) return "—";
  return value > 0 ? `+${number(value)}` : number(value);
}
function percent(value: number | null): string {
  if (value === null) return "—";
  return `${value > 0 ? "+" : ""}${value.toLocaleString(undefined, { maximumFractionDigits: 2 })}%`;
}
function utc(value: number): string {
  return new Date(value).toISOString().replace("T", " ").replace(/\.\d{3}Z$/, " UTC");
}

function HistoryChart({ summary }: { summary: TrendSummary }) {
  const chart = useMemo(() => trendChart(summary), [summary]);
  const width = 680;
  const height = 200;
  const left = 84;
  const right = 20;
  const top = 16;
  const bottom = 30;
  const x = (value: number) => left + value * (width - left - right);
  const y = (value: number) => top + value * (height - top - bottom);
  if (!summary.knownValues) return <p className="muted">No available values to chart for the selected metric.</p>;
  return (
    <figure>
      <svg viewBox={`0 0 ${width} ${height}`} width="100%" role="img" aria-label={`${metricDefinition(summary.metric).label} across saved analyses. Numeric data is also available in the table below.`}>
        {[0, 0.5, 1].map((fraction) => (
          <g key={fraction}>
            <line x1={left} x2={width - right} y1={y(fraction)} y2={y(fraction)} stroke="currentColor" strokeOpacity={0.18} />
            <text x={left - 8} y={y(fraction) + 4} textAnchor="end" fontSize={11} fill="currentColor">{Math.round(chart.maximum * (1 - fraction)).toLocaleString()}</text>
          </g>
        ))}
        {chart.points.map((point, index) => {
          const previous = chart.points[index - 1];
          return point.connected && previous?.y != null && point.y !== null
            ? <line key={`line-${point.point.id}`} x1={x(previous.x)} y1={y(previous.y)} x2={x(point.x)} y2={y(point.y)} stroke="var(--s1, currentColor)" strokeWidth={2} />
            : null;
        })}
        {chart.points.map((point) => point.y === null ? null : (
          <circle key={point.point.id} cx={x(point.x)} cy={y(point.y)} r={3} fill="var(--s1, currentColor)">
            <title>{utc(point.point.timestamp)} · {number(point.point.value)} · {point.point.profile}</title>
          </circle>
        ))}
        {chart.firstTime !== null ? <text x={left} y={height - 6} fontSize={11} fill="currentColor">{new Date(chart.firstTime).toISOString().slice(0, 10)}</text> : null}
        {chart.lastTime !== null ? <text x={width - right} y={height - 6} textAnchor="end" fontSize={11} fill="currentColor">{new Date(chart.lastTime).toISOString().slice(0, 10)}</text> : null}
      </svg>
      <figcaption className="muted">Dates use UTC; horizontal distances reflect elapsed time. Breaks indicate unavailable or non-comparable intervals. {chart.sampled ? `The chart samples ${chart.points.length} of ${summary.points.length} scans; the table and export retain all selected records.` : "Each visible point is one saved scan."}</figcaption>
    </figure>
  );
}

/** A trend of recorded summary values from the same repository's saved scans.
 * No additional repository read is started: the comparison picker owns history
 * pagination and this panel consumes the already-loaded scan summaries. */
export function HistoryTrendPanel({ scans, partial, onSelect }: Props) {
  const [metric, setMetric] = useState<HistoryMetric>("codeLines");
  const [filter, setFilter] = useState<TrendFilter>({});
  const [sort, setSort] = useState<TrendSort>("newest");
  const [page, setPage] = useState(0);
  const [download, downloadError] = useDownload();
  const options = useMemo(() => trendOptions(scans), [scans]);
  const result = useMemo(() => {
    try { return { summary: buildHistoryTrend(scans, metric, filter, partial), error: null }; }
    catch (reason) { return { summary: null, error: reason instanceof Error ? reason.message : String(reason) }; }
  }, [scans, metric, filter, partial]);
  const summary = result.summary;
  const table = useMemo(() => trendPage(summary?.points ?? [], page, TABLE_SIZE, sort), [summary, page, sort]);
  const updateFilter = (change: Partial<TrendFilter>) => {
    setFilter((old) => ({ ...old, ...change }));
    setPage(0);
  };
  const scanById = useMemo(() => new Map(scans.map((scan) => [scan.id, scan])), [scans]);
  return (
    <details>
      <summary>Saved analysis trends</summary>
      <p>Explore how recorded measurements vary across saved analyses of this repository. These values are snapshots, not a measure of lines contributed to Git.</p>
      <div className="filters">
        <label htmlFor="history-trend-metric">Metric</label>
        <select id="history-trend-metric" value={metric} onChange={(event) => { setMetric(event.target.value as HistoryMetric); setPage(0); }}>
          {HISTORY_METRICS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}
        </select>
        <label htmlFor="history-trend-profile">Profile</label>
        <select id="history-trend-profile" value={filter.profile ?? ""} onChange={(event) => updateFilter({ profile: event.target.value })}>
          <option value="">All profiles</option>
          {options.profiles.map((value) => <option key={value} value={value}>{value}</option>)}
        </select>
        <label htmlFor="history-trend-version">Analyzer version</label>
        <select id="history-trend-version" value={filter.toolVersion ?? ""} onChange={(event) => updateFilter({ toolVersion: event.target.value })}>
          <option value="">All versions</option>
          {options.toolVersions.map((value) => <option key={value} value={value}>{value}</option>)}
        </select>
        <label htmlFor="history-trend-from">From date (UTC)</label>
        <input id="history-trend-from" type="date" value={filter.from ?? ""} onChange={(event) => updateFilter({ from: event.target.value })} />
        <label htmlFor="history-trend-through">Through date (UTC)</label>
        <input id="history-trend-through" type="date" value={filter.through ?? ""} onChange={(event) => updateFilter({ through: event.target.value })} />
        <label htmlFor="history-trend-revision">Revision prefix</label>
        <input id="history-trend-revision" value={filter.revision ?? ""} maxLength={64} onChange={(event) => updateFilter({ revision: event.target.value })} />
        <button type="button" onClick={() => { setFilter({}); setPage(0); }}>Clear trend filters</button>
      </div>
      <p className="muted">{metricDefinition(metric).description}</p>
      {result.error ? <ErrorBox>{result.error}</ErrorBox> : null}
      {summary ? (
        <>
          {summary.partialHistory ? <Note caution>Older saved scans have not all been loaded. Use the history loading controls above to extend this range.</Note> : null}
          {summary.truncated ? <Note caution>The trend is limited to the first 10,000 loaded scans.</Note> : null}
          {summary.invalidScans || summary.duplicateScans ? <Note caution>{summary.invalidScans} invalid and {summary.duplicateScans} duplicate scan records were omitted.</Note> : null}
          {summary.points.length ? (
            <>
              <dl className="facts wide">
                <dt>Selected scans</dt><dd>{summary.points.length} ({summary.filteredOut} excluded by filters)</dd>
                <dt>Available values</dt><dd>{summary.knownValues} · {summary.missingValues} unavailable</dd>
                <dt>Lowest / highest recorded</dt><dd>{number(summary.min)} / {number(summary.max)}</dd>
                <dt>First / last recorded</dt><dd>{number(summary.first?.value ?? null)} / {number(summary.last?.value ?? null)}</dd>
                <dt>Observed net change</dt><dd>{summary.netChange === null ? "Not established" : `${delta(summary.netChange)} (${percent(summary.netPercent)})`}</dd>
                <dt>Visible intervals</dt><dd>{summary.comparableIntervals} with matching profile and version; {summary.interruptedIntervals} interrupted</dd>
              </dl>
              <Note caution>Scan summaries do not record complete configuration or stage coverage. Even matching profile and version do not guarantee the same branch, input, ignore rules or analysis completeness. Differences are recorded observations, not proof of source-code changes.</Note>
              <HistoryChart summary={summary} />
              <div className="filters">
                <label htmlFor="history-trend-sort">Table order</label>
                <select id="history-trend-sort" value={sort} onChange={(event) => { setSort(event.target.value as TrendSort); setPage(0); }}>
                  <option value="newest">Newest first</option><option value="oldest">Oldest first</option>
                  <option value="largest">Highest value</option><option value="smallest">Lowest value</option>
                </select>
                <button type="button" onClick={() => download("repodna-saved-history.csv", historyTrendCsv(summary), "text/csv;charset=utf-8")}>Export selected trend CSV</button>
              </div>
              {downloadError ? <ErrorBox>{downloadError}</ErrorBox> : null}
              <p className="muted">The export contains scan IDs, dates, revision identifiers, profiles, versions and selected measurements. Review it before sharing.</p>
              <div className="table-scroll">
                <table>
                  <thead><tr><th scope="col">Scan time</th><th scope="col">Profile / version</th><th scope="col">Revision</th><th scope="col">Value</th><th scope="col">Observed change</th><th scope="col">Action</th></tr></thead>
                  <tbody>{table.rows.map((point) => (
                    <tr key={point.id}>
                      <th scope="row">{utc(point.timestamp)}</th>
                      <td>{point.profile || "Unknown"} · {point.toolVersion || "Unknown"}</td>
                      <td className="mono" title={point.revision ?? undefined}>{point.revision?.slice(0, 12) ?? "Unknown"}</td>
                      <td>{number(point.value)}</td>
                      <td>
                        {point.gap ? <span title={gapLabel(point.gap)}>— {gapLabel(point.gap)}</span> : `${delta(point.delta)}${point.percentChange === null ? "" : ` (${percent(point.percentChange)})`}`}
                        {point.sameRevision ? <span className="muted"> · same revision</span> : null}
                        {point.sameFingerprint ? <span className="muted"> · same fingerprint</span> : null}
                      </td>
                      <td><button type="button" onClick={() => { const scan = scanById.get(point.id); if (scan) onSelect(scan); }} aria-label={`Compare with analysis at ${utc(point.timestamp)}, ${point.id}`}>Compare</button></td>
                    </tr>
                  ))}</tbody>
                </table>
              </div>
              {table.pages > 1 ? <div className="actions"><button type="button" disabled={table.page === 0} onClick={() => setPage(table.page - 1)}>Previous trend page</button><span>Page {table.page + 1} of {table.pages}</span><button type="button" disabled={table.page + 1 >= table.pages} onClick={() => setPage(table.page + 1)}>Next trend page</button></div> : null}
            </>
          ) : <p className="muted">No saved scans match these filters.</p>}
        </>
      ) : null}
    </details>
  );
}
