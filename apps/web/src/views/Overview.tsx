import { confidenceLabel, findingCounts, type RepositoryDna } from "@repodna/schema";
import { categorize, compact, date, percent, thousands, type Theme } from "@repodna/visualization";
import { ShareBar } from "../charts/ShareBar";
import { Chip, Note, PageHeader, Panel, SectionStatus, Tile } from "../components/common";
import { DataTable } from "../components/DataTable";
import { FindingItem } from "../components/FindingItem";
import { highlights } from "../lib/findings";
import { count, languageNamer, unitFor } from "../lib/names";
import { href } from "../lib/router";
import { originLabel, useApp, useDataset } from "../state";

/**
 * Shares of first-party code in programming, markup, and stylesheet languages (as computed
 * by the analysis), folded to seven named languages plus "Other", colored by fixed slot.
 */
export function languageShares(dna: RepositoryDna, theme: Theme) {
  const languages = dna.languages.languages.filter((language) => language.share > 0);
  return categorize(
    languages,
    (l) => l.share,
    (l) => l.name,
    theme,
  ).map((c) => ({
    label: c.label,
    value: c.value,
    color: c.color,
  }));
}

export function Overview() {
  const { dna, warnings, origin } = useDataset();
  const { theme } = useApp();
  const counts = findingCounts(dna);
  const git = dna.git;
  const shares = languageShares(dna, theme);
  const active = dna.findings.filter((f) => !f.suppressed);
  const revision = dna.analysisMetadata.revision?.slice(0, 12);
  const languageName = languageNamer(dna);
  const primary = dna.languages.primary[0] ?? dna.languages.languages[0]?.id;
  return (
    <>
      <PageHeader title={dna.identity.name}>
        {dna.identity.description ?? "No description was found in the README or manifests."}
      </PageHeader>
      <p className="muted" style={{ marginTop: -8 }}>
        {originLabel({ dna, origin, warnings })}
        {revision ? ` · revision ${revision}` : ""} · {dna.analysisMetadata.profile} profile ·
        RepoDNA {dna.tool.version}
      </p>
      {origin.kind !== "stored" ? (
        <Note>
          This is a snapshot of the repository as it was analyzed on{" "}
          {date(dna.analysisMetadata.generatedAt)}, not its current state.
        </Note>
      ) : null}
      {warnings.map((warning) => (
        <Note key={warning} caution>
          {warning}
        </Note>
      ))}
      <div className="tiles" style={{ marginTop: 16 }}>
        <Tile
          label="Files"
          value={compact(dna.structure.totalFiles)}
          note={`${dna.structure.sizeClass} repository`}
        />
        <Tile label="Code lines" value={compact(dna.structure.codeLines)} />
        <Tile
          label="Primary language"
          value={languageName(primary)}
          note={count(dna.languages.languages.length, "language", "languages")}
        />
        <Tile
          label="Commits"
          value={git.commitCount > 0 ? compact(git.commitCount) : "–"}
          note={
            git.commitCount > 0
              ? count(git.ownership.contributors, "contributor", "contributors")
              : "No Git history analyzed"
          }
        />
        <Tile
          label="Architecture"
          value={dna.architecture.style || "–"}
          note={
            dna.architecture.style
              ? `${confidenceLabel(dna.architecture.styleConfidence)} confidence · ${dna.architecture.modules.length} modules`
              : undefined
          }
        />
        <Tile
          label="Findings"
          value={thousands(counts.critical + counts.warning + counts.attention + counts.info)}
          note={`${counts.critical} critical · ${count(counts.warning, "warning", "warnings")}`}
        />
      </div>

      <div className="grid two">
        <Panel
          title="Language map"
          description="Share of first-party code in programming, markup, and stylesheet languages. Generated and vendored files are not counted; the table also lists data and prose files."
          table={
            <DataTable
              rows={dna.languages.languages}
              rowKey={(l) => l.id}
              columns={[
                { key: "name", header: "Language", cell: (l) => l.name, sort: (l) => l.name },
                { key: "kind", header: "Kind", cell: (l) => l.kind, sort: (l) => l.kind },
                {
                  key: "files",
                  header: "Files",
                  cell: (l) => thousands(l.files),
                  sort: (l) => l.files,
                  numeric: true,
                },
                {
                  key: "code",
                  header: "Code lines",
                  cell: (l) => thousands(l.codeLines),
                  sort: (l) => l.codeLines,
                  numeric: true,
                },
                {
                  key: "share",
                  header: "Share",
                  cell: (l) => percent(l.share),
                  sort: (l) => l.share,
                  numeric: true,
                },
              ]}
              initialSort={{ key: "code", descending: true }}
            />
          }
        >
          {shares.length > 0 ? (
            <ShareBar shares={shares} label="Share of first-party code by language" />
          ) : (
            <p className="muted">No code was detected.</p>
          )}
        </Panel>

        <Panel
          title="Project DNA"
          description="Each dimension is scaled from 0 to 1 and describes the repository; none of them is a grade."
          table={
            <DataTable
              rows={dna.fingerprint.dimensions}
              rowKey={(d) => d.id}
              columns={[
                { key: "label", header: "Dimension", cell: (d) => d.label },
                {
                  key: "value",
                  header: "Value",
                  cell: (d) => d.value.toFixed(2),
                  sort: (d) => d.value,
                  numeric: true,
                },
                {
                  key: "raw",
                  header: "Measured",
                  cell: (d) => `${Number(d.raw.toFixed(2))} ${unitFor(d.raw, d.unit)}`,
                  numeric: true,
                },
                {
                  key: "confidence",
                  header: "Confidence",
                  cell: (d) => confidenceLabel(d.confidence),
                },
              ]}
            />
          }
        >
          {dna.fingerprint.dimensions.map((dimension) => (
            <div key={dimension.id} style={{ marginBottom: 10 }}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                <span>{dimension.label}</span>
                <span className="muted">{dimension.value.toFixed(2)}</span>
              </div>
              <div
                className="meter"
                role="meter"
                aria-label={dimension.label}
                aria-valuemin={0}
                aria-valuemax={1}
                aria-valuenow={dimension.value}
                title={dimension.description}
              >
                <div style={{ width: `${Math.round(dimension.value * 100)}%` }} />
              </div>
            </div>
          ))}
          <p className="muted mono" style={{ marginBottom: 0 }}>
            DNA hash {dna.fingerprint.dnaHash}
          </p>
        </Panel>
      </div>

      <Panel
        title="First look"
        description="Answers built from the analysis; each is labeled as a fact or an interpretation."
      >
        {dna.insights.firstLook.length === 0 ? (
          <p className="muted">No first-look answers in this analysis.</p>
        ) : null}
        {dna.insights.firstLook.map((answer) => (
          <div key={answer.id} className="finding">
            <h3>
              {answer.question}
              <Chip>{answer.kind === "fact" ? "Fact" : "Interpretation"}</Chip>
              <Chip>{confidenceLabel(answer.confidence)} confidence</Chip>
            </h3>
            <p>{answer.answer}</p>
          </div>
        ))}
      </Panel>

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Where to start reading"
          description="Files ranked by how central, active, and documented they are."
        >
          <DataTable
            rows={dna.insights.importantFiles}
            rowKey={(f) => f.path}
            limit={10}
            columns={[
              { key: "path", header: "File", cell: (f) => <span className="path">{f.path}</span> },
              { key: "why", header: "Why", cell: (f) => f.reasons.join("; ") },
            ]}
          />
        </Panel>
        <Panel
          title="Most important findings"
          description={
            <>
              Signals backed by evidence, not verdicts. <a href={href("/findings")}>All findings</a>
            </>
          }
        >
          {active.length === 0 ? <p className="muted">No findings.</p> : null}
          {highlights(active, 4).map((finding) => (
            <FindingItem key={finding.id} finding={finding} />
          ))}
        </Panel>
      </div>

      {dna.insights.recentChanges ? (
        <Panel
          title={`Last ${dna.insights.recentChanges.windowDays} days`}
          description="What changed recently."
        >
          <div className="tiles">
            <Tile label="Commits" value={thousands(dna.insights.recentChanges.commits)} />
            <Tile label="Contributors" value={thousands(dna.insights.recentChanges.contributors)} />
            <Tile
              label="Files changed"
              value={thousands(dna.insights.recentChanges.filesChanged)}
            />
            <Tile label="Lines added" value={compact(dna.insights.recentChanges.insertions)} />
            <Tile label="Lines removed" value={compact(dna.insights.recentChanges.deletions)} />
          </div>
          <DataTable
            rows={dna.insights.recentChanges.directories}
            rowKey={(d) => d.path}
            limit={8}
            columns={[
              {
                key: "path",
                header: "Area",
                cell: (d) => <span className="path">{d.path || "(root)"}</span>,
              },
              {
                key: "commits",
                header: "Commits",
                cell: (d) => thousands(d.commits),
                sort: (d) => d.commits,
                numeric: true,
              },
              {
                key: "churn",
                header: "Lines changed",
                cell: (d) => thousands(d.churn),
                sort: (d) => d.churn,
                numeric: true,
              },
            ]}
          />
        </Panel>
      ) : null}
      <SectionStatus status={dna.structure.status} notes={dna.structure.notes} />
    </>
  );
}
