# Complexity and code quality

RepoDNA's quality signals are measurements with documented methods, not verdicts. Each one
says what was measured, which threshold was crossed, and what the measurement cannot see.
All thresholds are [configurable](configuration.md#thresholds-when-signals-are-reported).

```sh
repodna hotspots
repodna show --section complexity,duplication
repodna findings --rule quality.
```

![Code quality view of RepoDNA's own analysis](images/quality.png)

## How much RepoDNA understands

RepoDNA does not compile code. Its language analyzers are *lexical*: a scanner that knows
each language's comments and strings feeds data-driven rules for imports, symbols,
function bodies, and decision points. Every language is labeled with the depth of analysis
behind it:

| Capability | Languages | What is measured |
|---|---|---|
| Lexical | 25, including Rust, C, C++, C#, Java, Kotlin, Go, Python, JavaScript, TypeScript, PHP, Ruby, Swift, and Dart | Lines, comments, imports, symbols, functions, complexity, and nesting |
| Line counting | 40, including HTML, CSS, JSON, YAML, TOML, Markdown, Dockerfile, Makefile, Haskell, and Julia | Lines of code, comments, and blanks |

The full list is in the [README](../README.md#supported-languages). Plugins can add
languages; see [plugins](plugins.md#language-definitions).

## Complexity

**Approximate cyclomatic complexity**, counted lexically: 1 plus the decision points
(branches, loops, case labels, boolean operators, and conditional expressions) inside each
function body. Functions are found with language-specific patterns, and each decision point
counts toward the innermost enclosing function. Test files are excluded.

Reports show the distribution of functions across the buckets 1–5, 6–10, 11–20, 21–50,
and 51+, and list the most complex functions. A function above `high_complexity` (15) is
reported as `quality.complex-function`.

## Size and nesting

| Signal | Threshold | Rule |
|---|---|---|
| Large file (code lines) | `large_file_lines` (1000) | `quality.large-file` |
| Long function (lines) | `large_function_lines` (80) | `quality.long-function` |
| Deep nesting (block depth) | `deep_nesting` (5) | `quality.deep-nesting` |

Test files are excluded from these signals.

## Duplication (deep profile)

Duplicated code is found on normalized token streams: literal values and formatting are
normalized away, identifiers are not, so renamed copies are not reported. Each file's token
stream is split into overlapping k-grams whose hashes are *winnowed*; two regions that share
at least `duplicate_min_tokens` (70) tokens always share a fingerprint, so every candidate
is found, and each candidate is then verified token by token and extended to its full
length. Data-like blocks with little variety (long lists of numbers or strings) are not
reported.

Results are clusters of places that share a block, and the share of analyzed code that is
duplicated. Rules: `quality.duplicate-block` and `quality.duplication-ratio`.

## Similar files (deep profile)

Each file is reduced to the set of its 5-token shingles. A 64-value MinHash signature
estimates the Jaccard similarity of two files, and locality-sensitive hashing (16 bands of 4
values) finds candidate pairs without comparing every pair. Pairs at or above `similarity`
(0.8) are reported as `quality.similar-files`: copies that have drifted apart, generated
variants, or templates.

## Markers

Comments that start with `TODO`, `FIXME`, `HACK`, `XXX`, `BUG`, or `DEPRECATED` (optionally
with an owner in parentheses) are counted and listed. Mentions elsewhere in prose do not
count. The `share` and `public` privacy presets remove the marker text. Rule:
`quality.markers`.

## Dead-code candidates

Files and directories that nothing in the repository appears to use:

- **Unreferenced files**: in languages whose imports RepoDNA resolves to files, a source file
  that no import or file reference points to and that is not an entry point, a test, or a
  file that conventions load by name.
- **Rust orphans**: `.rs` files that no `mod` declaration includes, which Cargo does not
  compile unless a `#[path]` attribute or `include!` refers to them.
- **Legacy directories**: directories named `legacy`, `old`, `deprecated`, `obsolete`,
  `unused`, `archive`, `archived`, `attic`, or `backup`, especially when unchanged for a
  year or more.

These are candidates, never verdicts: code can be used through configuration, run-time
loading, reflection, code generation, or from outside the repository. Each candidate
carries its confidence and the reason it was listed. Rule: `quality.dead-code`.

## Hotspots

Hotspots combine change history with complexity and size; see
[Git history](git-analysis.md#hotspots).

## What these signals cannot tell you

- Whether code is correct, tested well, or easy to change in practice.
- Semantics: a complex function may be the clearest way to express a hard problem.
- Anything about code RepoDNA cannot parse lexically, such as macros or code generated at
  build time.

Use the signals to decide where to look, not to score people or projects.
