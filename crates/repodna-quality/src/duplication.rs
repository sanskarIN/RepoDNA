//! Duplicate code detection on normalized token streams.
//!
//! Each file's token stream is split into overlapping k-grams whose rolling hashes are
//! winnowed (the minimum hash of every window of `w` consecutive k-grams is kept). Two
//! regions that share at least `min_tokens` tokens always share a winnowed fingerprint, so
//! every candidate pair is found; each candidate is then verified token by token and
//! extended to its maximal length. Literal values and formatting are normalized away by the
//! tokenizer, identifiers are not, so renamed copies are not reported.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use repodna_core::hash::stable_id;
use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::quality::{CodeLocation, DuplicateCluster, DuplicationReport};

use crate::{QualityFile, TokenStream};

/// Maximum clusters kept in the report.
pub const MAX_CLUSTERS: usize = 200;

/// Maximum occurrences listed per cluster (the total is always recorded).
pub const MAX_OCCURRENCES: usize = 50;

/// Fingerprints shared by more places than this are compared against their first
/// occurrence only (linear instead of quadratic work for boilerplate repeated everywhere).
const MAX_PAIRWISE: usize = 64;

/// Base of the polynomial rolling hash (odd, so multiplication is invertible mod 2^64).
const BASE: u64 = 0x0000_0100_0000_01b3;

/// How duplication is detected, for reports.
pub const DUPLICATION_METHOD: &str = "Token streams with comments, whitespace, import statements, \
and literal values normalized away are compared with winnowed rolling-hash fingerprints; every \
candidate is verified token by token and extended to its maximal length. Fragments repeated in \
more than 64 places are compared with their first occurrence. Identifiers must match, so renamed \
copies are not reported. Test files are excluded, because repeated setup code in \
tests is usually intentional.";

/// A verified duplicate: a token range in one stream that equals a range in another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Match {
    a: u32,
    a_start: u32,
    b: u32,
    b_start: u32,
    len: u32,
}

/// Rolling hashes of every `k`-gram in `hashes`.
fn kgram_hashes(hashes: &[u32], k: usize) -> Vec<u64> {
    if hashes.len() < k || k == 0 {
        return Vec::new();
    }
    let high = (1..k).fold(1u64, |power, _| power.wrapping_mul(BASE));
    let mut hash = hashes[..k].iter().fold(0u64, |hash, &token| {
        hash.wrapping_mul(BASE).wrapping_add(u64::from(token))
    });
    let mut result = Vec::with_capacity(hashes.len() - k + 1);
    result.push(hash);
    for index in k..hashes.len() {
        hash = hash
            .wrapping_sub(u64::from(hashes[index - k]).wrapping_mul(high))
            .wrapping_mul(BASE)
            .wrapping_add(u64::from(hashes[index]));
        result.push(hash);
    }
    result
}

/// Winnowing: the rightmost minimum of every window of `window` consecutive hashes.
fn winnow(kgrams: &[u64], window: usize) -> Vec<(u64, usize)> {
    let mut selected: Vec<(u64, usize)> = Vec::new();
    if kgrams.is_empty() {
        return selected;
    }
    let window = window.min(kgrams.len()).max(1);
    for start in 0..=kgrams.len() - window {
        let mut best = start;
        for position in start..start + window {
            if kgrams[position] <= kgrams[best] {
                best = position;
            }
        }
        if selected.last().is_none_or(|&(_, last)| last != best) {
            selected.push((kgrams[best], best));
        }
    }
    selected
}

/// Verifies a candidate pair and extends it to its maximal length.
fn extend(streams: &[&TokenStream], a: (u32, u32), b: (u32, u32), k: usize) -> Option<Match> {
    let (sa, sb) = (&streams[a.0 as usize].hashes, &streams[b.0 as usize].hashes);
    let (mut pa, mut pb) = (a.1 as usize, b.1 as usize);
    if sa[pa..pa + k] != sb[pb..pb + k] {
        return None;
    }
    while pa > 0 && pb > 0 && sa[pa - 1] == sb[pb - 1] {
        pa -= 1;
        pb -= 1;
    }
    let mut len = k + (a.1 as usize - pa);
    while pa + len < sa.len() && pb + len < sb.len() && sa[pa + len] == sb[pb + len] {
        len += 1;
    }
    if a.0 == b.0 {
        // Within one file, only the non-overlapping part of a self-similar region counts.
        len = len.min(pa.abs_diff(pb));
    }
    let (first, second) = if (a.0, pa) <= (b.0, pb) {
        ((a.0, pa), (b.0, pb))
    } else {
        ((b.0, pb), (a.0, pa))
    };
    Some(Match {
        a: first.0,
        a_start: u32::try_from(first.1).ok()?,
        b: second.0,
        b_start: u32::try_from(second.1).ok()?,
        len: u32::try_from(len).ok()?,
    })
}

fn sequence_key(hashes: &[u32]) -> u64 {
    hashes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, &token| {
            (hash ^ u64::from(token)).wrapping_mul(BASE)
        })
}

/// Sums the distinct lines covered by `ranges` (inclusive line intervals).
fn covered_lines(mut ranges: Vec<(u32, u32)>) -> u64 {
    ranges.sort_unstable();
    let mut total = 0u64;
    let mut current: Option<(u32, u32)> = None;
    for (start, end) in ranges {
        match current {
            Some((cs, ce)) if start <= ce.saturating_add(1) => current = Some((cs, ce.max(end))),
            Some((cs, ce)) => {
                total += u64::from(ce - cs + 1);
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((cs, ce)) = current {
        total += u64::from(ce - cs + 1);
    }
    total
}

/// Detects duplicated code among non-test files with token streams.
pub fn detect_duplication(files: &[QualityFile<'_>], min_tokens: u32) -> DuplicationReport {
    let min_tokens = min_tokens.max(10);
    let threshold = min_tokens as usize;
    let window = (threshold / 4).clamp(1, 16);
    let k = threshold - window + 1;

    let mut report = DuplicationReport {
        status: SectionStatus::Analyzed,
        min_tokens,
        method: DUPLICATION_METHOD.to_owned(),
        ..DuplicationReport::default()
    };
    let mut owners: Vec<usize> = Vec::new();
    let mut streams: Vec<&TokenStream> = Vec::new();
    let mut untokenized = 0usize;
    for (position, file) in files.iter().enumerate().filter(|(_, file)| !file.test) {
        match file.tokens {
            Some(tokens) => {
                report.analyzed_lines += file.lines.code;
                if tokens.len() >= threshold {
                    owners.push(position);
                    streams.push(tokens);
                }
            }
            None if file.analysis.is_some() => untokenized += 1,
            None => {}
        }
    }
    if untokenized > 0 {
        report.status = SectionStatus::Partial;
    }

    // Fingerprint index.
    let mut index: HashMap<u64, Vec<(u32, u32)>> = HashMap::new();
    for (stream, tokens) in streams.iter().enumerate() {
        let kgrams = kgram_hashes(&tokens.hashes, k);
        for (hash, position) in winnow(&kgrams, window) {
            if let (Ok(stream), Ok(position)) = (u32::try_from(stream), u32::try_from(position)) {
                index.entry(hash).or_default().push((stream, position));
            }
        }
    }

    // Verified maximal matches.
    let mut matches: HashSet<Match> = HashSet::new();
    let mut try_pair = |a: (u32, u32), b: (u32, u32)| {
        if a.0 == b.0 && (a.1.abs_diff(b.1) as usize) < k {
            return;
        }
        if let Some(found) = extend(&streams, a, b, k)
            && found.len >= min_tokens
        {
            matches.insert(found);
        }
    };
    for entries in index.values_mut().filter(|entries| entries.len() > 1) {
        entries.sort_unstable();
        if entries.len() > MAX_PAIRWISE {
            for &other in &entries[1..] {
                try_pair(entries[0], other);
            }
            continue;
        }
        for i in 0..entries.len() {
            for j in i + 1..entries.len() {
                try_pair(entries[i], entries[j]);
            }
        }
    }

    // Group identical sequences into clusters.
    let mut groups: HashMap<(u64, u32), BTreeSet<(u32, u32)>> = HashMap::new();
    for found in &matches {
        let start = found.a_start as usize;
        let hashes = &streams[found.a as usize].hashes[start..start + found.len as usize];
        let group = groups.entry((sequence_key(hashes), found.len)).or_default();
        group.insert((found.a, found.a_start));
        group.insert((found.b, found.b_start));
    }
    let mut candidates: Vec<(u32, Vec<(u32, u32)>)> = groups
        .into_iter()
        .map(|((_, len), locations)| (len, locations.into_iter().collect()))
        .collect();
    candidates.sort_by(|a, b| {
        let weight = |c: &(u32, Vec<(u32, u32)>)| u64::from(c.0) * (c.1.len() as u64 - 1);
        weight(b)
            .cmp(&weight(a))
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| b.0.cmp(&a.0))
    });

    // Drop clusters whose every occurrence lies inside an occurrence already kept.
    let mut covered: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    let mut clusters = Vec::new();
    let mut line_ranges: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for (len, locations) in candidates {
        let inside = |stream: u32, start: u32| {
            covered
                .get(&stream)
                .is_some_and(|ranges| ranges.iter().any(|&(s, e)| start >= s && start + len <= e))
        };
        if locations
            .iter()
            .all(|&(stream, start)| inside(stream, start))
        {
            continue;
        }
        let mut occurrences = Vec::with_capacity(locations.len());
        for &(stream, start) in &locations {
            covered
                .entry(stream)
                .or_default()
                .push((start, start + len));
            let tokens = streams[stream as usize];
            let first = tokens.lines[start as usize];
            let last = tokens.lines[(start + len - 1) as usize];
            line_ranges.entry(stream).or_default().push((first, last));
            occurrences.push(CodeLocation {
                path: files[owners[stream as usize]].path.to_owned(),
                start_line: first,
                end_line: last,
            });
        }
        occurrences.sort_by(|a, b| a.path.cmp(&b.path).then(a.start_line.cmp(&b.start_line)));
        let id_parts: Vec<String> = occurrences
            .iter()
            .map(|o| format!("{}:{}-{}", o.path, o.start_line, o.end_line))
            .collect();
        let language = files[owners[locations[0].0 as usize]]
            .language
            .unwrap_or("unknown")
            .to_owned();
        let occurrence_count = u32::try_from(occurrences.len()).unwrap_or(u32::MAX);
        let lines = occurrences[0].end_line - occurrences[0].start_line + 1;
        occurrences.truncate(MAX_OCCURRENCES);
        clusters.push(DuplicateCluster {
            id: stable_id(&id_parts),
            tokens: len,
            lines,
            language,
            occurrence_count,
            occurrences,
        });
    }
    report.duplicated_lines = line_ranges.into_values().map(covered_lines).sum();
    report.ratio = if report.analyzed_lines == 0 {
        0.0
    } else {
        round4((report.duplicated_lines as f64 / report.analyzed_lines as f64).min(1.0))
    };
    clusters.sort_by_key(|cluster| {
        (
            Reverse(u64::from(cluster.tokens) * u64::from(cluster.occurrence_count - 1)),
            cluster.id.clone(),
        )
    });
    clusters.truncate(MAX_CLUSTERS);
    report.clusters = clusters;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_and_tokenize};

    const BLOCK: &str = "def process(items, limit):
    total = 0
    seen = set()
    for item in items:
        if item.value > limit and item.name not in seen:
            total += item.value * 2
            seen.add(item.name)
        elif item.value < 0:
            total -= item.value
        else:
            total = total + item.weight
    result = [entry for entry in seen if entry.startswith(\"x\")]
    return total, sorted(result), len(seen)
";

    struct Analyzed {
        path: &'static str,
        analysis: FileAnalysis,
        tokens: TokenStream,
        test: bool,
    }

    fn analyze(path: &'static str, text: &str, test: bool) -> Analyzed {
        let spec = LanguageRegistry::builtin().detect_path(path).unwrap();
        let (analysis, tokens) = analyze_and_tokenize(spec, text);
        let tokens = TokenStream::for_file(&tokens, Some(&analysis));
        Analyzed {
            path,
            analysis,
            tokens,
            test,
        }
    }

    fn files(analyzed: &[Analyzed]) -> Vec<QualityFile<'_>> {
        analyzed
            .iter()
            .map(|a| QualityFile {
                path: a.path,
                language: Some(a.analysis.language.as_str()),
                test: a.test,
                lines: a.analysis.lines,
                analysis: Some(&a.analysis),
                tokens: Some(&a.tokens),
            })
            .collect()
    }

    #[test]
    fn finds_copies_that_differ_only_in_literals_and_formatting() {
        let changed_literals = BLOCK.replace("* 2", "* 3").replace("\"x\"", "\"y\"");
        let reformatted = format!(
            "import os\n\n# helper\n{}\n\ndef other():\n    return os.sep\n",
            changed_literals
        );
        let analyzed = [
            analyze(
                "src/a.py",
                &format!("import sys\n{BLOCK}\ndef unique_a():\n    return sys.argv\n"),
                false,
            ),
            analyze("src/b.py", &reformatted, false),
            analyze("src/c.py", "def small():\n    return 1\n", false),
            analyze("tests/test_a.py", BLOCK, true),
        ];
        let files = files(&analyzed);
        let report = detect_duplication(&files, 50);
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert_eq!(report.clusters.len(), 1, "{:?}", report.clusters);
        let cluster = &report.clusters[0];
        assert!(cluster.tokens >= 50);
        assert_eq!(cluster.language, "python");
        let paths: Vec<&str> = cluster
            .occurrences
            .iter()
            .map(|o| o.path.as_str())
            .collect();
        assert_eq!(paths, vec!["src/a.py", "src/b.py"]);
        assert_eq!(cluster.occurrence_count, 2);
        assert_eq!(cluster.occurrences[0].start_line, 2);
        assert!(report.duplicated_lines >= 24);
        assert!(report.ratio > 0.5 && report.ratio <= 1.0);
    }

    #[test]
    fn finds_repeats_within_one_file_without_overlap() {
        let text = format!("{BLOCK}\n{}", BLOCK.replace("process", "process_again"));
        let analyzed = [analyze("src/repeat.py", &text, false)];
        let files = files(&analyzed);
        let report = detect_duplication(&files, 40);
        assert_eq!(report.clusters.len(), 1);
        let occurrences = &report.clusters[0].occurrences;
        assert_eq!(occurrences.len(), 2);
        assert!(occurrences[0].end_line < occurrences[1].start_line);
    }

    #[test]
    fn reports_partial_results_and_no_false_positives() {
        let unique = analyze("src/u.py", BLOCK, false);
        let mut files = files(std::slice::from_ref(&unique));
        let untokenized = QualityFile {
            tokens: None,
            ..files[0]
        };
        files.push(untokenized);
        let report = detect_duplication(&files, 50);
        assert!(report.clusters.is_empty());
        assert_eq!(report.duplicated_lines, 0);
        assert_eq!(report.status, SectionStatus::Partial);
    }

    #[test]
    fn helpers_behave() {
        let hashes: Vec<u32> = (0..20).collect();
        let kgrams = kgram_hashes(&hashes, 5);
        assert_eq!(kgrams.len(), 16);
        assert_eq!(kgrams[3], kgram_hashes(&hashes[3..8], 5)[0]);
        let winnowed = winnow(&[5, 3, 3, 9, 1, 7], 3);
        assert_eq!(winnowed, vec![(3, 2), (1, 4)]);
        assert_eq!(covered_lines(vec![(1, 3), (2, 5), (8, 8)]), 6);
        assert!(kgram_hashes(&hashes[..3], 5).is_empty());
    }
}
