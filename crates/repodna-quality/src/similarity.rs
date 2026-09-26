//! Similar-file detection with MinHash signatures and locality-sensitive hashing.
//!
//! Each file is reduced to the set of its 5-token shingles. A MinHash signature of 64 values
//! estimates the Jaccard similarity of two such sets; banding the signature (16 bands of 4
//! values) finds candidate pairs without comparing every pair of files. Candidates are then
//! scored on the full signature and kept when the estimate reaches the threshold.

use std::collections::{BTreeSet, HashMap};

use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::quality::{SimilarFilePair, SimilarityReport};

use crate::QualityFile;

/// Signature length.
const PERMUTATIONS: usize = 64;

/// Rows per LSH band (`PERMUTATIONS / ROWS` bands).
const ROWS: usize = 4;

/// Tokens per shingle.
const SHINGLE: usize = 5;

/// Files with fewer tokens are too small for a meaningful similarity estimate.
pub const MIN_TOKENS: usize = 50;

/// Maximum pairs kept in the report.
pub const MAX_PAIRS: usize = 100;

/// Maximum files per LSH bucket examined pairwise.
const MAX_BUCKET: usize = 50;

/// How similarity is measured, for reports.
pub const SIMILARITY_METHOD: &str = "Each file is reduced to the set of its 5-token shingles \
(comments, whitespace, imports, and literal values normalized away). A 64-value MinHash \
signature estimates the Jaccard similarity of two files; 16 bands of 4 values select candidate \
pairs, which are kept when the estimate reaches the threshold. Only files of the same language \
with at least 50 tokens are compared, and test files are excluded.";

/// SplitMix64 finalizer: a fast, well-distributed 64-bit mixing function.
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// MinHash signature of the shingles of `hashes`.
fn signature(hashes: &[u32]) -> [u64; PERMUTATIONS] {
    let mut signature = [u64::MAX; PERMUTATIONS];
    for window in hashes.windows(SHINGLE) {
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
    signature
}

fn estimate(a: &[u64; PERMUTATIONS], b: &[u64; PERMUTATIONS]) -> f64 {
    let equal = a.iter().zip(b).filter(|(x, y)| x == y).count();
    equal as f64 / PERMUTATIONS as f64
}

/// Finds pairs of similar files among non-test files with token streams.
pub fn detect_similarity(files: &[QualityFile<'_>], threshold: f64) -> SimilarityReport {
    let mut report = SimilarityReport {
        status: SectionStatus::Analyzed,
        threshold,
        method: SIMILARITY_METHOD.to_owned(),
        ..SimilarityReport::default()
    };
    let candidates: Vec<(usize, [u64; PERMUTATIONS])> = files
        .iter()
        .enumerate()
        .filter(|(_, file)| !file.test && file.language.is_some())
        .filter_map(|(index, file)| {
            let tokens = file.tokens?;
            (tokens.len() >= MIN_TOKENS).then(|| (index, signature(&tokens.hashes)))
        })
        .collect();

    let mut buckets: HashMap<(usize, u64), Vec<usize>> = HashMap::new();
    for (position, (_, signature)) in candidates.iter().enumerate() {
        for (band, rows) in signature.chunks(ROWS).enumerate() {
            let key = rows.iter().fold(0u64, |hash, &value| mix(hash ^ value));
            buckets.entry((band, key)).or_default().push(position);
        }
    }
    let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
    for members in buckets.values_mut().filter(|members| members.len() > 1) {
        members.sort_unstable();
        members.truncate(MAX_BUCKET);
        for i in 0..members.len() {
            for j in i + 1..members.len() {
                pairs.insert((members[i], members[j]));
            }
        }
    }

    let mut similar: Vec<SimilarFilePair> = pairs
        .into_iter()
        .filter_map(|(i, j)| {
            let (a, b) = (&candidates[i], &candidates[j]);
            let (file_a, file_b) = (&files[a.0], &files[b.0]);
            if file_a.language != file_b.language {
                return None;
            }
            let similarity = estimate(&a.1, &b.1);
            (similarity >= threshold).then(|| SimilarFilePair {
                a: file_a.path.to_owned(),
                b: file_b.path.to_owned(),
                similarity: round4(similarity),
            })
        })
        .collect();
    similar.sort_by(|x, y| {
        y.similarity
            .total_cmp(&x.similarity)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    if similar.len() > MAX_PAIRS {
        report.notes.push(format!(
            "{} similar pairs were found; the report keeps the {MAX_PAIRS} most similar.",
            similar.len()
        ));
        similar.truncate(MAX_PAIRS);
    }
    report.similar_files = similar;
    report
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
