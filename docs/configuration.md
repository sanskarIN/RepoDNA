# Configuration

RepoDNA works without any configuration. When you want to tune it, settings come from
several layers, applied in this order (later layers win):

1. Built-in defaults.
2. Your **user configuration**, `config.toml` in the RepoDNA configuration directory
   (`repodna config path` prints where it is).
3. The analyzed repository's **`repodna.toml`**, at its root.
4. A file passed with **`--config FILE`**.
5. **Command-line options**, such as `--profile deep` or `--max-commits 5000`.

`--no-user-config` and `--no-project-config` skip layers 2 and 3. Suppressions and ignore
patterns from every layer are combined; every other setting is replaced by later layers.

## What a repository can and cannot set

A repository's `repodna.toml` is written by whoever controls the repository, which may not
be you. It can therefore only tune *what is analyzed*: profiles, thresholds, ignore
patterns, classification, suppressions, and report defaults. The following are honored
**only** from your user configuration, `--config`, or the command line, and are ignored
(with a warning) when they appear in a repository's `repodna.toml`:

- `[ai]`: AI providers
- `[plugins]`: which plugins run
- `[execution]`: running detected build and test commands
- `privacy.remote_ai` and `privacy.telemetry`

## Getting started

```sh
repodna init                  # writes a commented repodna.toml into the current repository
repodna config init           # creates your user configuration from a commented template
repodna config show           # the effective configuration and where each value came from
repodna config validate       # checks every layer for mistakes
```

Unknown keys are errors, so a typo is reported instead of silently ignored. The JSON
Schema of the file is published at [`schemas/repodna-config.schema.json`](../schemas/repodna-config.schema.json)
(or `repodna schema config`); editors that understand JSON Schema for TOML can use it for
completion and validation.

## Example `repodna.toml`

```toml
[analysis]
profile = "deep"
max_commits = 20000

[ignore]
patterns = ["fixtures/large/", "*.snap"]

[classification]
generated = ["src/generated/**"]
vendor = ["third_party/**"]

[thresholds]
large_file_lines = 1500
high_complexity = 20

[[suppress]]
rule = "security.secret"
path = "tests/fixtures/**"
reason = "Intentional fake credentials used by tests"

[report]
theme = "technical"
```

## Example user configuration

```toml
[analysis]
profile = "standard"

[privacy]
anonymize_contributors = true

[performance]
parallelism = 8

[plugins]
enabled = ["license-headers"]

[ai]
provider = "command"
command = ["ollama", "run", "llama3.2"]
```

## Reference

### `[analysis]`: what to analyze

| Key | Default | Meaning |
|---|---|---|
| `profile` | `"standard"` | `quick`, `standard`, `deep`, `history-only`, `architecture-only`, `dependencies-only`, or `security-only`. See [profiles](analysis-engine.md#profiles). |
| `include_git` | `true` | Analyze Git history. |
| `include_history` | `true` | Reconstruct evolution: epochs, events, and Time Machine snapshots. |
| `include_architecture` | `true` | Infer architecture. |
| `include_dependencies` | `true` | Analyze manifests and lockfiles. |
| `include_quality` | `true` | Compute quality signals. |
| `include_security` | `true` | Scan for security signals. |
| `include_tests` | `true` | Detect tests. |
| `include_build` | `true` | Detect build systems and environment requirements. |
| `include_docs` | `true` | Analyze documentation. |
| `respect_gitignore` | `true` | Respect `.gitignore`, `.ignore`, and Git exclude files. |
| `max_file_bytes` | `2097152` | Files larger than this are classified but not read. |
| `max_commits` | `50000` | Most commits to analyze (`0` = unlimited). |
| `artifact_commits` | `1000` | Most commits stored in the artifact. |
| `max_file_edges` | `25000` | Most file-level dependency edges stored. |
| `max_symbols` | `25000` | Most symbols stored. |
| `snapshots` | `12` | Number of Time Machine snapshots to sample. |

The `include_*` switches turn stages off in any profile; they never turn on a stage that
the profile does not run.

### `[ignore]`: paths to exclude

| Key | Default | Meaning |
|---|---|---|
| `patterns` | `[]` | Additional gitignore-style patterns. |
| `use_default_patterns` | `true` | Also exclude `node_modules/`, `bower_components/`, `.venv/`, `__pycache__/`, `.tox/`, `.mypy_cache/`, `.pytest_cache/`, `.gradle/`, `.next/`, `.nuxt/`, `.turbo/`, and `.parcel-cache/`. |

### `[classification]`: override how files are classified

Each key takes glob patterns.

| Key | Meaning |
|---|---|
| `generated` | Treat matching paths as generated code. |
| `vendor` | Treat matching paths as vendored third-party code. |
| `tests` | Treat matching paths as tests. |
| `docs` | Treat matching paths as documentation. |
| `source` | Treat matching paths as first-party source even if they look generated or vendored. |

RepoDNA also reads `linguist-generated`, `linguist-vendored`, and `linguist-documentation`
attributes from `.gitattributes` files.

### `[thresholds]`: when signals are reported

| Key | Default | Meaning |
|---|---|---|
| `large_file_lines` | `1000` | Code lines above which a file is reported as large. |
| `large_function_lines` | `80` | Lines above which a function is reported as long. |
| `high_complexity` | `15` | Approximate cyclomatic complexity above which a function is reported. |
| `deep_nesting` | `5` | Block nesting depth above which a function is reported. |
| `duplicate_min_tokens` | `70` | Minimum duplicated block size, in normalized tokens. |
| `similarity` | `0.8` | Minimum estimated similarity (0–1) for similar-file pairs. |
| `hotspot_min_commits` | `3` | Minimum commits for a file to be ranked as a hotspot. |
| `high_churn_share` | `0.3` | Share of recent churn (0–1) above which a directory is a high-churn area. |
| `dependency_concentration` | `0.3` | Share of internal module edges (0–1) pointing at one module above which concentration is reported. |
| `dormant_days` | `90` | Minimum gap in days to report a dormant period. |
| `recent_days` | `90` | Size of the "recent" window, in days before the latest commit. |
| `stale_manifest_days` | `730` | Days a manifest must be unchanged before a stale-declarations signal is reported. |
| `large_binary_bytes` | `5242880` | Size above which a committed binary file is reported. |

### `[[suppress]]`: accept known findings

Each `[[suppress]]` table hides matching findings from CI failures and marks them as
suppressed in reports (they stay visible, with the reason).

| Key | Required | Meaning |
|---|---|---|
| `rule` | yes | Rule identifier or wildcard, for example `security.secret` or `quality.*`. |
| `reason` | yes | Why the finding is accepted. |
| `path` | no | Glob; the rule then applies only to findings about matching paths. |
| `id` | no | One exact finding identifier, as shown by `repodna findings`. |

Rule identifiers are listed in [the rules reference](repository-dna.md#rules).

### `[report]`: report defaults

| Key | Default | Meaning |
|---|---|---|
| `theme` | `"professional"` | `professional`, `minimal`, `technical`, or `dark`. |
| `privacy` | `"local"` | Privacy preset applied to exported reports: `local`, `share`, or `public`. |
| `branding` | `true` | Show the "RepoDNA · Made by the Sanskar" credit in generated visuals. |
| `sections` | `[]` | Report sections to include (empty = all); see `repodna show --list`. |

### `[privacy]`

| Key | Default | Meaning |
|---|---|---|
| `telemetry` | `false` | Always `false`: RepoDNA collects no telemetry. The key exists to make that explicit. |
| `remote_ai` | `false` | Allow AI providers that send data to other machines (user configuration only). |
| `anonymize_contributors` | `false` | Replace contributor names with pseudonyms in artifacts. |
| `include_commit_messages` | `true` | Store commit subject lines in artifacts. |
| `redact_paths_in_logs` | `false` | Replace repository paths in log output with hashes. |

### `[performance]`

| Key | Default | Meaning |
|---|---|---|
| `cache` | `true` | Reuse per-file results for unchanged files. Entries are keyed by file content, language definition, and RepoDNA version. |
| `parallelism` | `"auto"` | Worker threads: `"auto"` (one per CPU) or a number. |

### `[plugins]` (user configuration only)

| Key | Default | Meaning |
|---|---|---|
| `enabled` | `[]` | Names of plugins to run. |
| `directories` | `[]` | Additional directories to search (relative paths are relative to the configuration directory). |
| `timeout_seconds` | `60` | Time limit per plugin. |
| `max_output_bytes` | `8388608` | Most bytes a plugin may write to standard output. |

See [plugins](plugins.md).

### `[execution]` (user configuration only)

RepoDNA detects build and test commands but never runs them unless you allow it here.

| Key | Default | Meaning |
|---|---|---|
| `allow_build_commands` | `false` | Run detected build commands. |
| `allow_test_commands` | `false` | Run detected test commands. |
| `timeout_seconds` | `600` | Time limit per command. |
| `max_output_lines` | `200` | Lines of output kept per command. |

See [tests and build](tests-and-build.md#running-commands).

### `[ai]` (user configuration only)

| Key | Default | Meaning |
|---|---|---|
| `provider` | `"none"` | `none`, `command`, `openai-compatible`, or `anthropic`. |
| `command` | `[]` | Command line for the `command` provider. |
| `endpoint` | none | Base URL for HTTP providers (required for `openai-compatible`; `anthropic` defaults to `https://api.anthropic.com`). |
| `model` | none | Model identifier (required for `openai-compatible` and `anthropic`). |
| `api_key_env` | none | Name of the environment variable that holds the API key. Keys are never stored in files. |
| `max_context_tokens` | `6000` | Most estimated tokens of repository context per request. |
| `max_output_tokens` | none | Most tokens the model may generate (default 16000 for `anthropic`, 2000 for the others). |
| `timeout_seconds` | `120` | Request time limit. |
| `input_cost_per_million`, `output_cost_per_million` | none | Prices you set yourself, used only for cost estimates. |
| `include_source_excerpts` | `false` | Allow short source excerpts in prompts. |

See [AI](ai.md).
