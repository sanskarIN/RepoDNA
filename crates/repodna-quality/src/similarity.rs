//! Similar-file detection with MinHash signatures and locality-sensitive hashing.
//!
//! Each file is reduced to the set of its 5-token shingles. A MinHash signature of 64 values
//! estimates the Jaccard similarity of two such sets; banding the signature (16 bands of 4
//! values) finds candidate pairs without comparing every pair of files. Candidates are then
//! scored on the full signature and kept when the estimate reaches the threshold.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use repodna_core::cancel::{CancellationToken, Cancelled};
use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::quality::{SimilarFilePair, SimilarityReport};

use crate::QualityFile;

/// Signature length.
const PERMUTATIONS: usize = 64;

/// Rows per LSH band (`PERMUTATIONS / ROWS` bands).
const ROWS: usize = 4;
const BANDS: usize = PERMUTATIONS / ROWS;

/// Tokens per shingle.
const SHINGLE: usize = 5;

/// Files with fewer tokens are too small for a meaningful similarity estimate.
pub const MIN_TOKENS: usize = 50;

/// Maximum pairs kept in the report.
pub const MAX_PAIRS: usize = 100;

/// Maximum files per LSH bucket examined pairwise.
const MAX_BUCKET: usize = 50;

/// Check cancellation periodically even inside a large file or bucket.
const CHECK_INTERVAL: usize = 256;

/// How similarity is measured, for reports.
pub const SIMILARITY_METHOD: &str = "Each file is reduced to the set of its 5-token shingles \
(comments, whitespace, imports, and literal values normalized away). A 64-value MinHash \
signature estimates the Jaccard similarity of two files; 16 bands of 4 values select candidate \
pairs, which are kept when the estimate reaches the threshold. Buckets are language-specific \
and retain at most 50 files in input order; capped buckets make the result partial. Only \
files with at least 50 tokens are compared, and test files are excluded.";

/// SplitMix64 finalizer: a fast, well-distributed 64-bit mixing function.
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// MinHash signature of the shingles of `hashes`.
fn signature(hashes: &[u32], cancel: &CancellationToken) -> Result<[u64; PERMUTATIONS], Cancelled> {
    let mut signature = [u64::MAX; PERMUTATIONS];
    for (index, window) in hashes.windows(SHINGLE).enumerate() {
        if index.is_multiple_of(CHECK_INTERVAL) {
            cancel.check()?;
        }
        let shingle = window
            .iter()
            .fold(0u64, |hash, &token| mix(hash ^ u64::from(token)));
        for (slot, value) in signature.iter_mut().enumerate() {
            let permuted = mix(shingle ^ (slot as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
            if permuted < *value {
                *value = permuted;
            }
        }
    }
    Ok(signature)
}

fn equal_slots(a: &[u64; PERMUTATIONS], b: &[u64; PERMUTATIONS]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x == y).count()
}

fn band_keys(signature: &[u64; PERMUTATIONS]) -> [u64; BANDS] {
    let mut keys = [0; BANDS];
    let (rows, remainder) = signature.as_chunks::<ROWS>();
    debug_assert!(remainder.is_empty());
    for (key, rows) in keys.iter_mut().zip(rows) {
        *key = rows.iter().fold(0u64, |hash, &value| mix(hash ^ value));
    }
    keys
}

struct Candidate {
    file_index: usize,
    signature: [u64; PERMUTATIONS],
    bands: [u64; BANDS],
    retained: [bool; BANDS],
}

#[derive(Default)]
struct Bucket {
    members: Vec<usize>,
    omitted: usize,
}

// A pair can occur in multiple LSH bands. Emit it only from the earliest
// band that actually retained both members. Equal hash keys are not enough:
// one candidate might have been dropped by an earlier bucket's capacity.
fn already_paired(a: &Candidate, b: &Candidate, band: usize) -> bool {
    (0..band).any(|earlier| {
        a.retained[earlier] && b.retained[earlier] && a.bands[earlier] == b.bands[earlier]
    })
}

// Ascending key means a better result: more equal slots, lexicographic
// paths, then original candidate order for equal paths. BinaryHeap holds
// the worst retained pair at its root, so storage is bounded by MAX_PAIRS.
type PairKey<'a> = (Reverse<usize>, &'a str, &'a str, usize, usize);

fn retain_pair<'a>(best: &mut BinaryHeap<PairKey<'a>>, key: PairKey<'a>) {
    if best.len() < MAX_PAIRS {
        best.push(key);
    } else if best.peek().is_some_and(|worst| key < *worst) {
        best.pop();
        best.push(key);
    }
}

/// Finds pairs of similar files among non-test files with token streams.
///
/// The non-cancellable convenience API uses a private token; the engine
/// uses [`detect_similarity_cancellable`] to propagate cancellation.
pub fn detect_similarity(files: &[QualityFile<'_>], threshold: f64) -> SimilarityReport {
    match detect_similarity_cancellable(files, threshold, &CancellationToken::new()) {
        Ok(report) => report,
        Err(_) => unreachable!("a private cancellation token cannot be cancelled"),
    }
}

/// Finds similar files with cooperative cancellation inside the expensive
/// signature, bucket and pair passes. Cancellation returns no partial report.
pub fn detect_similarity_cancellable(
    files: &[QualityFile<'_>],
    threshold: f64,
    cancel: &CancellationToken,
) -> Result<SimilarityReport, Cancelled> {
    cancel.check()?;
    let mut report = SimilarityReport {
        status: SectionStatus::Analyzed,
        threshold,
        method: SIMILARITY_METHOD.to_owned(),
        ..SimilarityReport::default()
    };
    let mut candidates = Vec::new();
    let mut buckets: HashMap<(&str, usize, u64), Bucket> = HashMap::new();
    let mut untokenized = 0usize;

    for (file_index, file) in files.iter().enumerate() {
        cancel.check()?;
        if file.test {
            continue;
        }
        let Some(language) = file.language else {
            continue;
        };
        let Some(tokens) = file.tokens else {
            if file.analysis.is_some() {
                untokenized += 1;
            }
            continue;
        };
        if tokens.len() < MIN_TOKENS {
            continue;
        }
        let signature = signature(&tokens.hashes, cancel)?;
        let bands = band_keys(&signature);
        let mut candidate = Candidate {
            file_index,
            signature,
            bands,
            retained: [false; BANDS],
        };
        for (band, key) in bands.into_iter().enumerate() {
            // Partition before applying capacity so unrelated languages
            // never evict a valid same-language comparison.
            let bucket = buckets.entry((language, band, key)).or_default();
            if bucket.members.len() < MAX_BUCKET {
                bucket.members.push(candidates.len());
                candidate.retained[band] = true;
            } else {
                bucket.omitted += 1;
            }
        }
        candidates.push(candidate);
    }

    let mut capped = 0usize;
    let mut omitted_memberships = 0usize;
    let mut found = 0usize;
    let mut comparisons = 0usize;
    let mut best = BinaryHeap::with_capacity(MAX_PAIRS);
    for ((_, band, _), bucket) in buckets {
        cancel.check()?;
        if bucket.omitted > 0 {
            capped += 1;
            omitted_memberships += bucket.omitted;
        }
        let members = bucket.members;
        for (position, &i) in members.iter().enumerate() {
            for &j in &members[position + 1..] {
                comparisons += 1;
                if comparisons.is_multiple_of(CHECK_INTERVAL) {
                    cancel.check()?;
                }
                let (a, b) = (&candidates[i], &candidates[j]);
                if already_paired(a, b, band) {
                    continue;
                }
                let equal = equal_slots(&a.signature, &b.signature);
                let similarity = equal as f64 / PERMUTATIONS as f64;
                if similarity >= threshold {
                    found += 1;
                    retain_pair(
                        &mut best,
                        (
                            Reverse(equal),
                            files[a.file_index].path,
                            files[b.file_index].path,
                            i,
                            j,
                        ),
                    );
                }
            }
        }
    }

    if untokenized > 0 {
        report.status = SectionStatus::Partial;
        report.notes.push(format!(
            "{untokenized} eligible file(s) were not tokenized; similarity coverage is partial."
        ));
    }
    if capped > 0 {
        report.status = SectionStatus::Partial;
        report.notes.push(format!(
            "{capped} language-specific fingerprint bucket(s) reached the {MAX_BUCKET}-file limit; \
             {omitted_memberships} bucket memberships were omitted. Some pairs may be missed."
        ));
    }
    if found > MAX_PAIRS {
        report.notes.push(format!(
            "{found} similar pairs were found among retained candidates; the report keeps the \
             {MAX_PAIRS} most similar."
        ));
    }
    report.similar_files = best
        .into_sorted_vec()
        .into_iter()
        .map(|(Reverse(equal), a, b, _, _)| SimilarFilePair {
            a: a.to_owned(),
            b: b.to_owned(),
            similarity: round4(equal as f64 / PERMUTATIONS as f64),
        })
        .collect();
    cancel.check()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenStream;
    use repodna_core::model::structure::LineCounts;

    fn stream(hashes: Vec<u32>) -> TokenStream {
        let lines = (1..=u32::try_from(hashes.len()).unwrap()).collect();
        TokenStream { hashes, lines }
    }

    fn file<'a>(path: &'a str, language: &'a str, tokens: &'a TokenStream) -> QualityFile<'a> {
        QualityFile {
            path,
            language: Some(language),
            test: false,
            lines: LineCounts::default(),
            analysis: None,
            tokens: Some(tokens),
        }
    }

    #[test]
    fn finds_near_identical_files_of_the_same_language() {
        let base: Vec<u32> = (0..400).map(|i| (i * 7919) % 1000).collect();
        let mut edited = base.clone();
        for position in [100, 200, 300] {
            edited[position] = 5_000;
        }
        let different: Vec<u32> = (0..400).map(|i| (i * 104_729 + 17) % 997 + 2_000).collect();
        let streams = [
            stream(base.clone()),
            stream(edited),
            stream(different),
            stream(base),
            stream((0..10).collect()),
        ];
        let files = [
            file("src/a.rs", "rust", &streams[0]),
            file("src/b.rs", "rust", &streams[1]),
            file("src/c.rs", "rust", &streams[2]),
            file("web/a.ts", "typescript", &streams[3]),
            file("src/tiny.rs", "rust", &streams[4]),
        ];
        let report = detect_similarity(&files, 0.8);
        assert_eq!(report.similar_files.len(), 1, "{:?}", report.similar_files);
        let pair = &report.similar_files[0];
        assert_eq!((pair.a.as_str(), pair.b.as_str()), ("src/a.rs", "src/b.rs"));
        assert!(pair.similarity >= 0.8 && pair.similarity < 1.0);
        assert_eq!(report.threshold, 0.8);
    }

    #[test]
    fn identical_files_have_similarity_one() {
        let hashes: Vec<u32> = (0..100).collect();
        let a = stream(hashes.clone());
        let b = stream(hashes);
        let files = [file("x.py", "python", &a), file("y.py", "python", &b)];
        let report = detect_similarity(&files, 0.8);
        assert_eq!(report.similar_files[0].similarity, 1.0);
        assert!(detect_similarity(&[], 0.8).similar_files.is_empty());
    }

    #[test]
    fn different_languages_cannot_evict_a_valid_same_language_pair() {
        let tokens = stream((0..100).collect());
        let paths: Vec<_> = (0..MAX_BUCKET).map(|i| format!("a/{i:03}.py")).collect();
        let mut files: Vec<_> = paths
            .iter()
            .map(|path| file(path, "python", &tokens))
            .collect();
        files.push(file("z/a.rs", "rust", &tokens));
        files.push(file("z/b.rs", "rust", &tokens));

        // The Rust pair would be evicted in the old global buckets. Use
        // lexicographically early Rust paths so it remains visible in the
        // bounded result even alongside 1,225 qualifying Python pairs.
        files[MAX_BUCKET].path = "0/a.rs";
        files[MAX_BUCKET + 1].path = "0/b.rs";
        let report = detect_similarity(&files, 1.0);
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert_eq!(report.similar_files.len(), MAX_PAIRS);
        assert_eq!(report.similar_files[0].a, "0/a.rs");
        assert_eq!(report.similar_files[0].b, "0/b.rs");
        assert!(report.notes.iter().any(|note| note.starts_with("1226 ")));
    }

    #[test]
    fn capped_buckets_report_partial_coverage_and_unique_pair_count() {
        let tokens = stream((0..100).collect());
        let paths: Vec<_> = (0..MAX_BUCKET + 3)
            .map(|i| format!("src/{i:03}.rs"))
            .collect();
        let files: Vec<_> = paths
            .iter()
            .map(|path| file(path, "rust", &tokens))
            .collect();
        let report = detect_similarity(&files, 1.0);
        assert_eq!(report.status, SectionStatus::Partial);
        assert_eq!(report.similar_files.len(), MAX_PAIRS);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("16 language-specific"))
        );
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("48 bucket memberships"))
        );
        assert!(report.notes.iter().any(|note| note.starts_with("1225 ")));
        assert_eq!(report, detect_similarity(&files, 1.0));
        assert!(
            report
                .similar_files
                .iter()
                .all(|pair| pair.a < paths[MAX_BUCKET] && pair.b < paths[MAX_BUCKET])
        );
    }

    #[test]
    fn uncapped_bucket_pair_deduplication_retains_earliest_actual_band() {
        let mut a = Candidate {
            file_index: 0,
            signature: [0; PERMUTATIONS],
            bands: [0; BANDS],
            retained: [true; BANDS],
        };
        let mut b = Candidate {
            file_index: 1,
            signature: [0; PERMUTATIONS],
            bands: [0; BANDS],
            retained: [true; BANDS],
        };
        assert!(!already_paired(&a, &b, 0));
        assert!(already_paired(&a, &b, 1));
        a.retained[0] = false;
        assert!(!already_paired(&a, &b, 1));
        b.bands[1] = 99;
        assert!(!already_paired(&a, &b, 2));
        assert!(already_paired(&a, &b, 3));
    }

    // A set-based reference independently constructs all retained pairs,
    // then performs the original full sort. It is intentionally not used
    // in the production path, where materializing the set is expensive.
    fn reference_pairs(files: &[QualityFile<'_>], threshold: f64) -> Vec<SimilarFilePair> {
        use std::collections::BTreeSet;
        let token = CancellationToken::new();
        let mut signatures = Vec::new();
        let mut buckets: HashMap<(&str, usize, u64), Vec<usize>> = HashMap::new();
        for (index, file) in files.iter().enumerate() {
            if file.test {
                continue;
            }
            let (Some(language), Some(tokens)) = (file.language, file.tokens) else {
                continue;
            };
            if tokens.len() < MIN_TOKENS {
                continue;
            }
            let sig = signature(&tokens.hashes, &token).unwrap();
            let position = signatures.len();
            signatures.push((index, sig));
            for (band, key) in band_keys(&sig).into_iter().enumerate() {
                buckets
                    .entry((language, band, key))
                    .or_default()
                    .push(position);
            }
        }
        let mut pairs = BTreeSet::new();
        for bucket in buckets.values_mut() {
            bucket.truncate(MAX_BUCKET);
            for (position, &i) in bucket.iter().enumerate() {
                for &j in &bucket[position + 1..] {
                    pairs.insert((i, j));
                }
            }
        }
        let mut result = Vec::new();
        for (i, j) in pairs {
            let (a, b) = (&signatures[i], &signatures[j]);
            let similarity = equal_slots(&a.1, &b.1) as f64 / PERMUTATIONS as f64;
            if similarity >= threshold {
                result.push(SimilarFilePair {
                    a: files[a.0].path.to_owned(),
                    b: files[b.0].path.to_owned(),
                    similarity: round4(similarity),
                });
            }
        }
        result.sort_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then_with(|| a.a.cmp(&b.a))
                .then_with(|| a.b.cmp(&b.b))
        });
        result.truncate(MAX_PAIRS);
        result
    }

    #[test]
    fn bounded_top_pairs_match_set_based_reference_across_mixed_buckets() {
        let paths: Vec<_> = (0..130)
            .map(|i| format!("src/{:03}.code", 129 - i))
            .collect();
        let streams: Vec<_> = (0..paths.len())
            .map(|i| {
                let mut hashes: Vec<u32> = (0..100).collect();
                for j in 0..i % 9 {
                    hashes[10 + j * 7] = (1_000 + i % 6) as u32;
                }
                stream(hashes)
            })
            .collect();
        let files: Vec<_> = paths
            .iter()
            .zip(&streams)
            .enumerate()
            .map(|(i, (path, tokens))| {
                let mut file = file(path, if i % 4 == 0 { "python" } else { "rust" }, tokens);
                file.test = i % 19 == 0;
                file
            })
            .collect();
        for threshold in [0.0, 0.5, 0.8, 1.0] {
            assert_eq!(
                detect_similarity(&files, threshold).similar_files,
                reference_pairs(&files, threshold)
            );
        }
    }

    #[test]
    fn cancelled_similarity_returns_no_report_even_for_empty_input() {
        let token = CancellationToken::new();
        token.cancel();
        let tokens = stream((0..100).collect());
        let files = [file("a.rs", "rust", &tokens)];
        assert_eq!(signature(&tokens.hashes, &token), Err(Cancelled));
        assert_eq!(
            detect_similarity_cancellable(&files, 0.8, &token),
            Err(Cancelled)
        );
        assert_eq!(
            detect_similarity_cancellable(&[], 0.8, &token),
            Err(Cancelled)
        );
    }

    #[test]
    fn untokenized_analyzable_files_make_coverage_partial() {
        use repodna_parser::{LanguageRegistry, analyze_source};
        let spec = LanguageRegistry::builtin().get("rust").unwrap();
        let analysis = analyze_source(spec, "fn f() {}");
        let tokens = stream((0..100).collect());
        let missing = QualityFile {
            analysis: Some(&analysis),
            tokens: None,
            ..file("missing.rs", "rust", &tokens)
        };
        let excluded = QualityFile {
            test: true,
            ..missing
        };
        let result = detect_similarity(&[missing, excluded], 0.8);
        assert_eq!(result.status, SectionStatus::Partial);
        assert!(result.similar_files.is_empty());
        assert!(result.notes[0].starts_with("1 eligible file"));
    }
}
