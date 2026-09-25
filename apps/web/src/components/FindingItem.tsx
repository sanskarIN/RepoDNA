import {
  categoryLabel,
  confidenceLabel,
  describeEvidence,
  isSuppressed,
  type Finding,
} from "@repodna/schema";
import { Chip, SeverityBadge } from "./common";

/** One finding with its reasoning, method, evidence, limitations, and next steps. */
export function FindingItem({ finding }: { finding: Finding }) {
  const suppressed = isSuppressed(finding);
  const limitations = finding.limitations ?? [];
  const nextSteps = finding.nextSteps ?? [];
  return (
    <article className="finding" aria-label={finding.title}>
      <h3>
        <SeverityBadge severity={finding.severity} />
        <span>{finding.title}</span>
        {suppressed ? <Chip title={finding.suppressed?.reason}>Suppressed</Chip> : null}
      </h3>
      {finding.summary ? <p>{finding.summary}</p> : null}
      <p className="muted">
        {categoryLabel(finding.category)} · {confidenceLabel(finding.confidence)} confidence ·{" "}
        <code>{finding.rule}</code>
      </p>
      <details>
        <summary>Evidence and method</summary>
        {finding.rationale ? <p>{finding.rationale}</p> : null}
        {finding.evidence.length > 0 ? (
          <ul className="evidence">
            {finding.evidence.map((evidence, index) => (
              <li key={index} className="path">
                {describeEvidence(evidence)}
              </li>
            ))}
          </ul>
        ) : null}
        {finding.method ? (
          <p>
            <strong>Method:</strong> {finding.method}
          </p>
        ) : null}
        {limitations.length > 0 ? (
          <>
            <p>
              <strong>Limitations</strong>
            </p>
            <ul className="evidence">
              {limitations.map((limitation) => (
                <li key={limitation}>{limitation}</li>
              ))}
            </ul>
          </>
        ) : null}
        {nextSteps.length > 0 ? (
          <>
            <p>
              <strong>Next steps</strong>
            </p>
            <ul className="evidence">
              {nextSteps.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ul>
          </>
        ) : null}
        {suppressed ? (
          <p className="muted">
            Suppressed by {finding.suppressed?.source}: {finding.suppressed?.reason}
          </p>
        ) : null}
      </details>
    </article>
  );
}
