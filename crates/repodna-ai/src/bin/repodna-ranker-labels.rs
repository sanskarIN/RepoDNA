//! Offline, opt-in annotation workflow for native evidence-ranker training.
//!
//! prepare ARTIFACT.json REPOSITORY_ID TASK OUTPUT.json [SUBJECT]
//! finalize REVIEWED-1.json [REVIEWED-2.json ...] OUTPUT.json

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::context::{ContextOptions, select_context};
use repodna_ai::ranking::{
    AnnotationExample, MAX_EXAMPLES, annotation_template, finish_annotations,
};
use repodna_ai::task::AiTask;
use repodna_core::model::artifact::RepositoryDna;

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LABELS_BYTES: u64 = 8 * 1024 * 1024;

/// Read a local JSON file with a hard byte budget.
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > limit {
        return Err(format!("input exceeds {limit}-byte limit").into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > limit {
        return Err(format!("input exceeds {limit}-byte limit").into());
    }
    Ok(bytes)
}

fn required_utf8<'a>(arg: &'a OsString, name: &str) -> Result<&'a str, io::Error> {
    arg.to_str()
        .ok_or_else(|| io::Error::other(format!("{name} must be UTF-8")))
}

/// Never modify an existing training set, source artifact or checkpoint.
fn save(path: &Path, json: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(json)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn prepare(args: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if !(5..=6).contains(&args.len()) {
        return Err("usage: repodna-ranker-labels prepare ARTIFACT.json REPOSITORY_ID TASK OUTPUT.json [SUBJECT]".into());
    }
    let repository = required_utf8(&args[2], "repository ID")?;
    let task_id = required_utf8(&args[3], "task ID")?;
    let subject = args.get(5).map(|x| required_utf8(x, "subject")).transpose()?;
    let task = AiTask::parse(task_id, subject).map_err(io::Error::other)?;
    let artifact: RepositoryDna = serde_json::from_slice(&read_bounded(
        Path::new(&args[1]),
        MAX_ARTIFACT_BYTES,
    )?)?;
    // No checkout path is supplied, so reading source excerpts is impossible.
    let context = select_context(
        &artifact,
        &task,
        &ContextOptions {
            max_tokens: 16_000,
            excerpt_root: None,
        },
    )?;
    let annotations = annotation_template(repository, &task.instructions(), &context)
        .map_err(io::Error::other)?;
    let output = serde_json::to_vec_pretty(&annotations)?;
    save(Path::new(&args[4]), &output)?;
    writeln!(
        io::stdout().lock(),
        "Prepared {} unlabeled evidence items; review and set each relevant=null to true or false. Local file only.",
        annotations.len()
    )?;
    Ok(())
}

fn finalize(args: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: repodna-ranker-labels finalize REVIEWED-1.json [REVIEWED-2.json ...] OUTPUT.json".into());
    }
    let output = Path::new(args.last().ok_or("missing output path")?);
    let mut reviewed = Vec::new();
    for input in &args[1..args.len() - 1] {
        let mut batch: Vec<AnnotationExample> =
            serde_json::from_slice(&read_bounded(Path::new(input), MAX_LABELS_BYTES)?)?;
        if reviewed.len() + batch.len() > MAX_EXAMPLES {
            return Err("too many annotation examples".into());
        }
        reviewed.append(&mut batch);
    }
    let labeled = finish_annotations(reviewed).map_err(io::Error::other)?;
    // E numbers restart with each evidence selection, so uniqueness is scoped
    // to (repository ID, exact task/query, evidence ID).
    let mut keys = HashSet::new();
    for example in &labeled {
        let identity = (
            example.repository.as_str(),
            example.query.as_str(),
            example.evidence.id.as_str(),
        );
        if !keys.insert(identity) {
            return Err("duplicate evidence ID within the same repository and question".into());
        }
    }
    save(output, &serde_json::to_vec_pretty(&labeled)?)?;
    writeln!(io::stdout().lock(), "Finalized {} reviewed labels.", labeled.len())?;
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(command) = args.first().and_then(|x| x.to_str()) else {
        return Err("expected prepare or finalize subcommand".into());
    };
    match command {
        "prepare" => prepare(&args),
        "finalize" => finalize(&args),
        _ => Err("expected prepare or finalize subcommand".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "annotation failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_ai::ranking::AnnotationExample;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    #[test]
    fn prepare_requires_explicit_review_before_finalize() {
        let temp = tempfile::tempdir().unwrap();
        let dna_path = temp.path().join("dna.json");
        let template_path = temp.path().join("unreviewed.json");
        let reviewed_path = temp.path().join("reviewed.json");
        let final_path = temp.path().join("labeled.json");
        let artifact = RepositoryDna::new(
            RepositoryIdentity {
                name: "sample-project".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        fs::write(&dna_path, serde_json::to_vec(&artifact).unwrap()).unwrap();
        let args = vec![
            OsString::from("prepare"),
            dna_path.as_os_str().to_owned(),
            OsString::from("sample-repository"),
            OsString::from("repository"),
            template_path.as_os_str().to_owned(),
        ];
        prepare(&args).unwrap();
        let mut rows: Vec<AnnotationExample> =
            serde_json::from_slice(&fs::read(&template_path).unwrap()).unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| row.relevant.is_none()));
        let finalize_args = vec![
            OsString::from("finalize"),
            template_path.as_os_str().to_owned(),
            final_path.as_os_str().to_owned(),
        ];
        assert!(finalize(&finalize_args).is_err());
        assert!(!final_path.exists());
        for row in &mut rows {
            row.relevant = Some(true);
        }
        fs::write(&reviewed_path, serde_json::to_vec(&rows).unwrap()).unwrap();
        let complete_args = vec![
            OsString::from("finalize"),
            reviewed_path.as_os_str().to_owned(),
            final_path.as_os_str().to_owned(),
        ];
        finalize(&complete_args).unwrap();
        let finished: Vec<repodna_ai::ranking::LabeledEvidence> =
            serde_json::from_slice(&fs::read(&final_path).unwrap()).unwrap();
        assert_eq!(rows.len(), finished.len());
        assert!(finished.iter().all(|row| row.relevant));
        assert!(finalize(&complete_args).is_err());
    }

    #[test]
    fn finalize_rejects_duplicate_evidence_in_combined_batches() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("labels.json");
        let output = temp.path().join("output.json");
        let data = vec![AnnotationExample {
            repository: "repo".into(),
            query: "question".into(),
            evidence: repodna_ai::EvidenceItem {
                id: "E1".into(),
                kind: repodna_ai::EvidenceKind::Metric,
                reference: "size".into(),
                detail: "Three files".into(),
            },
            relevant: Some(true),
        }];
        fs::write(&input, serde_json::to_vec(&data).unwrap()).unwrap();
        let args = vec![
            OsString::from("finalize"),
            input.as_os_str().to_owned(),
            input.as_os_str().to_owned(),
            output.as_os_str().to_owned(),
        ];
        assert!(finalize(&args).is_err());
        assert!(!output.exists());
    }
}
