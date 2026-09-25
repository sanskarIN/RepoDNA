# Troubleshooting

Start with:

```sh
repodna doctor
```

It checks the installation, storage, the cache, Git, language definitions, configuration,
plugins, and AI settings, and says what to do about each problem. Every error message
names what went wrong, and most end with a `hint:` line. Exit codes are listed in
[the CLI reference](cli.md#exit-codes).

## Analysis

**No history is reported.** History needs Git on your `PATH` (or `REPODNA_GIT` pointing to
it) and the root of a Git working tree. RepoDNA analyzes a subdirectory of a repository, an
archive, or a directory without `.git` as files only, and the History section says why. In
CI, checkouts are often shallow (`fetch-depth: 1`); fetch the full history
(`fetch-depth: 0` with `actions/checkout`) for complete results.

**A Git URL is refused.** Unencrypted `http://` and `git://` URLs need
`--allow-insecure-urls`, and hosts on private networks need `--allow-private-hosts`.
`ext::` and local transports are always refused.

**A private repository cannot be cloned.** RepoDNA never lets Git prompt for credentials.
Use a URL your Git can access without prompting (for example an SSH URL with a key in your
agent, or a credential helper), or clone the repository yourself and analyze the directory.

**The analysis is slow.** Try `--profile quick` (no history) or `--profile standard`
instead of `deep`, limit history with `--max-commits`, and exclude large generated or
vendored directories with `[ignore] patterns`. Repeat analyses reuse the per-file cache
unless you pass `--no-cache` or `--no-store`. See the [benchmarks](../benchmarks/README.md)
for typical times.

**Some files are not analyzed.** Files larger than `analysis.max_file_bytes` (2 MB) are
classified but not read, binary files are not read, and paths matched by `.gitignore`,
`.ignore`, or `[ignore] patterns` are skipped. `repodna show --section structure` reports
how many files were skipped and which ignore patterns applied.

**Generated or vendored code distorts the results.** Mark it in `repodna.toml`
(`[classification] generated = ["src/gen/**"]`, `vendor = ["third_party/**"]`) or with
`linguist-generated` and `linguist-vendored` in `.gitattributes`.

**A finding is wrong for this repository.** Findings are signals, not verdicts. Accept it
with a reason, so it stays visible but no longer fails CI:

```toml
[[suppress]]
rule = "quality.large-file"
path = "src/generated/**"
reason = "Generated from the schema"
```

If you think the rule itself is wrong, [open an issue](https://github.com/sanskarIN/RepoDNA/issues/new/choose).

**A language is not recognized.** It is listed under unrecognized files. A
[language plugin](plugins.md#language-definitions) can add it.

## Configuration

**`invalid configuration` (exit code 4).** The message names the file and the key.
Unknown keys are errors, which catches typos. Run `repodna config validate`.

**A setting in `repodna.toml` has no effect.** `[ai]`, `[plugins]`, `[execution]`,
`privacy.remote_ai`, and `privacy.telemetry` are ignored in a repository's configuration,
with a warning; set them in your user configuration (`repodna config path`).
`repodna config show` prints every effective value and where it came from.

## Storage

**`local storage could not be read or written` (exit code 6).** Check that the data
directory is writable (see [privacy](privacy.md#what-is-stored-and-where)); set
`REPODNA_HOME` to use another directory.

**The database is damaged.** `repodna cache repair` checks it, moves a damaged database
aside (it is never deleted automatically), and rebuilds the index from the stored
artifacts. `repodna cache reset` deletes the database and cache but keeps the artifacts.

**Writing a report is refused.** RepoDNA does not overwrite files or write into directories
it did not create, and never writes reports into the root of a Git working tree. Choose
another `--output`, or pass `--force`.

## Web interface

**The page says the build does not include the interactive web interface.** The binary was
built without the web interface (for example with `cargo install --git`). Install a
release binary, or build the interface first (`npm ci && npm run build -w @repodna/web`)
and reinstall; see [installation](installation.md#build-from-source).

**The page asks you to sign in, or the API returns 401.** Open the link that
`repodna serve` printed; it contains the session token. A new token is generated every time
the server starts.

**The port is in use.** `repodna serve --port 0` picks a free port.

## Desktop app

**It does not start on Linux.** Install WebKitGTK 4.1 (`libwebkit2gtk-4.1-0` on Debian and
Ubuntu, `webkit2gtk4.1` on Fedora).

**macOS or Windows refuses to open it.** The app is not code-signed; see
[unsigned binaries](installation.md#unsigned-binaries).

## AI explanations

| Message | What to do |
|---|---|
| AI features are off | Configure `[ai]` in your user configuration; see [AI](ai.md). |
| Remote providers are not allowed | Set `privacy.remote_ai = true` or pass `--allow-remote-ai`. |
| The API key is missing | Export the variable named by `ai.api_key_env`. |
| The request timed out | Raise `ai.timeout_seconds`, or use a smaller model. |

`repodna explain --dry-run` shows what would be sent without contacting anything.

## Plugins

**A plugin is not found.** `repodna plugins list` shows what was discovered and where; pass
`--plugin-dir` or add the directory to `[plugins] directories`.

**A plugin fails.** Its error output is shown as a warning, and the rest of the analysis is
unaffected. `repodna plugins check DIR` validates its manifest and language definitions.
Raise `[plugins] timeout_seconds` for slow analyzers.

## Reporting a problem

Include the output of `repodna version --json` and `repodna doctor --json`, the command you
ran, and what you expected. Use the [issue templates](https://github.com/sanskarIN/RepoDNA/issues/new/choose).
Report security vulnerabilities privately, as described in [SECURITY.md](../SECURITY.md).
