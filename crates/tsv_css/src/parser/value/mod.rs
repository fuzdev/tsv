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
pub mod spacing;
pub mod strings;

use crate::ast::internal::CssValue;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;

// Re-export public functions
pub use colors::{parse_color, parse_color_function};
pub use dimensions::parse_dimension;
pub use functions::parse_function_arguments;
pub use spacing::should_add_space_between;
pub use strings::parse_string_literal;

// Note: contains_space_separator and contains_comma are used internally by ValueParser
// but not exported publicly

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
    // Extract value directly from source using source-relative positions
    let value_str = source_relative_span.extract(source);
    let trimmed = value_str.trim();

    if trimmed.is_empty() {
        return CssValue::Identifier {
            span: Span {
                start: base_offset + source_relative_span.start,
                end: base_offset + source_relative_span.end,
            },
        };
    }

    // Calculate adjusted span for trimmed value (relative to source)
    let trim_start_offset = value_str.len() - value_str.trim_start().len();
    let trim_end_offset = value_str.len() - value_str.trim_end().len();
    let source_relative_adjusted = Span {
        start: source_relative_span.start + trim_start_offset as u32,
        end: source_relative_span.end - trim_end_offset as u32,
    };

    // Calculate absolute span for ValueParser (includes base_offset)
    let absolute_span = Span {
        start: base_offset + source_relative_adjusted.start,
        end: base_offset + source_relative_adjusted.end,
    };

    // Use ValueParser for accurate span tracking through same-source recursion
    let parser = parser::ValueParser::new(trimmed, absolute_span);
    parser.parse(arena)
}

// Old parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_value_string() → use parse_value_from_source() instead
// - parse_value_or_list() → handled internally by ValueParser
// See: parser::ValueParser for the new implementation

/// Parse a single CSS value (no lists)
pub(crate) fn parse_single_value<'arena>(
    s: &str,
    span: Span,
    arena: &'arena Bump,
) -> Option<CssValue<'arena>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // String literal
    if let Some(val) = parse_string_literal(s, span, arena) {
        return Some(val);
    }

    // Function call or color function
    if let Some(paren_pos) = s.find('(')
        && let Some((name, args, true)) = extract_function_parts(s, paren_pos)
    {
        // Try color function first
        if let Some(color) = parse_color_function(&name.to_lowercase(), args) {
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

    for (i, ch) in s[paren_pos..].char_indices() {
        match ch {
            '(' => paren_count += 1,
            ')' => {
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
