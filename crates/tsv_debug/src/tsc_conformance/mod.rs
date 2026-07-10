//! tsc_conformance — ad-hoc queries over the TypeScript-Go conformance baselines.
//!
//! Tool #1 of the typechecker conformance harness (the "ask important questions"
//! tool). ZERO typechecker code: every query here is derived from the committed
//! tsgo `*.errors.txt` baselines alone. The corpus *input* files live in a git
//! submodule that is often unmaterialized, so any question needing test inputs or
//! directives degrades gracefully rather than crashing.
//!
//! The shared `.errors.txt` summary-block parser in [`baseline`] is the seed a
//! later slice extends into a full round-trip renderer.

pub mod baseline;
pub mod discovery;
pub mod query;

pub use discovery::{baselines_dir, corpus_materialized, discover_baselines};
pub use query::{denominators, histogram, tests_by_code};
