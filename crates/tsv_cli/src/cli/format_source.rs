//! The single in-process "format with our formatter" entry point.
//!
//! Shared by the production `format` command and `tsv_debug`'s tooling
//! (`compare`, `ast_diff`, fixture validation) so "ours" has exactly one
//! definition — a drift between what the CLI emits and what validation
//! checks would silently skew comparisons against prettier.

use crate::cli::input::ParserType;

/// Parse and format `source` with our formatter, keyed by parser type.
pub fn format_source(source: &str, parser_type: ParserType) -> Result<String, String> {
    match parser_type {
        ParserType::Svelte => tsv_svelte::parse(source)
            .map(|ast| tsv_svelte::format(&ast, source))
            .map_err(|e| e.to_string()),
        ParserType::Css => tsv_css::parse(source)
            .map(|ast| tsv_css::format(&ast, source))
            .map_err(|e| e.to_string()),
        ParserType::TypeScript => tsv_ts::parse(source)
            .map(|ast| tsv_ts::format(&ast, source))
            .map_err(|e| e.to_string()),
    }
}
