//! Helpers for the English sentences RepoDNA generates.

/// A number with the matching noun form: `count(1, "file", "files")` is `"1 file"`.
pub fn count(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
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
