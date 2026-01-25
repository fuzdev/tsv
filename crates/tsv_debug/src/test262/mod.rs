//! test262 integration for parser validation.
//!
//! This module provides tools to run the ECMAScript conformance test suite
//! against tsv's TypeScript parser.

pub mod discovery;
pub mod frontmatter;
pub mod runner;

pub use discovery::{DiscoveryOptions, discover_tests};
pub use runner::{TestSummary, format_failure, run_test};
