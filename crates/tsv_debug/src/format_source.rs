//! "Format with tsv", keyed by parser type: the one spelling every tool here formats through.
//!
//! The format itself is each language crate's public one-shot `format` (`tsv_ts::format` /
//! `format_with_goal`, `tsv_css::format`, `tsv_svelte::format`) — the same parse and print
//! the `tsv format` CLI runs through each language's `Formatter`, which shares the one-shot
//! function's body. This module only dispatches on [`ParserType`], so `compare`, `ast_diff`,
//! fixture validation and every audit agree on what "ours" is. The CLI keeps its own copy
//! of this dispatch (`commands/format.rs`), so a change to which function or goal a
//! `ParserType` reaches belongs in both.
//!
//! # The source type is optional here
//!
//! A named TypeScript goal is exact. An unnamed one — every audit that calls
//! [`format_source`] — parses at `Module` and retries at `Script` only if that *fails*, so
//! a legacy sloppy script formats without anyone naming a grammar, and a module-valid
//! source is never reinterpreted. Which error a source both grammars reject reports is
//! stated once, in `tsv_ts`'s crate docs (§Goals).
//!
//! `tsv format <path>` names a goal without being told one: it reads the path's extension
//! (`tsv_ts::Goal::from_extension`), which settles the goal for `.mjs`/`.mts` and nothing
//! else. The named-path tools here (`compare`, `ast_diff`) read the same extension so
//! "ours" is what the CLI would emit on the file; the fixture-tree audits deliberately do
//! not (every seed takes the fallback — `docs/audits.md`).

use tsv_cli::cli::input::ParserType;

/// Parse and format `source` with tsv's formatter, keyed by parser type, with no
/// TypeScript source type named.
///
/// # Errors
///
/// The language's format error: a parse error (for TypeScript, under both goals), the
/// size cap, or a format refusal.
pub fn format_source(source: &str, parser_type: ParserType) -> Result<String, tsv_lang::Error> {
    format_source_with_source_type(source, parser_type, None)
}

/// [`format_source`] against a TypeScript source type that may be named.
///
/// `Some(goal)` is exact: `Goal::Module` is correct for Svelte and ~all real TS;
/// `Goal::Script` parses a standalone script, which is sloppy unless a `"use strict"`
/// directive prologue says otherwise — so a `with` statement and a leading-zero numeric
/// literal format only through that goal. `None` is the unnamed source type that takes the
/// module-then-script fallback. The goal is consulted only for the
/// `ParserType::TypeScript` arm (Svelte is always a module, CSS has no goal).
///
/// # Errors
///
/// As [`format_source`], a TypeScript parse error at the named goal alone.
pub fn format_source_with_source_type(
    source: &str,
    parser_type: ParserType,
    goal: Option<tsv_ts::Goal>,
) -> Result<String, tsv_lang::Error> {
    match parser_type {
        ParserType::Svelte => tsv_svelte::format(source),
        ParserType::Css => tsv_css::format(source),
        ParserType::TypeScript => match goal {
            Some(goal) => tsv_ts::format_with_goal(source, goal),
            None => tsv_ts::format(source),
        },
    }
}
