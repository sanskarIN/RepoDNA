//! Redaction of likely secrets in free text.
//!
//! RepoDNA stores short snippets of repository text: TODO comments, commit subjects, and the
//! output of opt-in commands. Before any such text enters an artifact it passes through
//! [`redact_secrets`], which replaces values that look like credentials with
//! [`REDACTED`]. The rules err on the side of redacting: a false positive costs a few
//! characters of context, a false negative could leak a credential into a shared report.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::{Captures, Regex};

/// Replacement text for redacted values.
pub const REDACTED: &str = "[redacted]";

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("invalid redaction pattern: {error}"))
}

/// Private key blocks, including a missing END line (truncated text).
static PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----(?:[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----|[\s\S]*)",
    )
});

/// Tokens with well-known prefixes.
static KNOWN_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    re(concat!(
        r"\b(?:",
        r"gh[pousr]_[A-Za-z0-9]{20,}",
        r"|github_pat_[A-Za-z0-9_]{20,}",
        r"|glpat-[A-Za-z0-9_\-]{20,}",
        r"|xox[abprs]-[A-Za-z0-9\-]{10,}",
        r"|sk-(?:proj-|ant-)?[A-Za-z0-9_\-]{20,}",
        r"|(?:sk|rk)_live_[A-Za-z0-9]{16,}",
        r"|(?:AKIA|ASIA)[A-Z0-9]{16}",
        r"|AIza[A-Za-z0-9_\-]{35}",
        r"|npm_[A-Za-z0-9]{36}",
        r"|pypi-[A-Za-z0-9_\-]{50,}",
        r"|SG\.[A-Za-z0-9_\-]{16,}\.[A-Za-z0-9_\-]{16,}",
        r"|eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}",
        r")"
    ))
});

/// Credentials embedded in URLs: `scheme://user:password@host`.
static URL_CREDENTIALS: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b([a-z][a-z0-9+.\-]*://)[^\s/@:]+(?::[^\s/@]*)?@"));

/// Assignments to secret-looking names: `password = "value"`, `API_KEY: value`.
static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"(?i)\b([a-z0-9_.\-]*(?:password|passwd|pwd|secret|token|api[_\-]?key|access[_\-]?key|private[_\-]?key|auth|credential)s?[a-z0-9_.\-]*)(\s*[:=]\s*)(["']?)([^\s"',;]{4,})"#,
    )
});

/// Long opaque strings that mix letters and digits, such as generic API keys.
static OPAQUE: LazyLock<Regex> = LazyLock::new(|| re(r"[A-Za-z0-9+/_\-]{32,}={0,2}"));

/// Returns `true` for strings that look like random credentials rather than words, paths,
/// or hashes: long, with upper- and lowercase letters and digits.
fn looks_opaque(candidate: &str) -> bool {
    let has_upper = candidate.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = candidate.bytes().any(|b| b.is_ascii_lowercase());
    let has_digit = candidate.bytes().any(|b| b.is_ascii_digit());
    has_upper && has_lower && has_digit
}

/// Applies `regex` to `output`, keeping the borrowed text when nothing actually changed.
fn replace_all(
    output: &mut Cow<'_, str>,
    regex: &Regex,
    replace: impl Fn(&Captures<'_>) -> String,
) {
    let replaced = match regex.replace_all(output.as_ref(), |c: &Captures<'_>| replace(c)) {
        Cow::Owned(replaced) => replaced,
        Cow::Borrowed(_) => return,
    };
    if replaced != output.as_ref() {
        *output = Cow::Owned(replaced);
    }
}

/// Replaces likely secrets in `text` with [`REDACTED`].
///
/// Returns the input unchanged (without allocating) when nothing was redacted.
pub fn redact_secrets(text: &str) -> Cow<'_, str> {
    let mut output = Cow::Borrowed(text);
    replace_all(&mut output, &PRIVATE_KEY, |_| REDACTED.to_owned());
    replace_all(&mut output, &KNOWN_TOKEN, |_| REDACTED.to_owned());
    replace_all(&mut output, &URL_CREDENTIALS, |c| {
        format!("{}{REDACTED}@", &c[1])
    });
    replace_all(&mut output, &ASSIGNMENT, |c| {
        if &c[4] == REDACTED || c[4].starts_with('[') {
            c[0].to_owned()
        } else {
            format!("{}{}{}{REDACTED}", &c[1], &c[2], &c[3])
        }
    });
    replace_all(&mut output, &OPAQUE, |c| {
        if looks_opaque(&c[0]) {
            REDACTED.to_owned()
        } else {
            c[0].to_owned()
        }
    });
    output
}

/// Returns `true` when [`redact_secrets`] would change `text`.
pub fn contains_secret(text: &str) -> bool {
    matches!(redact_secrets(text), Cow::Owned(ref changed) if changed != text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds token-shaped test values at runtime so that no credential-like literal
    /// appears in the source.
    fn fake(prefix: &str, body: &str, repeat: usize) -> String {
        format!("{prefix}{}", body.repeat(repeat))
    }

    #[test]
    fn redacts_known_token_formats() {
        let github = fake(&["gh", "p_"].concat(), "aB3", 12);
        let aws = fake(&["AK", "IA"].concat(), "ABCD2345", 2);
        let slack = fake(&["xo", "xb-"].concat(), "123-abc", 3);
        let text = format!("TODO rotate {github} and {aws}; also {slack}");
        let redacted = redact_secrets(&text);
        assert_eq!(
            redacted,
            "TODO rotate [redacted] and [redacted]; also [redacted]"
        );
        assert!(contains_secret(&text));
    }

    #[test]
    fn redacts_assignments_and_url_credentials() {
        assert_eq!(
            redact_secrets("FIXME: password = \"hunter22\" in staging"),
            "FIXME: password = \"[redacted]\" in staging"
        );
        assert_eq!(
            redact_secrets("DB_PASSWORD=swordfish9"),
            "DB_PASSWORD=[redacted]"
        );
        assert_eq!(
            redact_secrets("clone https://bob:pa55@example.com/repo.git"),
            "clone https://[redacted]@example.com/repo.git"
        );
        assert_eq!(redact_secrets("api_key: [redacted]"), "api_key: [redacted]");
    }

    #[test]
    fn redacts_private_keys_and_opaque_strings() {
        let key = format!(
            "-----BEGIN {}PRIVATE KEY-----\nMIIB\n-----END {}PRIVATE KEY-----",
            "RSA ", "RSA "
        );
        assert_eq!(
            redact_secrets(&format!("key: {key} end")),
            "key: [redacted] end"
        );
        let opaque = fake("Zx9", "Qm4TpL8w", 4);
        assert_eq!(
            redact_secrets(&format!("uses {opaque} here")),
            "uses [redacted] here"
        );
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        for text in [
            "TODO: handle the empty case",
            "see crates/repodna-core/src/model/architecture.rs",
            "revert 3f2c9a1b8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b",
            "tokenize the input before parsing",
            "the author field is optional",
        ] {
            assert!(matches!(redact_secrets(text), Cow::Borrowed(_)), "{text}");
            assert!(!contains_secret(text));
        }
    }
}
