// Literal value printing for TypeScript
//
// This module handles all primitive value types:
// - Numbers (with normalization: hex lowercase, scientific notation, etc.)
// - Strings (quote selection and escaping)
// - Booleans, null, undefined
// - Identifiers (with optional markers and type annotations)
// - Regex literals
// - Spread elements

use super::super::Printer;
use crate::ast::internal::{self, LiteralValue};
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

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
    /// Print a literal value
    pub(in crate::printer) fn print_literal(&mut self, literal: &internal::Literal) {
        match &literal.value {
            LiteralValue::Number(_) => {
                // Extract raw literal and normalize it
                let raw = literal.span.extract(self.source);
                let normalized = normalize_number_literal(raw);
                self.write(&normalized);
            }
            LiteralValue::String { content: _, quote } => {
                // Extract raw literal from source (preserves escape sequences)
                let raw_literal = literal.span.extract(self.source);

                // Extract content without surrounding quotes
                let raw_content = &raw_literal[1..raw_literal.len() - 1];

                // Format using shared utility (handles quote selection and escaping)
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());

                self.write(&formatted);
            }
            LiteralValue::BigInt(_) => {
                // Extract raw literal and normalize it (lowercases hex digits)
                let raw = literal.span.extract(self.source);
                self.write(&normalize_number_literal(raw));
            }
            LiteralValue::Boolean(b) => {
                self.write(if *b { "true" } else { "false" });
            }
            LiteralValue::Null => {
                self.write("null");
            }
            LiteralValue::Undefined => {
                self.write("undefined");
            }
        }
    }

    /// Build a Doc for a literal
    pub(in crate::printer) fn build_literal_doc(&self, literal: &internal::Literal) -> Doc {
        match &literal.value {
            LiteralValue::Number(_) => {
                // Extract raw literal and normalize it
                let raw = literal.span.extract(self.source);
                doc::text_owned(normalize_number_literal(raw))
            }
            LiteralValue::String { content: _, quote } => {
                let raw_literal = literal.span.extract(self.source);
                let raw_content = &raw_literal[1..raw_literal.len() - 1];
                let formatted =
                    format_string_literal(raw_content, *quote, StringFormatOptions::default());
                doc::text_owned(formatted)
            }
            LiteralValue::BigInt(_) => {
                // Extract raw literal and normalize it (lowercases hex digits)
                let raw = literal.span.extract(self.source);
                doc::text_owned(normalize_number_literal(raw))
            }
            LiteralValue::Boolean(b) => doc::text(if *b { "true" } else { "false" }),
            LiteralValue::Null => doc::text("null"),
            LiteralValue::Undefined => doc::text("undefined"),
        }
    }

    /// Print an identifier
    pub(in crate::printer) fn print_identifier(&mut self, identifier: &internal::Identifier) {
        // Print decorators (for parameter decorators)
        if let Some(decorators) = &identifier.decorators {
            for decorator in decorators {
                self.write("@");
                self.print_expression(&decorator.expression);
                self.write(" ");
            }
        }

        // Resolve symbol from interner using centralized helper
        let name = self.resolve_symbol(identifier.name);
        self.write(&name);

        // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`)
        if identifier.optional {
            self.write("?");
        }

        // Handle type annotations
        if let Some(type_annotation) = &identifier.type_annotation {
            self.print_type_annotation(type_annotation);
        }
    }

    /// Print a private identifier: `#name`
    pub(in crate::printer) fn print_private_identifier(
        &mut self,
        pid: &internal::PrivateIdentifier,
    ) {
        self.write("#");
        let name = self.resolve_symbol(pid.name);
        self.write(&name);
    }

    /// Build a Doc for a private identifier
    pub(super) fn build_private_identifier_doc(&self, pid: &internal::PrivateIdentifier) -> Doc {
        let name = self.resolve_symbol(pid.name);
        doc::concat(vec![doc::text("#"), doc::text_owned(name)])
    }

    /// Build a Doc for an identifier
    pub(in crate::printer) fn build_identifier_doc(&self, id: &internal::Identifier) -> Doc {
        let mut parts = Vec::new();

        // Handle decorators (for parameter decorators)
        if let Some(decorators) = &id.decorators {
            for decorator in decorators {
                parts.push(doc::text("@"));
                parts.push(self.build_expression_doc(&decorator.expression));
                parts.push(doc::text(" "));
            }
        }

        // Add identifier name
        let name = self.resolve_symbol(id.name);
        parts.push(doc::text_owned(name));

        // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`)
        if id.optional {
            parts.push(doc::text("?"));
        }

        // Handle type annotations
        if let Some(type_annotation) = &id.type_annotation {
            parts.push(self.build_type_annotation_doc(type_annotation));
        }

        // Optimize for common case: single part (just the name)
        match &parts[..] {
            [single] => single.clone(),
            _ => doc::concat(parts),
        }
    }

    /// Print a regex literal: /pattern/flags
    /// Flags are sorted alphabetically to match prettier's output.
    pub(super) fn print_regex_literal(&mut self, regex: &internal::RegexLiteral) {
        self.write("/");
        self.write(&regex.pattern);
        self.write("/");
        self.write(&sort_regex_flags(&regex.flags));
    }

    /// Build a Doc for a regex literal
    /// Flags are sorted alphabetically to match prettier's output.
    pub(super) fn build_regex_doc(&self, regex: &internal::RegexLiteral) -> Doc {
        doc::text_owned(format!(
            "/{}/{}",
            regex.pattern,
            sort_regex_flags(&regex.flags)
        ))
    }

    /// Print a spread element
    pub(super) fn print_spread_element(&mut self, spread: &internal::SpreadElement) {
        self.write("...");
        self.print_expression(&spread.argument);
    }

    /// Build a Doc for a spread element
    pub(in crate::printer) fn build_spread_doc(&self, spread: &internal::SpreadElement) -> Doc {
        doc::concat(vec![
            doc::text("..."),
            self.build_expression_doc(&spread.argument),
        ])
    }
}
