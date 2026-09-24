//! RepoDNA plugins.
//!
//! A plugin is a directory with a `repodna-plugin.toml` manifest. It can ship declarative
//! language definitions (pure data, compiled with linear-time regular expressions), an
//! analyzer command that speaks a versioned JSON protocol over standard input and output,
//! or both. Plugins run only when enabled by name in the user configuration or on the
//! command line; a repository's own configuration can never enable them.
//!
//! Analyzers run without a shell, from the plugin directory, with a minimal environment
//! (API keys and tokens are not passed), a time limit, and an output limit. Their findings
//! and metrics are validated and namespaced under `plugin.<name>.`. Only analyzers granted
//! the `repository-files` permission receive the checkout path. These constraints are not
//! an operating-system sandbox; a plugin runs with the user's permissions.

pub mod discover;
pub mod manifest;
pub mod protocol;

pub use discover::{Discovery, Plugin, discover};
pub use manifest::{MANIFEST_FILE, PLUGIN_API, Permission, PluginManifest};
