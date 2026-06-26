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
/// allocates a fresh, source-pre-sized arena per call. A driver that formats many
/// sources should reuse one arena across them via [`format_source_in`] (see the
/// `format` command's worker loop).
pub fn format_source(source: &str, parser_type: ParserType) -> Result<String, String> {
    // The arena owns the internal AST; it lives only for the parse+format here
    // (`format` returns an owned `String`, so nothing borrowed escapes). Pre-sized
    // to the source so the parse pays one chunk alloc, not a doubling tail.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    format_source_in(source, parser_type, &arena)
}

/// Parse and format `source` into a caller-provided arena.
///
/// The internal AST is bump-allocated into `arena`, but nothing borrowed from it
/// escapes — `format` returns an owned `String` — so the caller may `arena.reset()`
/// the moment this returns and reuse the same `Bump` for the next source. This is
/// what lets `tsv format <dir>` keep one arena per worker thread (retaining the
/// largest chunk across files) instead of allocating a fresh arena per file.
pub fn format_source_in(
    source: &str,
    parser_type: ParserType,
    arena: &bumpalo::Bump,
) -> Result<String, String> {
    match parser_type {
        ParserType::Svelte => tsv_svelte::parse(source, arena)
            .map(|ast| tsv_svelte::format(&ast, source))
            .map_err(|e| e.to_string()),
        ParserType::Css => tsv_css::parse(source, arena)
            .map(|ast| tsv_css::format(&ast, source))
            .map_err(|e| e.to_string()),
        ParserType::TypeScript => tsv_ts::parse(source, arena)
            .map(|ast| tsv_ts::format(&ast, source))
            .map_err(|e| e.to_string()),
    }
}
