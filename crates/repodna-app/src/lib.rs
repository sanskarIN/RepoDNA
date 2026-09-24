//! Application services shared by the RepoDNA command-line interface, the local server,
//! and the desktop app.
//!
//! The analysis engine turns a repository into an artifact. This crate adds everything
//! around it that front ends need in the same way: locating user configuration, plugins,
//! and storage; assembling the effective configuration; running analyses with the cache,
//! plugins, and local history; loading earlier analyses from files or storage; writing
//! reports without overwriting files RepoDNA did not create; and optional AI
//! explanations. Front ends only parse input and present results.

pub mod analysis;
pub mod config;
pub mod error;
pub mod explain;
pub mod export;
pub mod paths;

pub use explain::{ExplainOutcome, ExplainRequest, explain, plan_explanation};
pub use export::{
    OUTPUT_MARKER, OutputFile, artifact_file_name, report_bundle, write_bundle, write_file,
};

pub use analysis::{
    AnalysisOutcome, AnalyzeOptions, Loaded, Source, Target, load_stored, load_target,
    resolve_target, run_analysis,
};
pub use config::{ConfigOptions, Overrides, load_config};
pub use error::{AppError, ErrorKind};
pub use paths::AppPaths;
