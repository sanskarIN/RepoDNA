import { useEffect, useMemo, useRef, useState } from "react";
import { confidenceLabel, type RepositoryDna } from "@repodna/schema";
import { date, thousands } from "@repodna/visualization";
import { Chip, CommandBox, ErrorBox, Note, PageHeader, Panel, Select } from "../components/common";
import { DataTable } from "../components/DataTable";
import type { PrivacyPreset, ReportFormat, ReportTheme, SaveFormat } from "../lib/backend";
import { useEngine } from "../components/BrowserAnalysis";
import { renderCardTask, renderReportTask } from "../engine/client";
import { useReportOperation } from "../lib/useReportOperation";
import {
  asReportTask, createPreview, mapReportTask, nativeSaveGate, releasePreview,
  type PreviewResource, type ReportTask,
} from "../lib/reportOperation";
import { artifactFileName, downloadBlob, useDownload } from "../lib/download";
import { unitFor } from "../lib/names";
import { useApp, useDataset } from "../state";

const THEMES: readonly (readonly [ReportTheme, string])[] = [
  ["professional", "Professional theme"],
  ["minimal", "Minimal theme"],
  ["technical", "Technical theme"],
  ["dark", "Dark theme"],
];

const PRIVACY: readonly (readonly [PrivacyPreset, string])[] = [
  ["local", "Local: the complete analysis"],
  ["share", "Share: without commit messages, marker text, or command output"],
  ["public", "Public: also pseudonymous contributors, no remote URLs or symbol names"],
];

/** Each report: its format, its name, and the button that saves it in the desktop app. */
const FORMATS: [ReportFormat, string, string][] = [
  ["html", "Interactive HTML report", "Save the HTML report…"],
  ["markdown", "Markdown report", "Save the Markdown report…"],
  ["json", "JSON artifact", "Save the analysis as JSON…"],
];

/** Visible state of a scoped generation or save operation. */
function OperationNotice<T>({ operation }: { operation: ReturnType<typeof useReportOperation<T>> }) {
  const { state } = operation;
  if (state.status === "failed") return <ErrorBox>{state.message}</ErrorBox>;
  if (state.status === "cancelled") return <p role="status">{state.label} cancelled.</p>;
  if (state.status !== "running") return null;
  return (
    <p role="status">
      {state.label}…{" "}
      {state.cancellable ? <button type="button" onClick={operation.cancel}>Cancel</button> : null}
    </p>
  );
}

/** The Project DNA card uses a scoped object URL; stale previews cannot be exported. */
function CardPreview({ source, render, renderPng, downloads = false, save }: {
  source: unknown;
  render: (dark: boolean) => ReportTask<string>;
  renderPng?: (dark: boolean) => ReportTask<Uint8Array<ArrayBuffer>>;
  downloads?: boolean;
  save?: (dark: boolean, png: boolean) => Promise<string | null>;
}) {
  const [dark, setDark] = useState(false);
  const [retry, setRetry] = useState(0);
  const scope = useMemo(() => ({ source, dark, retry }), [source, dark, retry]);
  const preview = useReportOperation<PreviewResource>(scope, releasePreview);
  const action = useReportOperation<string | Uint8Array<ArrayBuffer> | null>(scope);
  const renderer = useRef(render);
  renderer.current = render;
  const launch = useRef(preview.start);
  launch.current = preview.start;
  const [download, failure] = useDownload();
  useEffect(() => {
    launch.current("Rendering card", () => mapReportTask(renderer.current(dark), createPreview));
  }, [scope, dark]);
  const ready = preview.state.status === "ready" ? preview.state.value : null;
  const busy = action.state.status === "running";
  const name = dark ? "dna-card-dark" : "dna-card";
  return (
    <Panel title="Project DNA card" description="A summary image of this analysis for READMEs, slides, and posts."
      actions={<button type="button" className="ghost" aria-pressed={dark} disabled={busy}
        onClick={() => setDark((value) => !value)}>{dark ? "Light card" : "Dark card"}</button>}>
      <OperationNotice operation={preview} />
      <OperationNotice operation={action} />
      {failure ? <ErrorBox>{failure}</ErrorBox> : null}
      {preview.state.status === "failed" || preview.state.status === "cancelled" ? (
        <button type="button" onClick={() => setRetry((value) => value + 1)}>Retry card</button>
      ) : null}
      {ready ? <img className="card-preview" src={ready.url} alt="Project DNA card for this repository" /> : null}
      {ready && downloads ? (
        <p className="actions">
          <button type="button" disabled={busy} onClick={() => download(`${name}.svg`, ready.svg, "image/svg+xml")}>Download SVG</button>
          {renderPng ? <button type="button" disabled={busy} onClick={() => {
            action.start("Rendering PNG", () => renderPng(dark), {}, (value) => {
              if (value instanceof Uint8Array) downloadBlob(`${name}.png`, new Blob([value], { type: "image/png" }));
            });
          }}>Download PNG</button> : null}
        </p>
      ) : null}
      {ready && save ? (
        <p className="actions">
          {([false, true] as const).map((png) => (
            <button key={String(png)} type="button" disabled={busy} onClick={() => {
              action.start("Saving card", () => asReportTask(save(dark, png)), { cancellable: false, deadlineMs: null });
            }}>{png ? "Save PNG…" : "Save SVG…"}</button>
          ))}
        </p>
      ) : null}
      {action.state.status === "ready" && typeof action.state.value === "string" ? <Note>Saved to {action.state.value}</Note> : null}
    </Panel>
  );
}

function StoredReports({ repositoryId, scanId, dna }: { repositoryId: string; scanId?: string; dna: RepositoryDna }) {
  const { backend } = useApp();
  const [theme, setTheme] = useState<ReportTheme>("professional");
  const [privacy, setPrivacy] = useState<PrivacyPreset>("local");
  const source = useMemo(() => ({ backend, repositoryId, scanId, dna }), [backend, repositoryId, scanId, dna]);
  const scope = useMemo(() => ({ source, theme, privacy }), [source, theme, privacy]);
  const operation = useReportOperation<string | null>(scope);
  if (!backend) return null;
  const options = { scan: scanId, theme, privacy };
  const save = (format: SaveFormat) => {
    const saveReport = backend.saveReport;
    if (!saveReport) return;
    operation.start("Saving report", () => asReportTask(nativeSaveGate(backend).run(() =>
      saveReport.call(backend, repositoryId, format, options),
    )), { cancellable: false, deadlineMs: null });
  };
  return (
    <>
      <Panel title="Reports" description="Generated from this stored analysis on your machine.">
        <div className="filters">
          <Select
            id="report-theme"
            label="Report theme"
            value={theme}
            options={THEMES}
            onChange={setTheme}
          />
          <Select
            id="report-privacy"
            label="Privacy preset"
            value={privacy}
            options={PRIVACY}
            onChange={setPrivacy}
          />
        </div>
        {backend.saveReport ? (
          <div className="actions">
            {FORMATS.map(([format, , saveLabel]) => (
              <button key={format} type="button" disabled={operation.state.status === "running"} onClick={() => save(format)}>
                {saveLabel}
              </button>
            ))}
            <button type="button" disabled={operation.state.status === "running"} onClick={() => save("bundle")}>
              Save the full report folder…
            </button>
          </div>
        ) : (
          <ul className="list">
            {FORMATS.map(([format, label]) => {
              const url = backend.reportUrl(repositoryId, format, options);
              return url ? (
                <li key={format}>
                  <span>{label}</span>
                  <a className="button" href={url} target="_blank" rel="noopener noreferrer">
                    Open
                  </a>
                </li>
              ) : null;
            })}
          </ul>
        )}
        {operation.state.status === "ready" && operation.state.value ? <Note>Saved to {operation.state.value}</Note> : null}
        <OperationNotice operation={operation} />
        <p className="muted">
          The full report folder (HTML, Markdown, JSON, CSV tables, and the card as SVG and PNG) is
          also available from the command line: <code>repodna report &lt;repository&gt;</code>.
        </p>
      </Panel>
      <CardPreview
        source={source}
        render={(dark) => asReportTask(backend.card(repositoryId, dark, scanId))}
        downloads={!backend.saveCard}
        save={
          backend.saveCard
            ? (dark, png) =>
                nativeSaveGate(backend).run(() => backend.saveCard?.(repositoryId, dark, png, scanId) ?? Promise.resolve(null))
            : undefined
        }
      />
    </>
  );
}

/**
 * Reports and the card of an analysis file, made in this browser by the WebAssembly build
 * of RepoDNA that the web version comes with.
 */
function BrowserReports({ dna }: { dna: RepositoryDna }) {
  const [theme, setTheme] = useState<ReportTheme>("professional");
  const [privacy, setPrivacy] = useState<PrivacyPreset>("local");
  const [download, failure] = useDownload();
  const artifact = useMemo(() => JSON.stringify(dna), [dna]);
  const scope = useMemo(() => ({ dna, theme, privacy }), [dna, theme, privacy]);
  const operation = useReportOperation<string>(scope);
  const base = artifactFileName(dna).replace(/\.repodna$/, "");
  const make = (format: "html" | "markdown") => {
    operation.start(`Making ${format.toUpperCase()} report`, () => renderReportTask(artifact, { format, theme, privacy }), {}, (text) => {
      download(`${base}.${format === "html" ? "html" : "md"}`, text, format === "html" ? "text/html" : "text/markdown");
    });
  };
  return (
    <>
      <Panel
        title="Reports"
        description="Made in this browser from the open analysis, as repodna report makes them. Nothing is uploaded."
      >
        <div className="filters">
          <Select
            id="report-theme"
            label="Report theme"
            value={theme}
            options={THEMES}
            onChange={setTheme}
          />
          <Select
            id="report-privacy"
            label="Privacy preset"
            value={privacy}
            options={PRIVACY}
            onChange={setPrivacy}
          />
        </div>
        <div className="actions">
          <button type="button" disabled={operation.state.status === "running"} onClick={() => make("html")}>
            Download the HTML report
          </button>
          <button type="button" disabled={operation.state.status === "running"} onClick={() => make("markdown")}>
            Download the Markdown report
          </button>
          <button
            type="button"
            onClick={() =>
              download(
                artifactFileName(dna),
                `${JSON.stringify(dna, null, 2)}\n`,
                "application/json",
              )
            }
          >
            Download the analysis as loaded
          </button>
        </div>
        <OperationNotice operation={operation} />
        {failure ? <ErrorBox>{failure}</ErrorBox> : null}
        <p className="muted">
          The privacy preset of this file is <strong>{dna.analysisMetadata.privacy.preset}</strong>;
          reports made from it can remove more, never add back what was removed.
        </p>
      </Panel>
      <CardPreview
        source={dna}
        render={(dark) => renderCardTask(artifact, dark)}
        renderPng={(dark) => renderCardTask(artifact, dark, true)}
        downloads
      />
    </>
  );
}

export function Reports() {
  const dataset = useDataset();
  const engine = useEngine();
  const [download, failure] = useDownload();
  const { dna, origin } = dataset;
  const meta = dna.analysisMetadata;
  const thresholds = Object.entries(meta.thresholds) as [string, number][];

  return (
    <>
      <PageHeader title="Reports & export">
        Share what RepoDNA found, and see exactly how this analysis was made.
      </PageHeader>
      {origin.kind === "stored" ? (
        <StoredReports repositoryId={origin.repositoryId} scanId={origin.scanId} dna={dna} />
      ) : engine ? (
        <BrowserReports dna={dna} />
      ) : (
        <Panel
          title="This analysis file"
          description="Opened on this machine; nothing was uploaded."
        >
          <p>
            <button
              type="button"
              onClick={() =>
                download(
                  artifactFileName(dna),
                  `${JSON.stringify(dna, null, 2)}\n`,
                  "application/json",
                )
              }
            >
              Download the analysis as loaded
            </button>
          </p>
          {failure ? <ErrorBox>{failure}</ErrorBox> : null}
          <p>To create reports or a DNA card from the file, run:</p>
          <CommandBox
            lines={[
              `repodna report ${artifactFileName(dna)} --format html --output report.html`,
              `repodna card ${artifactFileName(dna)} --output dna-card.svg`,
            ]}
          />
          <p className="muted">
            The privacy preset of this file is <strong>{meta.privacy.preset}</strong>; reports made
            from it can remove more, never add back what was removed.
          </p>
        </Panel>
      )}

      <Panel title="How this analysis was made">
        <dl className="facts">
          <dt>Repository</dt>
          <dd>
            {dna.identity.name}
            {dna.identity.owner ? ` by ${dna.identity.owner}` : ""}
          </dd>
          <dt>Input</dt>
          <dd>
            {meta.input.kind.replace(/-/g, " ")}: <span className="path">{meta.input.display}</span>
          </dd>
          {meta.revision ? (
            <>
              <dt>Revision</dt>
              <dd className="mono">
                {meta.revision}
                {meta.dirty ? " (with uncommitted changes)" : ""}
              </dd>
            </>
          ) : null}
          <dt>Generated</dt>
          <dd>
            {meta.generatedAt.replace("T", " ").replace(/\.\d+/, "")} in{" "}
            {(meta.durationMs / 1000).toFixed(1)} s
          </dd>
          <dt>Profile</dt>
          <dd>{meta.profile}</dd>
          <dt>RepoDNA</dt>
          <dd>
            {dna.tool.version} · schema {dna.schemaVersion}
          </dd>
          <dt>Platform</dt>
          <dd>
            {meta.platform.os} {meta.platform.arch}
          </dd>
          <dt>Analysis ID</dt>
          <dd className="mono">{meta.id}</dd>
          <dt>Configuration</dt>
          <dd>
            <span className="mono">{meta.configHash.slice(0, 16)}</span>
            {meta.configSources.length > 0 ? (
              <>
                {" "}
                from <span className="path">{meta.configSources.join(", ")}</span>
              </>
            ) : (
              " (defaults)"
            )}
          </dd>
          <dt>Privacy</dt>
          <dd>
            {meta.privacy.preset} preset · network {meta.privacy.networkUsed ? "used" : "not used"}{" "}
            · remote AI {meta.privacy.remoteAi ? "used" : "not used"} · telemetry{" "}
            {meta.privacy.telemetry ? "on" : "none"}
            {meta.privacy.redactions.length > 0 ? (
              <ul className="evidence">
                {meta.privacy.redactions.map((redaction) => (
                  <li key={redaction}>{redaction}</li>
                ))}
              </ul>
            ) : null}
          </dd>
          {meta.ai ? (
            <>
              <dt>AI</dt>
              <dd>
                {meta.ai.provider} {meta.ai.model} · {thousands(meta.ai.requests)}{" "}
                {meta.ai.requests === 1 ? "request" : "requests"}{" "}
                {meta.ai.remote ? "to a remote service" : "on this machine"}
              </dd>
            </>
          ) : null}
        </dl>
        {meta.partial ? (
          <Note caution>Some analyzers did not finish, so parts of this analysis are missing.</Note>
        ) : null}
        {meta.warnings.length > 0 ? (
          <ul className="evidence">
            {meta.warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        ) : null}
      </Panel>

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Analyzers">
          <DataTable
            rows={meta.analyzers}
            rowKey={(a) => a.stage}
            limit={30}
            columns={[
              { key: "label", header: "Analyzer", cell: (a) => a.label },
              {
                key: "status",
                header: "Status",
                cell: (a) => (
                  <>
                    {a.status}
                    {a.message ? <span className="muted"> · {a.message}</span> : null}
                  </>
                ),
                sort: (a) => a.status,
              },
              {
                key: "duration",
                header: "Time",
                cell: (a) => `${thousands(a.durationMs)} ms`,
                sort: (a) => a.durationMs,
                numeric: true,
              },
            ]}
          />
        </Panel>
        <Panel title="Data sources" description="Everything this analysis read.">
          <DataTable
            rows={meta.dataSources}
            rowKey={(s, index) => `${s.kind}:${s.name}:${index}`}
            columns={[
              { key: "name", header: "Source", cell: (s) => s.name },
              { key: "kind", header: "Kind", cell: (s) => s.kind.replace(/-/g, " ") },
              { key: "detail", header: "Detail", cell: (s) => s.detail },
            ]}
          />
          {dna.plugins.length > 0 ? (
            <>
              <h3>Plugins</h3>
              <DataTable
                rows={dna.plugins}
                rowKey={(p) => p.name}
                columns={[
                  { key: "name", header: "Plugin", cell: (p) => `${p.name} ${p.version}` },
                  {
                    key: "status",
                    header: "Status",
                    cell: (p) => (p.message ? `${p.status} · ${p.message}` : p.status),
                  },
                  {
                    key: "findings",
                    header: "Findings",
                    cell: (p) => thousands(p.findings),
                    numeric: true,
                  },
                ]}
              />
            </>
          ) : null}
        </Panel>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="Thresholds"
          description="Limits that decide when a measurement becomes a finding. Change them in repodna.toml."
        >
          <DataTable
            rows={thresholds}
            rowKey={([key]) => key}
            limit={20}
            columns={[
              { key: "name", header: "Threshold", cell: ([key]) => <code>{key}</code> },
              {
                key: "value",
                header: "Value",
                cell: ([, value]) => thousands(value),
                numeric: true,
              },
            ]}
          />
        </Panel>
        <Panel
          title="Suppressions"
          description={`${
            meta.suppressedFindings === 1
              ? "1 finding was"
              : `${thousands(meta.suppressedFindings)} findings were`
          } suppressed by these rules.`}
        >
          {meta.suppressions.length === 0 ? (
            <p className="muted">No suppression rules were configured.</p>
          ) : (
            <DataTable
              rows={meta.suppressions}
              rowKey={(s, index) => `${s.rule}:${s.path ?? ""}:${index}`}
              columns={[
                { key: "rule", header: "Rule", cell: (s) => <code>{s.rule}</code> },
                {
                  key: "path",
                  header: "Path",
                  cell: (s) => <span className="path">{s.path ?? "any"}</span>,
                },
                { key: "reason", header: "Reason", cell: (s) => s.reason },
              ]}
            />
          )}
        </Panel>
      </div>

      <Panel
        title="Measurements"
        description="Every raw metric behind the findings and the Project DNA, with its definition."
      >
        <DataTable
          rows={dna.metrics.raw}
          rowKey={(m) => m.id}
          limit={15}
          columns={[
            {
              key: "label",
              header: "Metric",
              cell: (m) => <span title={m.definition}>{m.label}</span>,
              sort: (m) => m.label,
            },
            {
              key: "value",
              header: "Value",
              cell: (m) => `${Number(m.value.toFixed(2))} ${unitFor(m.value, m.unit)}`,
              numeric: true,
            },
            { key: "confidence", header: "Confidence", cell: (m) => confidenceLabel(m.confidence) },
            { key: "method", header: "Method", cell: (m) => m.method },
          ]}
        />
        {dna.metrics.confidence.length > 0 ? (
          <>
            <h3>Confidence by section</h3>
            <ul className="evidence">
              {dna.metrics.confidence.map((c) => (
                <li key={c.section}>
                  <Chip>{confidenceLabel(c.confidence)}</Chip> {c.section}: {c.reason}
                </li>
              ))}
            </ul>
          </>
        ) : null}
      </Panel>
      <p className="muted">Analysis generated {date(meta.generatedAt)}.</p>
    </>
  );
}
