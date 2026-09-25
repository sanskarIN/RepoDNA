import { thousands } from "@repodna/visualization";
import { TreemapChart } from "../charts/TreemapChart";
import { EvidenceList, Omittable, PageHeader, Panel, SectionStatus } from "../components/common";
import { DataTable } from "../components/DataTable";
import { useDataset } from "../state";

export function Hotspots() {
  const { dna } = useDataset();
  const git = dna.git;
  const hotspots = git.hotSpots;
  const functions = dna.codeQuality.complexity.topFunctions;
  const churn = [...git.fileHistory].sort((a, b) => b.commits - a.commits);

  return (
    <>
      <PageHeader title="Hotspots">
        Files that change often <em>and</em> are large, complex, or widely used. Changes there cost
        the most, so they are the first places to review, test, and document.
      </PageHeader>
      {hotspots.length === 0 ? (
        <>
          <SectionStatus
            status={git.status === "analyzed" ? "analyzed" : git.status}
            notes={git.notes}
          />
          <p className="muted">
            {git.commitCount === 0
              ? "Hotspots need Git history, and this analysis has none."
              : `No file reached ${dna.analysisMetadata.thresholds.hotspot_min_commits} commits, the minimum for a hotspot.`}
          </p>
        </>
      ) : (
        <>
          <Panel
            title="Hotspot map"
            description="Area shows lines of code; darker cells changed more often recently."
            table={
              <DataTable
                rows={hotspots}
                rowKey={(h) => h.path}
                initialSort={{ key: "rank", descending: false }}
                columns={[
                  {
                    key: "rank",
                    header: "Rank",
                    cell: (h) => h.rank,
                    sort: (h) => h.rank,
                    numeric: true,
                  },
                  {
                    key: "path",
                    header: "File",
                    cell: (h) => <span className="path">{h.path}</span>,
                    sort: (h) => h.path,
                  },
                  {
                    key: "score",
                    header: "Score",
                    cell: (h) => h.score.toFixed(2),
                    sort: (h) => h.score,
                    numeric: true,
                  },
                  {
                    key: "commits",
                    header: "Commits",
                    cell: (h) => thousands(h.commits),
                    sort: (h) => h.commits,
                    numeric: true,
                  },
                  {
                    key: "recent",
                    header: "Recent commits",
                    cell: (h) => thousands(h.recentCommits),
                    sort: (h) => h.recentCommits,
                    numeric: true,
                  },
                  {
                    key: "churn",
                    header: "Lines changed",
                    cell: (h) => thousands(h.churn),
                    sort: (h) => h.churn,
                    numeric: true,
                  },
                  {
                    key: "complexity",
                    header: "Complexity",
                    cell: (h) => thousands(h.complexity),
                    sort: (h) => h.complexity,
                    numeric: true,
                  },
                  {
                    key: "dependents",
                    header: "Used by files",
                    cell: (h) => thousands(h.dependents),
                    sort: (h) => h.dependents,
                    numeric: true,
                  },
                  {
                    key: "lines",
                    header: "Lines",
                    cell: (h) => thousands(h.lines),
                    sort: (h) => h.lines,
                    numeric: true,
                  },
                ]}
              />
            }
          >
            <TreemapChart
              label={`Treemap of ${hotspots.length} hotspots sized by lines and shaded by recent commits`}
              data={hotspots.map((h) => ({
                label: h.path,
                size: Math.max(1, h.lines),
                heat: h.recentCommits,
                details: `rank ${h.rank} · ${thousands(h.commits)} commits · complexity ${thousands(h.complexity)}`,
              }))}
              sizeUnit="lines"
              heatLabel={`commits in the last ${git.recentWindowDays} days`}
            />
          </Panel>

          <Panel
            title="Why these files"
            description="The top hotspots with what makes each one stand out."
          >
            {hotspots.slice(0, 8).map((h) => (
              <div key={h.path} className="finding">
                <h3>
                  <span className="muted">#{h.rank}</span> <span className="path">{h.path}</span>
                </h3>
                <p>{h.interpretation}</p>
                <ul className="evidence">
                  {h.reasons.map((reason) => (
                    <li key={reason}>{reason}</li>
                  ))}
                </ul>
                {h.evidence.length > 0 ? (
                  <details>
                    <summary>Evidence</summary>
                    <EvidenceList evidence={h.evidence} />
                  </details>
                ) : null}
              </div>
            ))}
          </Panel>
        </>
      )}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Most complex functions" description={dna.codeQuality.complexity.method}>
          {functions.length === 0 ? (
            <p className="muted">No functions were measured.</p>
          ) : (
            <DataTable
              rows={functions}
              rowKey={(f) => `${f.path}:${f.line}:${f.name}`}
              initialSort={{ key: "cyclomatic", descending: true }}
              limit={12}
              columns={[
                {
                  key: "name",
                  header: "Function",
                  cell: (f) => <Omittable text={f.name} mono />,
                  sort: (f) => f.name,
                },
                {
                  key: "where",
                  header: "Where",
                  cell: (f) => <span className="path">{`${f.path}:${f.line}`}</span>,
                },
                {
                  key: "cyclomatic",
                  header: "Complexity",
                  cell: (f) => thousands(f.cyclomatic),
                  sort: (f) => f.cyclomatic,
                  numeric: true,
                },
                {
                  key: "lines",
                  header: "Lines",
                  cell: (f) => thousands(f.lines),
                  sort: (f) => f.lines,
                  numeric: true,
                },
                {
                  key: "nesting",
                  header: "Nesting",
                  cell: (f) => f.nesting,
                  sort: (f) => f.nesting,
                  numeric: true,
                },
              ]}
            />
          )}
        </Panel>
        <Panel title="Files that change most">
          {churn.length === 0 ? (
            <p className="muted">No file history in this analysis.</p>
          ) : (
            <DataTable
              rows={churn}
              rowKey={(f) => f.path}
              initialSort={{ key: "commits", descending: true }}
              limit={12}
              columns={[
                {
                  key: "path",
                  header: "File",
                  cell: (f) => (
                    <span
                      className="path"
                      title={
                        (f.previousPaths ?? []).length > 0
                          ? `Previously ${(f.previousPaths ?? []).join(", ")}`
                          : undefined
                      }
                    >
                      {f.path}
                    </span>
                  ),
                  sort: (f) => f.path,
                },
                {
                  key: "commits",
                  header: "Commits",
                  cell: (f) => thousands(f.commits),
                  sort: (f) => f.commits,
                  numeric: true,
                },
                {
                  key: "recent",
                  header: "Recent",
                  cell: (f) => thousands(f.recentCommits),
                  sort: (f) => f.recentCommits,
                  numeric: true,
                },
                {
                  key: "authors",
                  header: "Authors",
                  cell: (f) => thousands(f.authors),
                  sort: (f) => f.authors,
                  numeric: true,
                },
                {
                  key: "lines",
                  header: "Lines changed",
                  cell: (f) => thousands(f.insertions + f.deletions),
                  sort: (f) => f.insertions + f.deletions,
                  numeric: true,
                },
              ]}
            />
          )}
        </Panel>
      </div>
    </>
  );
}
