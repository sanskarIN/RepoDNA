import {
  categoryLabel,
  confidenceLabel,
  describeEvidence,
  isSuppressed,
  severityLabel,
  type Finding,
} from "@repodna/schema";

/**
 * Findings can contain text from untrusted repositories. Escape Markdown punctuation,
 * inline HTML and newlines so source text cannot create links or extra report sections.
 */
function plainMarkdown(value: string): string {
  return value
    .replace(/[\u0000-\u001f\u007f]+/g, " ")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/([\\`*_{}\[\]()#+\-.!|~])/g, "\\$1")
    .trim();
}

function bullet(label: string, value: string): string {
  return `- **${label}:** ${plainMarkdown(value)}`;
}

function section(label: string, values: readonly string[]): string[] {
  return values.length > 0
    ? [`**${label}**`, ...values.map((value) => `- ${plainMarkdown(value)}`), ""]
    : [];
}

/**
 * Exports the complete matching findings (not merely the current page) as a
 * deterministic, human-readable review document. This is not a full artifact.
 */
export function findingsMarkdown(findings: readonly Finding[]): string {
  const lines = [
    "# RepoDNA findings review",
    "",
    `Matching findings: ${findings.length}`,
    "",
    "Generated from the currently filtered findings. This is a review snapshot, not a full RepoDNA artifact.",
    "Repository paths, commit messages and evidence may be sensitive; review before sharing.",
    "",
  ];
  for (const [index, finding] of findings.entries()) {
    lines.push(`## ${index + 1}. ${plainMarkdown(finding.title)}`, "");
    lines.push(
      bullet("Severity", severityLabel(finding.severity)),
      bullet("Category", categoryLabel(finding.category)),
      bullet("Rule", finding.rule),
      bullet("Confidence", confidenceLabel(finding.confidence)),
      bullet("Suppressed", isSuppressed(finding) ? "Yes" : "No"),
    );
    if (finding.paths?.length) {
      lines.push(bullet("Paths", finding.paths.join("; ")));
    }
    lines.push("", bullet("Observation", finding.summary));
    if (finding.rationale) lines.push(bullet("Why it matters", finding.rationale));
    if (finding.method) lines.push(bullet("Method", finding.method));
    if (finding.suppressed) {
      lines.push(bullet("Suppression reason", finding.suppressed.reason));
      lines.push(bullet("Suppression source", finding.suppressed.source));
    }
    lines.push(
      "",
      ...section("Evidence", (finding.evidence ?? []).map(describeEvidence)),
      ...section("Limitations", finding.limitations ?? []),
      ...section("Suggested next steps", finding.nextSteps ?? []),
      "---",
      "",
    );
  }
  return `${lines.join("\n").trimEnd()}\n`;
}
