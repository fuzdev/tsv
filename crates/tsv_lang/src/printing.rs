// Shared printing utilities for printers
//
// This module provides common printing logic used across language printers
// (TypeScript, CSS, Svelte) to eliminate code duplication.

use crate::Span;
use crate::escapes::swap_quote_escaping;

/// Options for string literal formatting
#[derive(Debug, Clone, Copy)]
pub struct StringFormatOptions {
    /// Prefer single quotes over double quotes (when counts are equal)
    /// Default: true (matches prettier-plugin-svelte)
    pub prefer_single_quotes: bool,
}

impl Default for StringFormatOptions {
    fn default() -> Self {
        Self {
            prefer_single_quotes: true,
        }
    }
}

/// Format a string literal with optimal quote selection
///
/// Takes raw string content (with escape sequences preserved) and formats it
/// by choosing the optimal quote character to minimize escaping.
///
/// # Algorithm
///
/// 1. Count single and double quotes in the content
/// 2. Choose quote that appears less frequently (minimize escaping)
/// 3. On tie, prefer single quotes (prettier default)
/// 4. If quote changed, swap escape sequences
/// 5. Return formatted string with quotes
///
/// # Arguments
///
/// * `raw_content` - String content without surrounding quotes (with escapes preserved)
/// * `original_quote` - The quote character in the original source (`'` or `"`)
/// * `options` - Formatting options (quote preference)
///
/// # Returns
///
/// Formatted string literal including surrounding quotes
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::{format_string_literal, StringFormatOptions};
///
/// // String with no quotes - uses preferred quote (single)
/// let result = format_string_literal("hello", '"', StringFormatOptions::default());
/// assert_eq!(result, "'hello'");
///
/// // String with single quotes - switches to double to avoid escaping
/// let result = format_string_literal("it's nice", '\'', StringFormatOptions::default());
/// assert_eq!(result, r#""it's nice""#);
///
/// // String with double quotes - stays single to minimize escaping
/// let result = format_string_literal(r#"say "hi""#, '\'', StringFormatOptions::default());
/// assert_eq!(result, r#"'say "hi"'"#);
///
/// // Preserves escape sequences
/// let result = format_string_literal(r"\u0041\n", '"', StringFormatOptions::default());
/// assert_eq!(result, r"'\u0041\n'");
/// ```
pub fn format_string_literal(
    raw_content: &str,
    original_quote: char,
    options: StringFormatOptions,
) -> String {
    // Count quotes in the raw content (with escapes) to make the best choice
    let single_count = raw_content.matches('\'').count();
    let double_count = raw_content.matches('"').count();

    // Choose optimal quote: less frequent quote = less escaping needed
    let optimal_quote = if double_count < single_count {
        '"'
    } else if single_count < double_count {
        '\''
    } else {
        // Tie-breaker: use preference (default: single quotes)
        if options.prefer_single_quotes {
            '\''
        } else {
            '"'
        }
    };

    // Swap quote escaping if needed, or use raw content as-is
    let final_content = if optimal_quote == original_quote {
        raw_content.to_string()
    } else {
        swap_quote_escaping(raw_content, original_quote, optimal_quote)
    };

    // Return formatted string with quotes
    format!("{optimal_quote}{final_content}{optimal_quote}")
}

/// Check if two positions are on the same line (no newline between them)
///
/// Returns `true` if there is no newline character between `prev_end` and `curr_start`.
/// Adjacent positions (where `prev_end == curr_start`) are considered to be on the same line.
///
/// # Arguments
///
/// * `source` - The source text
/// * `prev_end` - End position of the first element
/// * `curr_start` - Start position of the second element
///
/// # Returns
///
/// `true` if the positions are on the same line, `false` otherwise.
/// Returns `false` if positions are invalid (out of order or out of bounds).
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::is_same_line;
///
/// let source = "foo\nbar";
/// assert_eq!(is_same_line(source, 0, 3), true);   // "foo" on same line
/// assert_eq!(is_same_line(source, 3, 4), false);  // crosses newline
/// assert_eq!(is_same_line(source, 4, 7), true);   // "bar" on same line
/// ```
pub fn is_same_line(source: &str, prev_end: u32, curr_start: u32) -> bool {
    let prev_end = prev_end as usize;
    let curr_start = curr_start as usize;

    // Adjacent tokens (no whitespace between them) are on the same line
    if prev_end == curr_start {
        return true;
    }

    // Validate positions are in order and within bounds
    if prev_end > curr_start || curr_start > source.len() {
        return false;
    }

    // Check if there's a newline between the positions
    let between = &source[prev_end..curr_start];
    !between.contains('\n')
}

/// Check if two spans are on the same line
///
/// This is a span-aware version of [`is_same_line`] that handles span ordering
/// and overlap detection. Spans can be provided in any order.
///
/// Returns `true` if:
/// - The spans overlap or touch (share a boundary)
/// - There is no newline between the end of the first span and start of the second
///
/// # Arguments
///
/// * `source` - The source text
/// * `span1` - First span
/// * `span2` - Second span
///
/// # Returns
///
/// `true` if the spans are on the same line, `false` otherwise.
/// Returns `false` if span positions are out of bounds.
///
/// # Examples
///
/// ```
/// use tsv_lang::{Span, printing::spans_on_same_line};
///
/// let source = "foo bar\nbaz";
/// let span1 = Span::new(0, 3);  // "foo"
/// let span2 = Span::new(4, 7);  // "bar"
/// let span3 = Span::new(8, 11); // "baz"
///
/// assert_eq!(spans_on_same_line(source, span1, span2), true);  // foo and bar
/// assert_eq!(spans_on_same_line(source, span1, span3), false); // crosses newline
/// assert_eq!(spans_on_same_line(source, span2, span1), true);  // order doesn't matter
/// ```
pub fn spans_on_same_line(source: &str, span1: Span, span2: Span) -> bool {
    // Determine which span comes first
    let (first, second) = if span1.start <= span2.start {
        (span1, span2)
    } else {
        (span2, span1)
    };

    // If spans overlap or touch, they're on the same line
    if first.end >= second.start {
        return true;
    }

    // Check if there's a newline between the spans
    is_same_line(source, first.end, second.start)
}

/// Check if there's a blank line (2+ newlines) between two positions
///
/// A blank line is defined as having 2 or more newline characters between the positions.
/// This is used to preserve source formatting when blank lines are significant.
///
/// # Arguments
///
/// * `source` - The source text
/// * `prev_end` - End position of the first element
/// * `curr_start` - Start position of the second element
///
/// # Returns
///
/// `true` if there are 2 or more newlines between the positions, `false` otherwise.
/// Returns `false` if positions are invalid (out of order or out of bounds).
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::has_blank_line_between;
///
/// let source = "foo\n\nbar";  // Two newlines = blank line
/// assert_eq!(has_blank_line_between(source, 3, 5), true);
///
/// let source2 = "foo\nbar";   // One newline = no blank line
/// assert_eq!(has_blank_line_between(source2, 3, 4), false);
/// ```
pub fn has_blank_line_between(source: &str, prev_end: u32, curr_start: u32) -> bool {
    let prev_end = prev_end as usize;
    let curr_start = curr_start as usize;

    // Validate positions are in order and within bounds
    if prev_end > curr_start || curr_start > source.len() {
        return false;
    }

    // Check if there are 2+ newlines (blank line) between the positions
    let between = &source[prev_end..curr_start];
    between.matches('\n').count() >= 2
}

/// Check if there's any newline between two positions in source
///
/// Used to detect source-triggered line breaks, e.g., newline after `{` in objects.
/// This is the key trigger for prettier's "source preservation" behavior where
/// objects expand to multiline when the source has a newline after opening brace.
///
/// # Arguments
///
/// * `source` - The source text
/// * `start` - Start position (e.g., after opening `{`)
/// * `end` - End position (e.g., start of first property)
///
/// # Returns
///
/// `true` if there's at least one newline between positions.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::has_newline_between;
///
/// let source = "{\na: 1}";
/// assert_eq!(has_newline_between(source, 1, 2), true);
///
/// let source2 = "{a: 1}";
/// assert_eq!(has_newline_between(source2, 1, 2), false);
/// ```
pub fn has_newline_between(source: &str, start: u32, end: u32) -> bool {
    let start = start as usize;
    let end = end as usize;

    if start > end || end > source.len() {
        return false;
    }

    source[start..end].contains('\n')
}

/// Check if a line ends with a JS/TypeScript string line continuation
///
/// A line continuation is a backslash (`\`) at the end of a line inside a string literal.
/// This causes the newline to be escaped, allowing the string to span multiple lines
/// in the source code without including the newline in the string value.
///
/// Example:
/// ```javascript
/// const s = 'hello \
/// world';  // value is "hello world"
/// ```
///
/// # Algorithm
///
/// Counts trailing backslashes - an odd number means line continuation,
/// an even number means escaped backslashes (not a continuation).
///
/// # Returns
///
/// `true` if the line ends with a line continuation (odd number of trailing backslashes).
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::is_line_continuation_ending;
///
/// assert!(is_line_continuation_ending("'hello \\"));      // Line continuation
/// assert!(is_line_continuation_ending("const x = 'a \\")); // Line continuation
/// assert!(!is_line_continuation_ending("'hello'"));       // Normal string end
/// assert!(!is_line_continuation_ending("'hello\\\\'"));   // Escaped backslash
/// assert!(!is_line_continuation_ending(""));              // Empty line
/// ```
pub fn is_line_continuation_ending(line: &str) -> bool {
    // Count trailing backslashes
    let mut backslash_count = 0;
    for c in line.chars().rev() {
        if c == '\\' {
            backslash_count += 1;
        } else {
            break;
        }
    }

    // Odd number of trailing backslashes = line continuation
    // Even number = escaped backslashes (\\) which is not a continuation
    backslash_count > 0 && backslash_count % 2 == 1
}

/// Strip common indentation from comment content based on its position in source
///
/// Detects the indentation level at the comment's position and removes that
/// same indentation from each line of the comment content. This is used when
/// formatting multi-line comments to preserve their internal structure while
/// removing the baseline indentation from the source code.
///
/// # Arguments
///
/// * `source` - The source text
/// * `content` - The comment content to process
/// * `comment_start` - The start position of the comment in the source
///
/// # Returns
///
/// The comment content with common indentation stripped from each line.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::strip_comment_indentation;
///
/// let source = "    /* Line 1\n       Line 2 */";
/// let content = " Line 1\n   Line 2 ";
/// let result = strip_comment_indentation(source, content, 4);
/// // Result: " Line 1\n   Line 2 " (4 spaces of indentation removed from each line)
/// ```
pub fn strip_comment_indentation(source: &str, content: &str, comment_start: u32) -> String {
    let comment_start = comment_start as usize;

    // Find start of line where comment begins
    let mut line_start = comment_start;
    while line_start > 0 && source.as_bytes()[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    // Find the indentation characters (spaces/tabs before the comment)
    let mut indentation_end = line_start;
    while indentation_end < source.len() {
        let ch = source.as_bytes()[indentation_end];
        if ch == b' ' || ch == b'\t' {
            indentation_end += 1;
        } else {
            break;
        }
    }

    let indentation = &source[line_start..indentation_end];

    // Strip this indentation from the start of each line in the comment
    if indentation.is_empty() {
        return content.to_string();
    }

    // Process line by line, stripping indentation from the start of each line
    let mut result = String::with_capacity(content.len());
    let line_iter = content.split_inclusive('\n');

    for line in line_iter {
        if let Some(stripped) = line.strip_prefix(indentation) {
            result.push_str(stripped);
        } else {
            result.push_str(line);
        }
    }

    result
}

/// Calculate the visual width of a string, treating tabs as `tab_width` columns.
///
/// This is useful for calculating line lengths when tabs may be present.
/// Each tab character contributes `tab_width` to the total, while all other
/// characters contribute 1.
///
/// # Example
/// ```
/// use tsv_lang::printing::visual_width;
///
/// assert_eq!(visual_width("hello", 2), 5);
/// assert_eq!(visual_width("\thello", 2), 7); // tab (2) + "hello" (5)
/// assert_eq!(visual_width("\thello", 4), 9); // tab (4) + "hello" (5)
/// ```
#[inline]
pub fn visual_width(s: &str, tab_width: usize) -> usize {
    s.chars()
        .map(|c| if c == '\t' { tab_width } else { 1 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_quotes_uses_preferred() {
        let result = format_string_literal("hello", '"', StringFormatOptions::default());
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_switches_to_minimize_escaping() {
        // Has single quote - switch to double
        let result = format_string_literal("it's", '\'', StringFormatOptions::default());
        assert_eq!(result, r#""it's""#);

        // Has double quote - stay single
        let result = format_string_literal(r#"say "hi""#, '\'', StringFormatOptions::default());
        assert_eq!(result, r#"'say "hi"'"#);
    }

    #[test]
    fn test_preserves_escape_sequences() {
        let result = format_string_literal(r"\u0041\n\t", '"', StringFormatOptions::default());
        assert_eq!(result, r"'\u0041\n\t'");
    }

    #[test]
    fn test_swaps_quote_escaping_when_changing_quotes() {
        // Original: "it\'s" with single quote
        // After: "it's" with double quote (unescape the single quote)
        let result = format_string_literal(r"it\'s", '\'', StringFormatOptions::default());
        assert_eq!(result, r#""it's""#);
    }

    #[test]
    fn test_prefer_double_quotes_option() {
        let options = StringFormatOptions {
            prefer_single_quotes: false,
        };
        let result = format_string_literal("hello", '\'', options);
        assert_eq!(result, r#""hello""#);
    }

    #[test]
    fn test_already_optimal_quote() {
        // Already using single quotes, no change needed
        let result = format_string_literal("hello", '\'', StringFormatOptions::default());
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_many_quotes_chooses_less_frequent() {
        // 3 double quotes vs 1 single quote - choose single (minimize escaping)
        // Original (with double quotes): "a "b" "c" "d" e's"
        // After switching to single: 'a "b" "c" "d" e\'s' (single quote gets escaped)
        let content = r#"a "b" "c" "d" e's"#;
        let result = format_string_literal(content, '"', StringFormatOptions::default());
        // Expected: single quote wrapper, double quotes unescaped, single quote escaped
        assert_eq!(result, "'a \"b\" \"c\" \"d\" e\\'s'");
    }
}
