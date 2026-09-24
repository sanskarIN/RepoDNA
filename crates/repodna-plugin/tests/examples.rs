//! The example plugins in `plugins/` load.

use std::path::{Path, PathBuf};

use repodna_core::config::PluginConfig;
use repodna_plugin::load_enabled;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

#[test]
fn example_plugins_load() {
    let config = PluginConfig {
        enabled: vec!["zig-language".into()],
        ..PluginConfig::default()
    };
    let loaded = load_enabled(&config, &[examples()], true);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.active, vec!["zig-language"]);
    assert_eq!(loaded.languages.len(), 1);
    assert_eq!(loaded.languages[0].id, "zig");
    assert!(loaded.extensions.is_empty());
}
