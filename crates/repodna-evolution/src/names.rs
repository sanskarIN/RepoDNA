//! How generated sentences refer to languages and snapshots.

use std::collections::BTreeMap;

use repodna_core::model::evolution::{Snapshot, SnapshotKind};

/// Display names of languages by identifier (`rust` → `Rust`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageNames(BTreeMap<String, String>);

impl LanguageNames {
    /// Names from `(identifier, name)` pairs.
    pub fn new(names: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(names.into_iter().collect())
    }

    /// The display name of `id`, or `id` itself for a language without one.
    pub fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.0.get(id).map_or(id, String::as_str)
    }
}

/// How a sentence refers to the moment of a snapshot: "the first commit", "v1.2.0", …
pub fn moment(snapshot: &Snapshot) -> String {
    match snapshot.kind {
        SnapshotKind::Initial if snapshot.label == "Initial commit" => {
            "the first commit".to_owned()
        }
        SnapshotKind::Initial => "the oldest analyzed commit".to_owned(),
        SnapshotKind::Current => "the current revision".to_owned(),
        SnapshotKind::Release | SnapshotKind::Sample => snapshot.label.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fall_back_to_identifiers() {
        let names = LanguageNames::new([("rust".to_owned(), "Rust".to_owned())]);
        assert_eq!(names.name("rust"), "Rust");
        assert_eq!(names.name("zig"), "zig");
    }
}
