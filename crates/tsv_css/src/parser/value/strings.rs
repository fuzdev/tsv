use crate::ast::internal::CssValue;
use crate::escapes;
use tsv_lang::Span;

/// Parse CSS string with proper quote handling and escape decoding
///
/// Extracts content between quotes and decodes CSS escape sequences.
/// The internal AST stores fully decoded strings for semantic correctness.
///
/// # Examples
/// - `"test"` → content: `test`, quote: `"`
/// - `"test\\n"` → content: `test\n` (decoded newline), quote: `"`
/// - `"\\41"` → content: `A` (decoded unicode U+0041), quote: `"`
///
/// # Architecture
/// - Lexer: Preserves raw escape sequences exactly as written
/// - Parser: Decodes standard CSS escapes into clean internal AST
/// - Conversion: Re-applies Svelte quirks when generating public JSON AST
///
/// This matches TypeScript's architecture and keeps the internal AST clean.
pub fn parse_string_literal(s: &str, span: Span) -> Option<CssValue> {
    if let Some(quote) = s.chars().next()
        && ((quote == '"' && s.ends_with('"')) || (quote == '\'' && s.ends_with('\'')))
    {
        // Extract content without quotes
        let raw_content = &s[1..s.len() - 1];

        // Decode CSS escape sequences for semantic representation
        // Internal AST stores decoded values; conversion layer re-applies Svelte quirks
        let content = escapes::decode_escape_sequences(raw_content);

        return Some(CssValue::String {
            content,
            quote,
            span,
        });
    }
    None
}
