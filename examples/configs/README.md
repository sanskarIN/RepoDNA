# Configuration examples

| File | Use it as | What it shows |
|---|---|---|
| [`repodna.toml`](repodna.toml) | `repodna.toml` at the root of a repository | Profile, ignore patterns, classification, thresholds, suppressions, and report defaults |
| [`strict-ci.toml`](strict-ci.toml) | A file passed with `--config` in CI | Tighter thresholds for a CI gate |
| [`user-config.toml`](user-config.toml) | Your user configuration (`repodna config path`) | Settings only you can enable: plugins, command execution, and an AI provider |

Check a file before using it:

```sh
repodna config validate --config examples/configs/strict-ci.toml
```

Every setting is described in [the configuration reference](../../docs/configuration.md).
