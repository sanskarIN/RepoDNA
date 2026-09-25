//! Risky constructs in code, configuration, CI workflows, and container definitions.
//!
//! Rules run on comment-stripped lines (when the language is known), so a pattern that only
//! appears in a comment or in documentation does not match. Each match is a prompt for
//! review, not proof of a vulnerability.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::confidence::Confidence;
use repodna_core::hash::stable_id;
use repodna_core::model::security::{PatternCandidate, PatternCategory};
use repodna_core::paths;
use repodna_parser::{LanguageSpec, scan};

use crate::{is_sample_path, test_lines};

/// Maximum pattern candidates reported per file.
const MAX_PER_FILE: usize = 50;

/// Lines longer than this are skipped (minified or generated content).
const MAX_LINE_BYTES: usize = 4_096;

/// Where a rule applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Files of these languages.
    Languages(&'static [&'static str]),
    /// GitHub Actions workflow files.
    Workflows,
    /// Dockerfiles, Compose files, and Kubernetes-style manifests.
    Containers,
}

/// A rule for one risky construct.
#[derive(Debug)]
pub struct PatternRule {
    /// Rule identifier.
    pub id: &'static str,
    /// Category.
    pub category: PatternCategory,
    /// What the rule detects.
    pub description: &'static str,
    /// Suggested review action.
    pub recommendation: &'static str,
    /// Confidence of a match outside tests and examples.
    pub confidence: Confidence,
    scopes: &'static [Scope],
    keywords: &'static [&'static str],
    pattern: Regex,
    /// The rule does not match when this pattern also matches the line.
    unless: Option<Regex>,
    /// The rule matches only in files whose text contains this fragment.
    requires: Option<&'static str>,
}

const SCRIPTING: &[&str] = &[
    "python",
    "javascript",
    "typescript",
    "go",
    "rust",
    "java",
    "kotlin",
    "csharp",
    "php",
    "ruby",
    "shell",
    "powershell",
    "yaml",
    "dockerfile",
    "makefile",
    "scala",
    "swift",
];
const SHELL_LIKE: &[&str] = &["shell", "dockerfile", "makefile", "yaml", "powershell"];
const PYTHON: &[&str] = &["python"];
const WEB: &[&str] = &["javascript", "typescript", "vue", "svelte", "html"];

#[allow(clippy::too_many_arguments)]
fn rule(
    id: &'static str,
    category: PatternCategory,
    description: &'static str,
    recommendation: &'static str,
    confidence: Confidence,
    scopes: &'static [Scope],
    keywords: &'static [&'static str],
    pattern: &str,
) -> PatternRule {
    PatternRule {
        id,
        category,
        description,
        recommendation,
        confidence,
        scopes,
        keywords,
        pattern: Regex::new(pattern)
            .unwrap_or_else(|error| panic!("invalid pattern rule {id}: {error}")),
        unless: None,
        requires: None,
    }
}

impl PatternRule {
    fn unless(mut self, pattern: &str) -> Self {
        self.unless = Some(
            Regex::new(pattern)
                .unwrap_or_else(|error| panic!("invalid exclusion for {}: {error}", self.id)),
        );
        self
    }

    fn requires(mut self, fragment: &'static str) -> Self {
        self.requires = Some(fragment);
        self
    }
}

/// Every line-based pattern rule, in reporting order.
pub static PATTERN_RULES: LazyLock<Vec<PatternRule>> = LazyLock::new(|| {
    use Confidence::{High, Low, Medium};
    use PatternCategory::{Code, Configuration, Container, Workflow};
    vec![
        rule(
            "tls-verification-disabled",
            Code,
            "TLS certificate verification is turned off",
            "Keep certificate verification on; if a private certificate authority is needed, configure it explicitly instead.",
            Medium,
            &[Scope::Languages(SCRIPTING)],
            &["verify", "cert_none", "unauthorized", "insecureskipverify", "danger_accept", "certificatecustomvalidation", "verify_none"],
            r#"verify\s*=\s*False\b|ssl\._create_unverified_context|\bCERT_NONE\b|rejectUnauthorized\s*:\s*false|NODE_TLS_REJECT_UNAUTHORIZED["']?\s*[=:]\s*["']?0|InsecureSkipVerify\s*:\s*true|danger_accept_invalid_(?:certs|hostnames)\s*\(\s*true|ServerCertificateCustomValidationCallback\s*=|CURLOPT_SSL_VERIFYPEER\s*,\s*(?:false|0)\b|VERIFY_NONE\b"#,
        ),
        rule(
            "curl-insecure",
            Code,
            "curl is run with certificate checks disabled",
            "Remove -k/--insecure and fix the certificate chain or trust store instead.",
            Low,
            &[Scope::Languages(SHELL_LIKE), Scope::Workflows, Scope::Containers],
            &["curl"],
            r"\bcurl\b[^|;&\n]*\s(?:--insecure\b|-[A-Za-z]*k[A-Za-z]*\b)",
        ),
        rule(
            "curl-pipe-shell",
            Code,
            "A downloaded script is executed directly",
            "Download the script first, verify its checksum or signature, and review it before running it.",
            Medium,
            &[Scope::Languages(SHELL_LIKE), Scope::Workflows, Scope::Containers],
            &["curl", "wget"],
            r"\b(?:curl|wget)\b[^|\n]*\|\s*(?:sudo\s+)?(?:ba|z|da|k)?sh\b",
        ),
        rule(
            "chmod-777",
            Code,
            "Files are made writable by everyone",
            "Grant only the permissions that are needed, for example 755 for executables or 644 for files.",
            Medium,
            &[Scope::Languages(SCRIPTING), Scope::Workflows, Scope::Containers],
            &["777", "rwx"],
            r"\bchmod\s+(?:-R\s+)?(?:0?777|[augo]*\+rwx)\b|os\.chmod\([^)]*0o?777",
        ),
        rule(
            "yaml-unsafe-load",
            Code,
            "YAML is loaded with a loader that can construct arbitrary objects",
            "Use yaml.safe_load or pass Loader=yaml.SafeLoader.",
            Medium,
            &[Scope::Languages(PYTHON)],
            &["yaml."],
            r"\byaml\.(?:unsafe_load|load|load_all)\s*\(",
        )
        .unless(r"Safe|BaseLoader"),
        rule(
            "pickle-deserialization",
            Code,
            "Data is deserialized with pickle, which can execute code",
            "Only unpickle data from trusted sources; prefer JSON or another data-only format for untrusted input.",
            Low,
            &[Scope::Languages(PYTHON)],
            &["pickle", "dill", "joblib"],
            r"\b(?:c?[Pp]ickle|dill|joblib)\.loads?\s*\(",
        ),
        rule(
            "shell-command-string",
            Code,
            "A command string is run through a shell",
            "Pass arguments as a list without shell=True, and never build the command from untrusted input.",
            Low,
            &[Scope::Languages(PYTHON)],
            &["shell", "os.system", "os.popen"],
            r"subprocess\.\w+\([^)]*shell\s*=\s*True|\bos\.(?:system|popen)\s*\(",
        ),
        rule(
            "eval-usage",
            Code,
            "Code is evaluated from a string",
            "Avoid eval; parse data with a real parser, and never evaluate untrusted input.",
            Low,
            &[Scope::Languages(&["javascript", "typescript", "python", "php", "ruby"])],
            &["eval"],
            r"(?:^|[^.\w])eval\s*\(",
        ),
        rule(
            "dangerous-html",
            Code,
            "An HTML string is inserted into the page",
            "Sanitize the HTML with a vetted library, or render text instead of markup.",
            Low,
            &[Scope::Languages(WEB)],
            &["innerhtml", "v-html", "document.write"],
            r"dangerouslySetInnerHTML|\.(?:inner|outer)HTML\s*=[^=]|\bv-html\b|document\.write\s*\(",
        ),
        rule(
            "debug-enabled",
            Configuration,
            "Debug mode is enabled in application settings",
            "Make debug mode depend on the environment and keep it off in production.",
            Medium,
            &[Scope::Languages(PYTHON)],
            &["debug"],
            r"^\s*DEBUG\s*=\s*True\b|\.run\([^)]*debug\s*=\s*True",
        ),
        rule(
            "cors-wildcard",
            Configuration,
            "Cross-origin requests are allowed from any origin",
            "List the origins that need access instead of allowing all of them, especially with credentials.",
            Low,
            &[Scope::Languages(SCRIPTING)],
            &["origin", "cors"],
            r#"Access-Control-Allow-Origin["']?\s*[:,]\s*["']\*|\borigin\s*:\s*["']\*["']|CORS_ORIGIN_ALLOW_ALL\s*=\s*True|allow_origins\s*=\s*\[\s*["']\*["']"#,
        ),
        rule(
            "workflow-pull-request-target",
            Workflow,
            "The workflow runs on pull_request_target with repository secrets and write access",
            "Make sure the workflow never checks out or runs code from the pull request; use pull_request for untrusted code.",
            Low,
            &[Scope::Workflows],
            &["pull_request_target"],
            r"\bpull_request_target\b",
        ),
        rule(
            "workflow-untrusted-checkout",
            Workflow,
            "A pull_request_target workflow checks out the pull request's code",
            "Do not check out or execute pull request code in pull_request_target workflows; split the work into an unprivileged pull_request workflow.",
            High,
            &[Scope::Workflows],
            &["pull_request", "refs/pull"],
            r"github\.event\.pull_request\.head\.(?:sha|ref)|refs/pull/",
        )
        .requires("pull_request_target"),
        rule(
            "workflow-write-all",
            Workflow,
            "The workflow token has write access to everything",
            "Declare only the permissions the jobs need, for example contents: read.",
            Low,
            &[Scope::Workflows],
            &["write-all"],
            r"^\s*permissions:\s*write-all\b",
        ),
        rule(
            "privileged-container",
            Container,
            "A container runs in privileged mode",
            "Drop privileged mode and grant only the specific capabilities the container needs.",
            Medium,
            &[Scope::Containers, Scope::Languages(&["shell"])],
            &["privileged"],
            r"^\s*privileged:\s*true\b|--privileged\b",
        ),
        rule(
            "docker-socket-mount",
            Container,
            "The Docker socket is mounted into a container",
            "Access to the Docker socket is equivalent to root on the host; avoid it or use a restricted proxy.",
            Medium,
            &[Scope::Containers, Scope::Workflows],
            &["docker.sock"],
            r"/var/run/docker\.sock",
        ),
    ]
});

/// The workflow expressions that carry text controlled by whoever opens an issue or pull
/// request.
static UNTRUSTED_EXPRESSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\$\{\{\s*github\.(?:event\.(?:issue|pull_request|comment|review|review_comment|discussion|head_commit|workflow_run)\.[A-Za-z_.]*(?:title|body|message|name|label|ref|email|head_branch)|head_ref)\b",
    )
    .unwrap_or_else(|error| panic!("invalid workflow expression pattern: {error}"))
});

static RUN_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*)(?:-\s+)?run:\s*(.*)$")
        .unwrap_or_else(|error| panic!("invalid run key pattern: {error}"))
});

fn is_workflow(path: &str) -> bool {
    (path.starts_with(".github/workflows/") || path.contains("/.github/workflows/"))
        && matches!(paths::extension(path).as_deref(), Some("yml" | "yaml"))
}

fn is_container_file(path: &str, language: &str) -> bool {
    let name = paths::file_name(path).to_ascii_lowercase();
    if language == "dockerfile" || name.starts_with("dockerfile") || name.ends_with(".dockerfile") {
        return true;
    }
    if language != "yaml" {
        return false;
    }
    name.starts_with("docker-compose")
        || name.starts_with("compose.")
        || path.split('/').any(|part| {
            matches!(
                part,
                "k8s" | "kubernetes" | "helm" | "charts" | "manifests" | "deploy" | "deployment"
            )
        })
}

/// A `.env` file with values, other than an example or template.
fn is_env_file(path: &str) -> bool {
    let name = paths::file_name(path).to_ascii_lowercase();
    (name == ".env" || name.starts_with(".env."))
        && ![
            ".example",
            ".sample",
            ".template",
            ".dist",
            ".defaults",
            ".schema",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

/// `true` for a line that contains regular expression syntax such as `\s*` or `(?:` at
/// least twice, which ordinary code that uses a construct does not.
fn defines_pattern(line: &str) -> bool {
    const SYNTAX: &[&str] = &[r"\s*", r"\s+", r"\b", "(?:", r"\(", r"\d", r"\w", r"\."];
    SYNTAX
        .iter()
        .filter(|syntax| line.contains(*syntax))
        .count()
        >= 2
}

fn lower(confidence: Confidence) -> Confidence {
    match confidence {
        Confidence::High => Confidence::Medium,
        _ if confidence == Confidence::Unavailable => Confidence::Unavailable,
        _ => Confidence::Low,
    }
}

/// Scans one file for risky patterns. `spec` is the detected language, if any.
pub fn scan_patterns(path: &str, text: &str, spec: Option<&LanguageSpec>) -> Vec<PatternCandidate> {
    let language = spec.map_or("", |spec| spec.id.as_str());
    let workflow = is_workflow(path);
    let container = is_container_file(path, language);
    let sample_path = is_sample_path(path);
    let (code_lines, tests): (Vec<String>, Vec<bool>) = match spec {
        Some(spec) => {
            let lines = scan(text, &spec.syntax).lines;
            let tests = test_lines(&lines, Some(spec));
            (lines.into_iter().map(|line| line.code).collect(), tests)
        }
        None => (text.lines().map(str::to_owned).collect(), Vec::new()),
    };
    let mut candidates = Vec::new();
    let mut push = |rule: &'static str,
                    category: PatternCategory,
                    description: &str,
                    recommendation: &str,
                    confidence: Confidence,
                    line: Option<u32>| {
        let in_tests = line
            .and_then(|line| usize::try_from(line).ok())
            .and_then(|line| tests.get(line.wrapping_sub(1)))
            .copied()
            .unwrap_or(false);
        let sample = sample_path || in_tests;
        if candidates.len() < MAX_PER_FILE {
            let line_text = line.map_or_else(String::new, |line| line.to_string());
            candidates.push(PatternCandidate {
                id: stable_id(&[rule, path, &line_text]),
                rule: rule.to_owned(),
                category,
                description: description.to_owned(),
                recommendation: recommendation.to_owned(),
                path: path.to_owned(),
                line,
                confidence: if sample {
                    lower(confidence)
                } else {
                    confidence
                },
            });
        }
    };

    let applicable: Vec<&PatternRule> = PATTERN_RULES
        .iter()
        .filter(|rule| {
            rule.scopes.iter().any(|scope| match scope {
                Scope::Languages(languages) => languages.contains(&language),
                Scope::Workflows => workflow,
                Scope::Containers => container,
            })
        })
        .filter(|rule| rule.requires.is_none_or(|fragment| text.contains(fragment)))
        .collect();

    let mut run_block: Option<usize> = None;
    for (index, line) in code_lines.iter().enumerate() {
        if line.len() > MAX_LINE_BYTES || line.trim().is_empty() {
            continue;
        }
        let number = u32::try_from(index + 1).ok();
        let lowered = line.to_ascii_lowercase();
        // Rule tables, linters, and tests spell risky constructs out as regular
        // expressions; such a line describes the construct rather than using it.
        let describes_pattern = defines_pattern(line);
        for rule in &applicable {
            if describes_pattern {
                break;
            }
            if !rule
                .keywords
                .iter()
                .any(|keyword| lowered.contains(keyword))
            {
                continue;
            }
            if rule.pattern.is_match(line)
                && rule
                    .unless
                    .as_ref()
                    .is_none_or(|unless| !unless.is_match(line))
            {
                push(
                    rule.id,
                    rule.category,
                    rule.description,
                    rule.recommendation,
                    rule.confidence,
                    number,
                );
            }
        }
        if workflow {
            // Track `run:` blocks so that expressions are flagged only inside shell code.
            let indent = line.len() - line.trim_start().len();
            let mut in_run = run_block.is_some_and(|run_indent| indent > run_indent);
            if !in_run {
                run_block = None;
            }
            if let Some(captures) = RUN_KEY.captures(line) {
                let run_indent = captures.get(1).map_or(0, |m| m.as_str().len());
                let rest = captures.get(2).map_or("", |m| m.as_str()).trim();
                if rest.starts_with('|') || rest.starts_with('>') || rest.is_empty() {
                    run_block = Some(run_indent);
                } else {
                    in_run = true;
                }
            }
            if in_run && UNTRUSTED_EXPRESSION.is_match(line) {
                push(
                    "workflow-script-injection",
                    PatternCategory::Workflow,
                    "Text controlled by issue or pull request authors is interpolated into a shell command",
                    "Pass the value through an environment variable (env: TITLE: ${{ ... }}) and quote \"$TITLE\" in the script.",
                    Confidence::Medium,
                    number,
                );
            }
        }
    }
    if is_env_file(path)
        && text.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#')
                && line
                    .split_once('=')
                    .is_some_and(|(key, value)| !key.trim().is_empty() && !value.trim().is_empty())
        })
    {
        push(
            "env-file-committed",
            PatternCategory::Configuration,
            "An environment file with values is committed",
            "Keep real environment files out of version control (add them to .gitignore) and commit an example file without values instead.",
            Confidence::Medium,
            None,
        );
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::LanguageRegistry;

    fn rules(path: &str, text: &str) -> Vec<(String, Option<u32>)> {
        let spec = LanguageRegistry::builtin().detect_path(path);
        scan_patterns(path, text, spec)
            .into_iter()
            .map(|candidate| (candidate.rule, candidate.line))
            .collect()
    }

    fn found(path: &str, text: &str) -> Vec<String> {
        rules(path, text)
            .into_iter()
            .map(|(rule, _)| rule)
            .collect()
    }

    #[test]
    fn rules_compile_and_ids_are_unique() {
        let mut ids: Vec<&str> = PATTERN_RULES.iter().map(|rule| rule.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn detects_code_patterns_but_not_in_comments() {
        let python = "import yaml, pickle, requests\n# requests.get(url, verify=False)\nrequests.get(url, verify=False)\ndata = yaml.load(text)\nsafe = yaml.load(text, Loader=yaml.SafeLoader)\nobj = pickle.loads(blob)\nsubprocess.run(cmd, shell=True)\nDEBUG = True\n";
        assert_eq!(
            rules("app/settings.py", python),
            vec![
                ("tls-verification-disabled".to_owned(), Some(3)),
                ("yaml-unsafe-load".to_owned(), Some(4)),
                ("pickle-deserialization".to_owned(), Some(6)),
                ("shell-command-string".to_owned(), Some(7)),
                ("debug-enabled".to_owned(), Some(8)),
            ]
        );
        let js = "const agent = new https.Agent({ rejectUnauthorized: false });\nel.innerHTML = html;\nif (a.innerHTML == b) {}\nconst x = eval(code);\nconst y = obj.eval(code);\n";
        assert_eq!(
            found("src/net.js", js),
            vec!["tls-verification-disabled", "dangerous-html", "eval-usage"]
        );
    }

    #[test]
    fn detects_shell_container_and_workflow_patterns() {
        let shell = "curl -fsSL https://example.com/install.sh | sudo bash\nchmod -R 777 /srv\ncurl -k https://internal/api\ndocker run --privileged image\n";
        assert_eq!(
            found("scripts/setup.sh", shell),
            vec![
                "curl-pipe-shell",
                "chmod-777",
                "curl-insecure",
                "privileged-container"
            ]
        );
        let compose = "services:\n  app:\n    privileged: true\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock\n";
        assert_eq!(
            found("docker-compose.yml", compose),
            vec!["privileged-container", "docker-socket-mount"]
        );
        let workflow = "on:\n  pull_request_target:\n    types: [opened]\npermissions: write-all\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@v7\n        with:\n          ref: ${{ github.event.pull_request.head.sha }}\n      - run: echo \"${{ github.event.issue.title }}\"\n      - run: |\n          echo start\n          echo \"${{ github.event.pull_request.body }}\"\n      - name: ${{ github.event.pull_request.title }}\n";
        assert_eq!(
            rules(".github/workflows/ci.yml", workflow),
            vec![
                ("workflow-pull-request-target".to_owned(), Some(2)),
                ("workflow-write-all".to_owned(), Some(4)),
                ("workflow-untrusted-checkout".to_owned(), Some(10)),
                ("workflow-script-injection".to_owned(), Some(11)),
                ("workflow-script-injection".to_owned(), Some(14)),
            ]
        );
    }

    #[test]
    fn flags_committed_env_files_but_not_examples() {
        assert_eq!(
            found(".env", "API_URL=https://api\nEMPTY=\n"),
            vec!["env-file-committed"]
        );
        assert!(found(".env.example", "API_URL=https://api\n").is_empty());
        assert!(found(".env", "# only comments\nEMPTY=\n").is_empty());
        let sample = scan_patterns(
            "tests/fixtures/app.py",
            "DEBUG = True\n",
            LanguageRegistry::builtin().get("python"),
        );
        assert_eq!(sample[0].confidence, Confidence::Low);
    }

    #[test]
    fn skips_regex_definitions_and_lowers_rust_test_code() {
        let text = "const RULE: &str = r\"verify\\s*=\\s*False\\b|\\bCERT_NONE\\b\";\nfn run() { client.danger_accept_invalid_certs(true); }\n#[cfg(test)]\nmod tests {\n    fn t() { client.danger_accept_invalid_certs(true); }\n}\n";
        let spec = LanguageRegistry::builtin().detect_path("src/client.rs");
        let found: Vec<(Option<u32>, Confidence)> = scan_patterns("src/client.rs", text, spec)
            .into_iter()
            .map(|candidate| (candidate.line, candidate.confidence))
            .collect();
        assert_eq!(
            found,
            vec![(Some(2), Confidence::Medium), (Some(5), Confidence::Low)]
        );
    }
}
