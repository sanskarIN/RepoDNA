import { useMemo, useState } from "react";
import { confidenceLabel, type Snapshot, type StoryStatement } from "@repodna/schema";
import { assignLayers, bytes, categorize, date, span, thousands } from "@repodna/visualization";
import { ArchitectureGraph } from "../charts/ArchitectureGraph";
import { BarList } from "../charts/BarList";
import { LineChart } from "../charts/LineChart";
import { ShareBar } from "../charts/ShareBar";
import {
  Chip,
  EvidenceList,
  PageHeader,
  Panel,
  SectionStatus,
  Select,
  Statements,
  Tile,
} from "../components/common";
import { DataTable } from "../components/DataTable";
import { useApp, useDataset } from "../state";

type Measure = "files" | "bytes" | "commits";

const MEASURES: readonly (readonly [Measure, string])[] = [
  ["files", "Files"],
  ["bytes", "Size"],
  ["commits", "Commits"],
];

const STORY: [keyof StoryParts, string][] = [
  ["origin", "How it started"],
  ["growth", "How it grew"],
  ["expansions", "Expansions"],
  ["rewrites", "Rewrites and restructuring"],
  ["inactivity", "Quiet periods"],
  ["finalPhase", "Latest phase"],
  ["currentState", "Where it stands"],
];

interface StoryParts {
  origin: StoryStatement[];
  growth: StoryStatement[];
  expansions: StoryStatement[];
  rewrites: StoryStatement[];
  inactivity: StoryStatement[];
  finalPhase: StoryStatement[];
  currentState: StoryStatement[];
}

/** Dates for long histories; day and time (UTC) when everything happened within a week. */
function timeLabels(timestamps: string[]): string[] {
  const first = Date.parse(timestamps[0] ?? "");
  const last = Date.parse(timestamps[timestamps.length - 1] ?? "");
  const week = 7 * 24 * 60 * 60 * 1000;
  if (!(last - first < week)) {
    return timestamps.map((timestamp) => date(timestamp));
  }
  return timestamps.map((timestamp) => `${timestamp.slice(5, 10)} ${timestamp.slice(11, 16)} UTC`);
}

function delta(current: number, previous: number | undefined): string | undefined {
  if (previous === undefined) {
    return undefined;
  }
  const change = current - previous;
  if (change === 0) {
    return "no change since the previous snapshot";
  }
  return `${change > 0 ? "+" : "−"}${thousands(Math.abs(change))} since the previous snapshot`;
}

function SnapshotView({
  snapshot,
  previous,
  languageName,
  languageOrder,
}: {
  snapshot: Snapshot;
  previous: Snapshot | undefined;
  languageName: (id: string) => string;
  languageOrder: string[];
}) {
  const { theme } = useApp();
  const shares = categorize(
    snapshot.languages.filter((l) => l.bytes > 0),
    (l) => l.bytes,
    (l) => l.id,
    theme,
    languageOrder,
  ).map((c) => ({
    label: c.items.length === 1 ? languageName(c.label) : c.label,
    value: c.value,
    color: c.color,
  }));
  const directories = [...snapshot.directories].sort((a, b) => b.files - a.files).slice(0, 10);
  const before = new Set(previous?.languages.map((l) => l.id) ?? []);
  const now = new Set(snapshot.languages.map((l) => l.id));
  const addedLanguages = previous ? [...now].filter((id) => !before.has(id)) : [];
  const removedLanguages = previous ? [...before].filter((id) => !now.has(id)) : [];
  const beforeDirs = new Set(previous?.directories.map((d) => d.path) ?? []);
  const nowDirs = new Set(snapshot.directories.map((d) => d.path));
  const addedDirs = previous ? [...nowDirs].filter((p) => !beforeDirs.has(p)) : [];
  const removedDirs = previous ? [...beforeDirs].filter((p) => !nowDirs.has(p)) : [];
  const architecture = snapshot.architecture;
  const graph = useMemo(() => {
    if (!architecture || architecture.modules.length === 0) {
      return null;
    }
    const ids = architecture.modules.map((m) => m.path);
    const layers = assignLayers(ids, architecture.edges);
    const fanIn = new Map<string, number>();
    const fanOut = new Map<string, number>();
    for (const edge of architecture.edges) {
      fanOut.set(edge.from, (fanOut.get(edge.from) ?? 0) + 1);
      fanIn.set(edge.to, (fanIn.get(edge.to) ?? 0) + 1);
    }
    return {
      modules: architecture.modules.map((m) => ({
        id: m.path,
        name: m.path.split("/").pop() || "(root)",
        layer: layers.get(m.path) ?? null,
        codeLines: m.bytes,
        fanIn: fanIn.get(m.path) ?? 0,
        fanOut: fanOut.get(m.path) ?? 0,
      })),
      edges: architecture.edges,
    };
  }, [architecture]);
  const [selected, setSelected] = useState<string | null>(null);

  return (
    <>
      <div className="tiles">
        <Tile
          label="Files"
          value={thousands(snapshot.files)}
          note={delta(snapshot.files, previous?.files)}
        />
        <Tile label="Size" value={bytes(snapshot.bytes)} />
        <Tile
          label="Test files"
          value={thousands(snapshot.testFiles)}
          note={delta(snapshot.testFiles, previous?.testFiles)}
        />
        <Tile
          label="Documentation files"
          value={thousands(snapshot.docFiles)}
          note={delta(snapshot.docFiles, previous?.docFiles)}
        />
        {snapshot.dependencies != null ? (
          <Tile
            label="Declared dependencies"
            value={thousands(snapshot.dependencies)}
            note={delta(snapshot.dependencies, previous?.dependencies ?? undefined)}
          />
        ) : null}
        <Tile
          label="Commit"
          value={<span className="mono">{snapshot.revision.slice(0, 10)}</span>}
          note={`commit ${thousands(snapshot.commitIndex)} of the history`}
        />
      </div>
      {previous &&
      (addedLanguages.length ||
        removedLanguages.length ||
        addedDirs.length ||
        removedDirs.length) ? (
        <ul className="evidence">
          {addedLanguages.length > 0 ? (
            <li>Languages added: {addedLanguages.map(languageName).join(", ")}</li>
          ) : null}
          {removedLanguages.length > 0 ? (
            <li>Languages gone: {removedLanguages.map(languageName).join(", ")}</li>
          ) : null}
          {addedDirs.length > 0 ? (
            <li>
              New top-level areas: <span className="path">{addedDirs.slice(0, 12).join(", ")}</span>
              {addedDirs.length > 12 ? ` and ${addedDirs.length - 12} more` : ""}
            </li>
          ) : null}
          {removedDirs.length > 0 ? (
            <li>
              Areas removed: <span className="path">{removedDirs.slice(0, 12).join(", ")}</span>
              {removedDirs.length > 12 ? ` and ${removedDirs.length - 12} more` : ""}
            </li>
          ) : null}
        </ul>
      ) : null}
      <div className="grid two">
        <Panel
          title="Languages then"
          description="Share of bytes in recognized files; colors follow each language across snapshots."
          table={
            <DataTable
              rows={snapshot.languages}
              rowKey={(l) => l.id}
              initialSort={{ key: "bytes", descending: true }}
              columns={[
                {
                  key: "name",
                  header: "Language",
                  cell: (l) => languageName(l.id),
                  sort: (l) => languageName(l.id),
                },
                {
                  key: "files",
                  header: "Files",
                  cell: (l) => thousands(l.files),
                  sort: (l) => l.files,
                  numeric: true,
                },
                {
                  key: "bytes",
                  header: "Size",
                  cell: (l) => bytes(l.bytes),
                  sort: (l) => l.bytes,
                  numeric: true,
                },
              ]}
            />
          }
        >
          {shares.length > 0 ? (
            <ShareBar shares={shares} label={`Languages at ${snapshot.label}`} />
          ) : (
            <p className="muted">No recognized source files in this snapshot.</p>
          )}
        </Panel>
        <Panel
          title="Largest areas then"
          table={
            <DataTable
              rows={snapshot.directories}
              rowKey={(d) => d.path}
              initialSort={{ key: "files", descending: true }}
              columns={[
                {
                  key: "path",
                  header: "Directory",
                  cell: (d) => <span className="path">{d.path || "(root)"}</span>,
                  sort: (d) => d.path,
                },
                {
                  key: "files",
                  header: "Files",
                  cell: (d) => thousands(d.files),
                  sort: (d) => d.files,
                  numeric: true,
                },
                {
                  key: "bytes",
                  header: "Size",
                  cell: (d) => bytes(d.bytes),
                  sort: (d) => d.bytes,
                  numeric: true,
                },
              ]}
            />
          }
        >
          <BarList
            label={`Files per directory at ${snapshot.label}`}
            unit={[" file", " files"]}
            bars={directories.map((d) => ({
              label: d.path || "(root)",
              value: d.files,
              details: bytes(d.bytes),
            }))}
          />
        </Panel>
      </div>
      {graph ? (
        <Panel
          title="Architecture then"
          description="Top-level modules at this snapshot and the imports between them (area = size)."
        >
          <ArchitectureGraph
            modules={graph.modules}
            dependencies={graph.edges}
            selected={selected}
            onSelect={setSelected}
            label={`Module graph at ${snapshot.label}`}
          />
        </Panel>
      ) : null}
    </>
  );
}

export function TimeMachine() {
  const { dna } = useDataset();
  const evolution = dna.evolution;
  const snapshots = useMemo(
    () => [...evolution.snapshots].sort((a, b) => a.commitIndex - b.commitIndex),
    [evolution.snapshots],
  );
  const [index, setIndex] = useState(() => Math.max(0, snapshots.length - 1));
  const [measure, setMeasure] = useState<Measure>("files");
  const [allEvents, setAllEvents] = useState(false);
  const names = useMemo(
    () => new Map(dna.languages.languages.map((l) => [l.id, l.name])),
    [dna.languages.languages],
  );
  const languageName = (id: string) => names.get(id) ?? id;
  const languageOrder = useMemo(() => {
    const largest = new Map<string, number>();
    for (const snapshot of snapshots) {
      for (const language of snapshot.languages) {
        largest.set(language.id, Math.max(largest.get(language.id) ?? 0, language.bytes));
      }
    }
    return [...largest.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .map(([id]) => id);
  }, [snapshots]);
  const snapshot = snapshots[Math.min(index, snapshots.length - 1)];
  const previous = index > 0 ? snapshots[index - 1] : undefined;
  const growth = evolution.growth;
  const events = [...evolution.events].sort((a, b) => a.date.localeCompare(b.date));
  const shownEvents = allEvents ? events : events.slice(-12);
  const story = evolution.archaeology as StoryParts;

  if (snapshots.length === 0 && growth.length === 0 && events.length === 0) {
    return (
      <>
        <PageHeader title="Time Machine" />
        <SectionStatus
          status={evolution.status === "analyzed" ? "unavailable" : evolution.status}
          notes={evolution.notes}
        />
        <p className="muted">
          The Time Machine rebuilds past states from Git history. Analyze a Git repository with the
          standard or deep profile to use it.
        </p>
      </>
    );
  }

  return (
    <>
      <PageHeader title="Time Machine">
        How the repository looked at points in its history, rebuilt from Git without checking
        anything out. {evolution.sampling}
      </PageHeader>
      <SectionStatus status={evolution.status} notes={evolution.notes} />

      {evolution.story.length > 0 ? (
        <Panel
          title="The story in brief"
          description="Facts come straight from history; interpretations are RepoDNA's reading of them."
        >
          <Statements statements={evolution.story} />
        </Panel>
      ) : null}

      {snapshot ? (
        <Panel
          title={`Snapshot: ${snapshot.label}`}
          description={`${date(snapshot.date)} · ${snapshot.kind === "release" ? "a release" : snapshot.kind === "initial" ? "the first commit" : snapshot.kind === "current" ? "the analyzed revision" : "a sample point"}`}
        >
          <div className="filters time-slider">
            <button
              type="button"
              onClick={() => setIndex((i) => Math.max(0, i - 1))}
              disabled={index === 0}
            >
              ← Earlier
            </button>
            <label className="visually-hidden" htmlFor="snapshot-slider">
              Snapshot
            </label>
            <input
              id="snapshot-slider"
              type="range"
              min={0}
              max={snapshots.length - 1}
              value={index}
              aria-valuetext={`${snapshot.label}, ${date(snapshot.date)}`}
              onChange={(event) => setIndex(Number(event.target.value))}
              style={{ flex: "1 1 240px" }}
            />
            <button
              type="button"
              onClick={() => setIndex((i) => Math.min(snapshots.length - 1, i + 1))}
              disabled={index === snapshots.length - 1}
            >
              Later →
            </button>
            <span className="muted">
              {index + 1} of {snapshots.length}
            </span>
          </div>
          <SnapshotView
            key={snapshot.revision}
            snapshot={snapshot}
            previous={previous}
            languageName={languageName}
            languageOrder={languageOrder}
          />
        </Panel>
      ) : null}

      {growth.length > 1 ? (
        <Panel
          title="Growth"
          actions={
            <Select
              id="growth-measure"
              label="Measure"
              value={measure}
              options={MEASURES}
              onChange={setMeasure}
            />
          }
          table={
            <DataTable
              rows={growth}
              rowKey={(g) => g.date}
              columns={[
                { key: "date", header: "Date", cell: (g) => date(g.date), sort: (g) => g.date },
                {
                  key: "files",
                  header: "Files",
                  cell: (g) => thousands(g.files),
                  sort: (g) => g.files,
                  numeric: true,
                },
                {
                  key: "bytes",
                  header: "Size",
                  cell: (g) => bytes(g.bytes),
                  sort: (g) => g.bytes,
                  numeric: true,
                },
                {
                  key: "commits",
                  header: "Commits so far",
                  cell: (g) => thousands(g.commits),
                  sort: (g) => g.commits,
                  numeric: true,
                },
              ]}
            />
          }
        >
          <LineChart
            label={`${MEASURES.find(([m]) => m === measure)?.[1] ?? measure} over time`}
            labels={timeLabels(growth.map((g) => g.date))}
            series={[
              {
                label: MEASURES.find(([m]) => m === measure)?.[1] ?? measure,
                color: "var(--s1)",
                values: growth.map((g) =>
                  measure === "files" ? g.files : measure === "bytes" ? g.bytes : g.commits,
                ),
              },
            ]}
          />
          {measure === "bytes" ? <p className="muted">Size is in bytes.</p> : null}
        </Panel>
      ) : null}

      {evolution.epochs.length > 0 ? (
        <Panel title="Epochs" description="Periods separated by changes in the pace of work.">
          <ol className="epochs">
            {evolution.epochs.map((epoch) => (
              <li key={epoch.index}>
                <strong>{epoch.label}</strong> <Chip>{epoch.kind}</Chip>
                <div className="muted">
                  {date(epoch.start)} to {date(epoch.end)} · {thousands(epoch.commits)} commits by{" "}
                  {thousands(epoch.contributors)} {epoch.contributors === 1 ? "person" : "people"} ·
                  +{thousands(epoch.insertions)} −{thousands(epoch.deletions)} lines
                </div>
                {epoch.focusAreas.length > 0 ? (
                  <div>
                    Focus: <span className="path">{epoch.focusAreas.join(", ")}</span>
                  </div>
                ) : null}
              </li>
            ))}
          </ol>
        </Panel>
      ) : null}

      {events.length > 0 ? (
        <Panel
          title="Key events"
          description={
            allEvents || events.length <= 12
              ? `${events.length} events, oldest first.`
              : `The latest 12 of ${events.length} events.`
          }
          actions={
            events.length > 12 ? (
              <button type="button" className="ghost" onClick={() => setAllEvents((v) => !v)}>
                {allEvents ? "Show latest" : "Show all"}
              </button>
            ) : null
          }
        >
          {shownEvents.map((event) => (
            <div key={event.id} className="finding">
              <h3>
                <span className="muted">{date(event.date)}</span> {event.title}{" "}
                <Chip>{confidenceLabel(event.confidence)} confidence</Chip>
              </h3>
              <p>{event.description}</p>
              {event.evidence.length > 0 ? (
                <details>
                  <summary>Evidence</summary>
                  <EvidenceList evidence={event.evidence} />
                </details>
              ) : null}
            </div>
          ))}
        </Panel>
      ) : null}

      {STORY.some(([key]) => (story[key] ?? []).length > 0) ? (
        <Panel
          title="Code archaeology"
          description="The history read as a story, section by section."
        >
          {STORY.map(([key, title]) =>
            (story[key] ?? []).length > 0 ? (
              <section key={key}>
                <h3>{title}</h3>
                <Statements statements={story[key]} />
              </section>
            ) : null,
          )}
        </Panel>
      ) : null}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Module ages"
          description="When each module first appeared and when it last changed."
        >
          <DataTable
            rows={evolution.moduleAges}
            rowKey={(m) => m.module}
            initialSort={{ key: "age", descending: true }}
            columns={[
              {
                key: "module",
                header: "Module",
                cell: (m) => <span className="path">{m.module}</span>,
                sort: (m) => m.module,
              },
              { key: "class", header: "Age", cell: (m) => m.class },
              {
                key: "age",
                header: "Since first commit",
                cell: (m) => span(m.ageDays),
                sort: (m) => m.ageDays,
                numeric: true,
              },
              {
                key: "last",
                header: "Last change",
                cell: (m) => date(m.lastChange),
                sort: (m) => m.lastChange,
              },
            ]}
          />
        </Panel>
        <Panel
          title="Areas that went quiet"
          description="Directories untouched for a long time before the latest commit."
        >
          {evolution.abandonedAreas.length === 0 ? (
            <p className="muted">Every area changed recently enough.</p>
          ) : (
            <DataTable
              rows={evolution.abandonedAreas}
              rowKey={(a) => a.path}
              columns={[
                {
                  key: "path",
                  header: "Directory",
                  cell: (a) => <span className="path">{a.path}</span>,
                  sort: (a) => a.path,
                },
                {
                  key: "files",
                  header: "Files",
                  cell: (a) => thousands(a.files),
                  sort: (a) => a.files,
                  numeric: true,
                },
                {
                  key: "last",
                  header: "Last change",
                  cell: (a) => date(a.lastChanged),
                  sort: (a) => a.lastChanged,
                },
                {
                  key: "quiet",
                  header: "Quiet for",
                  cell: (a) => span(a.daysBeforeLatest),
                  sort: (a) => a.daysBeforeLatest,
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
