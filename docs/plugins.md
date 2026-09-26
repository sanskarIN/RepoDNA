# Plugins

A plugin is a directory with a `repodna-plugin.toml` manifest. It can add:

- **Language definitions**: pure data that teaches RepoDNA a new language (file names,
  comments, strings, imports, symbols, and complexity keywords). No code runs.
- **An analyzer**: a program in any language that receives a description of the analysis
  as JSON and answers with findings, metrics, and notes.

Two complete examples live in [`plugins/`](../plugins):
[`zig-language`](../plugins/zig-language) (a language definition) and
[`license-headers`](../plugins/license-headers) (an analyzer written in Python).

## Using plugins

Plugins run only when you enable them by name, in your user configuration or on the
command line. A repository's own `repodna.toml` can never enable a plugin.

```sh
repodna plugins path                 # the user plugin directory
cp -r plugins/zig-language "$(repodna plugins path)/"
repodna plugins list                 # discovered plugins and whether they are enabled
repodna plugins show zig-language    # manifest and permissions
repodna plugins enable zig-language  # adds it to [plugins] enabled in your user configuration
repodna analyze .
```

For a single run, without installing or enabling anything permanently:

```sh
repodna analyze . --plugin-dir plugins --plugin license-headers
```

RepoDNA searches for plugins in the directories given with `--plugin-dir`, then in the
user plugin directory, then in the directories listed in `[plugins] directories`. Each
plugin is a subdirectory containing `repodna-plugin.toml`.

`repodna plugins check DIR` validates a plugin (its manifest and language definitions)
without running it.

## The manifest

```toml
name = "license-headers"          # lowercase letters, digits, and hyphens
version = "1.0.0"
description = "Reports which source files declare an SPDX license identifier near the top."
api = 1                           # the protocol version
command = ["python3", "license_headers.py"]
permissions = ["repository-files"]
languages = []                    # language definition files, relative to the plugin directory
license = "Apache-2.0"
homepage = "https://example.com/plugin"   # optional
```

| Key | Required | Meaning |
|---|---|---|
| `name` | yes | Unique name: lowercase letters, digits, and inner hyphens. |
| `version` | yes | The plugin's version. |
| `api` | yes | Protocol version; RepoDNA 1.0 speaks version `1`. |
| `description` | no | One line. |
| `command` | no | The analyzer command. It runs without a shell, from the plugin directory; a first element containing a path separator is resolved inside the plugin directory. |
| `permissions` | no | `repository-files` gives the analyzer the path of the checkout. |
| `languages` | no | Declarative language definition files. |
| `license`, `homepage` | no | Shown by `repodna plugins show`. |

## Language definitions

A language definition is a TOML file. Every pattern is a regular expression compiled by
the Rust [`regex`](https://docs.rs/regex) crate, which matches in linear time; patterns
that capture a name or path need exactly one capture group. This is the
[Zig definition](../plugins/zig-language/languages/zig.toml):

```toml
id = "zig"
name = "Zig"
kind = "programming"
extensions = ["zig"]
line_comments = ["//"]
strings = [{ open = "\"", escape = "\\" }]
imports = ['@import\("([^"]+)"\)']
functions = ['^\s*(?:pub\s+)?(?:export\s+|extern\s+|inline\s+)?fn\s+([A-Za-z_]\w*)']
types = [
  { pattern = '^\s*(?:pub\s+)?const\s+([A-Z]\w*)\s*=\s*(?:extern\s+|packed\s+)?struct\b', kind = "struct" },
  { pattern = '^\s*(?:pub\s+)?const\s+([A-Z]\w*)\s*=\s*(?:extern\s+)?enum\b', kind = "enum" },
]
body = "braces"
complexity_keywords = ["if", "for", "while", "switch", "catch", "orelse"]
complexity_operators = ["and", "or"]
```

| Key | Meaning |
|---|---|
| `id`, `name` | Stable identifier and display name. A definition with the `id` of a built-in language replaces it. |
| `kind` | `programming`, `markup`, `stylesheet`, `data`, `prose`, or `build`. |
| `extensions`, `filenames`, `shebangs` | How files are recognized: extensions without the dot, exact file names, and interpreter names in `#!` lines. |
| `line_comments` | Tokens that start a comment, such as `//` or `#`. |
| `block_comments`, `nested_block_comments` | Pairs such as `[["/*", "*/"]]`, and whether they nest. |
| `strings` | Tables with `open`, and optionally `close`, `escape`, and `multiline`. |
| `imports` | Patterns whose capture group is an imported path or module. They run on lines with comments removed. |
| `package` | A pattern capturing a package or namespace declaration. |
| `functions` | Patterns whose capture group is a function name. |
| `types` | Tables with a `pattern` and a `kind`: `class`, `struct`, `enum`, `interface`, `trait`, `module`, or `type`. |
| `body` | How bodies are delimited, for function length and nesting: `braces`, `indentation`, `end`, or `none`. |
| `complexity_keywords`, `complexity_operators` | Decision points counted for approximate complexity. |

Symbol and import patterns run on *masked* lines: comments are removed and string contents
are blanked, so text inside strings and comments never creates symbols. Definition files
are limited to 256 KB and each pattern to 1,000 characters.

## Analyzers

### Lifecycle

1. `repodna analyze` finishes its built-in stages.
2. RepoDNA starts the plugin's `command` and writes one JSON **request** to its standard
   input.
3. The plugin writes one JSON **response** to standard output and exits with status 0.
   Anything it writes to standard error is shown only if it fails.
4. RepoDNA validates the response and adds the findings, metrics, and notes to the
   analysis, recording a run entry for the plugin.

A plugin that fails, exceeds its time limit, or writes invalid output is reported as a
warning; the rest of the analysis is unaffected.

### The request

```json
{
  "api": 1,
  "repodnaVersion": "1.0.0",
  "plugin": { "name": "license-headers", "version": "1.0.0" },
  "repository": {
    "name": "my-project",
    "root": "/home/me/src/my-project",
    "revision": "4f2c9e1d...",
    "primaryLanguages": ["rust"]
  },
  "files": [
    {
      "path": "src/main.rs",
      "language": "rust",
      "category": "source",
      "bytes": 2048,
      "codeLines": 71,
      "generated": false,
      "vendored": false,
      "binary": false
    }
  ],
  "filesTruncated": false
}
```

`repository.root` is present only when the plugin has the `repository-files` permission.
Paths are repository-relative. The file list holds at most 50,000 files; `filesTruncated`
says whether it was cut.

### The response

```json
{
  "api": 1,
  "findings": [
    {
      "rule": "missing-header",
      "severity": "info",
      "confidence": "high",
      "title": "3 source files have no SPDX license identifier",
      "summary": "Files without an identifier include src/a.rs, src/b.rs, and src/c.rs.",
      "rationale": "A license identifier in each file makes the license clear when files are copied.",
      "paths": ["src/a.rs", "src/b.rs", "src/c.rs"],
      "evidence": [{ "kind": "file", "path": "src/a.rs", "line": 1 }],
      "nextSteps": ["Add an SPDX-License-Identifier comment."],
      "limitations": ["Only the first 20 lines of each file are read."]
    }
  ],
  "metrics": [
    { "id": "spdx-coverage", "label": "Files with a header", "value": 0.8, "unit": "ratio", "definition": "Share of source files with an identifier." }
  ],
  "notes": ["Checked 120 files."]
}
```

- `severity` is `info` (default), `attention`, `warning`, or `critical`; `confidence` is
  `low`, `medium` (default), or `high`.
- Evidence items have a `kind`: `file` (`path`, `line`, `endLine`, `note`), `directory`
  (`path`, `note`), `symbol` (`path`, `name`, `line`), `commit` (`hash`, `date`, `summary`),
  `dependency-edge` (`from`, `to`, `path`, `line`), `package` (`ecosystem`, `name`,
  `version`, `manifest`), `metric` (`metric`, `value`, `threshold`, `unit`), or
  `observation` (`text`).
- Rule and metric identifiers are namespaced: `missing-header` from the plugin
  `license-headers` becomes `plugin.license-headers.missing-header`.
- Paths must be repository-relative; text is cleaned of control characters and shortened;
  a response may hold at most 1,000 findings and 200 metrics. Items that fail validation
  are dropped and listed in the plugin's run entry.

Suppressions in `repodna.toml` apply to plugin findings like any others:

```toml
[[suppress]]
rule = "plugin.license-headers.*"
path = "examples/**"
reason = "Examples are public domain"
```

## Security

Analyzer plugins run:

- without a shell, from their own directory;
- with a minimal environment (`PATH`, home and temporary directories, and locale, plus
  `REPODNA_PLUGIN_API` and `REPODNA_VERSION`), so API keys and tokens in your environment
  are not visible to them;
- with the time and output limits from `[plugins]` (60 seconds and 8 MB by default);
- with **your user's permissions**. These limits are not an operating-system sandbox. Only
  enable plugins that you have read and trust.

Language definitions contain no code and cannot run anything.

## Writing a plugin

1. Create a directory with a `repodna-plugin.toml`.
2. For a language, add a definition file and list it under `languages`; for an analyzer,
   write a program that reads the request from standard input and prints the response.
3. Run `repodna plugins check path/to/plugin`.
4. Try it: `repodna analyze some/repository --plugin-dir path/to --plugin your-plugin`.
5. Inspect the result with `repodna findings --rule plugin.` or in the web interface.

The [`license-headers`](../plugins/license-headers/license_headers.py) example is a
complete analyzer in about a hundred lines of Python using only the standard library.
