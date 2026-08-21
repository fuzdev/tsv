//! The single in-process "format with our formatter" entry point.
//!
//! Shared by the production `format` command and `tsv_debug`'s tooling
//! (`compare`, `ast_diff`, fixture validation) so "ours" has exactly one
//! definition — a drift between what the CLI emits and what validation
//! checks would silently skew comparisons against prettier.

use crate::cli::input::ParserType;

/// Parse and format `source` with our formatter, keyed by parser type.
///
/// Single-shot entry point (the `--content`/`--stdin` path, `tsv_debug` tooling):
/// allocates a fresh, source-pre-sized AST arena and a fresh doc arena per call. A
/// driver that formats many sources should reuse both across them via
/// [`format_source_in`] (see the `format` command's worker loop).
pub fn format_source(source: &str, parser_type: ParserType) -> Result<String, String> {
    format_source_with_goal(source, parser_type, tsv_ts::Goal::Module)
}

/// [`format_source`] against an explicit TypeScript parse [`Goal`](tsv_ts::Goal).
///
/// `Goal::Module` (via [`format_source`]) is correct for Svelte and ~all real
/// TS; `Goal::Script` parses a standalone strict script. The goal is consulted
/// only for the `ParserType::TypeScript` arm (Svelte is always a module, CSS
/// has no goal).
pub fn format_source_with_goal(
    source: &str,
    parser_type: ParserType,
    goal: tsv_ts::Goal,
) -> Result<String, String> {
    // The arena owns the internal AST; it lives only for the parse+format here
    // (`format` returns an owned `String`, so nothing borrowed escapes). Pre-sized
    // to the source so the parse pays one chunk alloc, not a doubling tail.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let doc_arena = tsv_lang::doc::arena::DocArena::for_source(source);
    format_source_in_with_goal(source, parser_type, goal, &arena, &doc_arena)
}

/// Parse and format `source` into caller-provided arenas.
///
/// The internal AST is bump-allocated into `arena` and the doc IR into
/// `doc_arena`, but nothing borrowed from either escapes — `format` returns an
/// owned `String` — so the caller may `arena.reset()` / `doc_arena.reset()` the
/// moment this returns and reuse both for the next source. This is what lets
/// `tsv format <dir>` keep one AST `Bump` and one `DocArena` per worker thread
/// (each retaining the largest chunk across files) instead of allocating fresh
/// arenas per file.
pub fn format_source_in(
    source: &str,
    parser_type: ParserType,
    arena: &bumpalo::Bump,
    doc_arena: &tsv_lang::doc::arena::DocArena,
) -> Result<String, String> {
    format_source_in_with_goal(source, parser_type, tsv_ts::Goal::Module, arena, doc_arena)
}

/// [`format_source_in`] against an explicit TypeScript parse [`Goal`](tsv_ts::Goal).
/// The shared implementation; `format_source_in` is the `Goal::Module` form.
pub fn format_source_in_with_goal(
    source: &str,
    parser_type: ParserType,
    goal: tsv_ts::Goal,
    arena: &bumpalo::Bump,
    doc_arena: &tsv_lang::doc::arena::DocArena,
) -> Result<String, String> {
    // The format path's line-terminator fold, ahead of the parse — the seam every consumer
    // of this module inherits (the `format` command and every `tsv_debug` audit), so a
    // printer never sees a `<CR>` and the doc-build's line splits agree with the output
    // about where the lines are. See `tsv_lang::printing::normalize_carriage_returns`; the
    // `parse` command deliberately skips it, its offsets being a drop-in contract over the
    // author's own bytes. Borrowed unchanged on a source with no `<CR>`.
    let source = tsv_lang::printing::normalize_carriage_returns(source);
    let source = source.as_ref();
    match parser_type {
        ParserType::Svelte => tsv_svelte::parse(source, arena)
            .map(|ast| tsv_svelte::format_in(&ast, source, doc_arena))
            .map_err(|e| e.to_string()),
        ParserType::Css => tsv_css::parse(source, arena)
            .map(|ast| tsv_css::format_in(&ast, source, doc_arena))
            .map_err(|e| e.to_string()),
        ParserType::TypeScript => tsv_ts::parse_with_goal(source, goal, arena)
            .map(|ast| tsv_ts::format_in(&ast, source, doc_arena))
            .map_err(|e| e.to_string()),
    }
}
