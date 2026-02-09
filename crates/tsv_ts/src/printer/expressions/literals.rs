// Literal value printing for TypeScript
//
// This module handles all primitive value types:
// - Numbers (with normalization: hex lowercase, scientific notation, etc.)
// - Strings (quote selection and escaping)
// - Booleans, null, undefined
// - Identifiers (with optional markers and type annotations)
// - Regex literals
// - Spread elements

use super::Printer;
use crate::ast::internal::{self, LiteralValue};
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

/// Format a string literal from the AST to its printed form.
///
/// Extracts the raw string from source, strips quotes, and formats it
/// according to the literal's quote style.
pub(crate) fn format_string_literal_from_ast(literal: &internal::Literal, source: &str) -> String {
    let raw_literal = literal.span.extract(source);
    let raw_content = &raw_literal[1..raw_literal.len() - 1];

    let quote = match &literal.value {
        LiteralValue::String { quote, .. } => *quote,
        _ => unreachable!("format_string_literal_from_ast called on non-string literal"),
    };

    format_string_literal(raw_content, quote, StringFormatOptions::default())
}

/// Normalize a number literal to match Prettier's output format.
///
/// Transformations:
/// - Hex to lowercase: `0xFF` → `0xff`
/// - Scientific notation to lowercase without `+`: `2E+10` → `2e10`
/// - Leading decimal gets zero: `.5` → `0.5`
/// - Trailing decimal removed: `5.` → `5`
/// - BigInt hex to lowercase: `0xFFn` → `0xffn`
/// - Numeric separators preserved
pub fn normalize_number_literal(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();

    // Handle leading decimal: .5 → 0.5
    if chars.first() == Some(&'.') {
        result.push('0');
        result.push_str(raw);
        return result;
    }

    // Check for BigInt suffix
    let is_bigint = chars.last() == Some(&'n');
    let num_end = if is_bigint { len - 1 } else { len };

    // Check for trailing decimal: 5. → 5
    if num_end > 0 && chars[num_end - 1] == '.' {
        // Copy everything except the trailing decimal
        for &c in &chars[..num_end - 1] {
            result.push(c.to_ascii_lowercase());
        }
        if is_bigint {
            result.push('n');
        }
        return result;
    }

    // Process the number, lowercasing hex digits and 'e'/'E', removing '+' after 'e'
    let mut i = 0;
    while i < num_end {
        let c = chars[i];
        if c == 'E' {
            result.push('e');
            // Skip '+' after e/E if present
            if i + 1 < num_end && chars[i + 1] == '+' {
                i += 1;
            }
        } else {
            result.push(c.to_ascii_lowercase());
        }
        i += 1;
    }

    if is_bigint {
        result.push('n');
    }

    result
}

/// Sort regex flags alphabetically to match Prettier's output format.
///
/// Prettier normalizes regex flags to alphabetical order (dgimsvy).
/// Example: `/pattern/vg` → `/pattern/gv`
pub fn sort_regex_flags(flags: &str) -> String {
    let mut chars: Vec<char> = flags.chars().collect();
    chars.sort_unstable();
    chars.into_iter().collect()
}

impl<'a> Printer<'a> {
    /// Build a Doc for a literal
    pub(in crate::printer) fn build_literal_doc(&self, literal: &internal::Literal) -> DocId {
        let d = self.d();
        match &literal.value {
            LiteralValue::Number(_) => {
                // Extract raw literal and normalize it
                let raw = literal.span.extract(self.source);
                d.text_owned(normalize_number_literal(raw))
            }
            LiteralValue::String { .. } => {
                d.text_owned(format_string_literal_from_ast(literal, self.source))
            }
            LiteralValue::BigInt(_) => {
                // Extract raw literal and normalize it (lowercases hex digits)
                let raw = literal.span.extract(self.source);
                d.text_owned(normalize_number_literal(raw))
            }
            LiteralValue::Boolean(b) => d.text(if *b { "true" } else { "false" }),
            LiteralValue::Null => d.text("null"),
            LiteralValue::Undefined => d.text("undefined"),
        }
    }

    /// Build a Doc for a private identifier
    pub(super) fn build_private_identifier_doc(&self, pid: &internal::PrivateIdentifier) -> DocId {
        let d = self.d();
        d.concat(&[d.text("#"), d.symbol(pid.name.to_u32())])
    }

    /// Build a Doc for an identifier
    pub(in crate::printer) fn build_identifier_doc(&self, id: &internal::Identifier) -> DocId {
        self.build_identifier_doc_inner(id, false)
    }

    /// Build a Doc for an identifier with wrapping type arguments.
    ///
    /// Used in variable declarations where TypeReference type arguments should
    /// break internally (e.g., `let x: Map<LongA, LongB>` breaks inside `<>`).
    pub(in crate::printer) fn build_identifier_doc_with_wrapping_type(
        &self,
        id: &internal::Identifier,
    ) -> DocId {
        self.build_identifier_doc_inner(id, true)
    }

    /// Inner implementation for identifier doc building.
    fn build_identifier_doc_inner(&self, id: &internal::Identifier, wrap_type_args: bool) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Handle decorators (for parameter decorators)
        if let Some(decorators) = &id.decorators {
            for decorator in decorators {
                parts.push(d.text("@"));
                parts.push(self.build_expression_doc(&decorator.expression));
                parts.push(d.text(" "));
            }
        }

        // Add identifier name
        parts.push(d.symbol(id.name.to_u32()));

        // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`)
        if id.optional {
            parts.push(d.text("?"));
        }

        // Handle type annotations
        if let Some(type_annotation) = &id.type_annotation {
            if wrap_type_args {
                parts.push(self.build_type_annotation_doc_wrapping(type_annotation));
            } else {
                parts.push(self.build_type_annotation_doc(type_annotation));
            }
        }

        // Optimize for common case: single part (just the name)
        if parts.len() == 1 {
            parts[0]
        } else {
            d.concat(&parts)
        }
    }

    /// Build a Doc for a regex literal
    /// Flags are sorted alphabetically to match prettier's output.
    pub(super) fn build_regex_doc(&self, regex: &internal::RegexLiteral) -> DocId {
        self.d().text_owned(format!(
            "/{}/{}",
            regex.pattern,
            sort_regex_flags(&regex.flags)
        ))
    }

    /// Build a Doc for a spread element
    pub(in crate::printer) fn build_spread_doc(&self, spread: &internal::SpreadElement) -> DocId {
        let d = self.d();
        let needs_parens =
            super::needs_parens(&spread.argument, super::ParenContext::SpreadArgument);
        let arg_doc = self.build_expression_doc(&spread.argument);

        // Check for comments between `...` and the argument (e.g., `.../* comment */ arr`)
        // The `...` is 3 chars, so comment region starts at span.start + 3
        let dots_end = spread.span.start + 3;
        let arg_start = spread.argument.span().start;
        // Use trailing_space variant: `.../* comment */ arg` (space after comment, not before)
        let comment_doc =
            self.build_inline_comments_between_doc_trailing_space_opt(dots_end, arg_start);

        if needs_parens {
            match comment_doc {
                Some(c) => d.concat(&[d.text("...("), c, arg_doc, d.text(")")]),
                None => d.concat(&[d.text("...("), arg_doc, d.text(")")]),
            }
        } else {
            match comment_doc {
                Some(c) => d.concat(&[d.text("..."), c, arg_doc]),
                None => d.concat(&[d.text("..."), arg_doc]),
            }
        }
    }
}
