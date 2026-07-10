//! tsc_conformance — ad-hoc queries over the TypeScript-Go conformance baselines.
//!
//! Tool #1 of the typechecker conformance harness (the "ask important questions"
//! tool). ZERO typechecker code: every query here is derived from the committed
//! tsgo `*.errors.txt` baselines alone. The corpus *input* files live in a git
//! submodule that is often unmaterialized, so any question needing test inputs or
//! directives degrades gracefully rather than crashing.
//!
//! [`baseline`] holds both the summary-block parser (the `query` tool's seed)
//! and the full-baseline parser ([`baseline::parse_baseline`]); [`render`] is
//! the faithful plain `.errors.txt` renderer ported from typescript-go;
//! [`pretty`] is its ANSI-colored `pretty=true` counterpart (model + parser +
//! renderer); [`roundtrip`] parses → renders → byte-compares every baseline
//! (the P0 self-check, `zero` checker code).
//!
//! The corpus-*input* side ([`corpus`], [`directives`], [`variants`],
//! [`options_meta`], [`index`]) ports the tsgo test harness: it indexes the
//! `tests/cases` inputs, parses their `// @` directives, expands their varyBy
//! variants, and joins the derived variants back to the on-disk baselines — the
//! substrate a future checker will drive, still zero checker code.

pub mod baseline;
pub mod corpus;
pub mod directives;
pub mod discovery;
pub mod index;
pub mod libs;
pub mod options_meta;
pub mod pretty;
pub mod query;
pub mod render;
pub mod roundtrip;
pub mod runner;
pub mod variants;

pub use discovery::{baselines_dir, corpus_materialized, discover_baselines};
pub use index::run_index;
pub use query::{denominators, histogram, tests_by_code};
pub use roundtrip::run_roundtrip;
pub use runner::{check_one, run_skeleton, RunFilter, RunOptions};
