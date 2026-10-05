import { useMemo } from "react";
import { confidenceLabel, type ModuleRecord, type RepositoryDna } from "@repodna/schema";
import { percent, thousands } from "@repodna/visualization";
import { ArchitectureGraph } from "../charts/ArchitectureGraph";
import {
  Chip,
  EvidenceList,
  Note,
  PageHeader,
  Panel,
  SectionStatus,
  Tile,
} from "../components/common";
import { DataTable } from "../components/DataTable";
import { href, navigate, useRoute } from "../lib/router";
import { useDataset } from "../state";

/** More modules than this make the graph unreadable; the table always lists all of them. */
const MAX_GRAPH_MODULES = 40;

function under(path: string, directory: string): boolean {
  return directory === "" || path === directory || path.startsWith(`${directory}/`);
}

/** The modules drawn in the graph: the largest and most central, plus the selected one. */
function graphModules(modules: ModuleRecord[], selected: string | null): ModuleRecord[] {
  if (modules.length <= MAX_GRAPH_MODULES) {
    return modules;
  }
  const ranked = [...modules].sort(
    (a, b) => b.centrality - a.centrality || b.codeLines - a.codeLines || a.id.localeCompare(b.id),
  );
  const shown = ranked.slice(0, MAX_GRAPH_MODULES);
  const extra = modules.find((m) => m.id === selected);
  if (extra && !shown.includes(extra)) {
    shown.push(extra);
  }
  return shown;
}

function ModuleDetails({ dna, module }: { dna: RepositoryDna; module: ModuleRecord }) {
  const byId = new Map(dna.architecture.modules.map((m) => [m.id, m]));
  const uses = dna.architecture.moduleEdges
    .filter((edge) => edge.from === module.id)
    .sort((a, b) => b.weight - a.weight);
  const usedBy = dna.architecture.moduleEdges
    .filter((edge) => edge.to === module.id)
    .sort((a, b) => b.weight - a.weight);
  const hotspots = dna.git.hotSpots.filter((h) => under(h.path, module.path)).slice(0, 5);
  const findings = dna.findings.filter(
    (f) => !f.suppressed && (f.paths ?? []).some((path) => under(path, module.path)),
  );
  const cycles = dna.architecture.cycles.filter((c) => c.members.includes(module.id));
  const external = module.externalDependencies ?? [];
  const importance = module.importance ?? [];
  const name = (id: string) => byId.get(id)?.name ?? id;

  return (
    <div aria-live="polite">
      <h3 style={{ marginTop: 0 }}>
        {module.name} <Chip>{module.kind}</Chip>
        {module.inferred ? (
          <Chip title="No manifest declares this module">Inferred from folders</Chip>
        ) : null}
      </h3>
      <p className="muted path">{module.path || "(repository root)"}</p>
      <dl className="facts">
        <dt>Size</dt>
        <dd>
          {thousands(module.files)} files · {thousands(module.codeLines)} code lines
          {module.language ? ` · mostly ${module.language}` : ""}
        </dd>
        <dt>Used by</dt>
        <dd>{module.fanIn === 1 ? "1 module" : `${module.fanIn} modules`}</dd>
        <dt>Uses</dt>
        <dd>{module.fanOut === 1 ? "1 module" : `${module.fanOut} modules`}</dd>
        <dt>Instability</dt>
        <dd title="Share of this module's dependencies that point outward: 0 means only used, 1 means only using.">
          {module.instability.toFixed(2)}
        </dd>
        <dt>Layer</dt>
        <dd>{module.layer ?? "In a cycle"}</dd>
        <dt>Confidence</dt>
        <dd>{confidenceLabel(module.confidence)}</dd>
      </dl>
      {importance.length > 0 ? (
        <>
          <h4>Why this component matters</h4>
          <ul className="evidence">
            {importance.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </>
      ) : (
        <p className="muted">
          Nothing marks this module as especially central: it is not widely used and holds no
          hotspots.
        </p>
      )}
      {uses.length > 0 ? (
        <>
          <h4>Depends on</h4>
          <ul className="evidence">
            {uses.map((edge) => (
              <li key={edge.to}>
                <a href={href("/architecture", { module: edge.to })}>{name(edge.to)}</a> ·{" "}
                {thousands(edge.weight)} {edge.weight === 1 ? "reference" : "references"}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {usedBy.length > 0 ? (
        <>
          <h4>Used by</h4>
          <ul className="evidence">
            {usedBy.map((edge) => (
              <li key={edge.from}>
                <a href={href("/architecture", { module: edge.from })}>{name(edge.from)}</a> ·{" "}
                {thousands(edge.weight)} {edge.weight === 1 ? "reference" : "references"}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {cycles.length > 0 ? (
        <Note caution>
          Part of{" "}
          {cycles.length === 1 ? "a dependency cycle" : `${cycles.length} dependency cycles`}:{" "}
          {cycles[0]?.path.map(name).join(" → ")}
        </Note>
      ) : null}
      {external.length > 0 ? (
        <>
          <h4>External packages</h4>
          <p className="path">{external.join(", ")}</p>
        </>
      ) : null}
      {hotspots.length > 0 ? (
        <>
          <h4>Hotspots inside</h4>
          <ul className="evidence">
            {hotspots.map((h) => (
              <li key={h.path} className="path">
                #{h.rank} {h.path}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {findings.length > 0 ? (
        <p>
          <a href={href("/findings", { q: module.path })}>
            {findings.length === 1 ? "1 finding" : `${findings.length} findings`} in this module
          </a>
        </p>
      ) : null}
      {module.evidence.length > 0 ? (
        <details>
          <summary>Evidence</summary>
          <EvidenceList evidence={module.evidence} />
        </details>
      ) : null}
    </div>
  );
}

export function Architecture() {
  const { dna } = useDataset();
  const route = useRoute();
  const architecture = dna.architecture;
  const selectedId = route.params.get("module");
  const selected = architecture.modules.find((m) => m.id === selectedId) ?? null;
  const byId = useMemo(
    () => new Map(architecture.modules.map((m) => [m.id, m])),
    [architecture.modules],
  );
  const shown = useMemo(
    () => graphModules(architecture.modules, selected?.id ?? null),
    [architecture.modules, selected],
  );
  const shownIds = useMemo(() => new Set(shown.map((m) => m.id)), [shown]);
  const edges = useMemo(
    () =>
      architecture.moduleEdges
        .filter((e) => shownIds.has(e.from) && shownIds.has(e.to))
        .map((e) => ({ from: e.from, to: e.to, weight: e.weight })),
    [architecture.moduleEdges, shownIds],
  );
  const central = [...architecture.modules]
    .sort((a, b) => b.centrality - a.centrality || b.codeLines - a.codeLines)
    .slice(0, 5);
  const name = (id: string) => byId.get(id)?.name ?? id;
  const resolved = architecture.resolvedImports;
  const total = resolved + architecture.unresolvedImports;

  return (
    <>
      <PageHeader title="Architecture">
        {architecture.style ? (
          <>
            Looks like a <strong>{architecture.style}</strong> (
            {confidenceLabel(architecture.styleConfidence)} confidence). Modules come from package
            manifests where they exist and from folders otherwise; dependencies come from imports
            RepoDNA could resolve.
          </>
        ) : (
          "Modules and the dependencies between them, from package manifests and resolved imports."
        )}
      </PageHeader>
      <SectionStatus status={architecture.status} notes={architecture.notes} />
      <div className="tiles">
        <Tile label="Modules" value={thousands(architecture.modules.length)} />
        <Tile
          label="Dependencies between modules"
          value={thousands(architecture.moduleEdges.length)}
        />
        <Tile label="Layers" value={thousands(architecture.layers.length)} />
        <Tile
          label="Cycles"
          value={thousands(architecture.cycles.length)}
          note={architecture.cycles.length === 0 ? "None detected" : "See below"}
        />
        <Tile
          label="Imports resolved"
          value={total > 0 ? percent(resolved / total) : "–"}
          note={`${thousands(resolved)} of ${thousands(total)}`}
        />
        <Tile label="External packages imported" value={thousands(architecture.external.length)} />
      </div>

      <Panel
        title="Module map"
        description={
          shown.length < architecture.modules.length
            ? `The ${shown.length} most central modules of ${architecture.modules.length}; the table lists all. Select a module to see why it matters.`
            : "Layer 0 on the left depends on no other module. Select a module to see why it matters."
        }
        table={
          <DataTable
            rows={architecture.modules}
            rowKey={(m) => m.id}
            initialSort={{ key: "centrality", descending: true }}
            columns={[
              {
                key: "name",
                header: "Module",
                cell: (m) => <a href={href("/architecture", { module: m.id })}>{m.name}</a>,
                sort: (m) => m.name,
              },
              {
                key: "path",
                header: "Path",
                cell: (m) => <span className="path">{m.path || "(root)"}</span>,
              },
              {
                key: "layer",
                header: "Layer",
                cell: (m) => m.layer ?? "cycle",
                sort: (m) => m.layer ?? 999,
                numeric: true,
              },
              {
                key: "files",
                header: "Files",
                cell: (m) => thousands(m.files),
                sort: (m) => m.files,
                numeric: true,
              },
              {
                key: "code",
                header: "Code lines",
                cell: (m) => thousands(m.codeLines),
                sort: (m) => m.codeLines,
                numeric: true,
              },
              {
                key: "in",
                header: "Used by",
                cell: (m) => m.fanIn,
                sort: (m) => m.fanIn,
                numeric: true,
              },
              {
                key: "out",
                header: "Uses",
                cell: (m) => m.fanOut,
                sort: (m) => m.fanOut,
                numeric: true,
              },
              {
                key: "centrality",
                header: "Centrality",
                cell: (m) => m.centrality.toFixed(2),
                sort: (m) => m.centrality,
                numeric: true,
              },
            ]}
          />
        }
      >
        {architecture.modules.length === 0 ? (
          <p className="muted">No modules were identified.</p>
        ) : (
          <ArchitectureGraph
            modules={shown.map((m) => ({
              id: m.id,
              name: m.name,
              layer: m.layer ?? null,
              codeLines: m.codeLines,
              fanIn: m.fanIn,
              fanOut: m.fanOut,
            }))}
            dependencies={edges}
            selected={selected?.id ?? null}
            onSelect={(id) => navigate("/architecture", id ? { module: id } : undefined)}
            label={`Dependency graph of ${shown.length} modules`}
          />
        )}
      </Panel>
      <Panel
        title={selected ? `Why ${selected.name} matters` : "Where to look first"}
        id="module-details"
      >
        {selected ? (
          <>
            <ModuleDetails dna={dna} module={selected} />
            <p>
              <button type="button" className="ghost" onClick={() => navigate("/architecture")}>
                Clear selection
              </button>
            </p>
          </>
        ) : (
          <>
            <p className="muted">The most central modules; select one here or in the map.</p>
            <ul className="list">
              {central.map((m) => (
                <li key={m.id}>
                  <div>
                    <strong>{m.name}</strong>
                    <div className="muted">
                      used by {m.fanIn} · uses {m.fanOut} · {thousands(m.codeLines)} code lines
                    </div>
                  </div>
                  <a className="button" href={href("/architecture", { module: m.id })}>
                    Details
                  </a>
                </li>
              ))}
            </ul>
          </>
        )}
      </Panel>

      {architecture.signals.length > 0 ? (
        <Panel
          title="Architecture signals"
          description="What the structure suggests, with how sure RepoDNA is."
        >
          {architecture.signals.map((signal) => (
            <div key={signal.id} className="finding">
              <h3>
                {signal.label} <Chip>{confidenceLabel(signal.confidence)} confidence</Chip>
              </h3>
              <p>{signal.description}</p>
              {signal.evidence.length > 0 ? (
                <details>
                  <summary>Evidence</summary>
                  <EvidenceList evidence={signal.evidence} />
                </details>
              ) : null}
            </div>
          ))}
        </Panel>
      ) : null}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Dependency cycles"
          description="Modules or files that depend on each other in a loop."
        >
          {architecture.cycles.length === 0 ? (
            <p className="muted">No cycles were found among resolved imports.</p>
          ) : (
            architecture.cycles.map((cycle) => (
              <div key={cycle.id} className="finding">
                <h3>
                  {cycle.level === "module" ? "Module cycle" : "File cycle"}{" "}
                  <Chip>{confidenceLabel(cycle.confidence)} confidence</Chip>
                </h3>
                <p className="path">
                  {(cycle.level === "module" ? cycle.path.map(name) : cycle.path).join(" → ")}
                </p>
                <details>
                  <summary>Evidence</summary>
                  <EvidenceList evidence={cycle.evidence} />
                </details>
              </div>
            ))
          )}
        </Panel>
        <Panel
          title="Layers"
          description="Each layer depends only on layers to its left, except in cycles."
        >
          {architecture.layers.length === 0 ? <p className="muted">No layers.</p> : null}
          <ul className="evidence">
            {architecture.layers.map((layer) => (
              <li key={layer.index}>
                <span className="muted">Layer {layer.index}:</span>{" "}
                {layer.modules.map(name).join(", ")}
              </li>
            ))}
          </ul>
          {architecture.isolatedModules.length > 0 ? (
            <p className="muted">
              Not connected to other modules: {architecture.isolatedModules.map(name).join(", ")}
            </p>
          ) : null}
        </Panel>
      </div>

      {architecture.packages.length > 0 || architecture.workspace ? (
        <Panel
          title="Packages"
          description={
            architecture.workspace
              ? `Declared as a workspace in ${architecture.workspace.manifest} (${architecture.workspace.tool}).`
              : "Package boundaries declared by manifests."
          }
        >
          <DataTable
            rows={architecture.packages}
            rowKey={(p) => `${p.ecosystem}:${p.manifest}`}
            columns={[
              { key: "name", header: "Package", cell: (p) => p.name, sort: (p) => p.name },
              {
                key: "ecosystem",
                header: "Ecosystem",
                cell: (p) => p.ecosystem,
                sort: (p) => p.ecosystem,
              },
              {
                key: "manifest",
                header: "Manifest",
                cell: (p) => <span className="path">{p.manifest}</span>,
              },
              {
                key: "internal",
                header: "Uses packages",
                cell: (p) => p.internalDependencies.join(", ") || "–",
                sort: (p) => p.internalDependencies.length,
              },
            ]}
          />
        </Panel>
      ) : null}

      <Panel
        title="External imports"
        description="Third-party packages and how many files import them."
      >
        <DataTable
          rows={architecture.external}
          rowKey={(e) => `${e.ecosystem ?? ""}:${e.name}`}
          initialSort={{ key: "importers", descending: true }}
          limit={15}
          columns={[
            { key: "name", header: "Package", cell: (e) => e.name, sort: (e) => e.name },
            { key: "ecosystem", header: "Ecosystem", cell: (e) => e.ecosystem ?? "–" },
            {
              key: "importers",
              header: "Importing files",
              cell: (e) => thousands(e.importers),
              sort: (e) => e.importers,
              numeric: true,
            },
            { key: "modules", header: "Modules", cell: (e) => e.modules.map(name).join(", ") },
          ]}
        />
      </Panel>
      <p className="muted">
        <strong>Method:</strong> {architecture.method}
        {architecture.fileEdgesTruncated
          ? " File-level edges were truncated to keep the artifact small."
          : ""}
      </p>
    </>
  );
}
