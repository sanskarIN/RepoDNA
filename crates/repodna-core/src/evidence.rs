//! Evidence: the verifiable facts that support a finding, metric, or explanation.
//!
//! Explainability is the core promise of RepoDNA. Every non-trivial conclusion carries a
//! list of evidence items that a person can open and check: a file and line range, a
//! commit, a dependency edge, a metric value, or a plain observation such as "no file
//! named CONTRIBUTING was found". Evidence never contains secret values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// A single verifiable fact supporting an analysis result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum Evidence {
    /// A repository file, optionally narrowed to a line or line range.
    File {
        /// Repository-relative path.
        path: String,
        /// First line of the relevant range (1-based).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        /// Last line of the relevant range (1-based, inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_line: Option<u32>,
        /// Short explanation of why the file is relevant.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// A repository directory.
    Directory {
        /// Repository-relative path of the directory (empty for the root).
        path: String,
        /// Short explanation of why the directory is relevant.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// A named symbol such as a function or class.
    Symbol {
        /// Repository-relative path of the file that defines the symbol.
        path: String,
        /// Symbol name.
        name: String,
        /// Line where the symbol is defined (1-based).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    /// A commit in the repository history.
    Commit {
        /// Full commit hash.
        hash: String,
        /// Commit timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date: Option<Timestamp>,
        /// Commit subject line, when the privacy settings allow it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// A dependency relationship between two files, modules, or packages.
    DependencyEdge {
        /// Source of the edge (the dependent).
        from: String,
        /// Target of the edge (the dependency).
        to: String,
        /// File in which the relationship is declared.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// Line of the declaration (1-based).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    /// An external package declared in a manifest or lockfile.
    Package {
        /// Package ecosystem, e.g. `cargo` or `npm`.
        ecosystem: String,
        /// Package name.
        name: String,
        /// Declared requirement or resolved version.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// Manifest or lockfile that declares the package.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manifest: Option<String>,
    },
    /// A measured value.
    Metric {
        /// Metric identifier, e.g. `git.file.commits`.
        metric: String,
        /// Measured value.
        value: f64,
        /// Threshold the value was compared with.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        threshold: Option<f64>,
        /// Unit of the value, e.g. `lines`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    /// A plain-language observation, used when evidence is the *absence* of something.
    Observation {
        /// The observation.
        text: String,
    },
}

impl Evidence {
    /// Evidence pointing at a whole file.
    pub fn file(path: impl Into<String>) -> Self {
        Self::File {
            path: path.into(),
            line: None,
            end_line: None,
            note: None,
        }
    }

    /// Evidence pointing at one line of a file.
    pub fn line(path: impl Into<String>, line: u32) -> Self {
        Self::File {
            path: path.into(),
            line: Some(line),
            end_line: None,
            note: None,
        }
    }

    /// Evidence pointing at an inclusive line range of a file.
    pub fn lines(path: impl Into<String>, start: u32, end: u32) -> Self {
        Self::File {
            path: path.into(),
            line: Some(start),
            end_line: Some(end.max(start)),
            note: None,
        }
    }

    /// Evidence pointing at a directory.
    pub fn directory(path: impl Into<String>) -> Self {
        Self::Directory {
            path: path.into(),
            note: None,
        }
    }

    /// Evidence pointing at a symbol definition.
    pub fn symbol(path: impl Into<String>, name: impl Into<String>, line: Option<u32>) -> Self {
        Self::Symbol {
            path: path.into(),
            name: name.into(),
            line,
        }
    }

    /// Evidence pointing at a commit.
    pub fn commit(hash: impl Into<String>, date: Option<Timestamp>) -> Self {
        Self::Commit {
            hash: hash.into(),
            date,
            summary: None,
        }
    }

    /// Evidence describing a dependency edge.
    pub fn edge(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::DependencyEdge {
            from: from.into(),
            to: to.into(),
            path: None,
            line: None,
        }
    }

    /// Evidence describing a dependency edge declared at a specific location.
    pub fn edge_at(
        from: impl Into<String>,
        to: impl Into<String>,
        path: impl Into<String>,
        line: Option<u32>,
    ) -> Self {
        Self::DependencyEdge {
            from: from.into(),
            to: to.into(),
            path: Some(path.into()),
            line,
        }
    }

    /// Evidence describing a measured metric value.
    pub fn metric(metric: impl Into<String>, value: f64) -> Self {
        Self::Metric {
            metric: metric.into(),
            value: finite(value),
            threshold: None,
            unit: None,
        }
    }

    /// Evidence describing a metric value compared against a threshold.
    pub fn metric_with_threshold(
        metric: impl Into<String>,
        value: f64,
        threshold: f64,
        unit: impl Into<String>,
    ) -> Self {
        Self::Metric {
            metric: metric.into(),
            value: finite(value),
            threshold: Some(finite(threshold)),
            unit: Some(unit.into()),
        }
    }

    /// A plain-language observation.
    pub fn observation(text: impl Into<String>) -> Self {
        Self::Observation { text: text.into() }
    }

    /// Adds an explanatory note to file or directory evidence; other kinds are unchanged.
    #[must_use]
    pub fn with_note(mut self, text: impl Into<String>) -> Self {
        match &mut self {
            Self::File { note, .. } | Self::Directory { note, .. } => *note = Some(text.into()),
            _ => {}
        }
        self
    }

    /// Returns the repository path this evidence refers to, when it refers to one.
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::File { path, .. } | Self::Directory { path, .. } | Self::Symbol { path, .. } => {
                Some(path)
            }
            Self::DependencyEdge { path, .. } => path.as_deref(),
            Self::Package { manifest, .. } => manifest.as_deref(),
            Self::Commit { .. } | Self::Metric { .. } | Self::Observation { .. } => None,
        }
    }

    /// Returns a compact single-line description suitable for terminals and Markdown.
    pub fn describe(&self) -> String {
        match self {
            Self::File {
                path,
                line,
                end_line,
                note,
            } => {
                let location = match (line, end_line) {
                    (Some(start), Some(end)) if end > start => format!("{path}:{start}-{end}"),
                    (Some(start), _) => format!("{path}:{start}"),
                    _ => path.clone(),
                };
                with_suffix(location, note.as_deref())
            }
            Self::Directory { path, note } => {
                let shown = if path.is_empty() {
                    "(repository root)"
                } else {
                    path
                };
                with_suffix(format!("{shown}/"), note.as_deref())
            }
            Self::Symbol { path, name, line } => match line {
                Some(line) => format!("{name} ({path}:{line})"),
                None => format!("{name} ({path})"),
            },
            Self::Commit {
                hash,
                date,
                summary,
            } => {
                let short: String = hash.chars().take(7).collect();
                let mut text = format!("commit {short}");
                if let Some(date) = date {
                    text.push_str(&format!(" ({})", date.date_string()));
                }
                with_suffix(text, summary.as_deref())
            }
            Self::DependencyEdge {
                from,
                to,
                path,
                line,
            } => {
                let edge = format!("dependency edge: {from} → {to}");
                match (path, line) {
                    (Some(path), Some(line)) => format!("{edge} ({path}:{line})"),
                    (Some(path), None) => format!("{edge} ({path})"),
                    _ => edge,
                }
            }
            Self::Package {
                ecosystem,
                name,
                version,
                manifest,
            } => {
                let mut text = format!("{ecosystem} package {name}");
                if let Some(version) = version {
                    text.push_str(&format!(" {version}"));
                }
                if let Some(manifest) = manifest {
                    text.push_str(&format!(" ({manifest})"));
                }
                text
            }
            Self::Metric {
                metric,
                value,
                threshold,
                unit,
            } => {
                let unit = unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
                match threshold {
                    Some(threshold) => format!(
                        "{metric} = {}{unit} (threshold {}{unit})",
                        format_number(*value),
                        format_number(*threshold)
                    ),
                    None => format!("{metric} = {}{unit}", format_number(*value)),
                }
            }
            Self::Observation { text } => text.clone(),
        }
    }
}

fn with_suffix(text: String, suffix: Option<&str>) -> String {
    match suffix {
        Some(suffix) if !suffix.is_empty() => format!("{text} — {suffix}"),
        _ => text,
    }
}

/// Replaces non-finite floating-point values with zero so they can be serialized as JSON.
pub fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// Formats a number without a trailing `.0` for whole values and with at most two decimals.
pub fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_kind_tag_and_camel_case_fields() {
        let evidence = Evidence::lines("src/lib.rs", 10, 20);
        let json = serde_json::to_value(&evidence).unwrap();
        assert_eq!(json["kind"], "file");
        assert_eq!(json["endLine"], 20);
        assert!(json.get("note").is_none());
        let back: Evidence = serde_json::from_value(json).unwrap();
        assert_eq!(back, evidence);
    }

    #[test]
    fn describes_every_kind() {
        assert_eq!(Evidence::lines("a.rs", 3, 9).describe(), "a.rs:3-9");
        assert_eq!(Evidence::line("a.rs", 3).describe(), "a.rs:3");
        assert_eq!(
            Evidence::directory("").with_note("root").describe(),
            "(repository root)/ — root"
        );
        assert_eq!(
            Evidence::commit("9ae21f0c", Timestamp::from_ymd(2024, 1, 2)).describe(),
            "commit 9ae21f0 (2024-01-02)"
        );
        assert_eq!(
            Evidence::edge("api", "transport").describe(),
            "dependency edge: api → transport"
        );
        assert_eq!(
            Evidence::metric_with_threshold("lines", 1200.0, 1000.0, "lines").describe(),
            "lines = 1200 lines (threshold 1000 lines)"
        );
        assert_eq!(Evidence::metric("ratio", 0.126).describe(), "ratio = 0.13");
        assert_eq!(
            Evidence::observation("No CONTRIBUTING file").describe(),
            "No CONTRIBUTING file"
        );
    }

    #[test]
    fn exposes_paths_and_sanitizes_numbers() {
        assert_eq!(Evidence::file("x.py").path(), Some("x.py"));
        assert_eq!(Evidence::edge("a", "b").path(), None);
        assert_eq!(
            Evidence::edge_at("a", "b", "src/a.ts", Some(4)).path(),
            Some("src/a.ts")
        );
        assert_eq!(finite(f64::NAN), 0.0);
        assert_eq!(finite(f64::INFINITY), 0.0);
        assert_eq!(format_number(2.0), "2");
        assert_eq!(format_number(2.5), "2.5");
    }
}
