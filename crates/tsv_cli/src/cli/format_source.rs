//! The single in-process "format with our formatter" entry point.
//!
//! Shared by the production `format` command and `tsv_debug`'s tooling
//! (`compare`, `ast_diff`, fixture validation) so "ours" has exactly one
//! definition — a drift between what the CLI emits and what validation
//! checks would silently skew comparisons against prettier.
//!
//! # The source type is optional here
//!
//! Every entry point resolves to [`format_source_in`], whose
//! TypeScript goal is an `Option`. A named one is exact. An unnamed one — an unset
//! `--source-type` on `--content`/`--stdin`, every `tsv_debug` audit that calls
//! [`format_source`] — parses at `Module` and retries at `Script` only if that
//! *fails* (`tsv_ts::parse_with_goal_or_fallback`), so a legacy sloppy script formats
//! without anyone naming a grammar, and a module-valid source is never reinterpreted.
//! When both attempts fail the module's error is reported if the script retry died on a
//! goal gate (`import`/`export`/`import.meta`), else the one that reached further into the
//! source, the module's on a tie (`tsv_ts::parse_with_goal_or_fallback`).
//!
//! `tsv format <path>` names a goal without being told one: it reads the path's
//! extension (`tsv_ts::Goal::from_extension`), which settles the goal for `.mjs`/`.mts`
//! and nothing else. A file that is an ES module by name has no legacy sloppy script to
//! fall back to. `tsv_debug`'s named-path tools (`compare`, `ast_diff`) read the same
//! extension so "ours" there is what the CLI would emit on the file; the fixture-tree
//! audits deliberately do not (every seed takes the fallback — `docs/audits.md`).

use crate::cli::input::ParserType;

/// Parse and format `source` with our formatter, keyed by parser type.
///
/// Single-shot entry point (the `--content`/`--stdin` path, `tsv_debug` tooling):
/// allocates a fresh, source-pre-sized AST arena and a fresh doc arena per call. A
/// driver that formats many sources should reuse both across them via
/// [`format_source_in`] (see the `format` command's worker loop).
pub fn format_source(source: &str, parser_type: ParserType) -> Result<String, String> {
    format_source_with_source_type(source, parser_type, None)
}

/// [`format_source`] against a TypeScript source type that may be named.
///
/// `Some(goal)` is exact: `Goal::Module` is correct for Svelte and ~all real TS;
/// `Goal::Script` parses a standalone script, which is sloppy unless a `"use strict"`
/// directive prologue says otherwise — so a `with` statement and a leading-zero
/// numeric literal format only through that goal. `None` is the unnamed source type
/// that takes the module-then-script fallback. The goal is consulted only for the
/// `ParserType::TypeScript` arm (Svelte is always a module, CSS has no goal).
pub fn format_source_with_source_type(
    source: &str,
    parser_type: ParserType,
    goal: Option<tsv_ts::Goal>,
) -> Result<String, String> {
    // The arena owns the internal AST; it lives only for the parse+format here
    // (`format` returns an owned `String`, so nothing borrowed escapes). Pre-sized
    // to the source so the parse pays one chunk alloc, not a doubling tail.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    let doc_arena = tsv_lang::doc::arena::DocArena::for_source(source);
    format_source_in(source, parser_type, goal, &arena, &doc_arena)
}

/// Parse and format `source` into caller-provided arenas, against a TypeScript
/// source type that may be named. The shared implementation of every entry point in
/// this module.
///
/// The internal AST is bump-allocated into `arena` and the doc IR into
/// `doc_arena`, but nothing borrowed from either escapes — `format` returns an
/// owned `String` — so the caller may `arena.reset()` / `doc_arena.reset()` the
/// moment this returns and reuse both for the next source. This is what lets
/// `tsv format <dir>` keep one AST `Bump` and one `DocArena` per worker thread
/// (each retaining the largest chunk across files) instead of allocating fresh
/// arenas per file.
///
/// Crate-private: every caller outside reaches it through one of the two entry
/// points above.
pub(crate) fn format_source_in(
    source: &str,
    parser_type: ParserType,
    goal: Option<tsv_ts::Goal>,
    arena: &bumpalo::Bump,
    doc_arena: &tsv_lang::doc::arena::DocArena,
) -> Result<String, String> {
    // The format path's line-terminator fold, ahead of the parse — the seam every consumer
    // of this module inherits (the `format` command and every `tsv_debug` audit), so a
    // printer never sees a `<CR>` and the doc-build's line splits agree with the output
    // about where the lines are. See `tsv_lang::printing::normalize_carriage_returns`; the
    // `parse` command deliberately skips it, its offsets being a drop-in contract over the
    // author's own bytes. Borrowed unchanged on a source with no `<CR>`, and the fold's
    // pass is the document's line verdict too — the `format_folded_in` siblings take it
    // rather than walking the source again.
    let folded = tsv_lang::printing::normalize_carriage_returns(source);
    let source = folded.text();
    match parser_type {
        ParserType::Svelte => tsv_svelte::parse(source, arena)
            .map(|ast| tsv_svelte::format_folded_in(&ast, &folded, doc_arena))
            .map_err(|e| e.to_string()),
        ParserType::Css => tsv_css::parse(source, arena)
            .map(|ast| tsv_css::format_folded_in(&ast, &folded, doc_arena))
            .map_err(|e| e.to_string()),
        ParserType::TypeScript => tsv_ts::parse_with_goal_or_fallback(source, goal, arena)
            .map(|ast| tsv_ts::format_folded_in(&ast, &folded, doc_arena))
            .map_err(|e| e.to_string()),
    }
}
