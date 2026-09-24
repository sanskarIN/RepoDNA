//! Change hotspots: files that combine frequent change with size, complexity, or reach.
//!
//! Each signal is converted to a percentile rank among the candidate files, so signals with
//! different units can be combined; the score is a weighted sum of the percentiles. A file
//! must have changed at least `min_commits` times to be a candidate.

use std::cmp::Ordering;

use repodna_core::evidence::Evidence;
use repodna_core::metric::round4;
use repodna_core::model::git::Hotspot;

/// Weights of the hotspot signals (they sum to 1).
pub const WEIGHTS: Weights = Weights {
    commits: 0.30,
    churn: 0.20,
    recent: 0.15,
    complexity: 0.15,
    size: 0.10,
    authors: 0.05,
    dependents: 0.05,
};

/// Percentile rank at or above which a signal is listed as a reason.
const REASON_PERCENTILE: f64 = 0.75;

/// Weight of each hotspot signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// Number of commits touching the file.
    pub commits: f64,
    /// Lines added plus deleted.
    pub churn: f64,
    /// Commits in the recent window.
    pub recent: f64,
    /// Highest function complexity.
    pub complexity: f64,
    /// Code lines.
    pub size: f64,
    /// Distinct authors.
    pub authors: f64,
    /// Files importing the file.
    pub dependents: f64,
}

/// How hotspots are scored, for reports.
pub const HOTSPOT_METHOD: &str = "Files changed in at least the minimum number of commits are \
ranked by a weighted sum of percentile ranks: commits 30%, churn 20%, recent commits 15%, \
highest function complexity 15%, code lines 10%, distinct authors 5%, and importing files 5%. \
A signal whose value is zero contributes nothing.";

/// Signals of one file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotspotInput<'a> {
    /// Repository-relative path.
    pub path: &'a str,
    /// Commits touching the file.
    pub commits: u32,
    /// Distinct authors.
    pub authors: u32,
    /// Lines added plus deleted.
    pub churn: u64,
    /// Commits in the recent window.
    pub recent_commits: u32,
    /// Code lines.
    pub lines: u64,
    /// Highest function complexity.
    pub complexity: u32,
    /// Files importing this file.
    pub dependents: u32,
}

/// Hotspot ranking settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotspotSettings {
    /// Minimum commits for a file to be a candidate.
    pub min_commits: u32,
    /// Length of the recent window in days (for wording).
    pub recent_days: u32,
    /// Maximum hotspots returned.
    pub limit: usize,
}

/// Percentile ranks of `values`: the share of values below each value, counting ties as
/// half. Zero values rank zero.
fn percentiles(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let count = sorted.len() as f64;
    values
        .iter()
        .map(|&value| {
            if value <= 0.0 {
                return 0.0;
            }
            let below = sorted.partition_point(|&v| v.total_cmp(&value) == Ordering::Less);
            let not_above = sorted.partition_point(|&v| v.total_cmp(&value) != Ordering::Greater);
            (below as f64 + 0.5 * (not_above - below) as f64) / count
        })
        .collect()
}

fn top_percent(percentile: f64) -> String {
    format!("{}", ((1.0 - percentile) * 100.0).ceil().max(1.0))
}

/// Ranks the strongest hotspots among `inputs`.
pub fn rank_hotspots(inputs: &[HotspotInput<'_>], settings: &HotspotSettings) -> Vec<Hotspot> {
    let candidates: Vec<&HotspotInput<'_>> = inputs
        .iter()
        .filter(|input| input.commits >= settings.min_commits && input.lines > 0)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let column = |f: fn(&HotspotInput<'_>) -> f64| -> Vec<f64> {
        percentiles(&candidates.iter().map(|input| f(input)).collect::<Vec<_>>())
    };
    let commits = column(|i| f64::from(i.commits));
    let churn = column(|i| i.churn as f64);
    let recent = column(|i| f64::from(i.recent_commits));
    let complexity = column(|i| f64::from(i.complexity));
    let size = column(|i| i.lines as f64);
    let authors = column(|i| f64::from(i.authors));
    let dependents = column(|i| f64::from(i.dependents));

    let mut scored: Vec<(f64, usize)> = (0..candidates.len())
        .map(|index| {
            let score = WEIGHTS.commits * commits[index]
                + WEIGHTS.churn * churn[index]
                + WEIGHTS.recent * recent[index]
                + WEIGHTS.complexity * complexity[index]
                + WEIGHTS.size * size[index]
                + WEIGHTS.authors * authors[index]
                + WEIGHTS.dependents * dependents[index];
            (round4(score), index)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| candidates[b.1].commits.cmp(&candidates[a.1].commits))
            .then_with(|| candidates[a.1].path.cmp(candidates[b.1].path))
    });
    scored.truncate(settings.limit);

    scored
        .iter()
        .enumerate()
        .map(|(position, &(score, index))| {
            let input = candidates[index];
            let strong = |percentile: f64| percentile >= REASON_PERCENTILE;
            let mut reasons = Vec::new();
            if strong(commits[index]) {
                reasons.push(format!(
                    "Changed in {} commits (top {}% by change frequency)",
                    input.commits,
                    top_percent(commits[index])
                ));
            }
            if strong(churn[index]) {
                reasons.push(format!(
                    "{} lines added or removed (top {}% by churn)",
                    input.churn,
                    top_percent(churn[index])
                ));
            }
            if strong(recent[index]) {
                reasons.push(format!(
                    "{} commits in the last {} days",
                    input.recent_commits, settings.recent_days
                ));
            }
            if strong(complexity[index]) {
                reasons.push(format!(
                    "Contains a function with complexity {}",
                    input.complexity
                ));
            }
            if strong(size[index]) {
                reasons.push(format!("{} code lines", input.lines));
            }
            if strong(authors[index]) {
                reasons.push(format!("Changed by {} authors", input.authors));
            }
            if strong(dependents[index]) {
                reasons.push(format!("{} files import it", input.dependents));
            }
            if reasons.is_empty() {
                reasons.push(format!("Changed in {} commits", input.commits));
            }
            let interpretation = if strong(commits[index]) && strong(complexity[index]) {
                "Frequently changed and complex: changes here are more likely to need careful review and good test coverage."
            } else if strong(commits[index]) && strong(dependents[index]) {
                "Frequently changed and widely used: a change here can affect many other files."
            } else if strong(recent[index]) {
                "Part of the current focus of development: it changed often recently."
            } else if strong(size[index]) {
                "Large and frequently changed: a candidate for a closer look at its responsibilities."
            } else {
                "Changes more often than most files in the repository."
            };
            Hotspot {
                path: input.path.to_owned(),
                rank: u32::try_from(position + 1).unwrap_or(u32::MAX),
                score,
                commits: input.commits,
                authors: input.authors,
                churn: input.churn,
                recent_commits: input.recent_commits,
                lines: input.lines,
                complexity: input.complexity,
                dependents: input.dependents,
                reasons,
                interpretation: interpretation.to_owned(),
                evidence: vec![
                    Evidence::file(input.path),
                    Evidence::metric("git.file.commits", f64::from(input.commits)),
                    Evidence::metric("git.file.churn", input.churn as f64),
                    Evidence::metric("git.file.recent_commits", f64::from(input.recent_commits)),
                    Evidence::metric("quality.file.max_complexity", f64::from(input.complexity)),
                ],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: &str, commits: u32, complexity: u32, lines: u64) -> HotspotInput<'_> {
        HotspotInput {
            path,
            commits,
            authors: 1,
            churn: u64::from(commits) * 10,
            recent_commits: 0,
            lines,
            complexity,
            dependents: 0,
        }
    }

    #[test]
    fn weights_sum_to_one() {
        let w = WEIGHTS;
        let total =
            w.commits + w.churn + w.recent + w.complexity + w.size + w.authors + w.dependents;
        assert!((total - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ranks_frequently_changed_complex_files_first() {
        let inputs = [
            input("src/core.rs", 40, 30, 900),
            input("src/util.rs", 10, 3, 100),
            input("src/rare.rs", 2, 50, 2_000),
            input("src/mid.rs", 12, 8, 300),
            input("src/deleted.rs", 30, 0, 0),
        ];
        let settings = HotspotSettings {
            min_commits: 3,
            recent_days: 90,
            limit: 10,
        };
        let hotspots = rank_hotspots(&inputs, &settings);
        let paths: Vec<&str> = hotspots.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths, vec!["src/core.rs", "src/mid.rs", "src/util.rs"]);
        let top = &hotspots[0];
        assert_eq!(top.rank, 1);
        assert!(top.score > hotspots[1].score);
        assert!(top.reasons[0].starts_with("Changed in 40 commits"));
        assert!(
            top.interpretation
                .starts_with("Frequently changed and complex")
        );
        assert!(rank_hotspots(&[], &settings).is_empty());
    }

    #[test]
    fn percentiles_count_ties_as_half_and_zero_as_zero() {
        assert_eq!(
            percentiles(&[0.0, 1.0, 1.0, 3.0]),
            vec![0.0, 0.5, 0.5, 0.875]
        );
        assert_eq!(top_percent(0.875), "13");
        assert_eq!(top_percent(1.0), "1");
    }
}
