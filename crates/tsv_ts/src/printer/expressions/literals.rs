// Literal value printing for TypeScript
//
// This module handles all primitive value types:
// - Numbers (with normalization: hex lowercase, scientific notation, etc.)
// - Strings (quote selection and escaping)
// - Booleans, null, undefined
// - Identifiers (with optional markers and type annotations)
// - Regex literals
// - Spread elements

use crate::ast::internal::{self, LiteralValue};
use crate::printer::Printer;
use crate::printer::analysis;
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

/// Format a directive prologue string (`'use strict'`) for printing.
///
/// Mirrors Prettier's `printDirective` (`language-js/print/literal.js`), which is
/// deliberately distinct from `printString`: a directive is an exact code-unit
/// sequence, so its escapes are never re-encoded. Only the *outer* quote is
/// swapped to the preferred style (single) — and only when the content holds no
/// quote of either kind. If the content contains a `'` or `"`, the raw literal is
/// preserved verbatim (swapping would force re-escaping, changing the directive;
/// see prettier#1555).
///
/// `raw` is the literal *with* its surrounding quotes (the source slice).
pub(crate) fn format_directive(raw: &str) -> String {
    let content = &raw[1..raw.len() - 1];
    if content.contains('\'') || content.contains('"') {
        raw.to_string()
    } else {
        // Preferred quote is single (matches `StringFormatOptions::default`).
        format!("'{content}'")
    }
}

/// Normalize a number literal to match Prettier's output format.
///
/// Mirrors Prettier's `printNumber` (`src/utilities/print-number.js`) so that
/// numerically-equal literals collapse to one canonical spelling:
/// - Lowercase everything: `0xFF` → `0xff`, `2E10` → `2e10`
/// - Strip `+` and leading zeros from the exponent: `1e+1` → `1e1`, `1.1e0010` → `1.1e10`
/// - Drop a zero exponent: `0.5e0` → `0.5`
/// - Leading decimal gets a zero: `.5` → `0.5`
/// - Strip trailing fractional zeros: `1.00500` → `1.005`
/// - Drop a trailing dot: `5.` → `5`, `1.e1` → `1e1`
///
/// BigInt literals are only lowercased (Prettier's `printBigInt`); the BigInt
/// grammar has no exponent or fraction, so the rest of the pipeline is inert.
pub fn normalize_number_literal(raw: &str) -> String {
    // Prettier short-circuits single-character literals (`0`, `1`).
    if raw.chars().count() == 1 {
        return raw.to_string();
    }

    // BigInt (`printBigInt`): lowercase only, suffix included (`0xFFn` → `0xffn`).
    if raw.ends_with('n') {
        return raw.to_ascii_lowercase();
    }

    print_number(raw)
}

/// Port of Prettier's `printNumber` regex pipeline (order matters).
fn print_number(raw: &str) -> String {
    let lowered = raw.to_ascii_lowercase();
    let s = strip_exponent_plus_and_zeros(&lowered);
    let s = strip_zero_exponent(&s);
    let s = ensure_leading_digit(&s);
    let s = strip_trailing_fraction_zeros(&s);
    strip_trailing_dot(&s)
}

/// `/^([+-]?[\d.]+e)(?:\+|(-))?0*(?=\d)/` → `$1$2`
/// Removes a `+` and any leading zeros from the exponent (keeps a `-`).
fn strip_exponent_plus_and_zeros(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    // Optional leading sign.
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    // `[\d.]+` (at least one digit-or-dot).
    let mantissa_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == mantissa_start || i >= bytes.len() || bytes[i] != b'e' {
        return s.to_string();
    }
    i += 1; // consume 'e'
    let after_e = i; // prefix `[+-]?[\d.]+e` ends here
    // Optional `+` (dropped) or `-` (kept).
    let mut sign = "";
    match bytes.get(i) {
        Some(b'+') => i += 1,
        Some(b'-') => {
            sign = "-";
            i += 1;
        }
        _ => {}
    }
    // Leading zeros, but the lookahead `(?=\d)` requires a following digit.
    while i < bytes.len() && bytes[i] == b'0' {
        i += 1;
    }
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return s.to_string();
    }
    // Rebuild: prefix through 'e', kept sign, then the remaining digits.
    format!("{}{}{}", &s[..after_e], sign, &s[i..])
}

/// `/^([+-]?[\d.]+)e[+-]?0+$/` → `$1`  (removes a whole zero exponent: `0.5e0` → `0.5`)
fn strip_zero_exponent(s: &str) -> String {
    let Some(e_idx) = s.find('e') else {
        return s.to_string();
    };
    let mantissa = &s[..e_idx];
    let exp = &s[e_idx + 1..];
    if mantissa.is_empty() {
        return s.to_string();
    }
    // mantissa must be `[+-]?[\d.]+`
    let m = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    if m.is_empty() || !m.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return s.to_string();
    }
    // exp must be `[+-]?0+`
    let e = exp.strip_prefix(['+', '-']).unwrap_or(exp);
    if !e.is_empty() && e.bytes().all(|b| b == b'0') {
        mantissa.to_string()
    } else {
        s.to_string()
    }
}

/// `/^([+-])?\./` → `$10.`  (`.5` → `0.5`, `-.5` → `-0.5`)
fn ensure_leading_digit(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('.') {
        format!("0.{rest}")
    } else if let Some(rest) = s.strip_prefix("+.") {
        format!("+0.{rest}")
    } else if let Some(rest) = s.strip_prefix("-.") {
        format!("-0.{rest}")
    } else {
        s.to_string()
    }
}

/// `/(\.\d+?)0+(?=e|$)/` → `$1`  (first match only; `1.00500` → `1.005`, `1.50` → `1.5`)
fn strip_trailing_fraction_zeros(s: &str) -> String {
    let Some(dot) = s.find('.') else {
        return s.to_string();
    };
    let bytes = s.as_bytes();
    // `\.\d+?` — need at least one digit after the dot.
    let mut i = dot + 1;
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return s.to_string();
    }
    // Non-greedy `\d+?` keeps the first digit, then we look for trailing zeros
    // that run up to `e` or end-of-string.
    // Find the end of the fractional digit run.
    let frac_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let frac_end = i; // first non-digit after fraction (could be 'e' or len)
    // Boundary must be `e` or end.
    if frac_end != bytes.len() && bytes[frac_end] != b'e' {
        return s.to_string();
    }
    // Strip trailing zeros from [frac_start, frac_end), keeping at least one digit.
    let mut keep = frac_end;
    while keep > frac_start + 1 && bytes[keep - 1] == b'0' {
        keep -= 1;
    }
    if keep == frac_end {
        return s.to_string();
    }
    format!("{}{}", &s[..keep], &s[frac_end..])
}

/// `/\.(?=e|$)/` → ``  (drop a trailing dot before `e` or end: `1.` → `1`, `1.e1` → `1e1`)
fn strip_trailing_dot(s: &str) -> String {
    let bytes = s.as_bytes();
    if let Some(dot) = s.find('.') {
        let after = dot + 1;
        if after == bytes.len() || bytes[after] == b'e' {
            return format!("{}{}", &s[..dot], &s[after..]);
        }
    }
    s.to_string()
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
                parts.push(self.build_decorator_expression_doc(decorator));
                parts.push(d.text(" "));
            }
        }

        // Add identifier name
        parts.push(d.symbol(id.name.to_u32()));

        // Compute name_end for comment extraction (used by optional and type annotation)
        let search_end = id
            .type_annotation
            .as_ref()
            .map_or(id.span.end, |ta| ta.span.start);
        let raw_name_end = analysis::skip_identifier_at(
            self.source.as_bytes(),
            id.span.start as usize,
            search_end as usize,
        ) as u32;

        // Handle optional marker (e.g., `a?` in `function fn(a?: number) {}`),
        // with comments between name and `?` (e.g., `a /* c */?`)
        let after_modifier = if id.optional {
            self.push_modifier_marker_doc(&mut parts, raw_name_end, b'?')
        } else {
            raw_name_end
        };

        // Handle type annotations
        if let Some(type_annotation) = &id.type_annotation {
            // Extract comments between name/modifier and `:` (e.g., `a /* c */: number`)
            if self.has_comments_between(after_modifier, type_annotation.span.start) {
                let comment_doc = self
                    .build_inline_comments_between_doc(after_modifier, type_annotation.span.start);
                if self.has_line_comments_between(after_modifier, type_annotation.span.start) {
                    // Line comment forces break: `a // c\n: number` → keep on separate line
                    parts.push(comment_doc);
                    parts.push(d.hardline());
                } else {
                    // Trailing space separates comment from `:` in the type annotation
                    parts.push(d.concat(&[comment_doc, d.text(" ")]));
                }
            }
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
        // A binaryish spread argument indents its continuation when it breaks
        // (`...(a &&\n\tb && {…})`), matching Prettier — a `SpreadElement` parent is
        // not in binaryish.js's `shouldNotIndent` set, so the logical/binary chain
        // gets the continuation indent. Non-binary arguments are unaffected.
        let arg_doc = self.build_expression_doc_with_indent_on_break(&spread.argument);

        // Check for comments between `...` and the argument (e.g., `.../* comment */ arr`)
        // The `...` is 3 chars, so comment region starts at span.start + 3
        let dots_end = spread.span.start + 3;
        let arg_start = spread.argument.span().start;
        // Use trailing_space variant: `.../* comment */ arg` (space after comment, not before)
        let comment_doc = self.build_rhs_comments_opt(dots_end, arg_start);

        // Check for trailing comments from stripped grouping parens: `...(x /* c */)`
        let argument_end = spread.argument.span().end;
        let has_trailing_comments = self.has_comments_between(argument_end, spread.span.end);

        let prefix = if needs_parens { "...(" } else { "..." };
        let mut parts = vec![d.text(prefix)];
        if let Some(c) = comment_doc {
            parts.push(c);
        }
        parts.push(arg_doc);
        if has_trailing_comments {
            // Handle same-line block comments and line comments here.
            // Own-line block comments are skipped — they're handled by the parent
            // (array/call) which places them as siblings after the spread's comma.
            // Using line_suffix for own-line block comments in spread causes them to
            // escape past the enclosing array/call brackets entirely.
            self.append_spread_trailing_paren_comments(&mut parts, argument_end, spread.span.end);
        }
        if needs_parens {
            parts.push(d.text(")"));
        }
        d.concat(&parts)
    }
}

/// Check if a string is a valid JS identifier (so prettier outputs it unquoted).
///
/// Built on the lexer's identifier grammar (`lexer::ident`) so any key we
/// unquote here can be re-lexed as an identifier (idempotency). Reserved words
/// count as identifiers here (prettier outputs them unquoted).
pub(in crate::printer) fn is_valid_js_identifier(s: &str) -> bool {
    use crate::lexer::ident::{is_id_continue, is_id_start};

    let mut chars = s.chars();

    match chars.next() {
        Some(c) if is_id_start(c) => {}
        _ => return false,
    }

    chars.all(is_id_continue)
}

#[cfg(test)]
mod tests {
    use super::format_directive as fd;
    use super::normalize_number_literal as norm;

    #[test]
    fn directive_swaps_outer_quote_to_single() {
        // Non-preferred (double) quote normalizes to single...
        assert_eq!(fd("\"use strict\""), "'use strict'");
        assert_eq!(fd("\"use asm\""), "'use asm'");
        // ...and a single-quoted directive is already canonical.
        assert_eq!(fd("'use strict'"), "'use strict'");
        // Inner escapes are preserved exactly — only the outer quote swaps.
        assert_eq!(fd("\"a\\nb\""), "'a\\nb'");
    }

    #[test]
    fn directive_kept_verbatim_when_content_has_a_quote() {
        // Content holds a `"`/`'` → verbatim (swapping would require re-escaping,
        // which would change an exact code-unit sequence).
        assert_eq!(fd("\"\\\"\""), "\"\\\"\""); // "\"" stays double
        assert_eq!(fd("'\\''"), "'\\''"); // '\'' stays single
        assert_eq!(fd("'a\"b'"), "'a\"b'"); // single-quoted, inner " → no swap
    }

    #[test]
    fn keeps_single_char_and_simple() {
        assert_eq!(norm("0"), "0");
        assert_eq!(norm("5"), "5");
        assert_eq!(norm("123"), "123");
        assert_eq!(norm("1.0"), "1.0"); // a lone fractional zero is kept
    }

    #[test]
    fn exponent_plus_and_leading_zeros() {
        assert_eq!(norm("2E+10"), "2e10");
        assert_eq!(norm("1e+1"), "1e1");
        assert_eq!(norm("1.1e0010"), "1.1e10");
        assert_eq!(norm("1e-05"), "1e-5"); // negative sign kept, zeros stripped
        assert_eq!(norm(".1e+0010"), "0.1e10");
    }

    #[test]
    fn zero_exponent_dropped() {
        assert_eq!(norm("0.5e0"), "0.5");
        assert_eq!(norm("1e0"), "1");
        assert_eq!(norm("1e-0"), "1");
    }

    #[test]
    fn leading_and_trailing_dot() {
        assert_eq!(norm(".5"), "0.5");
        assert_eq!(norm("-.5"), "-0.5");
        assert_eq!(norm("5."), "5");
        assert_eq!(norm("1.e1"), "1e1");
    }

    #[test]
    fn trailing_fraction_zeros() {
        assert_eq!(norm("1.00500"), "1.005");
        assert_eq!(norm("1.50"), "1.5");
        assert_eq!(norm("0.0000"), "0.0");
        assert_eq!(norm("500600.001230045000"), "500600.001230045");
    }

    #[test]
    fn radix_literals_only_lowercased() {
        assert_eq!(norm("0xFF"), "0xff");
        assert_eq!(norm("0xE5"), "0xe5"); // the 'e' is a hex digit, not an exponent
        assert_eq!(norm("0o17"), "0o17");
        assert_eq!(norm("0B101"), "0b101");
    }

    #[test]
    fn bigint_keeps_suffix() {
        assert_eq!(norm("100n"), "100n");
        assert_eq!(norm("0xFFn"), "0xffn");
        assert_eq!(norm("0x1Fn"), "0x1fn");
    }
}
