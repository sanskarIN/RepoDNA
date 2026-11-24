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
fn signature(
    hashes: &[u32],
    cancel: &CancellationToken,
) -> Result<[u64; PERMUTATIONS], Cancelled> {
    let mut signature = [u64::MAX; PERMUTATIONS];
    for (index, window) in hashes.windows(SHINGLE).enumerate() {
        if index % CHECK_INTERVAL == 0 {
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
    for (key, rows) in keys.iter_mut().zip(signature.chunks_exact(ROWS)) {
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
                if comparisons % CHECK_INTERVAL == 0 {
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
}
