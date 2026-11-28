import { useEffect, useMemo, useRef, useState } from "react";
import { findingCounts, type RepositoryDna } from "@repodna/schema";
import { date, percent, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import { ErrorBox, Note, PageHeader, Panel } from "../components/common";
import { DataTable } from "../components/DataTable";
import { HistoryTrendPanel } from "../components/HistoryTrendPanel";
import { AnalysisChanges } from "../components/AnalysisChanges";
import { repositoryRelation } from "../lib/analysisComparison";
import { demoIndex, loadDemo, loadFile } from "../lib/demo";
import { useStoredHistory } from "../lib/useStoredHistory";
import {
  ComparisonPickerController,
  EMPTY_COMPARISON_PICKER,
  matchingRepositories,
  otherStoredScans,
  pickerPage,
  REPOSITORY_PAGE_SIZE,
  SCAN_PAGE_SIZE,
  type ComparisonSide,
} from "../lib/comparisonPicker";
import { useApp, useDataset } from "../state";

interface Row {
  measure: string;
  a: string;
  b: string;
  /** Numeric difference a − b, when both are numbers. */
  difference?: number;
  format?: (value: number) => string;
}

function rows(a: RepositoryDna, b: RepositoryDna): Row[] {
  const list: Row[] = [];
  const count = (
    measure: string,
    pick: (dna: RepositoryDna) => number,
    format = thousands,
    available: (dna: RepositoryDna) => boolean = () => true,
  ) => {
    const x = available(a) ? pick(a) : null;
    const y = available(b) ? pick(b) : null;
    list.push({
      measure,
      a: x !== null && Number.isFinite(x) ? format(x) : "Not available",
      b: y !== null && Number.isFinite(y) ? format(y) : "Not available",
      difference: x !== null && y !== null && Number.isFinite(x) && Number.isFinite(y) ? x - y : undefined,
      format,
    });
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
  count("Files", (d) => d.structure.totalFiles, thousands, (d) => d.structure.status === "analyzed");
  count("Code lines", (d) => d.structure.codeLines, thousands, (d) => d.structure.status === "analyzed");
  count("Languages", (d) => d.languages.languages.length, thousands, (d) => d.languages.status === "analyzed");
  text("Architecture", (d) => d.architecture.style);
  count("Modules", (d) => d.architecture.modules.length, thousands, (d) => d.architecture.status === "analyzed");
  count("Dependencies between modules", (d) => d.architecture.moduleEdges.length, thousands, (d) => d.architecture.status === "analyzed");
  count("Dependency cycles", (d) => d.architecture.cycles.length, thousands, (d) => d.architecture.status === "analyzed");
  count("Declared dependencies", (d) => d.dependencies.directCount, thousands, (d) => d.dependencies.status === "analyzed");
  count("Commits", (d) => d.git.commitCount, thousands, (d) => d.git.status === "analyzed");
  count("Contributors", (d) => d.git.ownership.contributors, thousands, (d) => d.git.status === "analyzed");
  text("Activity", (d) =>
    d.git.commitCount > 0 ? d.git.activity.level.replace("-", " ") : "no history",
  );
  count("Test files", (d) => d.tests.testFiles, thousands, (d) => d.tests.status === "analyzed");
  count("Test lines share", (d) => d.tests.testRatio, (v) => percent(v), (d) => d.tests.status === "analyzed");
  count("Documentation files", (d) => d.docs.docFiles, thousands, (d) => d.docs.status === "analyzed");
  count("Average complexity", (d) => d.codeQuality.complexity.averageCyclomatic, decimal, (d) => d.codeQuality.status === "analyzed" && d.codeQuality.complexity.functionsAnalyzed > 0);
  count(
    "Duplicated lines",
    (d) => d.codeQuality.duplication.ratio,
    (v) => percent(v),
    (d) => d.codeQuality.duplication.status === "analyzed" && d.codeQuality.duplication.analyzedLines > 0,
  );
  count("Critical findings", (d) => findingCounts(d).critical, thousands, (d) => !d.analysisMetadata.partial);
  count("Warnings", (d) => findingCounts(d).warning, thousands, (d) => !d.analysisMetadata.partial);
  count("Attention findings", (d) => findingCounts(d).attention, thousands, (d) => !d.analysisMetadata.partial);
  count("Informational findings", (d) => findingCounts(d).info, thousands, (d) => !d.analysisMetadata.partial);
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

function Picker({ onPick }: { onPick: (side: ComparisonSide) => void }) {
  const { backend, dataset } = useApp();
  const [state, setState] = useState(EMPTY_COMPARISON_PICKER);
  const [over, setOver] = useState(false);
  const [repositoryQuery, setRepositoryQuery] = useState("");
  const [repositoryPage, setRepositoryPage] = useState(0);
  const [scanPage, setScanPage] = useState(0);
  const fileInput = useRef<HTMLInputElement>(null);
  const controller = useRef<ComparisonPickerController | null>(null);
  const origin = dataset?.origin;
  const repositoryId = origin?.kind === "stored" ? origin.repositoryId : null;
  const currentScanId = origin?.kind === "stored"
    ? origin.scanId ?? dataset?.dna.analysisMetadata.id
    : dataset?.dna.analysisMetadata.id;
  const history = useStoredHistory(backend, repositoryId);

  useEffect(() => {
    setState(EMPTY_COMPARISON_PICKER);
    setRepositoryQuery("");
    setRepositoryPage(0);
    setScanPage(0);
    const instance = new ComparisonPickerController(backend, demoIndex, onPick, setState);
    controller.current = instance;
    void instance.loadInventory();
    return () => {
      instance.dispose();
      if (controller.current === instance) controller.current = null;
    };
  }, [backend, dataset?.dna]);

  const scans = otherStoredScans(history.scans ?? [], currentScanId);
  const scanChoices = pickerPage(scans, scanPage, SCAN_PAGE_SIZE);
  const repositories = matchingRepositories(state.repositories, repositoryId, repositoryQuery);
  const repositoryChoices = pickerPage(repositories, repositoryPage, REPOSITORY_PAGE_SIZE);
  const compareFile = (file: File | undefined) => {
    if (file) void controller.current?.select(file.name, async () => (await loadFile(file)).artifact);
  };
  const otherDemos = state.demos.filter(
    (entry) => !(origin?.kind === "demo" && (origin.file ? origin.file === entry.file : origin.title === entry.title)),
  );

  return (
    <Panel
      title="Compare with"
      description="Another analysis of this repository shows what changed; another repository shows how they differ."
    >
      {state.inventoryError ? <ErrorBox>{state.inventoryError}</ErrorBox> : null}
      {state.inventoryError ? <button type="button" onClick={() => void controller.current?.loadInventory()}>Retry lists</button> : null}
      {state.selectionError ? <ErrorBox>{state.selectionError}</ErrorBox> : null}
      {state.selection ? (
        <div role="status"><p>Opening {state.selection}…</p><button type="button" onClick={() => controller.current?.cancelSelection()}>Cancel selection</button></div>
      ) : null}
      <div className={backend ? "grid two" : undefined}>
        {backend ? (
          <div>
            {repositoryId ? (
              <>
                <h3>Other analyses of this repository</h3>
                {history.loading && history.scans === null ? <p role="status">Loading stored history…</p> : null}
                {history.error ? <ErrorBox>{history.error}</ErrorBox> : null}
                {history.error && history.scans === null ? <button type="button" onClick={() => void history.refresh(repositoryId)}>Retry history</button> : null}
                {history.scans && !history.loading && scans.length === 0 && !history.cursor ? (
                  <p className="muted">There are no other stored analyses of it.</p>
                ) : null}
                <ul className="list">
                  {scanChoices.items.map((scan) => (
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
                          void controller.current?.selectScan(repositoryId, scan, `analysis of ${date(scan.generatedAt)}`)
                        }
                      >
                        Compare
                      </button>
                    </li>
                  ))}
                </ul>
                {scanChoices.pages > 1 ? <PickerNavigation page={scanChoices.page} pages={scanChoices.pages} onPage={setScanPage} label="scan" /> : null}
                {history.cursor ? <button type="button" disabled={history.loading} onClick={history.more}>{history.loading ? "Loading older analyses…" : "Load older analyses"}</button> : null}
                {history.scans?.length ? <HistoryTrendPanel key={repositoryId} scans={history.scans} partial={Boolean(history.cursor || history.loading || history.error)} onSelect={(scan) => void controller.current?.selectScan(repositoryId, scan, `analysis of ${date(scan.generatedAt)}`)} /> : null}
              </>
            ) : null}
            <>
              <h3>Stored repositories</h3>
              {state.inventoryLoading ? <p role="status">Loading repositories…</p> : null}
              <label htmlFor="compare-repository-filter">Filter stored repositories</label>
              <input id="compare-repository-filter" value={repositoryQuery} onChange={(event) => { setRepositoryQuery(event.target.value); setRepositoryPage(0); }} />
              {!state.inventoryLoading && repositories.length === 0 ? (
                <p className="muted">No other stored repositories match.</p>
              ) : null}
              <ul className="list">
                {repositoryChoices.items.map((repository) => (
                    <li key={repository.id}>
                      <span>{repository.name}</span>
                      <button
                        type="button"
                        onClick={() =>
                          void controller.current?.selectRepository(repository)
                        }
                      >
                        Compare
                      </button>
                    </li>
                  ))}
              </ul>
              {repositoryChoices.pages > 1 ? <PickerNavigation page={repositoryChoices.page} pages={repositoryChoices.pages} onPage={setRepositoryPage} label="repository" /> : null}
            </>
          </div>
        ) : null}
        <div>
          <h3>{otherDemos.length > 0 ? "A file or demo" : "A file"}</h3>
          <div
            className={over ? "dropzone over" : "dropzone"}
            onDragOver={(event) => {
              event.preventDefault();
              setOver(true);
            }}
            onDragLeave={() => setOver(false)}
            onDrop={(event) => {
              event.preventDefault();
              setOver(false);
              compareFile(event.dataTransfer.files[0]);
            }}
          >
            <p>
              <strong>Drop an analysis file here to compare with it</strong>
            </p>
            <p>
              <button type="button" onClick={() => fileInput.current?.click()}>
                Open an analysis file…
              </button>
            </p>
            <input
              ref={fileInput}
              type="file"
              accept=".repodna,.json,application/json"
              className="visually-hidden"
              aria-label="Analysis file to compare with"
              onChange={(event) => {
                const file = event.target.files?.[0];
                // Cleared, so that choosing the same file again opens it again.
                event.target.value = "";
                compareFile(file);
              }}
            />
          </div>
          {otherDemos.length > 0 ? (
            <ul className="list">
              {otherDemos.map((entry) => (
                <li key={entry.file}>
                  <span>{entry.title}</span>
                  <button
                    type="button"
                    onClick={() =>
                      void controller.current?.select(entry.title, async () => (await loadDemo(entry)).artifact)
                    }
                  >
                    Compare
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      </div>
    </Panel>
  );
}

function PickerNavigation({ page, pages, onPage, label }: { page: number; pages: number; onPage: (page: number) => void; label: string }) {
  return (
    <div className="actions" aria-label={`${label} pages`}>
      <button type="button" disabled={page === 0} onClick={() => onPage(page - 1)}>Previous {label} page</button>
      <span>Page {page + 1} of {pages}</span>
      <button type="button" disabled={page + 1 >= pages} onClick={() => onPage(page + 1)}>Next {label} page</button>
    </div>
  );
}

export function Compare() {
  const { dna, origin } = useDataset();
  const [selection, setSelection] = useState<{ current: RepositoryDna; side: ComparisonSide; assumeSame: boolean } | null>(null);
  const active = selection?.current === dna ? selection : null;
  const other = active?.side ?? null;
  const assumeSame = active?.assumeSame ?? false;
  const table = useMemo(() => (other ? rows(dna, other.dna) : []), [dna, other]);
  const relation = other ? repositoryRelation(
    dna,
    other.dna,
    origin.kind === "stored" ? origin.repositoryId : null,
    other.repositoryId,
  ) : null;
  const same = relation?.relation === "same" || (relation?.relation === "unknown" && assumeSame);
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
            <button type="button" className="ghost" onClick={() => setSelection(null)}>
              Compare with something else
            </button>
          </p>
          {other.dna.analysisMetadata.profile !== dna.analysisMetadata.profile ? (
            <Note caution>
              The analyses used different profiles ({dna.analysisMetadata.profile} and{" "}
              {other.dna.analysisMetadata.profile}), so some measurements are not comparable.
            </Note>
          ) : null}
          {relation ? <p className="muted">{relation.reason}</p> : null}
          {relation?.relation === "unknown" ? (
            <label>
              <input type="checkbox" checked={assumeSame} onChange={(event) => { if (active) setSelection({ ...active, assumeSame: event.target.checked }); }} />
              Treat these snapshots as the same repository for the change view
            </label>
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
          {same ? <AnalysisChanges key={other.dna.analysisMetadata.id} current={dna} baseline={other.dna} /> : null}
        </>
      ) : (
        <Picker onPick={(side) => setSelection({ current: dna, side, assumeSame: false })} />
      )}
    </>
  );
}
