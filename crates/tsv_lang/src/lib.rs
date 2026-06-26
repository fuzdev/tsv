//! Language-agnostic foundation primitives for tsv
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `LocationTracker` - line/column information
//! - `ParseError` - error types and result aliases
//! - `OutputBuffer` - shared printer output utilities
//! - `config` - hardcoded formatter settings (`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT`)
//! - `Comment` - shared comment type
//! - `doc` - document builder primitives for prettier-compatible formatting
//! - `escapes` - escape sequence utilities for printers
//! - `printing` - shared printing utilities for printers
//! - `parser` - shared parser utilities
//! - `interner` - string interner utilities for printers
//! - `json` - sizing heuristic for public-AST JSON output buffers

mod comment;
mod config;
pub mod doc;
mod error;
mod escapes;
mod interner;
mod location;
mod output;
mod parser;
pub mod printing;
mod sizing;
pub mod source_scan;
mod span;

pub use comment::{
    ClassifiedComments, Comment, CommentPosition, classify_comment, classify_comment_fast,
    comments_after, comments_in_range, find_first_comment_from, has_comments_in_range,
    has_line_comments_in_range, has_multiline_block_comments_in_range, is_format_ignore_directive,
    is_format_ignore_range_end, is_format_ignore_range_start,
};
pub use config::{EmbedContext, INDENT, LayoutMode, PRINT_WIDTH, TAB_WIDTH};
pub use error::{ErrorContext, ParseError, Result};
pub use interner::{InfallibleResolve, SharedInterner, SymbolResolver, SymbolToU32};
pub use location::{ByteToCharMap, LocationTracker, Position, SourceLocation};
pub use output::{OutputBuffer, write_indent};
pub use parser::PeekData;
pub use sizing::{estimated_ast_arena_capacity, estimated_json_capacity};
pub use span::Span;
