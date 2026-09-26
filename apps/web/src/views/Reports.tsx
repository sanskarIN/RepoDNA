import { useEffect, useState } from "react";
import { confidenceLabel } from "@repodna/schema";
import { date, thousands } from "@repodna/visualization";
import { Chip, ErrorBox, Note, PageHeader, Panel, Select } from "../components/common";
import { DataTable } from "../components/DataTable";
import type { PrivacyPreset, ReportFormat, ReportTheme, SaveFormat } from "../lib/backend";
import { artifactFileName, downloadText } from "../lib/download";
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

const FORMATS: [ReportFormat, string][] = [
  ["html", "Interactive HTML report"],
  ["markdown", "Markdown report"],
  ["json", "JSON artifact"],
];

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The Project DNA card, rendered by the same code as `repodna card`. */
function CardPreview({ repositoryId, scanId }: { repositoryId: string; scanId?: string }) {
  const { backend } = useApp();
  const [dark, setDark] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backend) {
      return;
    }
    let cancelled = false;
    let objectUrl: string | null = null;
    backend.card(repositoryId, dark, scanId).then(
      (text) => {
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(new Blob([text], { type: "image/svg+xml" }));
        setSvg(text);
        setUrl(objectUrl);
        setError(null);
      },
      (reason: unknown) => !cancelled && setError(message(reason)),
    );
    return () => {
      cancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  }, [backend, repositoryId, scanId, dark]);

  return (
    <Panel
      title="Project DNA card"
      description="A summary image of this analysis for READMEs, slides, and posts."
      actions={
        <button
          type="button"
          className="ghost"
          aria-pressed={dark}
          onClick={() => setDark((value) => !value)}
        >
          {dark ? "Light card" : "Dark card"}
        </button>
      }
    >
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      {url ? (
        <img className="card-preview" src={url} alt="Project DNA card for this repository" />
      ) : null}
      {svg && !backend?.saveReport ? (
        <p>
          <button
            type="button"
            onClick={() =>
              downloadText(dark ? "dna-card-dark.svg" : "dna-card.svg", svg, "image/svg+xml")
            }
          >
            Download SVG
          </button>
        </p>
      ) : null}
    </Panel>
  );
}

function StoredReports({ repositoryId, scanId }: { repositoryId: string; scanId?: string }) {
  const { backend } = useApp();
  const [theme, setTheme] = useState<ReportTheme>("professional");
  const [privacy, setPrivacy] = useState<PrivacyPreset>("local");
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!backend) {
    return null;
  }
  const options = { scan: scanId, theme, privacy };
  const save = async (format: SaveFormat) => {
    setError(null);
    setSaved(null);
    try {
      const path = await backend.saveReport?.(repositoryId, format, options);
      if (path) {
        setSaved(path);
      }
    } catch (reason) {
      setError(message(reason));
    }
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
            {FORMATS.map(([format, label]) => (
              <button key={format} type="button" onClick={() => void save(format)}>
                Save {label.charAt(0).toLowerCase() + label.slice(1)}…
              </button>
            ))}
            <button type="button" onClick={() => void save("bundle")}>
              Save the full report folder…
            </button>
            <button type="button" onClick={() => void save("card")}>
              Save the DNA card…
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
        {saved ? <Note>Saved to {saved}</Note> : null}
        {error ? <ErrorBox>{error}</ErrorBox> : null}
        <p className="muted">
          The full report folder (HTML, Markdown, JSON, CSV tables, and the card as SVG and PNG) is
          also available from the command line: <code>repodna report &lt;repository&gt;</code>.
        </p>
      </Panel>
      <CardPreview repositoryId={repositoryId} scanId={scanId} />
    </>
  );
}

export function Reports() {
  const dataset = useDataset();
  const { dna, origin } = dataset;
  const meta = dna.analysisMetadata;
  const thresholds = Object.entries(meta.thresholds) as [string, number][];

  return (
    <>
      <PageHeader title="Reports & export">
        Share what RepoDNA found, and see exactly how this analysis was made.
      </PageHeader>
      {origin.kind === "stored" ? (
        <StoredReports repositoryId={origin.repositoryId} scanId={origin.scanId} />
      ) : (
        <Panel
          title="This analysis file"
          description="Opened on this machine; nothing was uploaded."
        >
          <p>
            <button
              type="button"
              onClick={() =>
                downloadText(
                  artifactFileName(dna),
                  `${JSON.stringify(dna, null, 2)}\n`,
                  "application/json",
                )
              }
            >
              Download the analysis as loaded
            </button>
          </p>
          <p>To create reports or a DNA card from the file, run:</p>
          <pre className="note mono">
            {`repodna report ${artifactFileName(dna)} --format html --output report.html\nrepodna card ${artifactFileName(dna)} --output dna-card.svg`}
          </pre>
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
            {meta.privacy.preset} preset · network {meta.privacy.networkUsed ? "used" : "not used"}
            {meta.privacy.remoteAi ? " · remote AI used" : ""} · telemetry{" "}
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
          description={`${thousands(meta.suppressedFindings)} findings were suppressed by these rules.`}
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
