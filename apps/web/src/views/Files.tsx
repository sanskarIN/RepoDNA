import { useMemo, useState } from "react";
import { confidenceLabel, type FileCategory, type FileRecord } from "@repodna/schema";
import { bytes, compact, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import {
  Chip,
  EvidenceList,
  PageHeader,
  Panel,
  SearchBox,
  SectionStatus,
  Select,
  Tile,
} from "../components/common";
import { DataTable } from "../components/DataTable";
import { count, languageNamer } from "../lib/names";
import { useRoute } from "../lib/router";
import { useDataset } from "../state";

const CATEGORY_LABELS: Record<FileCategory, string> = {
  source: "Source",
  test: "Tests",
  documentation: "Documentation",
  configuration: "Configuration",
  "ci-cd": "CI/CD",
  build: "Build",
  manifest: "Manifests",
  lockfile: "Lockfiles",
  generated: "Generated",
  asset: "Assets",
  binary: "Binary",
  vendor: "Vendored",
  "build-output": "Build output",
  "ide-metadata": "Editor settings",
  container: "Containers",
  infrastructure: "Infrastructure",
  data: "Data",
  license: "License",
  other: "Other",
};

function flags(file: FileRecord): string[] {
  const list: string[] = [];
  if (file.generated) {
    list.push("generated");
  }
  if (file.vendored) {
    list.push("vendored");
  }
  if (file.binary) {
    list.push("binary");
  }
  if (file.skipped) {
    list.push(file.skipped === "too-large" ? "too large to read" : "unreadable");
  }
  return list;
}

export function Files() {
  const { dna } = useDataset();
  const route = useRoute();
  const structure = dna.structure;
  const [query, setQuery] = useState(() => route.params.get("q") ?? "");
  const [category, setCategory] = useState<"all" | FileCategory>("all");
  const [language, setLanguage] = useState("all");
  const languageName = useMemo(() => languageNamer(dna), [dna]);
  const languages = useMemo(
    () =>
      [
        ...new Set(structure.files.map((f) => f.language).filter((l): l is string => Boolean(l))),
      ].sort(),
    [structure.files],
  );
  const filtered = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    return structure.files.filter(
      (f) =>
        (category === "all" || f.category === category) &&
        (language === "all" || f.language === language) &&
        terms.every((term) => f.path.toLowerCase().includes(term)),
    );
  }, [structure.files, query, category, language]);
  const symbols = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) {
      return [];
    }
    return structure.symbols.filter((s) =>
      terms.every((term) => `${s.name} ${s.path}`.toLowerCase().includes(term)),
    );
  }, [structure.symbols, query]);
  const categories = [...structure.categories].sort((a, b) => b.files - a.files);
  const topDirectories = structure.directories
    .filter((d) => d.depth === 1)
    .sort((a, b) => b.codeLines - a.codeLines);

  return (
    <>
      <PageHeader title="Files">
        Every file RepoDNA looked at, with its category, language, and size. Ignored paths (from
        .gitignore and configuration) are not listed.
      </PageHeader>
      <SectionStatus status={structure.status} notes={structure.notes} />
      <div className="tiles">
        <Tile
          label="Files"
          value={thousands(structure.totalFiles)}
          note={`${structure.sizeClass} repository`}
        />
        <Tile
          label="Lines"
          value={compact(structure.totalLines)}
          note={`${compact(structure.codeLines)} code · ${compact(structure.commentLines)} comments`}
        />
        <Tile label="Size" value={bytes(structure.totalBytes)} />
        <Tile
          label="Generated"
          value={thousands(structure.generatedFiles)}
          note={`${thousands(structure.vendoredFiles)} vendored`}
        />
        <Tile
          label="Binary"
          value={thousands(structure.binaryFiles)}
          note={`${thousands(structure.skippedFiles)} skipped`}
        />
        <Tile label="Directories" value={thousands(structure.directories.length)} />
      </div>
      {structure.truncated ? (
        <p className="note caution">
          The file list was truncated to keep the artifact small; totals still count every file.
        </p>
      ) : null}

      <div className="grid two">
        <Panel
          title="What the files are"
          table={
            <DataTable
              rows={categories}
              rowKey={(c) => c.category}
              columns={[
                {
                  key: "category",
                  header: "Category",
                  cell: (c) => CATEGORY_LABELS[c.category],
                  sort: (c) => CATEGORY_LABELS[c.category],
                },
                {
                  key: "files",
                  header: "Files",
                  cell: (c) => thousands(c.files),
                  sort: (c) => c.files,
                  numeric: true,
                },
                {
                  key: "lines",
                  header: "Lines",
                  cell: (c) => thousands(c.lines),
                  sort: (c) => c.lines,
                  numeric: true,
                },
                {
                  key: "bytes",
                  header: "Size",
                  cell: (c) => bytes(c.bytes),
                  sort: (c) => c.bytes,
                  numeric: true,
                },
              ]}
            />
          }
        >
          <BarList
            label="Files per category"
            unit={[" file", " files"]}
            bars={categories.map((c) => ({
              label: CATEGORY_LABELS[c.category],
              value: c.files,
              details: `${thousands(c.lines)} lines · ${bytes(c.bytes)}`,
            }))}
          />
        </Panel>
        <Panel
          title="Top-level directories"
          table={
            <DataTable
              rows={structure.directories}
              rowKey={(d) => d.path}
              initialSort={{ key: "code", descending: true }}
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
                  key: "code",
                  header: "Code lines",
                  cell: (d) => thousands(d.codeLines),
                  sort: (d) => d.codeLines,
                  numeric: true,
                },
                {
                  key: "bytes",
                  header: "Size",
                  cell: (d) => bytes(d.bytes),
                  sort: (d) => d.bytes,
                  numeric: true,
                },
                { key: "language", header: "Mostly", cell: (d) => languageName(d.primaryLanguage) },
              ]}
            />
          }
        >
          {topDirectories.length === 0 ? (
            <p className="muted">All files are at the top level.</p>
          ) : (
            <BarList
              label="Code lines per top-level directory"
              bars={topDirectories.slice(0, 12).map((d) => ({
                label: d.path,
                value: d.codeLines,
                details: `${count(d.files, "file", "files")}${d.primaryLanguage ? ` · mostly ${languageName(d.primaryLanguage)}` : ""}`,
              }))}
              unit={[" line", " lines"]}
            />
          )}
        </Panel>
      </div>

      {structure.entrypoints.length > 0 ? (
        <Panel title="Entry points" description="Where execution or use of this code starts.">
          <DataTable
            rows={structure.entrypoints}
            rowKey={(e) => `${e.kind}:${e.path}`}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (e) => <span className="path">{e.path}</span>,
                sort: (e) => e.path,
              },
              { key: "kind", header: "Kind", cell: (e) => e.kind, sort: (e) => e.kind },
              { key: "reason", header: "Why", cell: (e) => e.reason },
              {
                key: "confidence",
                header: "Confidence",
                cell: (e) => confidenceLabel(e.confidence),
              },
              {
                key: "evidence",
                header: "Evidence",
                cell: (e) =>
                  e.evidence.length > 0 ? (
                    <details>
                      <summary>{e.evidence.length}</summary>
                      <EvidenceList evidence={e.evidence} />
                    </details>
                  ) : (
                    "–"
                  ),
              },
            ]}
          />
        </Panel>
      ) : null}

      <Panel title="Explorer" id="explorer">
        <div className="filters">
          <SearchBox
            value={query}
            onChange={setQuery}
            label="Search files and symbols"
            placeholder="Search paths and symbols"
          />
          <Select
            id="file-category"
            label="Category"
            value={category}
            options={[
              ["all", "All categories"] as const,
              ...categories.map((c) => [c.category, CATEGORY_LABELS[c.category]] as const),
            ]}
            onChange={setCategory}
          />
          <Select
            id="file-language"
            label="Language"
            value={language}
            options={[
              ["all", "All languages"] as const,
              ...languages
                .map((l) => [l, languageName(l)] as const)
                .sort((a, b) => a[1].localeCompare(b[1])),
            ]}
            onChange={setLanguage}
          />
          <span className="muted" aria-live="polite">
            {filtered.length === structure.files.length
              ? `${thousands(filtered.length)} files`
              : `${thousands(filtered.length)} of ${thousands(structure.files.length)} files`}
          </span>
        </div>
        <DataTable
          rows={filtered}
          rowKey={(f) => f.path}
          initialSort={{ key: "path", descending: false }}
          limit={50}
          columns={[
            {
              key: "path",
              header: "Path",
              cell: (f) => (
                <>
                  <span className="path">{f.path}</span>{" "}
                  {flags(f).map((flag) => (
                    <Chip key={flag}>{flag}</Chip>
                  ))}
                </>
              ),
              sort: (f) => f.path,
            },
            {
              key: "category",
              header: "Category",
              cell: (f) => CATEGORY_LABELS[f.category],
              sort: (f) => f.category,
            },
            {
              key: "language",
              header: "Language",
              cell: (f) => languageName(f.language),
              sort: (f) => languageName(f.language),
            },
            {
              key: "code",
              header: "Code lines",
              cell: (f) => (f.lines ? thousands(f.lines.code) : "–"),
              sort: (f) => f.lines?.code ?? -1,
              numeric: true,
            },
            {
              key: "bytes",
              header: "Size",
              cell: (f) => bytes(f.bytes),
              sort: (f) => f.bytes,
              numeric: true,
            },
            {
              key: "complexity",
              header: "Most complex function",
              cell: (f) =>
                f.analysis && f.analysis.functions > 0 ? thousands(f.analysis.cyclomaticMax) : "–",
              sort: (f) => f.analysis?.cyclomaticMax ?? -1,
              numeric: true,
            },
            {
              key: "module",
              header: "Module",
              cell: (f) => f.module ?? "–",
              sort: (f) => f.module ?? "",
            },
          ]}
        />
        {symbols.length > 0 ? (
          <>
            <h3>Matching symbols</h3>
            <DataTable
              rows={symbols}
              rowKey={(s, index) => `${s.path}:${s.line}:${s.name}:${index}`}
              limit={20}
              columns={[
                {
                  key: "name",
                  header: "Symbol",
                  cell: (s) => <span className="mono">{s.name}</span>,
                  sort: (s) => s.name,
                },
                { key: "kind", header: "Kind", cell: (s) => s.kind, sort: (s) => s.kind },
                {
                  key: "where",
                  header: "Where",
                  cell: (s) => <span className="path">{`${s.path}:${s.line}`}</span>,
                  sort: (s) => s.path,
                },
                {
                  key: "complexity",
                  header: "Complexity",
                  cell: (s) => (s.complexity != null ? thousands(s.complexity) : "–"),
                  sort: (s) => s.complexity ?? -1,
                  numeric: true,
                },
              ]}
            />
          </>
        ) : null}
      </Panel>
    </>
  );
}
