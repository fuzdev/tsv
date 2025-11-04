//! Language-agnostic foundation primitives for TSV
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `LocationTracker` - line/column information
//! - `ParseError` - error types and result aliases
//! - `OutputBuffer` - shared printer output utilities
//! - `quotes` - smart quote selection for string literals
//! - `escapes` - escape sequence utilities for printers
//! - `printing` - shared printing utilities for printers

mod error;
pub mod escapes;
mod location;
mod output;
pub mod printing;
pub mod quotes;
mod span;

pub use error::{ErrorContext, ParseError, Result};
pub use location::LocationTracker;
pub use output::{OutputBuffer, write_indent};
pub use span::Span;
