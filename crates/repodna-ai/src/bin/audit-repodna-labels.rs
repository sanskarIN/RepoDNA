//! Inspect local training, validation and final-test labels before fitting.
//!
//! Usage: `audit-repodna-labels TRAIN.json VALIDATION.json TEST.json`
//! Reports structural readiness, coverage and possible copied-evidence
//! contamination. It cannot verify reviewer expertise or license compliance.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, training_data_fingerprint, validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;

fn load(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > MAX_DATASET_BYTES {
        return Err("dataset exceeds 8 MiB".into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > MAX_DATASET_BYTES {
        return Err("dataset grew beyond 8 MiB".into());
    }
    let samples: Vec<LabeledEvidence> = serde_json::from_slice(&bytes)?;
    if samples.len() > MAX_EXAMPLES {
        return Err("dataset exceeds maximum example count".into());
    }
    if samples.iter().any(|sample| {
        sample.repository.trim().is_empty()
            || sample.repository.len() > 200
            || sample.query.trim().is_empty()
            || sample.query.len() > 4_000
            || sample.evidence.detail.trim().is_empty()
            || sample.evidence.detail.len() > 8_000
    }) {
        return Err("invalid or oversized label fields".into());
    }
    Ok(samples)
}

fn evidence_signature(sample: &LabeledEvidence) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Exclude repository and evidence ID intentionally: cloned repository
    // analysis could have new IDs while repeating evidence and questions.
    Ok(serde_json::to_vec(&(
        &sample.query,
        &sample.evidence.kind,
        &sample.evidence.reference,
        &sample.evidence.detail,
    ))?)
}

fn signatures(samples: &[LabeledEvidence]) -> Result<HashSet<Vec<u8>>, Box<dyn std::error::Error>> {
    samples.iter().map(evidence_signature).collect()
}

fn summary(samples: &[LabeledEvidence]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut per_repo: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_query: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    let mut per_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut positives = 0_usize;
    for sample in samples {
        let repository = sample.repository.trim().to_lowercase();
        *per_repo.entry(repository.clone()).or_default() += 1;
        let counts = per_query
            .entry((repository, sample.query.clone()))
            .or_default();
        if sample.relevant {
            positives += 1;
            counts.0 += 1;
        } else {
            counts.1 += 1;
        }
        let kind = serde_json::to_value(sample.evidence.kind)?;
        let kind = kind
            .as_str()
            .ok_or("evidence kind did not serialize as string")?;
        *per_kind.entry(kind.to_owned()).or_default() += 1;
    }
    let groups_with_positive = per_query.values().filter(|(yes, _)| *yes > 0).count();
    let groups_with_negative = per_query.values().filter(|(_, no)| *no > 0).count();
    let groups_with_both = per_query
        .values()
        .filter(|(yes, no)| *yes > 0 && *no > 0)
        .count();
    let largest_repo = per_repo.values().copied().max().unwrap_or(0);
    let largest_repo_share = largest_repo as f64 / samples.len().max(1) as f64;
    let unique_evidence = signatures(samples)?.len();
    Ok(serde_json::json!({
        "examples": samples.len(),
        "repositories": per_repo.len(),
        "questions": per_query.len(),
        "positiveLabels": positives,
        "negativeLabels": samples.len() - positives,
        "questionsWithRelevantEvidence": groups_with_positive,
        "questionsWithIrrelevantEvidence": groups_with_negative,
        "questionsWithBothLabels": groups_with_both,
        "questionsWithoutRelevantEvidence": per_query.len() - groups_with_positive,
        "largestRepositoryExampleShare": largest_repo_share,
        "evidenceKinds": per_kind,
        "repeatedEvidenceSignaturesWithinSplit": samples.len() - unique_evidence,
        "labelFingerprintSha256": training_data_fingerprint(samples)?,
    }))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(train), Some(validation), Some(test), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: audit-repodna-labels TRAIN.json VALIDATION.json TEST.json".into());
    };
    let train = load(Path::new(&train))?;
    let validation = load(Path::new(&validation))?;
    let test = load(Path::new(&test))?;

    // Fail closed for invalid IDs, duplicated IDs, excerpts, missing content
    // and canonical repository overlap. These are hard structural errors.
    validate_repository_split(&train, &validation).map_err(io::Error::other)?;
    validate_repository_split(&train, &test).map_err(io::Error::other)?;
    validate_repository_split(&validation, &test).map_err(io::Error::other)?;

    let train_summary = summary(&train)?;
    let validation_summary = summary(&validation)?;
    let test_summary = summary(&test)?;
    if train_summary["positiveLabels"] == 0
        || train_summary["negativeLabels"] == 0
        || train_summary["questionsWithBothLabels"] == 0
    {
        return Err("training requires both labels in at least one question".into());
    }
    if validation_summary["questionsWithRelevantEvidence"] == 0 {
        return Err("validation has no answerable labeled questions".into());
    }
    if test_summary["questionsWithRelevantEvidence"] == 0 {
        return Err("final test has no answerable labeled questions".into());
    }

    let train_keys = signatures(&train)?;
    let valid_keys = signatures(&validation)?;
    let test_keys = signatures(&test)?;
    let train_validation = train_keys.intersection(&valid_keys).count();
    let train_test = train_keys.intersection(&test_keys).count();
    let validation_test = valid_keys.intersection(&test_keys).count();

    // These are warnings, not proof of bad labels. Common metadata can have
    // identical text in unrelated repositories, and genuinely copied repos
    // can escape exact matching by changing even one character.
    let mut warnings = Vec::new();
    for (name, record) in [
        ("training", &train_summary),
        ("validation", &validation_summary),
        ("finalTest", &test_summary),
    ] {
        let min_repos = if name == "training" { 5 } else { 3 };
        if record["repositories"].as_u64().unwrap_or(0) < min_repos {
            warnings.push(format!(
                "{name}: fewer than {min_repos} independent repository identifiers; assess generalization risk"
            ));
        }
        if record["largestRepositoryExampleShare"]
            .as_f64()
            .unwrap_or(0.0)
            > 0.60
        {
            warnings.push(format!(
                "{name}: one repository supplies over 60% of examples"
            ));
        }
        if record["repeatedEvidenceSignaturesWithinSplit"]
            .as_u64()
            .unwrap_or(0)
            > 0
        {
            warnings.push(format!(
                "{name}: exact question/evidence text repeats within the split; inspect possible double counting"
            ));
        }
    }
    if train_validation > 0 || train_test > 0 || validation_test > 0 {
        warnings.push(
            "identical question/evidence content occurs across different splits; investigate possible clone or annotation leakage"
                .to_owned(),
        );
    }
    if train_summary["positiveLabels"].as_u64().unwrap_or(0) * 20 < train.len() as u64
        || train_summary["negativeLabels"].as_u64().unwrap_or(0) * 20 < train.len() as u64
    {
        warnings.push("training has fewer than 5% examples of one label".to_owned());
    }

    // Only aggregated counts and fingerprints; never print paths, evidence
    // details, original questions, repository IDs or individual labels.
    let report = serde_json::json!({
        "structuralChecksPassed": true,
        "training": train_summary,
        "validation": validation_summary,
        "finalTest": test_summary,
        "matchingEvidenceSignaturesAcrossSplits": {
            "trainingValidation": train_validation,
            "trainingFinalTest": train_test,
            "validationFinalTest": validation_test,
        },
        "warnings": warnings,
        "note": "Structural checks cannot verify label correctness, repository independence beyond IDs and exact-content matches, annotation rights, real-world accuracy or factual explanation fidelity. Dataset fingerprints can reveal repeated dataset use."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "label audit failed: {error}");
        std::process::exit(1);
    }
}
