//! Helpers for the English sentences RepoDNA generates.

/// A number with the matching noun form: `count(1, "file", "files")` is `"1 file"`.
pub fn count(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// A measurement's unit, such as "files" or "effective languages", in the singular when the
/// value reads as 1 with two decimals: `unit_for(1.0, "effective languages")` is
/// `"effective language"`. Units that are not counts ("ratio", "share of code files") are
/// returned unchanged.
pub fn unit_for(value: f64, unit: &str) -> String {
    #[allow(clippy::float_cmp)]
    let one = (value * 100.0).round() == 100.0;
    if !one || unit.starts_with("share of") {
        return unit.to_owned();
    }
    // A plural noun, optionally with a qualifier before it ("dependent modules") or a
    // participle after it ("checks met").
    let (before, noun, after) = match unit.strip_suffix(" met") {
        Some(noun) => ("", noun, " met"),
        None => match unit.rsplit_once(' ') {
            Some((qualifier, noun)) => (qualifier, noun, ""),
            None => ("", unit, ""),
        },
    };
    let singular = if let Some(stem) = noun.strip_suffix("ies") {
        format!("{stem}y")
    } else if ["ches", "shes", "sses", "xes"]
        .iter()
        .any(|ending| noun.ends_with(ending))
    {
        noun[..noun.len() - 2].to_owned()
    } else if noun.ends_with('s') && !noun.ends_with("ss") {
        noun[..noun.len() - 1].to_owned()
    } else {
        return unit.to_owned();
    };
    let space = if before.is_empty() { "" } else { " " };
    format!("{before}{space}{singular}{after}")
}

/// Joins items as English prose: `a`, `a and b`, or `a, b, and c`.
pub fn join_list<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [only] => only.as_ref().to_owned(),
        [first, second] => format!("{} and {}", first.as_ref(), second.as_ref()),
        [rest @ .., last] => {
            let head: Vec<&str> = rest.iter().map(AsRef::as_ref).collect();
            format!("{}, and {}", head.join(", "), last.as_ref())
        }
    }
}

/// Joins alternatives as English prose: `a`, `a or b`, or `a, b, or c`.
pub fn join_alternatives<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [only] => only.as_ref().to_owned(),
        [first, second] => format!("{} or {}", first.as_ref(), second.as_ref()),
        [rest @ .., last] => {
            let head: Vec<&str> = rest.iter().map(AsRef::as_ref).collect();
            format!("{}, or {}", head.join(", "), last.as_ref())
        }
    }
}

/// Lowercases the first letter of a sentence so it can continue another sentence: "Declared
/// in Cargo.toml" becomes "declared in Cargo.toml". Text that starts with an acronym, a path,
/// or a name in capitals ("README", "UI", "CLI") is left alone.
pub fn continue_sentence(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => first
            .to_lowercase()
            .chain(text[first.len_utf8()..].chars())
            .collect(),
        _ => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_lists_read_naturally() {
        assert_eq!(count(1, "file", "files"), "1 file");
        assert_eq!(count(0, "file", "files"), "0 files");
        assert_eq!(count(2, "contributor", "contributors"), "2 contributors");
        assert_eq!(join_list::<&str>(&[]), "");
        assert_eq!(join_list(&["Rust"]), "Rust");
        assert_eq!(join_list(&["Rust", "Go"]), "Rust and Go");
        assert_eq!(join_list(&["Rust", "Go", "C"]), "Rust, Go, and C");
        assert_eq!(
            join_alternatives(&["npm test", "cargo test"]),
            "npm test or cargo test"
        );
        assert_eq!(join_alternatives(&["a", "b", "c"]), "a, b, or c");
    }

    #[test]
    fn puts_units_in_the_singular_for_one() {
        assert_eq!(unit_for(1.0, "files"), "file");
        assert_eq!(unit_for(2.0, "files"), "files");
        assert_eq!(unit_for(0.0, "files"), "files");
        assert_eq!(unit_for(1.5, "checks met"), "checks met");
        assert_eq!(unit_for(1.001, "effective languages"), "effective language");
        assert_eq!(unit_for(1.0, "checks met"), "check met");
        assert_eq!(unit_for(1.0, "effective languages"), "effective language");
        assert_eq!(unit_for(1.0, "dependencies"), "dependency");
        assert_eq!(unit_for(1.0, "branches"), "branch");
        assert_eq!(unit_for(1.0, "releases"), "release");
        assert_eq!(unit_for(1.0, "ratio"), "ratio");
        assert_eq!(unit_for(1.0, "share of code files"), "share of code files");
    }

    #[test]
    fn continues_sentences_without_touching_names() {
        assert_eq!(
            continue_sentence("Declared in crates/cli/Cargo.toml"),
            "declared in crates/cli/Cargo.toml"
        );
        assert_eq!(
            continue_sentence("README at the root"),
            "README at the root"
        );
        assert_eq!(
            continue_sentence("src/lib.rs is the root"),
            "src/lib.rs is the root"
        );
        assert_eq!(continue_sentence(""), "");
        assert_eq!(continue_sentence("A"), "A");
    }
}
