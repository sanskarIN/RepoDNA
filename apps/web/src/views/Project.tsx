import {
  confidenceLabel,
  type CommandCandidate,
  type DocCheckStatus,
  type ExecutionResult,
} from "@repodna/schema";
import { thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
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
import { useDataset } from "../state";

const DOC_STATUS: Record<DocCheckStatus, { text: string; color: string }> = {
  present: { text: "Present", color: "var(--good)" },
  partial: { text: "Partial", color: "var(--attention)" },
  "not-detected": { text: "Not detected", color: "var(--info)" },
};

function Commands({ commands, empty }: { commands: readonly CommandCandidate[]; empty: string }) {
  if (commands.length === 0) {
    return <p className="muted">{empty}</p>;
  }
  return (
    <DataTable
      rows={commands}
      rowKey={(c, index) => `${c.purpose}:${c.command}:${c.workingDirectory}:${index}`}
      limit={12}
      columns={[
        { key: "purpose", header: "Purpose", cell: (c) => c.purpose, sort: (c) => c.purpose },
        { key: "command", header: "Command", cell: (c) => <code>{c.command}</code> },
        {
          key: "where",
          header: "Run in",
          cell: (c) => <span className="path">{c.workingDirectory || "."}</span>,
        },
        {
          key: "source",
          header: "Found in",
          cell: (c) => <span className="path">{c.source}</span>,
        },
        {
          key: "status",
          header: "Status",
          cell: (c) =>
            c.verified ? (
              <Chip title="RepoDNA ran this command because execution was enabled">
                Ran successfully
              </Chip>
            ) : (
              <Chip>Detected, not run</Chip>
            ),
        },
      ]}
    />
  );
}

function Executions({ executions }: { executions: readonly ExecutionResult[] }) {
  if (executions.length === 0) {
    return null;
  }
  return (
    <>
      <h3>Commands RepoDNA ran</h3>
      <Note caution>
        These ran because command execution was explicitly enabled for this analysis.
      </Note>
      {executions.map((run, index) => (
        <details key={`${run.command}-${index}`} className="finding">
          <summary>
            <code>{run.command}</code> ·{" "}
            {run.cancelled
              ? "cancelled"
              : run.timedOut
                ? "timed out"
                : run.success
                  ? "succeeded"
                  : `failed (exit ${run.exitCode ?? "unknown"})`}{" "}
            in {(run.durationMs / 1000).toFixed(1)} s
          </summary>
          <pre className="note mono">{run.outputTail.join("\n") || "(no output)"}</pre>
        </details>
      ))}
    </>
  );
}

export function Project() {
  const { dna } = useDataset();
  const tests = dna.tests;
  const builds = dna.builds;
  const docs = dna.docs;
  const onboarding = dna.insights.onboarding;
  const glossary = dna.insights.glossary;

  return (
    <>
      <PageHeader title="Tests, build & docs">
        How the project is tested, built, and documented, from the files that configure them.
        Commands are shown as found in the repository and are never run unless you enable execution.
      </PageHeader>

      {onboarding.length > 0 ? (
        <Panel
          title="Getting started"
          description="A reading and setup order assembled from the repository. Review commands before running them."
        >
          <ol className="steps">
            {onboarding.map((step) => (
              <li key={step.title}>
                <strong>{step.title}</strong>
                <p>{step.description}</p>
                {step.commands.length > 0 ? (
                  <pre className="note mono">{step.commands.join("\n")}</pre>
                ) : null}
                {step.paths.length > 0 ? (
                  <p className="path muted">{step.paths.join(", ")}</p>
                ) : null}
              </li>
            ))}
          </ol>
        </Panel>
      ) : null}

      <h2 className="section-title">Tests</h2>
      <SectionStatus status={tests.status} notes={tests.notes} />
      <div className="tiles">
        <Tile
          label="Test files"
          value={thousands(tests.testFiles)}
          note={
            tests.inlineTestFiles > 0
              ? `plus ${thousands(tests.inlineTestFiles)} source files with inline tests`
              : undefined
          }
        />
        <Tile label="Test lines" value={thousands(tests.testLines)} />
        <Tile
          label="Test files per source file"
          value={tests.sourceFiles > 0 ? tests.testRatio.toFixed(2) : "–"}
          note={`${thousands(tests.sourceFiles)} source files`}
        />
        <Tile
          label="Frameworks"
          value={
            tests.frameworks.length > 0
              ? tests.frameworks.map((f) => f.name).join(", ")
              : "None detected"
          }
        />
        <Tile
          label="Coverage files"
          value={
            tests.coverageArtifacts.length > 0 ? thousands(tests.coverageArtifacts.length) : "None"
          }
          note="Coverage is read, never measured"
        />
      </div>
      <div className="grid two">
        <Panel
          title="Kinds of tests"
          table={
            <DataTable
              rows={tests.kinds}
              rowKey={(k) => k.kind}
              columns={[
                { key: "kind", header: "Kind", cell: (k) => k.kind },
                {
                  key: "files",
                  header: "Files",
                  cell: (k) => thousands(k.files),
                  sort: (k) => k.files,
                  numeric: true,
                },
              ]}
            />
          }
        >
          {tests.kinds.length === 0 ? (
            <p className="muted">No tests were recognized.</p>
          ) : (
            <BarList
              label="Test files by kind"
              unit={[" file", " files"]}
              bars={tests.kinds.map((k) => ({ label: k.kind, value: k.files }))}
            />
          )}
          {tests.testDirectories.length > 0 ? (
            <p className="muted">
              Test directories: <span className="path">{tests.testDirectories.join(", ")}</span>
            </p>
          ) : null}
        </Panel>
        <Panel title="How to run the tests">
          <Commands commands={tests.commands} empty="No test command was found." />
          {tests.ciCommands.length > 0 ? (
            <>
              <h3>Run in CI</h3>
              <Commands commands={tests.ciCommands} empty="" />
            </>
          ) : null}
          <Executions executions={tests.executions} />
        </Panel>
      </div>

      <h2 className="section-title">Build</h2>
      <SectionStatus status={builds.status} notes={builds.notes} />
      <div className="grid two">
        <Panel title="Build systems and tools">
          {builds.systems.length === 0 ? (
            <p className="muted">No build system was recognized.</p>
          ) : null}
          <ul className="list">
            {builds.systems.map((tool) => (
              <li key={`${tool.ecosystem ?? ""}:${tool.name}`}>
                <div>
                  <strong>{tool.name}</strong>
                  {tool.ecosystem ? <span className="muted"> · {tool.ecosystem}</span> : null}
                  {tool.evidence.length > 0 ? (
                    <details>
                      <summary>Evidence</summary>
                      <EvidenceList evidence={tool.evidence} />
                    </details>
                  ) : null}
                </div>
                <Chip>{confidenceLabel(tool.confidence)} confidence</Chip>
              </li>
            ))}
          </ul>
        </Panel>
        <Panel title="Requirements" description="Tools and versions the project declares it needs.">
          <DataTable
            rows={builds.requirements}
            rowKey={(r, index) => `${r.kind}:${r.name}:${index}`}
            columns={[
              { key: "name", header: "Requirement", cell: (r) => r.name, sort: (r) => r.name },
              { key: "kind", header: "Kind", cell: (r) => r.kind, sort: (r) => r.kind },
              {
                key: "version",
                header: "Version",
                cell: (r) => <span className="mono">{r.version ?? "any"}</span>,
              },
              {
                key: "source",
                header: "Declared in",
                cell: (r) => <span className="path">{r.source}</span>,
              },
            ]}
          />
        </Panel>
      </div>
      <Panel title="Build and run commands">
        <Commands commands={builds.commands} empty="No build or run commands were found." />
        <Executions executions={builds.executions} />
      </Panel>
      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Continuous integration">
          {builds.ci.length === 0 ? <p className="muted">No CI configuration was found.</p> : null}
          <DataTable
            rows={builds.ci}
            rowKey={(c) => c.path}
            columns={[
              { key: "provider", header: "Provider", cell: (c) => c.provider },
              { key: "path", header: "File", cell: (c) => <span className="path">{c.path}</span> },
              { key: "jobs", header: "Jobs", cell: (c) => c.jobs.join(", ") || "–" },
            ]}
          />
        </Panel>
        <Panel title="Configuration files">
          <DataTable
            rows={builds.configurationFiles}
            rowKey={(c) => c.path}
            limit={12}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (c) => <span className="path">{c.path}</span>,
                sort: (c) => c.path,
              },
              { key: "purpose", header: "Purpose", cell: (c) => c.purpose, sort: (c) => c.purpose },
            ]}
          />
          {builds.containers.length > 0 ? (
            <p className="muted">
              Container files: <span className="path">{builds.containers.join(", ")}</span>
            </p>
          ) : null}
        </Panel>
      </div>

      <h2 className="section-title">Documentation</h2>
      <SectionStatus status={docs.status} notes={docs.notes} />
      <div className="tiles">
        <Tile
          label="Documentation files"
          value={thousands(docs.docFiles)}
          note={`${thousands(docs.docLines)} lines`}
        />
        <Tile
          label="README"
          value={docs.readme ? `${thousands(docs.readme.words)} words` : "Not found"}
          note={docs.readme?.path}
        />
        <Tile
          label="License"
          value={docs.license?.spdx ?? (docs.license ? "Unrecognized" : "Not found")}
          note={
            docs.license
              ? `${docs.license.path} · ${confidenceLabel(docs.license.confidence)} confidence`
              : undefined
          }
        />
        <Tile label="Examples" value={thousands(docs.examples.length)} />
      </div>
      <div className="grid two">
        <Panel
          title="Documentation checklist"
          description="What a newcomer usually looks for. Optional items are marked."
        >
          <ul className="checklist">
            {docs.checks.map((check) => (
              <li key={check.id}>
                <span
                  className="severity"
                  style={{ ["--sev" as string]: DOC_STATUS[check.status].color }}
                >
                  {DOC_STATUS[check.status].text}
                </span>{" "}
                {check.label}
                {check.optional ? <span className="muted"> (optional)</span> : null}
                {check.evidence.length > 0 ? (
                  <details>
                    <summary>Evidence</summary>
                    <EvidenceList evidence={check.evidence} />
                  </details>
                ) : null}
              </li>
            ))}
          </ul>
        </Panel>
        <Panel title="README">
          {docs.readme ? (
            <>
              <dl className="facts">
                <dt>File</dt>
                <dd className="path">{docs.readme.path}</dd>
                <dt>Length</dt>
                <dd>
                  {thousands(docs.readme.words)} words · {thousands(docs.readme.lines)} lines
                </dd>
                <dt>Installation</dt>
                <dd>{docs.readme.hasInstallation ? "Covered" : "Not found"}</dd>
                <dt>Usage</dt>
                <dd>{docs.readme.hasUsage ? "Covered" : "Not found"}</dd>
                <dt>Code examples</dt>
                <dd>{thousands(docs.readme.codeBlocks)}</dd>
                <dt>Links and images</dt>
                <dd>
                  {thousands(docs.readme.links)} links · {thousands(docs.readme.images)} images
                </dd>
              </dl>
              {docs.readme.headings.length > 0 ? (
                <details>
                  <summary>{docs.readme.headings.length} headings</summary>
                  <ul className="evidence">
                    {docs.readme.headings.map((heading, index) => (
                      <li key={`${heading}-${index}`}>{heading}</li>
                    ))}
                  </ul>
                </details>
              ) : null}
            </>
          ) : (
            <p className="muted">No README was found.</p>
          )}
          {docs.docDirectories.length > 0 ? (
            <p className="muted">
              Documentation directories:{" "}
              <span className="path">{docs.docDirectories.join(", ")}</span>
            </p>
          ) : null}
          {docs.examples.length > 0 ? (
            <p className="muted">
              Examples: <span className="path">{docs.examples.slice(0, 10).join(", ")}</span>
              {docs.examples.length > 10 ? ` and ${docs.examples.length - 10} more` : ""}
            </p>
          ) : null}
        </Panel>
      </div>

      {glossary.length > 0 ? (
        <Panel
          title="Glossary"
          description="Terms this repository uses, as RepoDNA understood them."
        >
          <dl className="facts wide">
            {glossary.map((term) => (
              <div key={term.term}>
                <dt>{term.term}</dt>
                <dd>{term.definition}</dd>
              </div>
            ))}
          </dl>
        </Panel>
      ) : null}
    </>
  );
}
