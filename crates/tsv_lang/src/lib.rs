//! Language-agnostic foundation primitives for tsv
//!
//! This crate provides core types shared across all language implementations:
//! - `Span` - source code location tracking
//! - `WireCoordinates` / `WirePoint` - a language's position coordinates (its line rule
//!   and how it counts a leading BOM), and one position in them
//! - `WireTables` / `WirePositions` - what a wire writer emits positions from (the
//!   byte→UTF-16 map, and the line table under the `locations` feature);
//!   `LocationMapper` - the line/column view a `loc` emitter reads from them
//! - `ParseError` - error types and result aliases
//! - `OutputBuffer` - shared printer output utilities
//! - `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` - hardcoded formatter settings, and
//!   `EmbedContext` / `LayoutMode` - the per-input embedding state
//! - `Comment` - shared comment type
//! - `census` - opt-in perf census counters (`census` feature)
//! - `comment_ledger` - print-once comment ledger (`comment_check` feature)
//! - `doc` - document builder primitives for prettier-compatible formatting
//! - `printing` - shared printing utilities for printers
//! - `source_scan` - trivia-aware source scanning between AST nodes
//! - `estimated_json_capacity` / `estimated_ast_arena_capacity` - pre-size heuristics
//!   for the wire-JSON output buffer and the parse-time bump arena
//! - `JsonWriter` - shared wire-JSON emission substrate (`json` feature)
//! - `FxHasher` / `FxHashMap` / `FxHashSet` - dep-free multiply-xor hasher for the
//!   side tables
//! - `swar` - word-at-a-time byte-search kernels shared by the line scans, the
//!   wire-JSON escape prescan, and the lexers' token-body scans

pub mod census;
mod comment;
#[cfg(feature = "comment_check")]
pub mod comment_ledger;
mod config;
pub mod doc;
mod error;
mod escapes;
mod hash;
#[cfg(feature = "json")]
mod json_writer;
mod location;
mod output;
pub mod printing;
mod sizing;
pub mod source_scan;
mod span;
pub mod swar;
mod whitespace;

pub use comment::{
    ClassifiedComments, Comment, CommentFreeWindow, CommentPosition, classify_comment,
    classify_comment_scan, comments_in_source_after, comments_in_source_after_comment,
    comments_in_source_from, comments_in_source_range, comments_on_page_from,
    comments_on_page_in_range, comments_to_emit_after, comments_to_emit_from,
    comments_to_emit_in_range, directive_alone_on_line, find_first_comment_from,
    has_comments_on_page_from, has_comments_on_page_in_range, has_comments_to_emit_from,
    has_comments_to_emit_in_range, has_line_comments_from, has_line_comments_in_range,
    has_line_spanning_comments_to_emit_from, has_line_spanning_comments_to_emit_in_range,
    has_multiline_block_comments_on_page_from, has_multiline_block_comments_on_page_in_range,
    is_format_ignore_directive, is_format_ignore_range_end, is_format_ignore_range_start,
    is_honored_format_ignore, is_indentable_block, is_region_end_marker,
    merge_nestled_block_comments, nestled_run_start, owned_leading_comment_at,
    range_too_narrow_for_a_comment,
};
pub use config::{EmbedContext, INDENT, LayoutMode, PRINT_WIDTH, TAB_WIDTH};
pub use error::{ParseError, Result, lex_err};
pub use hash::{FxBuildHasher, FxHashMap, FxHashSet, FxHasher};
#[cfg(feature = "json")]
pub use json_writer::{JsonWriter, StageRun, StagedDigits, write_array, write_or_null};
pub use location::{
    BOM, LeadingBom, LineRule, LocationMapper, Position, Wire, WireCoordinates, WirePoint,
    WirePositions, WireTables, leading_bom_len,
};
pub use output::{OutputBuffer, write_indent};
pub use sizing::{estimated_ast_arena_capacity, estimated_json_capacity};
pub use span::Span;
pub use whitespace::{
    is_js_whitespace, trim_end_js_whitespace, trim_js_whitespace, trim_start_js_whitespace,
};
