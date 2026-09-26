# zig-language (example language plugin)

Adds [Zig](https://ziglang.org) to RepoDNA's language registry: file detection (`.zig`),
comment-aware line counts, `@import` extraction for the architecture graph,
function and type symbols, and cyclomatic complexity estimates.

This plugin is **pure data**. It has no `command`, so nothing is executed; RepoDNA only
reads `languages/zig.toml`. Every pattern in it is compiled by the Rust `regex` crate,
which guarantees linear-time matching.

## Install

Copy this directory into your plugin directory and enable it by name:

```sh
repodna plugins path              # prints the plugin directory
cp -r plugins/zig-language "$(repodna plugins path)/"
repodna plugins enable zig-language
```

Or enable it for one run:

```sh
repodna analyze . --plugin-dir plugins --plugin zig-language
```

## Permissions

None. Language plugins cannot run code or read anything besides their own definition file.

## Lifecycle

The definition is loaded when an analysis starts with the plugin enabled, and it applies
only to that analysis. A definition that fails to parse is reported as a warning and the
analysis continues without it.
