import { useState } from "react";
import { confidenceLabel, type MarkerKind } from "@repodna/schema";
import { percent, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import { Columns } from "../charts/Columns";
import {
  Chip,
  EvidenceList,
  Omittable,
  PageHeader,
  Panel,
  SectionStatus,
  Tile,
} from "../components/common";
import { DataTable } from "../components/DataTable";
import { languageNamer, unitFor } from "../lib/names";
import { useDataset } from "../state";

const MARKERS: Record<MarkerKind, string> = {
  todo: "TODO",
  fixme: "FIXME",
  hack: "HACK",
  xxx: "XXX",
  bug: "BUG",
  deprecated: "Deprecated",
};

export function Quality() {
  const { dna } = useDataset();
  const quality = dna.codeQuality;
  const complexity = quality.complexity;
  const duplication = quality.duplication;
  const similar = dna.similarity;
  const thresholds = dna.analysisMetadata.thresholds;
  const [allCandidates, setAllCandidates] = useState(false);
  const languageName = languageNamer(dna);
  const candidates = allCandidates
    ? quality.deadCodeCandidates
    : quality.deadCodeCandidates.slice(0, 12);

  return (
    <>
      <PageHeader title="Code quality">
        Measurements that point at code worth a closer look. They are signals for review, not
        grades: a complex function can be exactly as complex as its problem.
      </PageHeader>
      <SectionStatus status={quality.status} notes={quality.notes} />
      <div className="tiles">
        <Tile label="Functions measured" value={thousands(complexity.functionsAnalyzed)} />
        <Tile
          label="Typical complexity"
          value={complexity.functionsAnalyzed > 0 ? complexity.medianCyclomatic.toFixed(1) : "–"}
          note={
            complexity.functionsAnalyzed > 0
              ? `median; average ${complexity.averageCyclomatic.toFixed(1)}`
              : undefined
          }
        />
        <Tile
          label="Duplicated lines"
          value={
            duplication.status === "analyzed" || duplication.status === "partial"
              ? percent(duplication.ratio)
              : "–"
          }
          note={
            duplication.status === "skipped"
              ? "Not analyzed with this profile"
              : `${thousands(duplication.clusters.length)} groups`
          }
        />
        <Tile
          label="Large functions"
          value={thousands(quality.largeFunctions.length)}
          note={`over ${thresholds.large_function_lines} lines`}
        />
        <Tile
          label="Large files"
          value={thousands(quality.largeFiles.length)}
          note={`over ${thresholds.large_file_lines} lines`}
        />
        <Tile
          label="Markers"
          value={thousands(quality.markers.total)}
          note="TODO, FIXME, and similar"
        />
      </div>

      <div className="grid two">
        <Panel
          title="Complexity distribution"
          description="How many functions fall in each cyclomatic complexity range."
          table={
            <DataTable
              rows={complexity.distribution}
              rowKey={(b) => b.label}
              columns={[
                { key: "label", header: "Complexity", cell: (b) => b.label },
                {
                  key: "count",
                  header: "Functions",
                  cell: (b) => thousands(b.count),
                  numeric: true,
                },
              ]}
            />
          }
        >
          {complexity.functionsAnalyzed === 0 ? (
            <p className="muted">No functions were measured.</p>
          ) : (
            <Columns
              label="Functions per complexity range"
              points={complexity.distribution.map((b) => ({ label: b.label, value: b.count }))}
              unit={[" function", " functions"]}
            />
          )}
        </Panel>
        <Panel
          title="Complexity by language"
          table={
            <DataTable
              rows={complexity.byLanguage}
              rowKey={(l) => l.language}
              columns={[
                {
                  key: "language",
                  header: "Language",
                  cell: (l) => languageName(l.language),
                  sort: (l) => languageName(l.language),
                },
                {
                  key: "functions",
                  header: "Functions",
                  cell: (l) => thousands(l.functions),
                  sort: (l) => l.functions,
                  numeric: true,
                },
                {
                  key: "average",
                  header: "Average",
                  cell: (l) => l.average.toFixed(1),
                  sort: (l) => l.average,
                  numeric: true,
                },
                {
                  key: "max",
                  header: "Highest",
                  cell: (l) => thousands(l.max),
                  sort: (l) => l.max,
                  numeric: true,
                },
              ]}
            />
          }
        >
          {complexity.byLanguage.length === 0 ? (
            <p className="muted">No functions were measured.</p>
          ) : (
            <BarList
              label="Average cyclomatic complexity by language"
              format={(v) => v.toFixed(1)}
              bars={[...complexity.byLanguage]
                .sort((a, b) => b.functions - a.functions)
                .slice(0, 10)
                .map((l) => ({
                  label: languageName(l.language),
                  value: l.average,
                  details: `${thousands(l.functions)} functions · highest ${thousands(l.max)}`,
                }))}
            />
          )}
        </Panel>
      </div>
      <p className="muted">
        <strong>Method:</strong> {complexity.method}
      </p>

      <div className="grid two">
        <Panel
          title="Large functions"
          description={`Longer than ${thresholds.large_function_lines} lines.`}
        >
          <DataTable
            rows={quality.largeFunctions}
            rowKey={(f) => `${f.path}:${f.line}:${f.name}`}
            initialSort={{ key: "lines", descending: true }}
            limit={10}
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
                key: "lines",
                header: "Lines",
                cell: (f) => thousands(f.lines),
                sort: (f) => f.lines,
                numeric: true,
              },
            ]}
          />
        </Panel>
        <Panel
          title="Deep nesting"
          description={`Nested more than ${thresholds.deep_nesting} levels.`}
        >
          <DataTable
            rows={quality.deepNesting}
            rowKey={(f) => `${f.path}:${f.line}:${f.name}`}
            initialSort={{ key: "nesting", descending: true }}
            limit={10}
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
                key: "nesting",
                header: "Depth",
                cell: (f) => f.nesting,
                sort: (f) => f.nesting,
                numeric: true,
              },
            ]}
          />
        </Panel>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Large files" description={`More than ${thresholds.large_file_lines} lines.`}>
          <DataTable
            rows={quality.largeFiles}
            rowKey={(f) => f.path}
            initialSort={{ key: "value", descending: true }}
            limit={10}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (f) => <span className="path">{f.path}</span>,
                sort: (f) => f.path,
              },
              {
                key: "value",
                header: "Size",
                cell: (f) => `${thousands(f.value)} ${unitFor(f.value, f.unit)}`,
                sort: (f) => f.value,
                numeric: true,
              },
            ]}
          />
        </Panel>
        <Panel title="Maintainability signals">
          {quality.maintainability.length === 0 ? <p className="muted">None.</p> : null}
          <DataTable
            rows={quality.maintainability}
            rowKey={(s) => s.id}
            columns={[
              { key: "label", header: "Signal", cell: (s) => s.label },
              {
                key: "value",
                header: "Value",
                cell: (s) => `${Number(s.value.toFixed(2))} ${unitFor(s.value, s.unit)}`,
                numeric: true,
              },
              { key: "description", header: "Meaning", cell: (s) => s.description },
            ]}
          />
        </Panel>
      </div>

      <Panel
        title="Duplicated code"
        description={
          duplication.status === "skipped"
            ? "Not analyzed with this profile; use the deep profile to find duplicated code."
            : `Blocks of at least ${duplication.minTokens} identical tokens. ${thousands(duplication.duplicatedLines)} of ${thousands(duplication.analyzedLines)} lines are duplicated.`
        }
      >
        {duplication.clusters.length === 0 ? (
          <p className="muted">
            {duplication.status === "skipped" ? "Skipped." : "No duplicated blocks were found."}
          </p>
        ) : (
          <DataTable
            rows={duplication.clusters}
            rowKey={(c) => c.id}
            initialSort={{ key: "lines", descending: true }}
            limit={10}
            columns={[
              {
                key: "lines",
                header: "Lines",
                cell: (c) => thousands(c.lines),
                sort: (c) => c.lines,
                numeric: true,
              },
              {
                key: "count",
                header: "Copies",
                cell: (c) => thousands(c.occurrenceCount),
                sort: (c) => c.occurrenceCount,
                numeric: true,
              },
              { key: "language", header: "Language", cell: (c) => languageName(c.language) },
              {
                key: "where",
                header: "Where",
                cell: (c) => (
                  <span className="path">
                    {c.occurrences
                      .slice(0, 3)
                      .map((o) => `${o.path}:${o.startLine}–${o.endLine}`)
                      .join(", ")}
                    {c.occurrences.length > 3 ? ` and ${c.occurrences.length - 3} more` : ""}
                  </span>
                ),
              },
            ]}
          />
        )}
        {duplication.method ? <p className="muted">{duplication.method}</p> : null}
      </Panel>

      {similar.similarFiles.length > 0 ? (
        <Panel
          title="Similar files"
          description={`Pairs at least ${percent(similar.threshold, 0)} alike. ${similar.method}`}
        >
          <DataTable
            rows={similar.similarFiles}
            rowKey={(p) => `${p.a}|${p.b}`}
            initialSort={{ key: "similarity", descending: true }}
            limit={10}
            columns={[
              { key: "a", header: "File", cell: (p) => <span className="path">{p.a}</span> },
              { key: "b", header: "Similar to", cell: (p) => <span className="path">{p.b}</span> },
              {
                key: "similarity",
                header: "Similarity",
                cell: (p) => percent(p.similarity, 0),
                sort: (p) => p.similarity,
                numeric: true,
              },
            ]}
          />
        </Panel>
      ) : null}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Markers"
          description="Comments that flag unfinished or questionable code."
          table={
            <DataTable
              rows={quality.markers.items}
              rowKey={(m, index) => `${m.path}:${m.line}:${index}`}
              columns={[
                {
                  key: "kind",
                  header: "Marker",
                  cell: (m) => MARKERS[m.kind],
                  sort: (m) => m.kind,
                },
                {
                  key: "where",
                  header: "Where",
                  cell: (m) => <span className="path">{`${m.path}:${m.line}`}</span>,
                  sort: (m) => m.path,
                },
                { key: "text", header: "Text", cell: (m) => <Omittable text={m.text} /> },
              ]}
            />
          }
        >
          {quality.markers.total === 0 ? (
            <p className="muted">No markers were found.</p>
          ) : (
            <BarList
              label="Markers by kind"
              bars={[...quality.markers.counts]
                .sort((a, b) => b.count - a.count)
                .map((m) => ({ label: MARKERS[m.kind], value: m.count }))}
            />
          )}
        </Panel>
        <Panel
          title="Possibly unused code"
          description="Candidates only: dynamic loading, reflection, and build scripts can use code that nothing imports."
        >
          {quality.deadCodeCandidates.length === 0 ? (
            <p className="muted">No candidates.</p>
          ) : (
            candidates.map((candidate) => (
              <div key={`${candidate.kind}:${candidate.path}`} className="finding">
                <h3>
                  <span className="path">{candidate.path}</span> <Chip>{candidate.label}</Chip>{" "}
                  <Chip>{confidenceLabel(candidate.confidence)} confidence</Chip>
                </h3>
                <p>{candidate.reason}</p>
                {candidate.evidence.length > 0 ? (
                  <details>
                    <summary>Evidence</summary>
                    <EvidenceList evidence={candidate.evidence} />
                  </details>
                ) : null}
              </div>
            ))
          )}
          {quality.deadCodeCandidates.length > 12 ? (
            <p>
              <button type="button" className="ghost" onClick={() => setAllCandidates((v) => !v)}>
                {allCandidates
                  ? "Show fewer"
                  : `Show all ${quality.deadCodeCandidates.length} candidates`}
              </button>
            </p>
          ) : null}
        </Panel>
      </div>
    </>
  );
}
