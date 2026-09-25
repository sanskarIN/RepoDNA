import { useEffect, useMemo, useRef, useState } from "react";
import { findingCounts, type RepositoryDna } from "@repodna/schema";
import { date, percent, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import { ErrorBox, Note, PageHeader, Panel } from "../components/common";
import { DataTable } from "../components/DataTable";
import type { RepositorySummary, ScanSummary } from "../lib/backend";
import { demoIndex, loadDemo, loadFile, type DemoEntry } from "../lib/demo";
import { useApp, useDataset } from "../state";

interface Side {
  dna: RepositoryDna;
  label: string;
}

interface Row {
  measure: string;
  a: string;
  b: string;
  /** Numeric difference a − b, when both are numbers. */
  difference?: number;
  format?: (value: number) => string;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function rows(a: RepositoryDna, b: RepositoryDna): Row[] {
  const list: Row[] = [];
  const count = (measure: string, pick: (dna: RepositoryDna) => number, format = thousands) => {
    const x = pick(a);
    const y = pick(b);
    list.push({ measure, a: format(x), b: format(y), difference: x - y, format });
  };
  const text = (measure: string, pick: (dna: RepositoryDna) => string) => {
    list.push({ measure, a: pick(a) || "–", b: pick(b) || "–" });
  };
  const decimal = (value: number) => value.toFixed(2);
  text("Main languages", (d) =>
    d.languages.languages
      .slice(0, 3)
      .map((l) => l.name)
      .join(", "),
  );
  count("Files", (d) => d.structure.totalFiles);
  count("Code lines", (d) => d.structure.codeLines);
  count("Languages", (d) => d.languages.languages.length);
  text("Architecture", (d) => d.architecture.style);
  count("Modules", (d) => d.architecture.modules.length);
  count("Dependencies between modules", (d) => d.architecture.moduleEdges.length);
  count("Dependency cycles", (d) => d.architecture.cycles.length);
  count("Declared dependencies", (d) => d.dependencies.directCount);
  count("Commits", (d) => d.git.commitCount);
  count("Contributors", (d) => d.git.ownership.contributors);
  text("Activity", (d) =>
    d.git.commitCount > 0 ? d.git.activity.level.replace("-", " ") : "no history",
  );
  count("Test files", (d) => d.tests.testFiles);
  count("Test files per source file", (d) => d.tests.testRatio, decimal);
  count("Documentation files", (d) => d.docs.docFiles);
  count("Average complexity", (d) => d.codeQuality.complexity.averageCyclomatic, decimal);
  count(
    "Duplicated lines",
    (d) => d.codeQuality.duplication.ratio,
    (v) => percent(v),
  );
  count("Critical findings", (d) => findingCounts(d).critical);
  count("Warnings", (d) => findingCounts(d).warning);
  count("Attention findings", (d) => findingCounts(d).attention);
  count("Informational findings", (d) => findingCounts(d).info);
  text(
    "Analyzed",
    (d) => `${date(d.analysisMetadata.generatedAt)} (${d.analysisMetadata.profile})`,
  );
  return list;
}

function signed(value: number, format: (value: number) => string): string {
  if (value === 0) {
    return "same";
  }
  return `${value > 0 ? "+" : "−"}${format(Math.abs(value))}`;
}

function difference<T>(a: readonly T[], b: readonly T[], key: (item: T) => string) {
  const inA = new Map(a.map((item) => [key(item), item]));
  const inB = new Map(b.map((item) => [key(item), item]));
  return {
    added: [...inA.entries()].filter(([k]) => !inB.has(k)).map(([, item]) => item),
    removed: [...inB.entries()].filter(([k]) => !inA.has(k)).map(([, item]) => item),
  };
}

function Changes({ a, b }: { a: RepositoryDna; b: RepositoryDna }) {
  const languages = difference(a.languages.languages, b.languages.languages, (l) => l.id);
  const dependencies = difference(
    a.dependencies.dependencies,
    b.dependencies.dependencies,
    (d) => `${d.ecosystem}:${d.name}`,
  );
  const findings = difference(
    a.findings.filter((f) => !f.suppressed),
    b.findings.filter((f) => !f.suppressed),
    (f) => f.id,
  );
  const files = difference(a.structure.files, b.structure.files, (f) => f.path);
  const list = (items: string[]) =>
    items.length === 0 ? (
      <span className="muted">none</span>
    ) : (
      <>
        {items.slice(0, 15).join(", ")}
        {items.length > 15 ? ` and ${items.length - 15} more` : ""}
      </>
    );
  return (
    <Panel
      title="What changed"
      description="Between the two analyses of this repository (the compared one is treated as the earlier)."
    >
      <dl className="facts wide">
        <dt>Languages added</dt>
        <dd>{list(languages.added.map((l) => l.name))}</dd>
        <dt>Languages gone</dt>
        <dd>{list(languages.removed.map((l) => l.name))}</dd>
        <dt>Dependencies added</dt>
        <dd>{list(dependencies.added.map((d) => d.name))}</dd>
        <dt>Dependencies removed</dt>
        <dd>{list(dependencies.removed.map((d) => d.name))}</dd>
        <dt>Files added</dt>
        <dd className="path">{list(files.added.map((f) => f.path))}</dd>
        <dt>Files removed</dt>
        <dd className="path">{list(files.removed.map((f) => f.path))}</dd>
        <dt>New findings</dt>
        <dd>{list(findings.added.map((f) => f.title))}</dd>
        <dt>No longer reported</dt>
        <dd>{list(findings.removed.map((f) => f.title))}</dd>
      </dl>
    </Panel>
  );
}

function Picker({ onPick }: { onPick: (side: Side) => void }) {
  const { backend, dataset } = useApp();
  const [repositories, setRepositories] = useState<RepositorySummary[]>([]);
  const [scans, setScans] = useState<ScanSummary[]>([]);
  const [demos, setDemos] = useState<DemoEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const origin = dataset?.origin;
  const repositoryId = origin?.kind === "stored" ? origin.repositoryId : null;
  const generatedAt = dataset?.dna.analysisMetadata.generatedAt;

  useEffect(() => {
    void demoIndex().then(setDemos);
    if (!backend) {
      return;
    }
    backend.repositories().then(setRepositories, (reason: unknown) => setError(message(reason)));
    if (repositoryId) {
      backend.repository(repositoryId).then(
        (detail) => setScans(detail.scans.filter((scan) => scan.generatedAt !== generatedAt)),
        (reason: unknown) => setError(message(reason)),
      );
    }
  }, [backend, repositoryId, generatedAt]);

  const pick = async (load: () => Promise<RepositoryDna>, label: string) => {
    setError(null);
    try {
      onPick({ dna: await load(), label });
    } catch (reason) {
      setError(message(reason));
    }
  };

  return (
    <Panel
      title="Compare with"
      description="Another analysis of this repository shows what changed; another repository shows how they differ."
    >
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      <div className={backend ? "grid two" : undefined}>
        {backend ? (
          <div>
            {repositoryId ? (
              <>
                <h3>Earlier analyses of this repository</h3>
                {scans.length === 0 ? (
                  <p className="muted">There are no other stored analyses of it.</p>
                ) : null}
                <ul className="list">
                  {scans.slice(0, 8).map((scan) => (
                    <li key={scan.id}>
                      <span>
                        {date(scan.generatedAt)} · {scan.profile}
                        {scan.revision ? (
                          <span className="mono"> · {scan.revision.slice(0, 10)}</span>
                        ) : null}
                      </span>
                      <button
                        type="button"
                        onClick={() =>
                          void pick(
                            () => backend.artifact(repositoryId, scan.id),
                            `analysis of ${date(scan.generatedAt)}`,
                          )
                        }
                      >
                        Compare
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
            <>
              <h3>Stored repositories</h3>
              {repositories.filter((r) => r.id !== repositoryId).length === 0 ? (
                <p className="muted">No other stored repositories.</p>
              ) : null}
              <ul className="list">
                {repositories
                  .filter((r) => r.id !== repositoryId)
                  .slice(0, 12)
                  .map((repository) => (
                    <li key={repository.id}>
                      <span>{repository.name}</span>
                      <button
                        type="button"
                        onClick={() =>
                          void pick(() => backend.artifact(repository.id), repository.name)
                        }
                      >
                        Compare
                      </button>
                    </li>
                  ))}
              </ul>
            </>
          </div>
        ) : null}
        <div>
          <h3>A file or demo</h3>
          <p>
            <button type="button" onClick={() => fileInput.current?.click()}>
              Open an analysis file…
            </button>
            <input
              ref={fileInput}
              type="file"
              accept=".repodna,.json,application/json"
              className="visually-hidden"
              aria-label="Analysis file to compare with"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) {
                  void pick(async () => (await loadFile(file)).artifact, file.name);
                }
              }}
            />
          </p>
          <ul className="list">
            {demos
              .filter((entry) => !(origin?.kind === "demo" && origin.title === entry.title))
              .map((entry) => (
                <li key={entry.file}>
                  <span>{entry.title}</span>
                  <button
                    type="button"
                    onClick={() =>
                      void pick(async () => (await loadDemo(entry)).artifact, entry.title)
                    }
                  >
                    Compare
                  </button>
                </li>
              ))}
          </ul>
        </div>
      </div>
    </Panel>
  );
}

export function Compare() {
  const { dna } = useDataset();
  const [other, setOther] = useState<Side | null>(null);
  const table = useMemo(() => (other ? rows(dna, other.dna) : []), [dna, other]);
  const same = other !== null && other.dna.identity.name === dna.identity.name;
  const thisLabel = same ? "This analysis" : dna.identity.name;
  const otherLabel = other ? (same ? other.label : other.dna.identity.name) : "";

  return (
    <>
      <PageHeader title="Compare">
        Side-by-side measurements of two analyses. Differences are facts, not rankings: a larger or
        busier repository is not a better one.
      </PageHeader>
      {other ? (
        <>
          <p>
            <button type="button" className="ghost" onClick={() => setOther(null)}>
              Compare with something else
            </button>
          </p>
          {other.dna.analysisMetadata.profile !== dna.analysisMetadata.profile ? (
            <Note caution>
              The analyses used different profiles ({dna.analysisMetadata.profile} and{" "}
              {other.dna.analysisMetadata.profile}), so some measurements are not comparable.
            </Note>
          ) : null}
          <Panel title={`${thisLabel} and ${otherLabel}`}>
            <DataTable
              rows={table}
              rowKey={(row) => row.measure}
              limit={50}
              columns={[
                { key: "measure", header: "Measure", cell: (row) => row.measure },
                { key: "a", header: thisLabel, cell: (row) => row.a, numeric: true },
                { key: "b", header: otherLabel, cell: (row) => row.b, numeric: true },
                {
                  key: "difference",
                  header: "Difference",
                  cell: (row) =>
                    row.difference !== undefined && row.format
                      ? signed(row.difference, row.format)
                      : "",
                  numeric: true,
                },
              ]}
            />
          </Panel>
          <div className="grid two" style={{ marginTop: 16 }}>
            <Panel
              title={`Project DNA: ${thisLabel}`}
              description="Each dimension from 0 to 1, on the same scale in both charts."
            >
              <BarList
                label={`Project DNA of ${thisLabel}`}
                max={1}
                format={(v) => v.toFixed(2)}
                bars={dna.fingerprint.dimensions.map((d) => ({ label: d.label, value: d.value }))}
              />
            </Panel>
            <Panel
              title={`Project DNA: ${otherLabel}`}
              description="Each dimension from 0 to 1, on the same scale in both charts."
            >
              <BarList
                label={`Project DNA of ${otherLabel}`}
                max={1}
                format={(v) => v.toFixed(2)}
                bars={other.dna.fingerprint.dimensions.map((d) => ({
                  label: d.label,
                  value: d.value,
                  color: "var(--s2)",
                }))}
              />
            </Panel>
          </div>
          {same ? <Changes a={dna} b={other.dna} /> : null}
        </>
      ) : (
        <Picker onPick={setOther} />
      )}
    </>
  );
}
