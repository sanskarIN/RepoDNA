import { useMemo, useState } from "react";
import type { DependencyRecord, DependencyScope } from "@repodna/schema";
import { date, percent, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import {
  Chip,
  Note,
  PageHeader,
  Panel,
  SearchBox,
  SectionStatus,
  Select,
  Tile,
} from "../components/common";
import { DataTable } from "../components/DataTable";
import { count } from "../lib/names";
import { useRoute } from "../lib/router";
import { useDataset } from "../state";

const SCOPES: readonly (readonly ["all" | DependencyScope, string])[] = [
  ["all", "All scopes"],
  ["runtime", "Runtime"],
  ["development", "Development"],
  ["build", "Build"],
  ["optional", "Optional"],
  ["peer", "Peer"],
];

function matches(dependency: DependencyRecord, terms: string[]): boolean {
  const text =
    `${dependency.name} ${dependency.ecosystem} ${dependency.requirement ?? ""} ${dependency.manifests.join(" ")}`.toLowerCase();
  return terms.every((term) => text.includes(term));
}

export function Dependencies() {
  const { dna } = useDataset();
  const route = useRoute();
  const report = dna.dependencies;
  const [query, setQuery] = useState(() => route.params.get("q") ?? "");
  const [scope, setScope] = useState<"all" | DependencyScope>("all");
  const [ecosystem, setEcosystem] = useState("all");
  const ecosystems = useMemo(
    () => [...new Set(report.dependencies.map((d) => d.ecosystem))].sort(),
    [report.dependencies],
  );
  const filtered = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    return report.dependencies.filter(
      (d) =>
        (scope === "all" || d.scope === scope) &&
        (ecosystem === "all" || d.ecosystem === ecosystem) &&
        matches(d, terms),
    );
  }, [report.dependencies, query, scope, ecosystem]);
  const internal = report.dependencies.filter((d) => d.internal).length;
  // Lockfiles are listed separately, with how many packages they pin.
  const manifests = report.manifests.filter((m) => m.kind === "manifest");

  return (
    <>
      <PageHeader title="Dependencies">
        Declared in manifests and pinned in lockfiles. RepoDNA reads these files; it installs
        nothing and contacts no registry.
      </PageHeader>
      <SectionStatus status={report.status} notes={report.notes} />
      <div className="tiles">
        <Tile
          label="External dependencies"
          value={thousands(report.directCount)}
          note={
            internal > 0
              ? `plus ${count(internal, "package", "packages")} of this repository`
              : undefined
          }
        />
        <Tile label="Locked packages" value={thousands(report.lockedCount)} />
        <Tile label="Ecosystems" value={thousands(report.ecosystems.length)} />
        <Tile label="Manifests" value={thousands(manifests.length)} />
        <Tile label="Lockfiles" value={thousands(report.lockfiles.length)} />
        <Tile
          label="Packages locked at several versions"
          value={thousands(report.duplicates.length)}
        />
      </div>
      <Note>{report.advisories.note}</Note>

      <div className="grid two">
        <Panel
          title="Declared dependencies by ecosystem"
          table={
            <DataTable
              rows={report.ecosystems}
              rowKey={(e) => e.ecosystem}
              columns={[
                {
                  key: "ecosystem",
                  header: "Ecosystem",
                  cell: (e) => e.ecosystem,
                  sort: (e) => e.ecosystem,
                },
                {
                  key: "runtime",
                  header: "Runtime",
                  cell: (e) => thousands(e.runtime),
                  sort: (e) => e.runtime,
                  numeric: true,
                },
                {
                  key: "other",
                  header: "Other scopes",
                  cell: (e) => thousands(e.other),
                  sort: (e) => e.other,
                  numeric: true,
                },
                {
                  key: "locked",
                  header: "Locked",
                  cell: (e) => thousands(e.lockedPackages),
                  sort: (e) => e.lockedPackages,
                  numeric: true,
                },
                { key: "manifests", header: "Manifests", cell: (e) => e.manifests, numeric: true },
              ]}
            />
          }
        >
          {report.ecosystems.length === 0 ? (
            <p className="muted">No dependency manifests were found.</p>
          ) : (
            <BarList
              label="Declared dependencies by ecosystem"
              bars={[...report.ecosystems]
                .sort((a, b) => b.runtime + b.other - (a.runtime + a.other))
                .map((e) => ({
                  label: e.ecosystem,
                  value: e.runtime + e.other,
                  details: `${thousands(e.runtime)} runtime · ${thousands(e.other)} other · ${thousands(e.lockedPackages)} locked`,
                }))}
            />
          )}
        </Panel>
        <Panel
          title="Most imported packages"
          description="Share of source files that import each package."
          table={
            <DataTable
              rows={report.concentration}
              rowKey={(c) => c.name}
              columns={[
                { key: "name", header: "Package", cell: (c) => c.name, sort: (c) => c.name },
                {
                  key: "importers",
                  header: "Importing files",
                  cell: (c) => thousands(c.importers),
                  sort: (c) => c.importers,
                  numeric: true,
                },
                {
                  key: "share",
                  header: "Share",
                  cell: (c) => percent(c.share),
                  sort: (c) => c.share,
                  numeric: true,
                },
              ]}
            />
          }
        >
          {report.concentration.length === 0 ? (
            <p className="muted">No imports of external packages were resolved.</p>
          ) : (
            <BarList
              label="Share of source files importing each package"
              format={(value) => percent(value)}
              bars={report.concentration.slice(0, 10).map((c) => ({
                label: c.name,
                value: c.share,
                details: `${thousands(c.importers)} importing files`,
              }))}
            />
          )}
        </Panel>
      </div>

      <Panel title="All dependencies" id="all-dependencies">
        <div className="filters">
          <SearchBox
            value={query}
            onChange={setQuery}
            label="Search dependencies"
            placeholder="Search by name, version, or manifest"
          />
          <Select
            id="dependency-scope"
            label="Scope"
            value={scope}
            options={SCOPES}
            onChange={setScope}
          />
          <Select
            id="dependency-ecosystem"
            label="Ecosystem"
            value={ecosystem}
            options={[
              ["all", "All ecosystems"] as const,
              ...ecosystems.map((e) => [e, e] as const),
            ]}
            onChange={setEcosystem}
          />
          <span className="muted" aria-live="polite">
            {filtered.length === report.dependencies.length
              ? `${thousands(filtered.length)} dependencies`
              : `${thousands(filtered.length)} of ${thousands(report.dependencies.length)}`}
          </span>
        </div>
        <DataTable
          rows={filtered}
          rowKey={(d, index) => `${d.ecosystem}:${d.name}:${d.scope}:${index}`}
          initialSort={{ key: "name", descending: false }}
          columns={[
            {
              key: "name",
              header: "Package",
              cell: (d) => (
                <>
                  {d.name}{" "}
                  {d.internal ? (
                    <Chip title="Another package in this repository">internal</Chip>
                  ) : null}
                </>
              ),
              sort: (d) => d.name,
            },
            {
              key: "ecosystem",
              header: "Ecosystem",
              cell: (d) => d.ecosystem,
              sort: (d) => d.ecosystem,
            },
            { key: "scope", header: "Scope", cell: (d) => d.scope, sort: (d) => d.scope },
            {
              key: "requirement",
              header: "Requirement",
              cell: (d) => <span className="mono">{d.requirement ?? "–"}</span>,
            },
            {
              key: "resolved",
              header: "Locked at",
              cell: (d) => <span className="mono">{(d.resolved ?? []).join(", ") || "–"}</span>,
            },
            {
              key: "manifests",
              header: "Declared in",
              cell: (d) => <span className="path">{d.manifests.join(", ")}</span>,
            },
          ]}
        />
      </Panel>

      {report.duplicates.length > 0 ? (
        <Panel
          title="Packages locked at several versions"
          description="Often harmless, but each extra version adds install size and review work."
        >
          <DataTable
            rows={report.duplicates}
            rowKey={(d) => `${d.lockfile}:${d.name}`}
            columns={[
              { key: "name", header: "Package", cell: (d) => d.name, sort: (d) => d.name },
              {
                key: "versions",
                header: "Versions",
                cell: (d) => <span className="mono">{d.versions.join(", ")}</span>,
                sort: (d) => d.versions.length,
                numeric: true,
              },
              {
                key: "lockfile",
                header: "Lockfile",
                cell: (d) => <span className="path">{d.lockfile}</span>,
              },
            ]}
          />
        </Panel>
      ) : null}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Manifests">
          <DataTable
            rows={manifests}
            rowKey={(m) => m.path}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (m) => <span className="path">{m.path}</span>,
                sort: (m) => m.path,
              },
              {
                key: "package",
                header: "Package",
                cell: (m) =>
                  m.packageName
                    ? `${m.packageName}${m.packageVersion ? ` ${m.packageVersion}` : ""}`
                    : "–",
              },
              {
                key: "entries",
                header: "Entries",
                cell: (m) => thousands(m.entries),
                sort: (m) => m.entries,
                numeric: true,
              },
              {
                key: "status",
                header: "Read",
                cell: (m) =>
                  m.status === "parsed"
                    ? "fully"
                    : `${m.status}${m.message ? `: ${m.message}` : ""}`,
              },
            ]}
          />
        </Panel>
        <Panel title="Lockfiles">
          {report.lockfiles.length === 0 ? <p className="muted">No lockfiles were found.</p> : null}
          <DataTable
            rows={report.lockfiles}
            rowKey={(l) => l.path}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (l) => <span className="path">{l.path}</span>,
                sort: (l) => l.path,
              },
              {
                key: "packages",
                header: "Packages",
                cell: (l) => thousands(l.packages),
                sort: (l) => l.packages,
                numeric: true,
              },
              {
                key: "missing",
                header: "Declared but not locked",
                cell: (l) => (l.missingFromLock ?? []).join(", ") || "–",
              },
              {
                key: "status",
                header: "Read",
                cell: (l) =>
                  l.status === "parsed"
                    ? "fully"
                    : `${l.status}${l.message ? `: ${l.message}` : ""}`,
              },
            ]}
          />
        </Panel>
      </div>

      {report.staleSignals.length > 0 ? (
        <Panel
          title="Manifests unchanged for a long time"
          description="Based on Git history; a quiet manifest can simply mean a stable project."
        >
          <DataTable
            rows={report.staleSignals}
            rowKey={(s) => s.manifest}
            columns={[
              {
                key: "manifest",
                header: "Manifest",
                cell: (s) => <span className="path">{s.manifest}</span>,
              },
              {
                key: "last",
                header: "Last changed",
                cell: (s) => date(s.lastChanged),
                sort: (s) => s.lastChanged,
              },
              {
                key: "days",
                header: "Days unchanged",
                cell: (s) => thousands(s.daysUnchanged),
                sort: (s) => s.daysUnchanged,
                numeric: true,
              },
              { key: "description", header: "Note", cell: (s) => s.description },
            ]}
          />
        </Panel>
      ) : null}
    </>
  );
}
