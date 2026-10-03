//! Parsing and checking the model's answer against the evidence that was sent.

use repodna_core::Confidence;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::Context;
use crate::prompt::MAX_POINTS;
use crate::text::sanitize;

/// Longest summary kept, in characters.
const MAX_SUMMARY: usize = 4_000;
/// Longest point or limitation kept, in characters.
const MAX_STATEMENT: usize = 1_000;
/// Most points kept (a little above what is requested).
const KEEP_POINTS: usize = MAX_POINTS + 8;
/// Most limitations kept.
const KEEP_LIMITATIONS: usize = 20;

/// How a statement is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    /// Cites evidence that was sent.
    Evidence,
    /// Marked by the model as going beyond the evidence.
    Inference,
    /// Cites no valid evidence.
    Unsupported,
}

impl Support {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Evidence => "supported by cited evidence",
            Self::Inference => "model inference, not established by the analysis",
            Self::Unsupported => "not supported by cited evidence",
        }
    }
}

/// One statement of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerPoint {
    /// The statement.
    pub text: String,
    /// Valid evidence identifiers it cites.
    pub evidence: Vec<String>,
    /// How it is supported.
    pub support: Support,
}

/// A checked answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Short summary written by the model.
    pub summary: String,
    /// Statements.
    pub points: Vec<AnswerPoint>,
    /// Model-reported confidence, lowered when statements lack support.
    pub confidence: Confidence,
    /// What the answer could not establish, from the model and from the checks.
    pub limitations: Vec<String>,
    /// `true` when the model returned the requested JSON structure.
    pub structured: bool,
    /// Cited identifiers that were not part of the evidence.
    pub invalid_citations: Vec<String>,
}

impl Answer {
    /// Distinct evidence identifiers cited by the answer, in first-cited order.
    pub fn cited(&self) -> Vec<&str> {
        let mut cited: Vec<&str> = Vec::new();
        for id in self.points.iter().flat_map(|point| &point.evidence) {
            if !cited.contains(&id.as_str()) {
                cited.push(id);
            }
        }
        cited
    }
}

/// Finds the first JSON object in `text` that has a `summary` field, tolerating code
/// fences and prose around it.
fn find_object(text: &str) -> Option<serde_json::Map<String, Value>> {
    for (start, _) in text.match_indices('{').take(64) {
        let mut stream = serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>();
        if let Some(Ok(Value::Object(map))) = stream.next()
            && map.contains_key("summary")
        {
            return Some(map);
        }
    }
    None
}

/// Normalizes a cited identifier: `"E3"`, `"e3"`, `"[E3]"`, `3`, or `{"id": "E3"}`.
fn citation(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim().trim_matches(|c| c == '[' || c == ']');
            let digits = trimmed
                .strip_prefix('E')
                .or_else(|| trimmed.strip_prefix('e'))
                .unwrap_or(trimmed);
            digits.parse::<u32>().ok().map(|n| format!("E{n}"))
        }
        Value::Number(number) => number.as_u64().map(|n| format!("E{n}")),
        Value::Object(map) => map.get("id").and_then(citation),
        _ => None,
    }
}

fn parse_confidence(value: Option<&Value>) -> Confidence {
    match value
        .and_then(Value::as_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("high") => Confidence::High,
        Some("medium") => Confidence::Medium,
        _ => Confidence::Low,
    }
}

fn strings(value: Option<&Value>, max: usize) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|text| sanitize(text, MAX_STATEMENT))
                .filter(|text| !text.is_empty())
                .take(max)
                .collect()
        })
        .unwrap_or_default()
}

/// Parses and checks `text` against the evidence in `context`.
pub fn check_answer(text: &str, context: &Context) -> Answer {
    let Some(object) = find_object(text) else {
        return Answer {
            summary: sanitize(text, MAX_SUMMARY),
            points: Vec::new(),
            confidence: Confidence::Low,
            limitations: vec![
                "The model did not return the requested structure, so its text is shown unverified."
                    .to_owned(),
            ],
            structured: false,
            invalid_citations: Vec::new(),
        };
    };

    let mut invalid = Vec::new();
    let mut points = Vec::new();
    for raw in object
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(KEEP_POINTS)
    {
        let (statement, cited, inference) = match raw {
            Value::String(text) => (text.as_str(), &[][..], false),
            Value::Object(map) => (
                map.get("text").and_then(Value::as_str).unwrap_or_default(),
                map.get("evidence")
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice),
                map.get("inference")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
            _ => continue,
        };
        let statement = sanitize(statement, MAX_STATEMENT);
        if statement.is_empty() {
            continue;
        }
        let mut evidence: Vec<String> = Vec::new();
        for id in cited.iter().filter_map(citation) {
            if context.item(&id).is_some() {
                if !evidence.contains(&id) {
                    evidence.push(id);
                }
            } else if !invalid.contains(&id) {
                invalid.push(id);
            }
        }
        let support = if inference {
            Support::Inference
        } else if evidence.is_empty() {
            Support::Unsupported
        } else {
            Support::Evidence
        };
        points.push(AnswerPoint {
            text: statement,
            evidence,
            support,
        });
    }

    let mut limitations = strings(object.get("limitations"), KEEP_LIMITATIONS);
    let mut confidence = parse_confidence(object.get("confidence"));
    let unsupported = points
        .iter()
        .filter(|point| point.support == Support::Unsupported)
        .count();
    if unsupported > 0 {
        confidence = confidence.min(Confidence::Medium);
        if unsupported * 2 > points.len() {
            confidence = Confidence::Low;
        }
        limitations.push(format!(
            "{unsupported} of {} statements cite no evidence that was provided and are marked as not supported.",
            points.len()
        ));
    }
    if !invalid.is_empty() {
        limitations.push(format!(
            "The answer cited evidence that was not provided ({}); those citations were removed.",
            invalid.join(", ")
        ));
    }
    let summary = object
        .get("summary")
        .and_then(Value::as_str)
        .map(|text| sanitize(text, MAX_SUMMARY))
        .unwrap_or_default();
    Answer {
        summary,
        points,
        confidence,
        limitations,
        structured: true,
        invalid_citations: invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};

    fn context() -> Context {
        Context {
            items: (1..=3)
                .map(|n| EvidenceItem {
                    id: format!("E{n}"),
                    kind: EvidenceKind::File,
                    reference: format!("src/{n}.rs"),
                    detail: "A file.".into(),
                })
                .collect(),
            omitted: 0,
            estimated_tokens: 30,
            excerpts: false,
        }
    }

    #[test]
    fn checks_citations_and_support() {
        let text = r#"Here is the answer:
```json
{"summary": "A small \u001b[31mlibrary.",
 "points": [
   {"text": "The client lives in src/1.rs.", "evidence": ["E1", "e2", 3, {"id": "E1"}]},
   {"text": "It probably handles retries.", "evidence": ["E2"], "inference": true},
   {"text": "It is used by 40 services.", "evidence": ["E9"]},
   "A bare statement."
 ],
 "confidence": "HIGH",
 "limitations": ["No tests were described."]}
```"#;
        let answer = check_answer(text, &context());
        assert!(answer.structured);
        assert_eq!(answer.summary, "A small [31mlibrary.");
        assert_eq!(answer.points.len(), 4);
        assert_eq!(answer.points[0].evidence, vec!["E1", "E2", "E3"]);
        assert_eq!(answer.points[0].support, Support::Evidence);
        assert_eq!(answer.points[1].support, Support::Inference);
        assert_eq!(answer.points[2].support, Support::Unsupported);
        assert_eq!(answer.points[3].support, Support::Unsupported);
        assert_eq!(answer.invalid_citations, vec!["E9"]);
        assert_eq!(answer.confidence, Confidence::Medium);
        assert_eq!(answer.cited(), vec!["E1", "E2", "E3"]);
        assert!(answer.limitations[0].contains("No tests"));
        assert!(answer.limitations.iter().any(|l| l.contains("2 of 4")));
        assert!(answer.limitations.iter().any(|l| l.contains("E9")));
    }

    #[test]
    fn lowers_confidence_when_most_statements_lack_support() {
        let text = r#"{"summary": "s", "points": [{"text": "a"}, {"text": "b", "evidence": ["E1"]}, {"text": "c"}], "confidence": "high"}"#;
        assert_eq!(check_answer(text, &context()).confidence, Confidence::Low);
        let fine = r#"{"summary": "s", "points": [{"text": "b", "evidence": ["E1"]}], "confidence": "high"}"#;
        let answer = check_answer(fine, &context());
        assert_eq!(answer.confidence, Confidence::High);
        assert!(answer.limitations.is_empty());
    }

    #[test]
    fn keeps_unstructured_text_as_unverified() {
        let answer = check_answer("I think it is a web server.", &context());
        assert!(!answer.structured);
        assert_eq!(answer.summary, "I think it is a web server.");
        assert_eq!(answer.confidence, Confidence::Low);
        assert!(answer.limitations[0].contains("unverified"));
        let skipped = check_answer(r#"{"note": 1} then {"summary": "ok"}"#, &context());
        assert!(skipped.structured);
        assert_eq!(skipped.summary, "ok");
    }
}
