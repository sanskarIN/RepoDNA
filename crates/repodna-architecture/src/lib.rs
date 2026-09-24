//! Architecture analysis for RepoDNA.
//!
//! This crate infers modules from the directory layout and package boundaries, resolves
//! the imports extracted by `repodna-parser` to files inside the repository, and builds
//! file- and module-level dependency graphs. From those graphs it derives cycles, layers,
//! fan-in and fan-out, instability, betweenness centrality, and architecture findings.
//!
//! Everything here is static and lexical: imports that are computed at runtime, generated
//! code, and reflection are invisible, and the report says so in its method notes.

pub mod graph;
