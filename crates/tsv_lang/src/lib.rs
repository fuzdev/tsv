//! Language-agnostic foundation primitives for tsv
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `LocationTracker` / `ByteToCharMap` / `LocationMapper` - line/column
//!   information and byte→UTF-16 position mapping
//! - `ParseError` - error types and result aliases
//! - `OutputBuffer` - shared printer output utilities
//! - `config` - hardcoded formatter settings (`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT`)
//! - `Comment` - shared comment type
//! - `comment_ledger` - print-once comment ledger (`comment_check` feature)
//! - `doc` - document builder primitives for prettier-compatible formatting
//! - `escapes` - escape sequence utilities for printers
//! - `printing` - shared printing utilities for printers
//! - `sizing` - sizing heuristics for public-AST JSON / arena buffers
//! - `json_writer` - shared wire-JSON emission substrate (`json` feature)

mod comment;
#[cfg(feature = "comment_check")]
pub mod comment_ledger;
mod config;
pub mod doc;
mod error;
mod escapes;
#[cfg(feature = "json")]
mod json_writer;
mod location;
mod output;
pub mod printing;
mod sizing;
pub mod source_scan;
mod span;

pub use comment::{
    ClassifiedComments, Comment, CommentPosition, classify_comment, classify_comment_fast,
    comments_in_source_after, comments_in_source_range, comments_on_page_in_range,
    comments_to_emit_after, comments_to_emit_in_range, find_first_comment_from,
    has_comments_on_page_in_range, has_comments_to_emit_in_range, has_line_comments_in_range,
    has_multiline_block_comments_on_page_in_range, is_format_ignore_directive,
    is_format_ignore_range_end, is_format_ignore_range_start,
};
pub use config::{EmbedContext, INDENT, LayoutMode, PRINT_WIDTH, TAB_WIDTH};
pub use error::{ErrorContext, ParseError, Result, lex_err};
#[cfg(feature = "json")]
pub use json_writer::{JsonWriter, write_array, write_or_null};
pub use location::{ByteToCharMap, LocationMapper, LocationTracker, Position};
pub use output::{OutputBuffer, write_indent};
pub use sizing::{estimated_ast_arena_capacity, estimated_json_capacity};
pub use span::Span;
