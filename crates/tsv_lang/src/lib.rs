//! Support crate for tsv's language crates — [`tsv_ts`](https://docs.rs/tsv_ts),
//! [`tsv_css`](https://docs.rs/tsv_css) and [`tsv_svelte`](https://docs.rs/tsv_svelte).
//! Depend on one of those, not on this one.
//!
//! **No stability promise outside the items they re-export:** [`Error`], [`ErrorKind`],
//! [`ErrorReport`], [`Result`], [`WirePoint`], [`WireCoordinates`], [`LineRule`],
//! [`LeadingBom`], and the sized-stack pair [`STACK_SIZE`] and [`run_with_stack`].
//! Everything else here is the plumbing the three share — the source spans, the doc IR,
//! the printing helpers, the wire-JSON writer, the comment scans, the depth limits —
//! hidden from these docs and free to change in any release.
//!
//! Part of [tsv](https://github.com/fuzdev/tsv).

#![warn(missing_docs)]

// A map of the source, most of it hidden:
// - `Span` - source code location tracking
// - `WireCoordinates` / `WirePoint` - a language's position coordinates (its line rule
//   and how it counts a leading BOM), and one position in them
// - `WireTables` / `WirePositions` - what a wire writer emits positions from (the
//   byte→UTF-16 map, and the line table under the `locations` feature);
//   `LocationMapper` - the line/column view a `loc` emitter reads from them
// - `Error` - error types and result aliases
// - `STACK_SIZE` / `run_with_stack` - the stack the parsers and printers are sized for,
//   and a way to run on one
// - `DepthBudget` / `NESTING_LIMIT` / `SPINE_LIMIT` - the parsers' depth limits
// - `OutputBuffer` - shared printer output utilities
// - `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` - hardcoded formatter settings, and
//   `EmbedContext` / `LayoutMode` - the per-input embedding state
// - `Comment` - shared comment type
// - `census` - opt-in perf census counters (`internal_census` feature)
// - `comment_ledger` - print-once comment ledger (`internal_comment_check` feature)
// - `fanout_probe` - opt-in work counters for the fanout audit (`internal_fanout_check` feature)
// - `doc` - document builder primitives for prettier-compatible formatting
// - `printing` - shared printing utilities for printers
// - `source_scan` - trivia-aware source scanning between AST nodes
// - `estimated_json_capacity` / `estimated_ast_arena_capacity` - pre-size heuristics
//   for the wire-JSON output buffer and the parse-time bump arena
// - `JsonWriter` - shared wire-JSON emission substrate (`json` feature)
// - `FxHasher` / `FxHashMap` / `FxHashSet` - dep-free multiply-xor hasher for the
//   side tables
// - `swar` - word-at-a-time byte-search kernels shared by the line scans, the
//   wire-JSON escape prescan, and the lexers' token-body scans

#[doc(hidden)]
pub mod census;
mod comment;
#[cfg(feature = "internal_comment_check")]
#[doc(hidden)]
pub mod comment_ledger;
mod config;
mod depth;
#[doc(hidden)]
// A hidden module renders nowhere public, so its links to private items are maintainer
// navigation (`docs:audit` documents private items); only the public-only docs.rs build
// fires the lint, which `docs:audit:docsrs` denies everywhere else.
#[expect(rustdoc::private_intra_doc_links)]
pub mod doc;
mod error;
mod escapes;
#[doc(hidden)]
pub mod fanout_probe;
mod hash;
#[cfg(feature = "json")]
mod json_writer;
mod location;
mod output;
#[doc(hidden)]
// See `doc` above.
#[expect(rustdoc::private_intra_doc_links)]
pub mod printing;
mod sizing;
#[doc(hidden)]
// See `doc` above.
#[expect(rustdoc::private_intra_doc_links)]
pub mod source_scan;
mod span;
mod stack;
#[doc(hidden)]
// See `doc` above.
#[expect(rustdoc::private_intra_doc_links)]
pub mod swar;
mod whitespace;

#[doc(hidden)]
pub use comment::{
    ClassifiedComments, Comment, CommentFreeWindow, CommentPosition, GapTrail, GapTrailScan,
    classify_comment, classify_comment_scan, comments_in_source_after,
    comments_in_source_after_comment, comments_in_source_from, comments_in_source_range,
    comments_on_page_from, comments_on_page_in_range, comments_to_emit_after,
    comments_to_emit_from, comments_to_emit_in_range, directive_alone_on_line,
    find_first_comment_from, has_comments_on_page_from, has_comments_on_page_in_range,
    has_comments_to_emit_from, has_comments_to_emit_in_range, has_line_comments_from,
    has_line_comments_in_range, has_line_spanning_comments_to_emit_from,
    has_line_spanning_comments_to_emit_in_range, has_multiline_block_comments_on_page_from,
    has_multiline_block_comments_on_page_in_range, is_format_ignore_directive,
    is_format_ignore_range_end, is_format_ignore_range_start, is_honored_format_ignore,
    is_indentable_block, is_region_end_marker, is_tree_shaking_annotation,
    merge_nestled_block_comments, nestled_run_start, owned_leading_comment_at,
    range_too_narrow_for_a_comment,
};
#[doc(hidden)]
pub use config::{EmbedContext, INDENT, LayoutMode, PRINT_WIDTH, TAB_WIDTH};
#[doc(hidden)]
pub use depth::{
    DepthBudget, DepthLimit, NESTING_FLOOR, NESTING_LIMIT, NESTING_SPINE_WEIGHT, SPINE_LIMIT,
    SpineCount,
};
pub use error::{Error, ErrorKind, ErrorReport, Result};
#[doc(hidden)]
pub use hash::{FxHashMap, FxHashSet, FxHasher};
#[cfg(feature = "json")]
#[doc(hidden)]
pub use json_writer::{JsonWriter, StageRun, write_array, write_or_null};
#[doc(hidden)]
pub use location::{
    BOM, LocationMapper, Position, Wire, WirePositions, WireTables, leading_bom_len,
};
pub use location::{LeadingBom, LineRule, WireCoordinates, WirePoint};
#[doc(hidden)]
pub use output::{OutputBuffer, write_indent};
#[doc(hidden)]
pub use sizing::{estimated_ast_arena_capacity, estimated_json_capacity};
#[doc(hidden)]
pub use span::Span;
pub use stack::{STACK_SIZE, run_with_stack};
#[doc(hidden)]
pub use whitespace::{
    is_js_whitespace, trim_end_js_whitespace, trim_js_whitespace, trim_start_js_whitespace,
};
