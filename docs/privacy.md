# Privacy

RepoDNA is local-first. It analyzes repositories on your machine, stores results on your
machine, and sends nothing anywhere unless you ask it to.

## What never happens

- **No telemetry.** RepoDNA does not collect usage data, crash reports, or analytics, and
  has no code that could. `privacy.telemetry` exists only to make that explicit; it is
  always `false`.
- **No uploads.** Analyses, reports, and cards are written to local files. Nothing is
  published unless you publish it.
- **No update checks** or other background network requests.
- **No secret values are stored.** Possible credentials are recorded by rule, file, line,
  and a keyed fingerprint (so equal values can be recognized within one scan), never by
  value.
- **No absolute local paths in artifacts.** Paths are relative to the repository.

## When RepoDNA uses the network

Only in these cases, each of which you start:

| Action | What is contacted |
|---|---|
| Analyzing a Git URL | The Git host, through your `git` installation, to clone the repository. |
| `repodna explain` with a configured HTTP provider | That provider's endpoint, with the evidence shown by `--dry-run`. Endpoints off this machine also need your consent (`privacy.remote_ai` or `--allow-remote-ai`). |
| Clicking a link in the web interface or desktop app | Your browser opens the link. |

Cloning is restricted by default: `http://` and `git://` URLs (unencrypted) and hosts on
private networks are refused unless you pass `--allow-insecure-urls` or
`--allow-private-hosts`. Credentials embedded in URLs are removed from everything RepoDNA
prints and stores.

`repodna serve` listens on 127.0.0.1 only and requires a session token; see
[the web interface](web.md#security).

## What is stored, and where

| Platform | Data | User configuration |
|---|---|---|
| Linux and other Unix | `$XDG_DATA_HOME/repodna` or `~/.local/share/repodna` | `$XDG_CONFIG_HOME/repodna/config.toml` or `~/.config/repodna/config.toml` |
| macOS | `~/Library/Application Support/RepoDNA` | the same directory, `config.toml` |
| Windows | `%LOCALAPPDATA%\RepoDNA` | `%APPDATA%\RepoDNA\config.toml` |

`REPODNA_HOME` overrides both: data and `config.toml` then live in that one directory.

The data directory holds:

- `repodna.db`: a SQLite database with the index of repositories, analyses, metrics, and
  findings, and the per-file analysis cache (keyed by file content, so unchanged files are
  not parsed again);
- `artifacts/`: each stored analysis as a JSON file;
- `explanations/`: cached AI explanations, if you use them.

The analyzed repositories are never written to. Remove stored data with `repodna clean`,
`repodna cache clear`, or `repodna cache reset`, or by deleting the directory. Use
`--no-store` to analyze without storing anything.

## What an artifact contains

An analysis artifact describes the repository: file paths, sizes, languages, symbols,
dependencies, findings with evidence, and Git history: commit hashes, dates, subjects,
contributor names and e-mail-derived identities, and remote URLs. That is what makes it
useful, and also why it deserves care before you share it.

## Privacy presets

Reports, cards, exports, and terminal views take `--privacy`:

| Preset | Removes |
|---|---|
| `local` (default for local views) | Nothing beyond what is never stored. |
| `share` (default for `repodna export`) | Commit messages, the text of marker comments (such as `TODO` notes), and output of commands you allowed RepoDNA to run. |
| `public` | Everything `share` removes, and also: contributor names and identifiers (replaced by numbered pseudonyms such as `contributor-1`), remote URLs (the repository's name and owner are kept), and symbol and function names. |

```sh
repodna report --privacy public -o public-report
repodna card --privacy public
repodna export --privacy public -o project.repodna
```

Each artifact records the preset and what was removed in `analysisMetadata.privacy`, and
the report's metadata section lists it.

To keep information out of the artifact in the first place:

```sh
repodna analyze . --anonymize            # contributor names become "Contributor 1", "Contributor 2", ...
repodna analyze . --no-commit-messages   # commit subjects are not kept
```

or permanently in your user configuration:

```toml
[privacy]
anonymize_contributors = true
include_commit_messages = false
```

## Before you publish a report

- Use `--privacy public` for anything that leaves your team.
- Read the report: file names and directory structure are visible in every preset.
- Findings about possible secrets name the file and line but never the value. If a real
  credential was found, rotate it: it is in the repository's history, not only in the
  report.

## AI

AI explanations are off by default. When configured, only the numbered evidence shown by
`repodna explain --dry-run` is sent; source code is not sent unless you enable excerpts.
See [AI](ai.md).
