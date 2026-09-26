# license-headers (example analyzer plugin)

Reports which source files carry an `SPDX-License-Identifier:` comment in their first
20 lines. It adds one metric, `plugin.license-headers.spdx-coverage` (share of source files
with an identifier), and, when some files lack one, one informational finding,
`plugin.license-headers.missing-spdx-identifier`, that lists them.

It is written in Python using only the standard library, to show that plugins can be
written in any language that reads and writes JSON.

## Install

```sh
cp -r plugins/license-headers "$(repodna plugins path)/"
repodna plugins enable license-headers
repodna analyze .
```

Or for a single run, without installing:

```sh
repodna analyze . --plugin-dir plugins --plugin license-headers
```

Requires `python3` on `PATH` (on Windows, change `command` in `repodna-plugin.toml` to
`["python", "license_headers.py"]` or `["py", "license_headers.py"]`).

## Permissions

`repository-files`: RepoDNA passes the absolute path of the checkout so the plugin can
open source files. The plugin reads at most 8 KB from the start of each source file and
never writes anything.

Like every analyzer plugin, it runs:

- without a shell, from its own directory;
- with a minimal environment (`PATH`, home and temporary directories, locale), so API
  keys and tokens in your environment are not visible to it;
- with the time and output limits from `[plugins]` in your user configuration;
- with **your user's permissions**. The limits above are not an operating-system
  sandbox, so only enable plugins you have read and trust.

## Lifecycle

1. `repodna analyze` finishes its built-in stages.
2. RepoDNA writes the request (repository name, revision, primary languages, file list,
   and the checkout path) to the plugin's standard input as JSON.
3. The plugin writes one JSON response to standard output and exits with status 0.
4. RepoDNA validates the response, namespaces rule and metric identifiers under
   `plugin.license-headers.`, and records a plugin run entry in the artifact. Suppressions
   in `repodna.toml` apply to plugin findings like any others.

A plugin that fails, times out, or writes invalid output is reported as a warning; the
rest of the analysis is unaffected.
