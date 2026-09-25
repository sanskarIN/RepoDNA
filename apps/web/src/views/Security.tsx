import { confidenceLabel } from "@repodna/schema";
import { thousands } from "@repodna/visualization";
import { Chip, Note, PageHeader, Panel, SectionStatus, Tile } from "../components/common";
import { DataTable } from "../components/DataTable";
import { useDataset } from "../state";

export function Security() {
  const { dna } = useDataset();
  const security = dna.security;
  const secrets = security.secrets;
  const inCode = secrets.filter((s) => !s.inTestOrExample);

  return (
    <>
      <PageHeader title="Security signals">
        Patterns that deserve a look from someone who knows the code.
      </PageHeader>
      <Note caution>{security.disclaimer}</Note>
      <SectionStatus status={security.status} notes={security.notes} />
      <div className="tiles">
        <Tile label="Files scanned" value={thousands(security.filesScanned)} />
        <Tile
          label="Possible secrets"
          value={thousands(secrets.length)}
          note={
            secrets.length > 0
              ? `${thousands(inCode.length)} outside tests and examples`
              : undefined
          }
        />
        <Tile label="Risky patterns" value={thousands(security.patterns.length)} />
        <Tile label="File permission issues" value={thousands(security.permissions.length)} />
      </div>
      <Note>{security.advisories.note}</Note>

      <Panel
        title="Possible secrets"
        description="Values that look like credentials. RepoDNA never stores or shows the value itself, only a short fingerprint to tell findings apart."
      >
        {secrets.length === 0 ? (
          <p className="muted">Nothing that looks like a credential was found.</p>
        ) : (
          <DataTable
            rows={secrets}
            rowKey={(s) => s.id}
            initialSort={{ key: "where", descending: false }}
            columns={[
              {
                key: "rule",
                header: "Looks like",
                cell: (s) => s.description,
                sort: (s) => s.rule,
              },
              {
                key: "where",
                header: "Where",
                cell: (s) => (
                  <>
                    <span className="path">{`${s.path}:${s.line}`}</span>{" "}
                    {s.inTestOrExample ? (
                      <Chip title="In a test, fixture, or example file">test or example</Chip>
                    ) : null}
                  </>
                ),
                sort: (s) => `${s.inTestOrExample ? 1 : 0}${s.path}`,
              },
              {
                key: "confidence",
                header: "Confidence",
                cell: (s) => confidenceLabel(s.confidence),
              },
              {
                key: "fingerprint",
                header: "Fingerprint",
                cell: (s) => <span className="mono">{s.fingerprint}</span>,
              },
            ]}
          />
        )}
      </Panel>

      <Panel
        title="Risky patterns"
        description="Code and configuration that is often, but not always, a problem."
      >
        {security.patterns.length === 0 ? (
          <p className="muted">None of the known risky patterns were found.</p>
        ) : (
          security.patterns.map((pattern) => (
            <div key={pattern.id} className="finding">
              <h3>
                {pattern.description} <Chip>{pattern.category}</Chip>{" "}
                <Chip>{confidenceLabel(pattern.confidence)} confidence</Chip>
              </h3>
              <p className="path">
                {pattern.path}
                {pattern.line != null ? `:${pattern.line}` : ""}
              </p>
              <p>
                <strong>What to check:</strong> {pattern.recommendation}
              </p>
              <p className="muted">
                <code>{pattern.rule}</code>
              </p>
            </div>
          ))
        )}
      </Panel>

      {security.permissions.length > 0 ? (
        <Panel title="File permissions">
          <DataTable
            rows={security.permissions}
            rowKey={(p) => p.path}
            columns={[
              {
                key: "path",
                header: "File",
                cell: (p) => <span className="path">{p.path}</span>,
                sort: (p) => p.path,
              },
              { key: "mode", header: "Mode", cell: (p) => <span className="mono">{p.mode}</span> },
              { key: "issue", header: "Issue", cell: (p) => p.issue },
            ]}
          />
        </Panel>
      ) : null}

      <Panel
        title="Rules used"
        description="What this scan looks for; anything else is out of scope."
      >
        <DataTable
          rows={security.rules}
          rowKey={(r) => r.id}
          limit={10}
          columns={[
            { key: "id", header: "Rule", cell: (r) => <code>{r.id}</code>, sort: (r) => r.id },
            { key: "kind", header: "Kind", cell: (r) => r.kind, sort: (r) => r.kind },
            { key: "description", header: "Looks for", cell: (r) => r.description },
          ]}
        />
      </Panel>
    </>
  );
}
