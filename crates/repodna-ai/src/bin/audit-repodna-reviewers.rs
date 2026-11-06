//! Offline, aggregate-only agreement audit for two independently reviewed
//! RepoDNA evidence annotation files.
//!
//! Usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json]
//! Reviewers must judge the exact same evidence records. This audit measures
//! consistency, not independence, factual correctness or reviewer expertise.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{AnnotationExample, MAX_EXAMPLES, finish_annotations};

#[path = "support/bounded_input.rs"]
mod bounded_input;

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;

fn load(path: &Path) -> Result<Vec<AnnotationExample>, Box<dyn std::error::Error>> {
    let bytes =
        bounded_input::read_bounded_file(path, MAX_DATASET_BYTES, "review file exceeds 8 MiB")?;
    let examples: Vec<AnnotationExample> = serde_json::from_slice(&bytes)?;
    if examples.is_empty() || examples.len() > MAX_EXAMPLES {
        return Err("review file must contain 1..=100000 records".into());
    }
    if examples.iter().any(|example| {
        example.repository.trim().is_empty()
            || example.repository.len() > 200
            || example.query.trim().is_empty()
            || example.query.len() > 4_000
            || example.evidence.detail.trim().is_empty()
            || example.evidence.detail.len() > 8_000
    }) {
        return Err("review file has missing or oversized fields".into());
    }
    // Validation also rejects null judgments, duplicate evidence IDs,
    // missing evidence fields and raw source excerpts.
    finish_annotations(examples.clone()).map_err(io::Error::other)?;
    Ok(examples)
}

/// Exact metadata identity (including evidence ID), excluding the label.
/// The entire record is compared so no reviewer can silently change a fact.
fn records(
    reviews: &[AnnotationExample],
) -> Result<BTreeMap<Vec<u8>, bool>, Box<dyn std::error::Error>> {
    let mut result = BTreeMap::new();
    for review in reviews {
        let identity = serde_json::to_vec(&(&review.repository, &review.query, &review.evidence))?;
        let relevant = review.relevant.ok_or("unreviewed annotation")?;
        if result.insert(identity, relevant).is_some() {
            return Err("duplicate annotation record".into());
        }
    }
    Ok(result)
}

fn agreement(
    a: &[AnnotationExample],
    b: &[AnnotationExample],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let first = records(a)?;
    let second = records(b)?;
    if first.len() != second.len() || !first.keys().eq(second.keys()) {
        return Err("review files contain different evidence records".into());
    }
    let mut both_relevant = 0_usize;
    let mut both_irrelevant = 0_usize;
    let mut only_a_relevant = 0_usize;
    let mut only_b_relevant = 0_usize;
    for (identity, first_label) in &first {
        let second_label = second
            .get(identity)
            .ok_or("review files contain different evidence records")?;
        match (*first_label, *second_label) {
            (true, true) => both_relevant += 1,
            (false, false) => both_irrelevant += 1,
            (true, false) => only_a_relevant += 1,
            (false, true) => only_b_relevant += 1,
        }
    }
    let count = first.len();
    let a_positive = both_relevant + only_a_relevant;
    let b_positive = both_relevant + only_b_relevant;
    let observed = (both_relevant + both_irrelevant) as f64 / count as f64;
    let a_rate = a_positive as f64 / count as f64;
    let b_rate = b_positive as f64 / count as f64;
    let chance = a_rate * b_rate + (1.0 - a_rate) * (1.0 - b_rate);
    // When both reviewers always choose the same one class, chance agreement
    // is one and kappa is undefined, even though observed agreement is 100%.
    let kappa = if chance >= 1.0 {
        None
    } else {
        Some((observed - chance) / (1.0 - chance))
    };

    Ok(serde_json::json!({
        "schema": "repodna-reviewer-agreement/1",
        "pairedAnnotations": count,
        "agreements": both_relevant + both_irrelevant,
        "disagreements": only_a_relevant + only_b_relevant,
        "bothRelevant": both_relevant,
        "bothIrrelevant": both_irrelevant,
        "onlyReviewerARelevant": only_a_relevant,
        "onlyReviewerBRelevant": only_b_relevant,
        "reviewerAPositiveRate": a_rate,
        "reviewerBPositiveRate": b_rate,
        "observedAgreement": observed,
        "expectedChanceAgreement": chance,
        "cohensKappa": kappa,
        "note": "Agreement is descriptive, not proof of annotation correctness, independent review, repository rights, or model accuracy. Undefined kappa is reported as null. No evidence text or reviewer identities are printed."
    }))
}

/// Check a third, explicitly finalized set of labels against both completed
/// independent reviews. This is a structural gate, *not* proof a human
/// adjudicator actually inspected each disputed case.
fn adjudication(
    a: &[AnnotationExample],
    b: &[AnnotationExample],
    final_review: &[AnnotationExample],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let first = records(a)?;
    let second = records(b)?;
    let final_labels = records(final_review)?;
    if first.len() != second.len()
        || first.len() != final_labels.len()
        || !first.keys().eq(second.keys())
        || !first.keys().eq(final_labels.keys())
    {
        return Err("adjudication files contain different evidence records".into());
    }

    let mut agreed_labels_preserved = 0_usize;
    let mut disagreements_adjudicated = 0_usize;
    let mut adjudicated_for_a = 0_usize;
    let mut adjudicated_for_b = 0_usize;
    let mut final_positive_labels = 0_usize;
    for (identity, first_label) in &first {
        let second_label = second
            .get(identity)
            .ok_or("adjudication files contain different evidence records")?;
        let final_label = final_labels
            .get(identity)
            .ok_or("adjudication files contain different evidence records")?;
        if *final_label {
            final_positive_labels += 1;
        }
        if first_label == second_label {
            // An override of two agreeing reviewers needs a separate documented
            // correction protocol, not a silent change to the finalized labels.
            if first_label != final_label {
                return Err(
                    "finalized label changes a judgment on which both reviewers agreed".into(),
                );
            }
            agreed_labels_preserved += 1;
        } else {
            disagreements_adjudicated += 1;
            if first_label == final_label {
                adjudicated_for_a += 1;
            } else {
                adjudicated_for_b += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "schema": "repodna-reviewer-adjudication/1",
        "structuralChecksPassed": true,
        "agreedLabelsPreserved": agreed_labels_preserved,
        "disagreementsAdjudicated": disagreements_adjudicated,
        "adjudicatedInFavorOfReviewerA": adjudicated_for_a,
        "adjudicatedInFavorOfReviewerB": adjudicated_for_b,
        "finalPositiveLabels": final_positive_labels,
        "finalNegativeLabels": final_labels.len() - final_positive_labels,
        "note": "All records are complete, paired and preserve agreed reviewer labels. This cannot prove that contested labels were independently adjudicated, that any label is correct, or that repository metadata may be published. Reviewer identities and evidence content are not emitted."
    }))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(a), Some(b), final_path, None) = (args.next(), args.next(), args.next(), args.next())
    else {
        return Err(
            "usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json]".into(),
        );
    };
    let first = load(Path::new(&a))?;
    let second = load(Path::new(&b))?;
    let mut report = agreement(&first, &second)?;
    if let Some(path) = final_path {
        // Load and validate the entire finalized dataset before producing any
        // output. Missing judgments and changed evidence fail closed.
        let finalized = load(Path::new(&path))?;
        report["adjudication"] = adjudication(&first, &second, &finalized)?;
    }
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "review agreement audit failed: {error}"
        );
        std::process::exit(1);
    }
}
