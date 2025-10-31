//! Language-agnostic foundation primitives for TSV
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `LocationTracker` - line/column information
//! - `ParseError` - error types and result aliases

mod error;
mod location;
mod span;

pub use error::{ErrorContext, ParseError, Result};
pub use location::LocationTracker;
pub use span::Span;
