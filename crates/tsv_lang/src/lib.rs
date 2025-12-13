//! Language-agnostic foundation primitives for TSV
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `LocationTracker` - line/column information
//! - `ParseError` - error types and result aliases
//! - `OutputBuffer` - shared printer output utilities
//! - `PrintConfig` - shared printer configuration
//! - `Comment` - shared comment type
//! - `doc` - document builder primitives for prettier-compatible formatting
//! - `quotes` - smart quote selection for string literals
//! - `escapes` - escape sequence utilities for printers
//! - `printing` - shared printing utilities for printers
//! - `parser` - shared parser utilities
//! - `interner` - string interner utilities for printers

mod comment;
mod config;
pub mod doc;
mod error;
pub mod escapes;
mod interner;
mod location;
mod output;
mod parser;
pub mod printing;
pub mod quotes;
mod span;

pub use comment::{
    Comment, comments_after, comments_in_range, find_first_comment_from, has_comments_in_range,
    has_line_comments_in_range,
};
pub use config::PrintConfig;
pub use error::{ErrorContext, ParseError, Result};
pub use interner::{InfallibleResolve, SymbolResolver};
pub use location::{LocationTracker, Position, SourceLocation};
pub use output::{OutputBuffer, write_indent};
pub use parser::PeekData;
pub use span::Span;
