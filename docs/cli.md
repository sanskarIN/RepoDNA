# Command-line reference

`repodna` is the command-line interface. Every command prints help with `--help`, and
`repodna --help` lists them all.

```sh
repodna <command> [options] [target]
```

- [Targets](#targets)
- [Options shared by most commands](#options-shared-by-most-commands)
- [Analyze](#analyze): `analyze`, `ci`
- [Explore in the terminal](#explore-in-the-terminal): `show`, `architecture`, `dependencies`,
  `history`, `hotspots`, `timeline`, `findings`
- [Reports and sharing](#reports-and-sharing): `report`, `card`, `badge`, `onboarding`,
  `compare`, `export`, `import`
- [Local storage](#local-storage): `list`, `clean`, `cache`
- [Setup and diagnostics](#setup-and-diagnostics): `init`, `config`, `plugins`, `doctor`,
  `version`, `schema`, `completions`
- [Web interface](#web-interface): `serve`
- [Exit codes](#exit-codes) and [environment variables](#environment-variables)

## Targets

Most commands take one target (default: the current directory). A target can be:

| Target | Example | What happens |
|---|---|---|
| A directory | `.` or `~/src/project` | Analyzed in place; nothing in it is modified. |
| A Git URL | `https://github.com/owner/repo` | Cloned into a temporary directory, analyzed, and deleted. |
| An archive | `project.zip`, `project.tar.gz` | Extracted with safety limits into a temporary directory. |
| An artifact file | `repodna.json`, `export.repodna` | Read as an existing analysis; nothing is analyzed. |
| A stored repository | `polyglot` or its identifier | The latest stored analysis is used; nothing is analyzed. |

A directory is analyzed again every time, which is quick when little has changed because
results for unchanged files come from the per-file cache. To read a stored analysis
instead, give a view command (`show`, `report`, `card`, and so on) the repository's name
from `repodna list`, or its identifier; it says which analysis it uses:

```text
Using the stored analysis of polyglot from 2026-09-25 (revision 540e92db6965). Pass a path to analyze again.
```

## Options shared by most commands

Global options:

| Option | Meaning |
|---|---|
| `--color auto\|always\|never` | When to use color. `NO_COLOR` is respected in `auto` mode. |
| `-q`, `--quiet` | Print only results and errors, without progress. |
| `--config FILE` | Read this configuration file after your user configuration. |
| `--no-user-config` | Ignore your user configuration file. |
| `--no-project-config` | Ignore the repository's own `repodna.toml`. |

Options that control how an analysis runs (accepted by every command that may analyze):

| Option | Meaning |
|---|---|
| `--profile NAME` | `quick`, `standard` (default), `deep`, `history-only`, `architecture-only`, `dependencies-only`, or `security-only`. See [the analysis engine](analysis-engine.md#profiles). |
| `--no-store` | Do not store the analysis locally (this also skips the cache). |
| `--no-cache` | Do not reuse per-file results from earlier runs. |
| `--plugin NAME` | Enable a plugin for this run (repeatable). |
| `--plugin-dir DIR` | Also search this directory for plugins (repeatable). |
| `--anonymize` | Replace contributor names with pseudonyms. |
| `--no-commit-messages` | Do not keep commit subjects in the artifact. |
| `--threads N` | Worker threads (`0` means one per CPU). |
| `--max-commits N` | Analyze at most this many commits. |
| `--clone-depth N` | Clone URLs with at most this many commits of history. |
| `--allow-private-hosts` | Allow cloning from hosts on private networks. |
| `--allow-insecure-urls` | Allow cloning over unencrypted `http://` and `git://`. |
| `--reproducible` | Record zero durations; with `SOURCE_DATE_EPOCH` set, repeated runs produce identical artifacts. |

Privacy presets, used by `--privacy` on the commands that print or write output:

| Preset | What is removed |
|---|---|
| `local` | Nothing beyond what is never stored: secret values and absolute paths. |
| `share` | Also commit messages, the text of marker comments (such as `TODO` notes), and command output. |
| `public` | Also contributor names (replaced by pseudonyms), remote URLs, and symbol names. |

See [privacy](privacy.md) for details.

## Analyze

### `repodna analyze` (alias `scan`)

Analyzes a repository, stores the result, and prints a summary.

```sh
repodna analyze                       # the current directory
repodna analyze ~/src/project --profile deep
repodna analyze https://github.com/sanskarIN/RepoDNA
repodna analyze . --output repodna-report   # also write a report bundle
repodna analyze . --format json > repodna.json
repodna analyze . --format markdown > REPORT.md
```

| Option | Meaning |
|---|---|
| `--format text\|json\|markdown` | What to print: a summary (default), the complete artifact, or the full report. |
| `-o`, `--output DIR` | Also write a report bundle into this directory. |
| `--privacy PRESET` | Privacy preset for printed and written output (default `local`). |
| `--force` | Write into an existing directory that RepoDNA did not create. |

Progress goes to standard error, so `--format json > file` captures only the artifact.
Press Ctrl+C to cancel; exit code 130 means the analysis was cancelled.

### `repodna ci`

Analyzes a repository and summarizes it for continuous integration, optionally failing
the job.

```sh
repodna ci --fail-on warning
repodna ci --format github --fail-on critical
repodna ci --format markdown --summary-file "$GITHUB_STEP_SUMMARY"
repodna ci --baseline main.json --new-only --fail-on warning
```

| Option | Meaning |
|---|---|
| `--fail-on never\|info\|attention\|warning\|critical` | Exit with code 5 when findings at or above this severity exist (default `never`). |
| `--baseline FILE` | Compare with a baseline artifact, for example one from the base branch. |
| `--new-only` | With `--baseline`, fail only on findings the baseline does not have. |
| `--format text\|markdown\|json\|github` | `github` prints GitHub Actions annotations plus a text summary. |
| `--summary-file FILE` | Also write the Markdown summary to this file. |

Suppressed findings never fail the job. See [examples/ci](../examples/ci) for a complete
workflow.

## Explore in the terminal

These commands print one part of an analysis. Each accepts a target, `--format
text|markdown|json` (json prints the relevant part of the artifact), and `--privacy`.

| Command | Shows |
|---|---|
| `repodna architecture` | Modules, dependencies between them, layers, cycles, and entry points |
| `repodna dependencies` | Declared dependencies, lockfiles, and dependency signals |
| `repodna history` | Commits, contributors, releases, activity, and quiet periods |
| `repodna hotspots` | Files that change often and are complex, and why |
| `repodna timeline` | Time Machine snapshots, epochs, events, and the project story |
| `repodna findings` | Findings with their evidence |

`repodna findings` also takes `--severity info|attention|warning|critical` (show that
severity and above), `--rule PREFIX` (for example `--rule security.`), and
`--include-suppressed`.

### `repodna show`

Prints any sections of the report in the terminal.

```sh
repodna show --list
repodna show --section summary,tests,build
repodna show polyglot --section architecture --format markdown
```

`--format` is `text` or `markdown`; for JSON, use `repodna report --format json`. The
sections are:

| Identifier | Section |
|---|---|
| `cover` | Project DNA |
| `summary` | Executive summary |
| `identity` | Repository identity |
| `languages` | Language map |
| `structure` | Project structure |
| `architecture` | Architecture |
| `dependencies` | Dependencies |
| `hotspots` | Code hotspots |
| `complexity` | Complexity signals |
| `duplication` | Duplication signals |
| `tests` | Tests |
| `build` | Build information |
| `documentation` | Documentation |
| `security` | Security signals |
| `history` | Git history |
| `contributors` | Contributor analytics |
| `time-machine` | Codebase Time Machine |
| `evolution` | Architectural evolution |
| `findings` | Major findings |
| `onboarding` | Onboarding guide |
| `evidence` | Evidence appendix |
| `metrics` | Raw metrics appendix |
| `metadata` | Tool and version metadata |

## Reports and sharing

### `repodna report`

Writes a report bundle (a directory with every format) or one format.

```sh
repodna report                                   # ./repodna-report/
repodna report ~/src/project -o project-report
repodna report --format html -o report.html
repodna report --format markdown --sections summary,architecture,findings
repodna report --format csv --table hotspots -o hotspots.csv
repodna report --format html --theme dark --privacy public -o public.html
```

| Option | Meaning |
|---|---|
| `--format bundle\|html\|markdown\|json\|csv` | Default `bundle`. |
| `-o`, `--output PATH` | A directory for bundles (default `repodna-report`), otherwise a file (default: standard output). |
| `--theme professional\|minimal\|technical\|dark` | HTML theme. |
| `--privacy PRESET` | Privacy preset. |
| `--sections IDS` | Comma-separated sections to include (default: all). |
| `--no-branding` | Omit the RepoDNA credit line. |
| `--table NAME` | CSV table: `files`, `findings` (default), `metrics`, `hotspots`, `dependencies`, `contributors`, or `languages`. |
| `--force` | Replace files or write into a directory RepoDNA did not create. |

A bundle contains `index.html` (a self-contained interactive page), `report.md`,
`repodna.json`, `dna-card.svg`, `dna-card.png`, and a `data/` directory with CSV tables.
See [reports](reports.md).

### `repodna card`

Writes the Project DNA card.

```sh
repodna card                          # dna-card.svg
repodna card --format png --dark -o dna-card-dark.png
repodna card --privacy public --no-branding
```

Options: `--format svg|png` (PNG is rendered at twice the size), `--dark`, `--no-branding`,
`-o FILE`, `--privacy`, `--force`. See [DNA cards](dna-cards.md).

### `repodna badge`

Writes SVG badges and prints the Markdown to paste into a README.

```sh
repodna badge                          # all badges into ./badges/
repodna badge --kind languages -o docs/badges
```

`--kind` is `all` (default), `dna`, `languages`, `architecture`, `activity`, or `tests`.

### `repodna onboarding`

Writes a developer onboarding guide as Markdown files.

```sh
repodna onboarding -o docs/onboarding
```

The guide (default directory `repodna-onboarding`) has a `README.md` and one file each for
the overview, setup, architecture, important files, testing, dependencies, and recent
changes.

### `repodna compare`

Compares two or more repositories or analyses.

```sh
repodna compare ~/src/service-a ~/src/service-b
repodna compare before.repodna after.repodna --format html -o compare.html
```

Options: `--format text|markdown|json|html`, `-o FILE`, `--theme`, `--privacy` (applied to
every analysis), `--force`.

### `repodna export` and `repodna import`

`export` writes a stored analysis as a portable `.repodna` file (default privacy preset
`share`); `import` stores an artifact on another machine.

```sh
repodna export polyglot                       # repodna-polyglot-<date>.repodna
repodna export . --privacy public -o public.repodna
repodna import public.repodna
```

Imported analyses are listed with the location `imported:<name>`.

## Local storage

| Command | What it does |
|---|---|
| `repodna list` (alias `ls`) | Lists stored repositories (`--format text\|json`). |
| `repodna clean TARGET` | Shows which stored analyses would be deleted; `--yes` deletes them. `--all` cleans every repository and `--keep N` keeps the newest N analyses of each. |
| `repodna cache stats` | Shows storage and cache sizes. |
| `repodna cache clear` | Clears the per-file analysis cache. |
| `repodna cache repair` | Checks the database, sets a damaged one aside, and rebuilds the index from stored artifacts. |
| `repodna cache reset` | Deletes the database and cache; stored artifacts are kept and can be re-indexed. |

```sh
repodna clean polyglot --keep 3          # dry run
repodna clean polyglot --keep 3 --yes
```

Repositories themselves are never touched. Storage locations are described in
[privacy](privacy.md#what-is-stored-and-where).

## Setup and diagnostics

| Command | What it does |
|---|---|
| `repodna init [DIR]` | Writes a commented `repodna.toml` into a repository (`--force` replaces an existing one). |
| `repodna config path` | Prints the path of your user configuration file. |
| `repodna config init` | Creates your user configuration from a commented template. |
| `repodna config show [DIR]` | Prints the effective configuration for a repository and where each value came from. |
| `repodna config validate [DIR]` | Checks every configuration layer. |
| `repodna plugins list` | Lists discovered plugins and whether they are enabled. |
| `repodna plugins show NAME` | Shows a plugin's manifest and permissions. |
| `repodna plugins enable NAME` / `disable NAME` | Changes your user configuration. |
| `repodna plugins check DIR` | Validates a plugin directory without running it. |
| `repodna plugins path` | Prints the user plugin directory. |
| `repodna doctor` | Checks the installation, storage, Git, configuration, and plugins (`--json`). `--export FILE` also writes a diagnostics bundle for bug reports; see below. |
| `repodna version` | Prints version information (`--json`). |
| `repodna schema [artifact\|config]` | Prints the JSON Schema of the artifact or of the configuration file. |
| `repodna completions SHELL` | Prints a completion script for `bash`, `elvish`, `fish`, `powershell`, or `zsh`. |

`repodna doctor --export diagnostics.zip` writes a ZIP file with the results of the checks,
the environment (versions, operating system, and the environment variables RepoDNA reads),
the effective configuration in the current directory, and summaries of storage and plugins.
It contains no source code, file contents, analysis results, or names of stored
repositories, and your home directory is shown as `~`. Read it before you attach it to an
issue; `--no-project-config` leaves out the current directory's `repodna.toml`. Like other
outputs, it replaces an existing file only if RepoDNA wrote it, unless you pass `--force`.

```sh
repodna completions bash > ~/.local/share/bash-completion/completions/repodna
repodna completions zsh > "${fpath[1]}/_repodna"
repodna completions fish > ~/.config/fish/completions/repodna.fish
```

## Web interface

### `repodna serve`

Serves the web interface and a local API on 127.0.0.1 only.

```sh
repodna serve                 # http://127.0.0.1:7878
repodna serve --port 0        # pick a free port
repodna serve --no-scan       # browse stored analyses only
```

It prints a sign-in link that contains a session token; open that link. Options:
`--port PORT` (default 7878), `--web-dir DIR` (serve the interface from a directory
instead of the built-in files), `--no-scan` (do not allow starting analyses from the
browser), and the analysis options above, which apply to analyses started from the browser.
See [the web interface](web.md).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Unexpected internal error |
| 2 | Invalid command-line usage |
| 3 | Input not found, unreadable, or unsupported |
| 4 | Invalid configuration |
| 5 | CI policy violated (findings at or above `--fail-on`) |
| 6 | Local storage could not be read or written |
| 7 | An external tool or service failed (Git or the network) |
| 130 | Cancelled (Ctrl+C) |

Errors are printed with a `hint:` line when RepoDNA knows what to do next.

## Environment variables

| Variable | Effect |
|---|---|
| `REPODNA_HOME` | One directory for storage and the user configuration (useful for portable installs, CI, and tests). |
| `REPODNA_GIT` | The Git executable to use (default: `git` on `PATH`). |
| `NO_COLOR` | Disables color in `auto` mode. |
| `SOURCE_DATE_EPOCH` | Used as the analysis time, so that repeated analyses of the same revision can produce identical artifacts (with `--reproducible`). |
