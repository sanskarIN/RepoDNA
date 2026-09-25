//! Secret detection. Values are never stored: only the rule, the location, and a keyed
//! fingerprint leave the scanner.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::confidence::Confidence;
use repodna_core::hash::{hmac_sha256, sha256, stable_id, to_hex};
use repodna_core::model::security::SecretCandidate;

use repodna_parser::LanguageSpec;

use crate::{is_sample_path, test_lines_of};

/// Lines longer than this are scanned only up to this many bytes (minified files).
const MAX_LINE_BYTES: usize = 8_192;

/// Maximum candidates reported per file.
const MAX_PER_FILE: usize = 50;

/// How strongly a rule's matches have to look random.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Check {
    /// The format itself is distinctive enough.
    Format,
    /// The value must reach this Shannon entropy in bits per character.
    Entropy(f64),
}

/// A rule that recognizes one kind of credential.
#[derive(Debug)]
pub struct SecretRule {
    /// Rule identifier, e.g. `github-token`.
    pub id: &'static str,
    /// What the rule detects.
    pub description: &'static str,
    /// Confidence of a match outside tests and examples.
    pub confidence: Confidence,
    /// Lowercase fragments of which at least one must occur on the line (a fast prefilter).
    keywords: &'static [&'static str],
    /// The pattern; capture group 1 is the secret value.
    pattern: Regex,
    check: Check,
}

fn rule(
    id: &'static str,
    description: &'static str,
    confidence: Confidence,
    keywords: &'static [&'static str],
    pattern: &str,
    check: Check,
) -> SecretRule {
    SecretRule {
        id,
        description,
        confidence,
        keywords,
        pattern: Regex::new(pattern)
            .unwrap_or_else(|error| panic!("invalid secret rule {id}: {error}")),
        check,
    }
}

/// Every secret rule, in reporting order.
pub static SECRET_RULES: LazyLock<Vec<SecretRule>> = LazyLock::new(|| {
    use Check::{Entropy, Format};
    use Confidence::{High, Low, Medium};
    vec![
        rule(
            "private-key",
            "Private key block",
            High,
            &["private key"],
            r"-----BEGIN ((?:RSA |DSA |EC |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?)-----",
            Format,
        ),
        rule(
            "aws-access-key-id",
            "AWS access key ID",
            High,
            &["akia", "asia"],
            r"\b((?:AKIA|ASIA)[A-Z0-9]{16})\b",
            Format,
        ),
        rule(
            "aws-secret-access-key",
            "AWS secret access key",
            Medium,
            &["aws"],
            r#"(?i)aws.{0,20}?(?:secret|private).{0,20}?["'\s:=]+([A-Za-z0-9/+=]{40})(?:[^A-Za-z0-9/+=]|$)"#,
            Entropy(4.0),
        ),
        rule(
            "github-token",
            "GitHub token",
            High,
            &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"],
            r"\b(gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{60,255})\b",
            Format,
        ),
        rule(
            "gitlab-token",
            "GitLab personal access token",
            High,
            &["glpat-"],
            r"\b(glpat-[A-Za-z0-9_\-]{20,})",
            Format,
        ),
        rule(
            "slack-token",
            "Slack token",
            High,
            &["xox"],
            r"\b(xox[abprs]-[A-Za-z0-9-]{10,})",
            Format,
        ),
        rule(
            "slack-webhook",
            "Slack incoming webhook URL",
            High,
            &["hooks.slack.com"],
            r"(https://hooks\.slack\.com/services/T[A-Za-z0-9_]+/B[A-Za-z0-9_]+/[A-Za-z0-9_]{20,})",
            Format,
        ),
        rule(
            "stripe-secret-key",
            "Stripe live secret key",
            High,
            &["_live_"],
            r"\b((?:sk|rk)_live_[A-Za-z0-9]{24,})",
            Format,
        ),
        rule(
            "google-api-key",
            "Google API key",
            Medium,
            &["aiza"],
            r"\b(AIza[A-Za-z0-9_\-]{35})",
            Format,
        ),
        rule(
            "anthropic-api-key",
            "Anthropic API key",
            High,
            &["sk-ant-"],
            r"\b(sk-ant-[a-z]+\d*-[A-Za-z0-9_\-]{40,})",
            Format,
        ),
        rule(
            "openai-api-key",
            "OpenAI API key",
            High,
            &["sk-"],
            r"\b(sk-(?:proj|svcacct|admin)-[A-Za-z0-9_\-]{40,})",
            Format,
        ),
        rule(
            "npm-token",
            "npm access token",
            High,
            &["npm_"],
            r"\b(npm_[A-Za-z0-9]{36})\b",
            Format,
        ),
        rule(
            "pypi-token",
            "PyPI API token",
            High,
            &["pypi-"],
            r"\b(pypi-AgEIcHlwaS5vcmc[A-Za-z0-9_\-]{50,})",
            Format,
        ),
        rule(
            "sendgrid-api-key",
            "SendGrid API key",
            High,
            &["sg."],
            r"\b(SG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43})",
            Format,
        ),
        rule(
            "azure-storage-key",
            "Azure storage account key",
            High,
            &["accountkey="],
            r"AccountKey=([A-Za-z0-9+/]{86}==)",
            Format,
        ),
        rule(
            "telegram-bot-token",
            "Telegram bot token",
            Medium,
            &[":aa"],
            r"\b(\d{8,10}:AA[A-Za-z0-9_\-]{33})\b",
            Format,
        ),
        rule(
            "url-credentials",
            "Password embedded in a URL",
            Medium,
            &["://"],
            r"\b[a-z][a-z0-9+.\-]*://[^\s:/@]+:([^\s/@]{3,})@[A-Za-z0-9.\-]+",
            Entropy(1.5),
        ),
        rule(
            "jwt",
            "JSON Web Token",
            Low,
            &["eyj"],
            r"\b(eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,})",
            Format,
        ),
        rule(
            "generic-secret",
            "Hard-coded secret assigned to a secret-like name",
            Low,
            &[
                "password", "passwd", "secret", "api_key", "apikey", "api-key", "token",
            ],
            r#"(?i)(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token)[a-z0-9_\-]*["']?\s*[:=]\s*["']([^"'\s]{8,})["']"#,
            Entropy(3.0),
        ),
    ]
});

/// Whole values commonly used as stand-ins in documentation and tests.
const PLACEHOLDER_VALUES: &[&str] = &[
    "pass", "password", "passwd", "pwd", "secret", "token", "test", "foo", "bar", "baz",
];

/// Host names reserved for documentation and testing (RFC 2606 and RFC 6761); credentials
/// for them cannot be real.
fn is_reserved_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let host = host.split([':', '/']).next().unwrap_or_default();
    ["example.com", "example.org", "example.net"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        || [".example", ".test", ".invalid"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
        || ["example", "test", "invalid"].contains(&host)
}

/// Values that are obviously placeholders rather than credentials.
fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if PLACEHOLDER_VALUES.contains(&lower.as_str()) {
        return true;
    }
    const FRAGMENTS: &[&str] = &[
        "example",
        "changeme",
        "change_me",
        "placeholder",
        "dummy",
        "sample",
        "your",
        "xxxx",
        "****",
        "redacted",
        "insert",
        "replace",
        "<",
        ">",
        "${",
        "{{",
        "%(",
        "process.env",
        "os.environ",
        "getenv",
        "secrets.",
        "vault:",
        "env(",
    ];
    FRAGMENTS.iter().any(|fragment| lower.contains(fragment))
        || value.bytes().all(|b| b == value.as_bytes()[0])
        || has_sequence(value, 8)
        || is_environment_variable_name(value)
}

/// `true` when `value` contains `length` consecutive characters counting up, as in
/// `ABCDEFGH` or `12345678`: made-up keys, not generated ones.
fn has_sequence(value: &str, length: usize) -> bool {
    let mut run = 1;
    for pair in value.as_bytes().windows(2) {
        if pair[0].is_ascii_alphanumeric() && pair[1] == pair[0].wrapping_add(1) {
            run += 1;
            if run >= length {
                return true;
            }
        } else {
            run = 1;
        }
    }
    false
}

/// `ANTHROPIC_API_KEY`-style names: settings such as `api_key_env = "MY_KEY"` name the
/// variable that holds a secret rather than holding one.
fn is_environment_variable_name(value: &str) -> bool {
    value.contains('_')
        && value.as_bytes()[0].is_ascii_uppercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Shannon entropy of `value` in bits per character.
pub fn shannon_entropy(value: &str) -> f64 {
    if value.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for byte in value.bytes() {
        counts[usize::from(byte)] += 1;
    }
    let length = value.len() as f64;
    counts
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = f64::from(count) / length;
            -p * p.log2()
        })
        .sum()
}

/// Lowers a confidence by one step (for matches in tests and examples).
fn lower(confidence: Confidence) -> Confidence {
    match confidence {
        Confidence::High => Confidence::Medium,
        Confidence::Medium | Confidence::Low => Confidence::Low,
        Confidence::Unavailable => Confidence::Unavailable,
    }
}

/// Scans text for secret candidates.
#[derive(Debug, Clone)]
pub struct SecretScanner {
    key: [u8; 32],
}

impl SecretScanner {
    /// Creates a scanner whose fingerprints are keyed with `key`.
    ///
    /// The key should be derived from the scanned content (for example a digest of every
    /// path and content hash), so fingerprints are stable for the same content but cannot
    /// be checked against guessed values by someone who only has the report.
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Scans the text of one file and fingerprints the matches with this scanner's key.
    /// `spec` is the file's detected language, if any.
    pub fn scan(
        &self,
        path: &str,
        text: &str,
        spec: Option<&LanguageSpec>,
    ) -> Vec<SecretCandidate> {
        find_secrets(path, text, spec)
            .into_iter()
            .map(|pending| pending.finalize(&self.key))
            .collect()
    }
}

/// A secret candidate whose fingerprint is computed once the scan key is known.
///
/// Only a SHA-256 digest of the rule and value is kept until then, never the value itself,
/// so callers can derive the key from the whole scanned content first.
#[derive(Debug, Clone)]
pub struct PendingSecret {
    candidate: SecretCandidate,
    digest: [u8; 32],
}

impl PendingSecret {
    /// The candidate, with an empty fingerprint.
    pub fn candidate(&self) -> &SecretCandidate {
        &self.candidate
    }

    /// Completes the candidate with a fingerprint keyed by `key`.
    pub fn finalize(mut self, key: &[u8; 32]) -> SecretCandidate {
        self.candidate.fingerprint = to_hex(&hmac_sha256(key, &self.digest)[..6]);
        self.candidate
    }
}

fn value_digest(rule: &str, value: &str) -> [u8; 32] {
    let mut message = Vec::with_capacity(rule.len() + value.len() + 1);
    message.extend_from_slice(rule.as_bytes());
    message.push(0);
    message.extend_from_slice(value.as_bytes());
    sha256(&message)
}

/// Finds secret candidates in the text of one file; fingerprints are completed later with
/// [`PendingSecret::finalize`]. `spec` is the file's detected language, if any.
pub fn find_secrets(path: &str, text: &str, spec: Option<&LanguageSpec>) -> Vec<PendingSecret> {
    let sample_path = is_sample_path(path);
    let tests = test_lines_of(text, spec);
    let lines: Vec<&str> = text.lines().collect();
    let mut found: Vec<PendingSecret> = Vec::new();
    for (index, full_line) in lines.iter().enumerate() {
        let line = truncate_bytes(full_line, MAX_LINE_BYTES);
        let lowered = line.to_ascii_lowercase();
        for rule in SECRET_RULES.iter() {
            if !rule
                .keywords
                .iter()
                .any(|keyword| lowered.contains(keyword))
            {
                continue;
            }
            for captures in rule.pattern.captures_iter(line) {
                let Some(value) = captures.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                let value = if rule.id == "private-key" {
                    // The key material, not the header, identifies a key.
                    &private_key_body(&lines, index)
                } else {
                    value
                };
                if rule.id != "private-key" && is_placeholder(value) {
                    continue;
                }
                if rule.id == "url-credentials"
                    && let Some(whole) = captures.get(0)
                    && let Some((_, host)) = whole.as_str().rsplit_once('@')
                    && is_reserved_host(host)
                {
                    continue;
                }
                if let Check::Entropy(minimum) = rule.check
                    && shannon_entropy(value) < minimum
                {
                    continue;
                }
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                let sample = sample_path || tests.get(index).copied().unwrap_or(false);
                found.push(PendingSecret {
                    candidate: SecretCandidate {
                        id: stable_id(&[rule.id, path, &line_number.to_string()]),
                        rule: rule.id.to_owned(),
                        description: rule.description.to_owned(),
                        path: path.to_owned(),
                        line: line_number,
                        confidence: if sample {
                            lower(rule.confidence)
                        } else {
                            rule.confidence
                        },
                        fingerprint: String::new(),
                        in_test_or_example: sample,
                    },
                    digest: value_digest(rule.id, value),
                });
                if found.len() >= MAX_PER_FILE {
                    return found;
                }
            }
        }
    }
    // One location can match several rules (a token that is also a generic secret);
    // keep the most specific, which comes first in rule order.
    found.dedup_by(|b, a| {
        a.candidate.line == b.candidate.line && b.candidate.rule == "generic-secret"
    });
    found
}

/// The body of a private key block starting at `start` (up to 64 lines).
fn private_key_body(lines: &[&str], start: usize) -> String {
    lines
        .iter()
        .skip(start + 1)
        .take(64)
        .take_while(|line| !line.starts_with("-----END"))
        .map(|line| line.trim())
        .collect()
}

fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assembles credential-shaped values at runtime so that no literal in this file looks
    /// like a real credential.
    fn fake(parts: &[&str]) -> String {
        parts.concat()
    }

    fn scan(path: &str, text: &str) -> Vec<SecretCandidate> {
        SecretScanner::new([7; 32]).scan(path, text, None)
    }

    #[test]
    fn rules_compile_and_ids_are_unique() {
        let mut ids: Vec<&str> = SECRET_RULES.iter().map(|rule| rule.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn detects_token_formats_without_storing_values() {
        let github = fake(&["gh", "p_", &"a1B2c3D4e5".repeat(4)]);
        let aws = fake(&["AK", "IA", "Z7Q2X9W4R5T1Y8U3"]);
        let text = format!("token = \"{github}\"\nkey_id: {aws}\nnothing here\n");
        let found = scan("deploy/config.yml", &text);
        let rules: Vec<(&str, u32, Confidence)> = found
            .iter()
            .map(|c| (c.rule.as_str(), c.line, c.confidence))
            .collect();
        assert_eq!(
            rules,
            vec![
                ("github-token", 1, Confidence::High),
                ("aws-access-key-id", 2, Confidence::High),
            ]
        );
        let json = format!("{found:?}");
        assert!(!json.contains(&github) && !json.contains(&aws));
        assert_eq!(found[0].fingerprint.len(), 12);
    }

    #[test]
    fn fingerprints_match_for_equal_values_only() {
        let value = fake(&["xo", "xb-", "7302948561-kq8w3n5z1x"]);
        let other = fake(&["xo", "xb-", "5829301746-zm4p7r2t9w"]);
        let text = format!("a={value}\nb={value}\nc={other}\n");
        let found = scan("src/notify.py", &text);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].fingerprint, found[1].fingerprint);
        assert_ne!(found[0].fingerprint, found[2].fingerprint);
        let rekeyed = SecretScanner::new([8; 32]).scan("src/notify.py", &text, None);
        assert_ne!(found[0].fingerprint, rekeyed[0].fingerprint);
    }

    #[test]
    fn deferred_fingerprints_match_immediate_ones() {
        let value = fake(&["xo", "xb-", "7302948561-kq8w3n5z1x"]);
        let text = format!("a={value}\n");
        let pending = find_secrets("src/a.py", &text, None);
        assert!(pending[0].candidate().fingerprint.is_empty());
        let finalized = pending[0].clone().finalize(&[7; 32]);
        assert_eq!(finalized, scan("src/a.py", &text)[0]);
    }

    #[test]
    fn private_keys_are_fingerprinted_by_their_body() {
        let header = fake(&["-----BEGIN ", "RSA PRIVATE KEY-----"]);
        let footer = fake(&["-----END ", "RSA PRIVATE KEY-----"]);
        let text = format!("{header}\nMIIEowIBAAKCAQEA1\nQ2F0cyBhcmUgZ3JlYXQ\n{footer}\n");
        let found = scan("config/server.key", &text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "private-key");
        assert!(!found[0].in_test_or_example);
        let in_tests = scan("tests/fixtures/server.key", &text);
        assert_eq!(in_tests[0].confidence, Confidence::Medium);
        assert!(in_tests[0].in_test_or_example);
    }

    #[test]
    fn skips_placeholders_references_and_low_entropy_values() {
        let text = [
            "password = \"changeme123\"",
            "api_key = \"${API_KEY}\"",
            "secret: \"aaaaaaaaaaaa\"",
            "password = os.environ[\"PASSWORD\"]",
            "url = \"postgres://user:<password>@db/app\"",
            &fake(&["AK", "IA", "IOSFODNN7EXAMPLE"]),
            &fake(&["AK", "IA", "ABCDEFGHIJKLMNOP"]),
        ]
        .join("\n");
        assert!(scan("src/settings.py", &text).is_empty());
        let real = fake(&["db_password = \"Tr0ub4", "dor&3xK9!q\""]);
        let found = scan("src/settings.py", &real);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "generic-secret");
        assert_eq!(found[0].confidence, Confidence::Low);
        let url = fake(&["postgres://admin:", "S3cr3tP4ss", "@db.internal/app"]);
        assert_eq!(scan("src/db.py", &url)[0].rule, "url-credentials");
    }

    #[test]
    fn skips_documentation_urls_and_placeholder_passwords() {
        let text = [
            "https://user:pass@github.com/acme/widget.git",
            "scheme://user:password@host",
            &fake(&["https://bob:", "pa55word", "@example.com/repo.git"]),
            &fake(&["https://bob:", "pa55word", "@git.example.org/repo.git"]),
            &fake(&["https://bob:", "pa55word", "@ci.test/repo.git"]),
        ]
        .join("\n");
        assert!(
            scan("src/url.rs", &text).is_empty(),
            "{:?}",
            scan("src/url.rs", &text)
        );
        assert!(is_reserved_host("Example.COM:8080/path"));
        assert!(!is_reserved_host("example.com.evil.io"));
        assert!(!is_reserved_host("localhost"));
    }

    #[test]
    fn entropy_and_truncation_helpers() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        assert!((shannon_entropy("abcd") - 2.0).abs() < 1e-9);
        assert_eq!(truncate_bytes("héllo", 2), "h");
        assert_eq!(lower(Confidence::High), Confidence::Medium);
    }

    #[test]
    fn environment_variable_names_are_not_secrets() {
        assert!(is_environment_variable_name("ANTHROPIC_API_KEY"));
        assert!(is_environment_variable_name("MY_PROVIDER_KEY_2"));
        assert!(!is_environment_variable_name("AKIAABCDEFGHIJKLMNOP"));
        assert!(!is_environment_variable_name("sk_live_Abc"));
        let found = scan("config.toml", "api_key_env = \"ANTHROPIC_API_KEY\"\n");
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn rust_test_modules_count_as_test_code() {
        let registry = repodna_parser::LanguageRegistry::builtin();
        let rust = registry.get("rust");
        let key = fake(&["AK", "IA", "Z7Q2X9W4R5T1Y8U3"]);
        let text = format!(
            "const ID: &str = \"{key}\";\n#[cfg(test)]\nmod tests {{\n    const ID: &str = \"{key}\";\n}}\n"
        );
        let found = SecretScanner::new([7; 32]).scan("src/aws.rs", &text, rust);
        let flags: Vec<(u32, bool, Confidence)> = found
            .iter()
            .map(|c| (c.line, c.in_test_or_example, c.confidence))
            .collect();
        assert_eq!(
            flags,
            vec![(1, false, Confidence::High), (4, true, Confidence::Medium)]
        );
    }

    #[test]
    fn counting_sequences_are_placeholders() {
        assert!(has_sequence("ABCDEFGHIJKLMNOP", 8));
        assert!(has_sequence("key-12345678", 8));
        assert!(!has_sequence("Z7Q2X9W4R5T1Y8U3", 8));
        assert!(!has_sequence("abcdefg", 8));
    }
}
