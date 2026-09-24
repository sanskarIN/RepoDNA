//! A small glob matcher for repository paths.
//!
//! Used for suppression rules and classification overrides. Semantics follow `.gitignore`
//! closely enough to be unsurprising:
//!
//! * `*` matches any characters except `/`; `?` matches one character except `/`.
//! * `[abc]`, `[a-z]`, and `[!abc]` match one character from (or not from) a set.
//! * `**` as a whole segment matches zero or more directories.
//! * A pattern without `/` (other than a trailing one) matches at any depth, like `**/pattern`.
//! * A leading `/` anchors the pattern to the repository root.
//! * A trailing `/` restricts the pattern to directories.
//! * A pattern that matches a directory also matches everything inside it.

use std::fmt;

/// Error returned for malformed patterns.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid glob pattern {pattern:?}: {reason}")]
pub struct GlobError {
    pattern: String,
    reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    AnyDirectories,
    Pattern(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(char),
    AnyChar,
    AnySequence,
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

/// A compiled glob pattern.
#[derive(Clone, PartialEq, Eq)]
pub struct Glob {
    source: String,
    segments: Vec<Segment>,
    directory_only: bool,
}

impl fmt::Debug for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Glob({:?})", self.source)
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

impl Glob {
    /// Compiles a pattern.
    pub fn new(pattern: &str) -> Result<Self, GlobError> {
        let error = |reason| GlobError {
            pattern: pattern.to_owned(),
            reason,
        };
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return Err(error("pattern is empty"));
        }
        let mut body = trimmed.replace('\\', "/");
        let directory_only = body.ends_with('/') && body.len() > 1;
        while body.ends_with('/') && body.len() > 1 {
            body.pop();
        }
        let anchored = body.starts_with('/');
        let body = body.trim_start_matches('/');
        if body.is_empty() {
            return Err(error("pattern matches nothing"));
        }
        let mut segments = Vec::new();
        if !anchored && !body.contains('/') {
            segments.push(Segment::AnyDirectories);
        }
        for part in body.split('/') {
            match part {
                "" => {}
                "**" => {
                    if segments.last() != Some(&Segment::AnyDirectories) {
                        segments.push(Segment::AnyDirectories);
                    }
                }
                other => segments.push(Segment::Pattern(parse_segment(other).map_err(error)?)),
            }
        }
        Ok(Self {
            source: pattern.to_owned(),
            segments,
            directory_only,
        })
    }

    /// The pattern as written.
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Returns `true` if `path` (repository-relative, `/`-separated) or one of its parent
    /// directories matches the pattern.
    pub fn matches(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        if parts.is_empty() {
            return false;
        }
        // Parent directories: prefixes of length 1..len-1. The full path counts as a
        // directory match only when the pattern is not directory-only.
        let last = if self.directory_only {
            parts.len() - 1
        } else {
            parts.len()
        };
        (1..=last).any(|length| match_segments(&self.segments, &parts[..length]))
    }
}

/// A set of globs that matches when any member matches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobSet {
    globs: Vec<Glob>,
}

impl GlobSet {
    /// Compiles every pattern; the first invalid pattern produces an error.
    pub fn new<S: AsRef<str>>(patterns: &[S]) -> Result<Self, GlobError> {
        let globs = patterns
            .iter()
            .map(|pattern| Glob::new(pattern.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { globs })
    }

    /// Returns `true` if any glob matches `path`.
    pub fn matches(&self, path: &str) -> bool {
        self.globs.iter().any(|glob| glob.matches(path))
    }

    /// Returns `true` when the set contains no patterns.
    pub fn is_empty(&self) -> bool {
        self.globs.is_empty()
    }
}

/// Matches `text` against a simple wildcard pattern where `*` matches any characters
/// (including `.` and `/`) and `?` matches one character. Used for rule identifiers.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let tokens: Vec<Token> = pattern
        .iter()
        .map(|&c| match c {
            '*' => Token::AnySequence,
            '?' => Token::AnyChar,
            other => Token::Literal(other),
        })
        .collect();
    match_tokens(&tokens, &text, true)
}

fn parse_segment(segment: &str) -> Result<Vec<Token>, &'static str> {
    let chars: Vec<char> = segment.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '*' => {
                if tokens.last() != Some(&Token::AnySequence) {
                    tokens.push(Token::AnySequence);
                }
                index += 1;
            }
            '?' => {
                tokens.push(Token::AnyChar);
                index += 1;
            }
            '[' => {
                let (token, next) = parse_class(&chars, index)?;
                tokens.push(token);
                index = next;
            }
            other => {
                tokens.push(Token::Literal(other));
                index += 1;
            }
        }
    }
    Ok(tokens)
}

fn parse_class(chars: &[char], start: usize) -> Result<(Token, usize), &'static str> {
    let mut index = start + 1;
    let negated = matches!(chars.get(index), Some('!' | '^'));
    if negated {
        index += 1;
    }
    let mut ranges = Vec::new();
    let mut first = true;
    loop {
        let Some(&c) = chars.get(index) else {
            return Err("unterminated character class");
        };
        if c == ']' && !first {
            index += 1;
            break;
        }
        first = false;
        if chars.get(index + 1) == Some(&'-') && chars.get(index + 2).is_some_and(|&e| e != ']') {
            let end = chars[index + 2];
            if end < c {
                return Err("character range is reversed");
            }
            ranges.push((c, end));
            index += 3;
        } else {
            ranges.push((c, c));
            index += 1;
        }
    }
    Ok((Token::Class { negated, ranges }, index))
}

fn match_segments(segments: &[Segment], parts: &[&str]) -> bool {
    match segments.split_first() {
        None => parts.is_empty(),
        Some((Segment::AnyDirectories, rest)) => {
            (0..=parts.len()).any(|skip| match_segments(rest, &parts[skip..]))
        }
        Some((Segment::Pattern(tokens), rest)) => match parts.split_first() {
            Some((part, remaining)) => {
                let chars: Vec<char> = part.chars().collect();
                match_tokens(tokens, &chars, false) && match_segments(rest, remaining)
            }
            None => false,
        },
    }
}

/// Iterative wildcard matching with single-star backtracking (linear in practice).
fn match_tokens(tokens: &[Token], text: &[char], star_crosses_slash: bool) -> bool {
    let (mut t, mut s) = (0usize, 0usize);
    let mut backtrack: Option<(usize, usize)> = None;
    while s < text.len() {
        let advanced = match tokens.get(t) {
            Some(Token::AnySequence) => {
                backtrack = Some((t, s));
                t += 1;
                continue;
            }
            Some(Token::AnyChar) => text[s] != '/' || star_crosses_slash,
            Some(Token::Literal(c)) => *c == text[s],
            Some(Token::Class { negated, ranges }) => {
                let inside = ranges
                    .iter()
                    .any(|&(lo, hi)| lo <= text[s] && text[s] <= hi);
                text[s] != '/' && inside != *negated
            }
            None => false,
        };
        if advanced {
            t += 1;
            s += 1;
        } else if let Some((star_t, star_s)) = backtrack {
            if !star_crosses_slash && text[star_s] == '/' {
                return false;
            }
            backtrack = Some((star_t, star_s + 1));
            t = star_t + 1;
            s = star_s + 1;
        } else {
            return false;
        }
    }
    tokens[t..].iter().all(|token| *token == Token::AnySequence)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, path: &str) -> bool {
        Glob::new(pattern).unwrap().matches(path)
    }

    #[test]
    fn slashless_patterns_match_at_any_depth() {
        assert!(matches("node_modules", "node_modules/react/index.js"));
        assert!(matches("node_modules", "web/node_modules/x.js"));
        assert!(matches("*.pem", "config/keys/server.pem"));
        assert!(!matches("*.pem", "config/keys/server.pem.txt"));
    }

    #[test]
    fn anchored_patterns_match_from_the_root() {
        assert!(matches("/build", "build/out.o"));
        assert!(!matches("/build", "src/build/out.o"));
        assert!(matches("docs/*.md", "docs/guide.md"));
        assert!(!matches("docs/*.md", "docs/api/guide.md"));
    }

    #[test]
    fn double_star_spans_directories() {
        assert!(matches("tests/**", "tests/unit/a.rs"));
        assert!(matches("tests/**/*.rs", "tests/a.rs"));
        assert!(matches("tests/**/*.rs", "tests/deep/nested/a.rs"));
        assert!(matches("**/fixtures/**", "crates/x/fixtures/secret.env"));
        assert!(!matches("tests/**/*.rs", "src/tests.rs"));
    }

    #[test]
    fn directory_patterns_only_match_directories() {
        assert!(matches("vendor/", "vendor/lib/a.c"));
        assert!(!matches("vendor/", "vendor"));
        assert!(matches("vendor", "vendor"));
    }

    #[test]
    fn classes_and_question_marks() {
        assert!(matches("file?.txt", "file1.txt"));
        assert!(!matches("file?.txt", "file10.txt"));
        assert!(matches("[abc].rs", "b.rs"));
        assert!(matches("[a-c].rs", "c.rs"));
        assert!(!matches("[!a-c].rs", "b.rs"));
        assert!(matches("[!a-c].rs", "d.rs"));
    }

    #[test]
    fn rejects_malformed_patterns() {
        assert!(Glob::new("").is_err());
        assert!(Glob::new("/").is_err());
        assert!(Glob::new("[abc").is_err());
        assert!(Glob::new("[z-a]").is_err());
    }

    #[test]
    fn stars_do_not_cross_segments() {
        assert!(
            matches("src/*", "src/a/b.rs"),
            "directory match covers contents"
        );
        assert!(!Glob::new("src/*.rs").unwrap().matches("src/a/b.rs"));
    }

    #[test]
    fn wildcards_for_rule_identifiers() {
        assert!(wildcard_match("security.*", "security.secret-candidate"));
        assert!(wildcard_match("*", "anything.at.all"));
        assert!(wildcard_match("architecture.cycle", "architecture.cycle"));
        assert!(!wildcard_match("architecture.cycle", "architecture.cycles"));
        assert!(wildcard_match("quality.?arge-file", "quality.large-file"));
    }

    #[test]
    fn glob_sets_match_any_member() {
        let set = GlobSet::new(&["*.lock", "dist/"]).unwrap();
        assert!(set.matches("Cargo.lock"));
        assert!(set.matches("dist/app.js"));
        assert!(!set.matches("src/app.js"));
        assert!(GlobSet::default().is_empty());
    }
}
