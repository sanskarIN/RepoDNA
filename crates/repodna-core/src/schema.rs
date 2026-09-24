//! JSON Schemas for the artifact and the configuration file.
//!
//! The schemas are generated from the Rust types, so they cannot drift from the
//! implementation. `repodna schema` prints them and the repository publishes them under
//! `schemas/` for CI systems, IDEs, and other integrations.

use schemars::generate::SchemaSettings;

use crate::config::Config;
use crate::model::artifact::RepositoryDna;
use crate::model::metadata::SCHEMA_VERSION;

/// Canonical URL of the published artifact schema.
pub const ARTIFACT_SCHEMA_ID: &str =
    "https://github.com/sanskarIN/RepoDNA/blob/main/schemas/repodna-artifact.schema.json";

/// Canonical URL of the published configuration schema.
pub const CONFIG_SCHEMA_ID: &str =
    "https://github.com/sanskarIN/RepoDNA/blob/main/schemas/repodna-config.schema.json";

/// Returns the JSON Schema (draft 2020-12) describing a RepositoryDNA artifact.
pub fn artifact_schema() -> serde_json::Value {
    let schema = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<RepositoryDna>();
    let mut value = schema.to_value();
    if let Some(object) = value.as_object_mut() {
        object.insert("$id".to_owned(), ARTIFACT_SCHEMA_ID.into());
        object.insert(
            "description".to_owned(),
            format!(
                "RepoDNA artifact, schema version {SCHEMA_VERSION}. Readers must ignore unknown fields."
            )
            .into(),
        );
    }
    value
}

/// Returns the JSON Schema (draft 2020-12) describing `repodna.toml`.
pub fn config_schema() -> serde_json::Value {
    let schema = SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator()
        .into_root_schema_for::<Config>();
    let mut value = schema.to_value();
    if let Some(object) = value.as_object_mut() {
        object.insert("$id".to_owned(), CONFIG_SCHEMA_ID.into());
        object.insert("title".to_owned(), "RepoDNA configuration".into());
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_schema_describes_required_sections() {
        let schema = artifact_schema();
        assert_eq!(schema["$id"], ARTIFACT_SCHEMA_ID);
        assert_eq!(schema["title"], "RepositoryDna");
        let required = schema["required"].as_array().unwrap();
        for field in ["schemaVersion", "tool", "identity", "analysisMetadata"] {
            assert!(required.iter().any(|value| value == field), "{field}");
        }
        let definitions = schema["$defs"].as_object().unwrap();
        for name in [
            "Finding",
            "Evidence",
            "FileRecord",
            "Timestamp",
            "Confidence",
        ] {
            assert!(definitions.contains_key(name), "{name}");
        }
    }

    #[test]
    fn config_schema_is_titled() {
        let schema = config_schema();
        assert_eq!(schema["title"], "RepoDNA configuration");
        assert!(schema["properties"].get("analysis").is_some());
        assert!(schema["properties"].get("suppress").is_some());
    }
}
