// CSS value parsing
//
// Parses CSS values into structured AST (CssValue enum).
// Handles identifiers, strings, numbers/dimensions, colors, functions, and lists.

pub mod colors;
pub(crate) mod cursor;
pub mod dimensions;
pub mod functions;
pub mod lists;
pub(crate) mod parser;
pub(crate) mod scan;
pub mod strings;

use crate::ast::internal::CssValue;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;

// Re-export public functions
pub use colors::{parse_color, parse_color_function};
pub use dimensions::parse_dimension;
pub use functions::parse_function_arguments;
pub use strings::parse_string_literal;

// Note: classify_separators is used internally by ValueParser but not exported publicly

/// Parse a CSS value into a structured CssValue
///
/// Extracts the value directly from source using the provided span, then parses
/// it using ValueParser for accurate span tracking with same-source recursion.
///
/// This ensures that nested value spans are accurate even with multiline formatting,
/// since we're working with the actual source text rather than reconstructed tokens.
///
/// # Arguments
/// * `source` - The CSS source text (may be a substring of the full document)
/// * `source_relative_span` - The span of the value relative to `source` (positions within source)
/// * `base_offset` - Offset to add to spans for absolute positions in full document
pub fn parse_value_from_source<'arena>(
    source: &str,
    source_relative_span: Span,
    base_offset: u32,
    arena: &'arena Bump,
) -> CssValue<'arena> {
    let value_str = source_relative_span.extract(source);
    let absolute_start = base_offset + source_relative_span.start;

    // An all-whitespace (or empty) value keeps the span it was handed.
    let Some((trimmed, leading)) = locate_value(value_str) else {
        return CssValue::Identifier {
            span: Span {
                start: absolute_start,
                end: base_offset + source_relative_span.end,
            },
        };
    };

    // The value's own span: where it starts, plus how long it is. The end needs
    // no separate trailing-whitespace offset — a span that begins at the value
    // and runs its length already excludes what follows it.
    let start = absolute_start + leading as u32;
    let absolute_span = Span {
        start,
        end: start + trimmed.len() as u32,
    };

    // ValueParser re-parses the same source text, so nested value spans stay
    // accurate through its same-source recursion.
    parser::ValueParser::new(trimmed, absolute_span).parse(arena)
}

/// A value's text with its surrounding whitespace removed, and how many bytes of
/// that whitespace preceded it. `None` when the value is entirely whitespace.
///
/// The span a declaration hands over is, in practice, already trimmed — real
/// stylesheets do not put a whitespace byte at either end of a value — so the
/// common case answers from two byte comparisons and never walks the text. That
/// matters because `str::trim*` is Unicode-aware: it decodes a `char` and tests
/// `White_Space` at each end, and the offsets a naive shape recovers cost a
/// second and third walk to learn what the first already knew.
///
/// ⚠️ Only an **ASCII non-whitespace** byte at each end settles it. A non-ASCII
/// byte cannot (a multi-byte char's leading byte says nothing about whether that
/// char is `White_Space`), and an ASCII whitespace byte means there is something
/// to trim; both fall through to the trimming path. The test is
/// `char::is_whitespace`, **not** `u8::is_ascii_whitespace` — the two disagree on
/// the **vertical tab**, and `trim` uses the former.
fn locate_value(value_str: &str) -> Option<(&str, usize)> {
    let bytes = value_str.as_bytes();
    let settled = |b: u8| b.is_ascii() && !char::from(b).is_whitespace();
    if let (Some(&first), Some(&last)) = (bytes.first(), bytes.last())
        && settled(first)
        && settled(last)
    {
        return Some((value_str, 0));
    }

    let after_leading = value_str.trim_start();
    let trimmed = after_leading.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    Some((trimmed, value_str.len() - after_leading.len()))
}

// Old parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_value_string() → use parse_value_from_source() instead
// - parse_value_or_list() → handled internally by ValueParser
// See: parser::ValueParser for the new implementation

/// Parse a single CSS value (no lists).
///
/// `s` is expected already trimmed: the sole caller (`ValueParser::build_leaf`)
/// forwards a trimmed range — the fast path passes `self.text()` (the boundary
/// check confirmed it is trimmed) and the two-pass fallback passes
/// `self.text().trim()` — so no `str::trim` is repeated here.
pub(crate) fn parse_single_value<'arena>(
    s: &str,
    span: Span,
    arena: &'arena Bump,
) -> Option<CssValue<'arena>> {
    if s.is_empty() {
        return None;
    }

    // String literal
    if let Some(val) = parse_string_literal(s, span, arena) {
        return Some(val);
    }

    // Function call or color function. Byte-position scan for the ASCII `(` — a
    // hot per-value-token path where `str::find(char)`'s CharSearcher state machine
    // outweighs a direct byte loop (equivalent: `(` is ASCII, self-synchronising).
    if let Some(paren_pos) = s.as_bytes().iter().position(|&b| b == b'(')
        && let Some((name, args, true)) = extract_function_parts(s, paren_pos)
    {
        // Try color function first
        if let Some(color) = parse_color_function(name, args) {
            return Some(CssValue::Color { color, span });
        }
        // Fall back to generic function
        // Calculate accurate span for arguments (inside parens)
        // The args string starts at: paren_pos + 1 (after opening paren)
        // The args string ends at: paren_pos + 1 + args.len()
        let args_start = paren_pos + 1;
        let args_span = Span {
            start: span.start + args_start as u32,
            end: span.start + args_start as u32 + args.len() as u32,
        };
        let parsed_args = parse_function_arguments(args, args_span, arena);
        // var()'s empty fallback (`var(--a,)`) is significant: per css-variables-1 the
        // trailing comma with an empty `<declaration-value>` substitutes nothing when the
        // variable is unset, distinct from `var(--a)`. The generic comma parser drops empty
        // elements, so restore the empty trailing fallback for var() specifically — other
        // functions (`rgb(0,0,0,)`, `min(1px,)`) correctly drop it, matching prettier.
        let final_args = if name.eq_ignore_ascii_case("var") && args.trim_end().ends_with(',') {
            let mut v = BumpVec::new_in(arena);
            v.extend(parsed_args.iter().cloned());
            v.push(CssValue::Identifier {
                span: Span {
                    start: args_span.end,
                    end: args_span.end,
                },
            });
            v.into_bump_slice()
        } else {
            parsed_args
        };
        return Some(CssValue::Function {
            name: arena.alloc_str(name),
            args: final_args,
            span,
        });
    }

    // Hex or named color
    if let Some(color) = parse_color(s) {
        return Some(CssValue::Color { color, span });
    }

    // Dimension (number with optional unit)
    if let Some(dim) = parse_dimension(s, span) {
        return Some(dim);
    }

    // Default to identifier (text recovered from `span` at print time)
    Some(CssValue::Identifier { span })
}

/// Extract function name and arguments, validating balanced parentheses.
/// Both returned strings borrow from `s` (the caller copies `name` into the
/// arena when storing; `args` is re-parsed, not stored).
fn extract_function_parts(s: &str, paren_pos: usize) -> Option<(&str, &str, bool)> {
    let name_part = s[..paren_pos].trim();

    // Validate function name: alphanumeric, hyphens, underscores only
    if name_part.is_empty()
        || !name_part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    // Find matching closing paren
    let mut paren_count = 0;
    let mut closing_paren_pos = None;

    // Byte scan: `(` / `)` are ASCII, so no UTF-8 lead/continuation byte collides with
    // them — the matching-paren offset is the same as a char scan, without decoding.
    for (i, &b) in s.as_bytes()[paren_pos..].iter().enumerate() {
        match b {
            b'(' => paren_count += 1,
            b')' => {
                paren_count -= 1;
                if paren_count == 0 {
                    closing_paren_pos = Some(paren_pos + i);
                    break;
                }
            }
            _ => {}
        }
    }

    // Closing paren must be at end of string
    let close_pos = closing_paren_pos?;
    if close_pos != s.len() - 1 {
        return None;
    }

    let args = &s[paren_pos + 1..close_pos];
    Some((name_part, args, true))
}

#[cfg(test)]
mod value_span_tests {
    use super::parse_value_from_source;
    use bumpalo::Bump;
    use tsv_lang::Span;

    /// The span `parse_value_from_source` gives the value it parsed.
    fn value_span(source: &str, start: u32, end: u32) -> Span {
        let arena = Bump::new();
        parse_value_from_source(source, Span { start, end }, 0, &arena).span()
    }

    /// The already-trimmed fast path must agree with the trimming path on where
    /// the value starts and ends. No corpus can grade this: real declaration
    /// spans arrive pre-trimmed (200K+ of them without one whitespace byte at
    /// either end), so the trimming path is only ever reached by inputs a
    /// stylesheet does not contain — and a span error inside it would sail
    /// through every fixture and corpus diff in the repo.
    #[test]
    fn trims_the_span_the_same_either_way() {
        // Pre-trimmed (the fast path) — the span comes back untouched.
        assert_eq!(value_span("red", 0, 3), Span { start: 0, end: 3 });
        assert_eq!(value_span("a red b", 2, 5), Span { start: 2, end: 5 });

        // Whitespace at either end must come off the span identically.
        for (source, span, want) in [
            (" red", (0, 4), (1, 4)),
            ("red ", (0, 4), (0, 3)),
            ("  red  ", (0, 7), (2, 5)),
            ("\tred\t", (0, 5), (1, 4)),
            ("\nred\n", (0, 5), (1, 4)),
            // The vertical tab is `White_Space` (so `trim` eats it) but NOT
            // `u8::is_ascii_whitespace` — asking the `char` is what keeps the
            // fast path's guard equivalent to the trim it stands in for.
            ("\x0bred\x0b", (0, 5), (1, 4)),
            ("\x0cred\x0c", (0, 5), (1, 4)),
        ] {
            assert_eq!(
                value_span(source, span.0, span.1),
                Span {
                    start: want.0,
                    end: want.1
                },
                "value span for {source:?}"
            );
        }
    }

    /// A non-ASCII boundary byte cannot settle the question (a lead byte says
    /// nothing about its char's `White_Space`), so it must fall through to the
    /// trimming path — where NBSP, which `char::is_whitespace` accepts, is
    /// trimmed, and a letter is not.
    #[test]
    fn non_ascii_boundaries_take_the_trimming_path() {
        assert_eq!(
            value_span("\u{a0}red\u{a0}", 0, 7),
            Span { start: 2, end: 5 }
        );
        assert_eq!(value_span("é", 0, 2), Span { start: 0, end: 2 });
    }
}
