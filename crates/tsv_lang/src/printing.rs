// Shared printing utilities for printers
//
// This module provides common printing logic used across language printers
// (TypeScript, CSS, Svelte) to eliminate code duplication.

use crate::Span;
use crate::escapes::swap_quote_escaping;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// Choose the optimal surrounding quote for a string's raw content: the quote
/// that appears less often inside needs fewer escapes. Ties prefer single
/// quotes (hardcoded — matches prettier-plugin-svelte; tsv is non-configurable).
///
/// Exposed so a caller can cheaply decide whether [`format_string_literal`]
/// would change the quote (when this returns the original quote, the formatted
/// output equals the verbatim source literal — no allocation needed).
#[inline]
pub fn optimal_string_quote(raw_content: &str) -> char {
    let single_count = raw_content.matches('\'').count();
    let double_count = raw_content.matches('"').count();
    // Double quotes only when they're strictly rarer (fewer escapes); otherwise
    // single — which also covers the tie, the hardcoded single-quote tie-breaker.
    if double_count < single_count {
        '"'
    } else {
        '\''
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
///
/// # Returns
///
/// Formatted string literal including surrounding quotes
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::format_string_literal;
///
/// // String with no quotes - uses preferred quote (single)
/// let result = format_string_literal("hello", '"');
/// assert_eq!(result, "'hello'");
///
/// // String with single quotes - switches to double to avoid escaping
/// let result = format_string_literal("it's nice", '\'');
/// assert_eq!(result, r#""it's nice""#);
///
/// // String with double quotes - stays single to minimize escaping
/// let result = format_string_literal(r#"say "hi""#, '\'');
/// assert_eq!(result, r#"'say "hi"'"#);
///
/// // Preserves escape sequences
/// let result = format_string_literal(r"\u0041\n", '"');
/// assert_eq!(result, r"'\u0041\n'");
/// ```
pub fn format_string_literal(raw_content: &str, original_quote: char) -> String {
    // Count quotes in the raw content (with escapes) to make the best choice.
    let optimal_quote = optimal_string_quote(raw_content);

    // Build the quoted literal in a single pre-sized allocation. On the common
    // path (quote unchanged) the content copies in directly; the swap path still
    // allocates inside `swap_quote_escaping`, but its result is copied in just
    // once here rather than via a second `format!` buffer.
    let mut result = String::with_capacity(raw_content.len() + 2);
    result.push(optimal_quote);
    if optimal_quote == original_quote {
        result.push_str(raw_content);
    } else {
        result.push_str(&swap_quote_escaping(
            raw_content,
            original_quote,
            optimal_quote,
        ));
    }
    result.push(optimal_quote);
    result
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

/// Check if there's a truly blank line between two positions in source.
///
/// Unlike [`has_blank_line_between`] which just counts newlines, this function
/// verifies that an intermediate line contains only whitespace. This correctly
/// handles cases where the parser strips grouping parentheses, leaving closing
/// `)` characters between newlines that look like blank lines to newline-counting
/// checks.
///
/// Returns `true` if there's a line containing only whitespace between two
/// newlines in the range `[prev_end, curr_start)`.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::has_blank_line_between_strict;
///
/// // Truly blank line: "foo\n\nbar"
/// assert_eq!(has_blank_line_between_strict("foo\n\nbar", 3, 5), true);
///
/// // Content between newlines: "foo\n)\nbar" (stripped parens)
/// assert_eq!(has_blank_line_between_strict("foo\n)\nbar", 3, 6), false);
///
/// // One newline: "foo\nbar"
/// assert_eq!(has_blank_line_between_strict("foo\nbar", 3, 4), false);
/// ```
pub fn has_blank_line_between_strict(source: &str, prev_end: u32, curr_start: u32) -> bool {
    let prev_end = prev_end as usize;
    let curr_start = curr_start as usize;

    if prev_end >= curr_start || curr_start > source.len() {
        return false;
    }

    let between = &source[prev_end..curr_start];
    let mut found_first_newline = false;
    let mut line_start = 0;

    for (i, byte) in between.bytes().enumerate() {
        if byte == b'\n' {
            if found_first_newline {
                // Check if the line between previous newline and this one is blank
                let line = &between[line_start..i];
                if line.bytes().all(|b| b == b' ' || b == b'\t' || b == b'\r') {
                    return true;
                }
            }
            found_first_newline = true;
            line_start = i + 1;
        }
    }

    false
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

//
// Line Breaks Table Functions (O(log n) binary search)
//
//
// These functions use a precomputed line breaks table for O(log n) lookups
// instead of O(n) string scans. The table is a Vec<u32> of newline byte offsets
// built during lexing.

/// Check if two positions are on the same line using precomputed line breaks.
///
/// This is the O(log n) version of [`is_same_line`] that uses binary search
/// instead of scanning the source string.
///
/// # Arguments
///
/// * `line_breaks` - Sorted slice of newline byte offsets
/// * `prev_end` - End position of the first element
/// * `curr_start` - Start position of the second element
///
/// # Returns
///
/// `true` if there is no newline between the positions, `false` otherwise.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::is_same_line_fast;
///
/// // Source: "foo\nbar" - newline at position 3
/// let line_breaks = vec![3u32];
/// assert_eq!(is_same_line_fast(&line_breaks, 0, 3), true);   // before newline
/// assert_eq!(is_same_line_fast(&line_breaks, 3, 4), false);  // crosses newline
/// assert_eq!(is_same_line_fast(&line_breaks, 4, 7), true);   // after newline
/// ```
#[inline]
pub fn is_same_line_fast(line_breaks: &[u32], prev_end: u32, curr_start: u32) -> bool {
    // Adjacent tokens are on the same line
    if prev_end == curr_start {
        return true;
    }

    // Positions out of order are not on the same line
    // (matches behavior of is_same_line which returns false for invalid ranges)
    if prev_end > curr_start {
        return false;
    }

    // Binary search: find first newline >= prev_end
    let idx = line_breaks.partition_point(|&pos| pos < prev_end);

    // If no newline found, or first newline is at/after curr_start, they're on same line
    line_breaks.get(idx).is_none_or(|&pos| pos >= curr_start)
}

/// Check if there's a blank line (2+ newlines) between two positions.
///
/// This is the O(log n) version of [`has_blank_line_between`] that uses binary
/// search instead of counting newlines in a string slice.
///
/// # Arguments
///
/// * `line_breaks` - Sorted slice of newline byte offsets
/// * `prev_end` - End position of the first element
/// * `curr_start` - Start position of the second element
///
/// # Returns
///
/// `true` if there are 2 or more newlines between the positions.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::has_blank_line_between_fast;
///
/// // Source: "foo\n\nbar" - newlines at positions 3 and 4
/// let line_breaks = vec![3u32, 4];
/// assert_eq!(has_blank_line_between_fast(&line_breaks, 0, 5), true);  // two newlines
///
/// // Source: "foo\nbar" - newline at position 3
/// let line_breaks = vec![3u32];
/// assert_eq!(has_blank_line_between_fast(&line_breaks, 0, 4), false); // one newline
/// ```
#[inline]
pub fn has_blank_line_between_fast(line_breaks: &[u32], prev_end: u32, curr_start: u32) -> bool {
    if prev_end >= curr_start {
        return false;
    }

    // Find first newline >= prev_end
    let first_idx = line_breaks.partition_point(|&pos| pos < prev_end);

    // Check if there's a newline in range
    let Some(&first_pos) = line_breaks.get(first_idx) else {
        return false;
    };
    if first_pos >= curr_start {
        return false;
    }

    // Check if there's a second newline before curr_start
    let second_idx = first_idx + 1;
    line_breaks
        .get(second_idx)
        .is_some_and(|&pos| pos < curr_start)
}

/// Check if there's any newline between two positions.
///
/// This is the O(log n) version of [`has_newline_between`] that uses binary
/// search instead of scanning the source string.
///
/// # Arguments
///
/// * `line_breaks` - Sorted slice of newline byte offsets
/// * `start` - Start position
/// * `end` - End position
///
/// # Returns
///
/// `true` if there's at least one newline between the positions.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::has_newline_between_fast;
///
/// // Source: "{\na: 1}" - newline at position 1
/// let line_breaks = vec![1u32];
/// assert_eq!(has_newline_between_fast(&line_breaks, 1, 2), true);
///
/// // Source: "{a: 1}" - no newlines
/// let line_breaks: Vec<u32> = vec![];
/// assert_eq!(has_newline_between_fast(&line_breaks, 1, 2), false);
/// ```
#[inline]
pub fn has_newline_between_fast(line_breaks: &[u32], start: u32, end: u32) -> bool {
    if start >= end {
        return false;
    }

    // Find first newline >= start
    let idx = line_breaks.partition_point(|&pos| pos < start);

    // Check if that newline is before end
    line_breaks.get(idx).is_some_and(|&pos| pos < end)
}

/// Build a line breaks table from source code.
///
/// Scans the source string and records the byte offset of each newline character.
/// Only records `\n` (LF) as the canonical newline - `\r\n` (CRLF) is handled by
/// recording the `\n` position.
///
/// # Arguments
///
/// * `source` - The source text
///
/// # Returns
///
/// A vector of byte offsets where newlines occur.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::build_line_breaks;
///
/// let source = "foo\nbar\nbaz";
/// let breaks = build_line_breaks(source);
/// assert_eq!(breaks, vec![3, 7]);
/// ```
pub fn build_line_breaks(source: &str) -> Vec<u32> {
    let mut breaks = Vec::new();
    build_line_breaks_into(source, &mut breaks);
    breaks
}

/// Like [`build_line_breaks`], filling a caller-provided (empty) table — the
/// seam behind the arena-parked line-break scratch
/// (`DocArena::take_line_breaks_scratch`), so multi-file drivers fill one warm
/// table per file instead of allocating a fresh `Vec`.
pub fn build_line_breaks_into(source: &str, breaks: &mut Vec<u32>) {
    // Pre-size to ~one newline per 32 bytes (average code lines run ~25–40
    // bytes), so typical files fill in one allocation instead of the doubling
    // chain (a no-op once the parked table is warm). Capacity-only — never
    // affects the recorded values.
    breaks.reserve(source.len() / 32);
    for (pos, ch) in source.bytes().enumerate() {
        if ch == b'\n' {
            breaks.push(pos as u32);
        }
    }
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

/// Returns `true` if a multi-line block comment is *indentable* in prettier's
/// sense: every line — with the `*` from the `/*` opener restored to the front
/// of the first line and the `*` from the `*/` closer restored to the end of
/// the last line — begins with `*` after trimming leading whitespace.
///
/// These are JSDoc (`/** … */`) and `*`-aligned (`/* … */`) block comments.
/// Their continuation lines get reindented to a single leading space (the
/// context indent is supplied separately by the layout). Non-indentable block
/// comments are preserved verbatim instead.
///
/// `lines` is the comment body *without* the `/*` / `*/` delimiters, already
/// split on `'\n'` — the caller splits once and feeds the same lines to the
/// indentable-comment builder, so the body isn't re-scanned per pass. Returns
/// `false` for single-line content. Mirrors prettier's `isIndentableBlockComment`.
///
/// # Example
/// ```
/// use tsv_lang::printing::is_indentable_block_comment;
///
/// let lines = |s: &'static str| s.split('\n').collect::<Vec<_>>();
/// assert!(is_indentable_block_comment(&lines("*\n * text\n ")));     // /** … */
/// assert!(is_indentable_block_comment(&lines("\n * text\n ")));      // /* * … */
/// assert!(is_indentable_block_comment(&lines("*\n *\n * text\n "))); // blank `*` line
/// assert!(!is_indentable_block_comment(&lines(" a\n   b "))); // a line lacks `*`
/// assert!(!is_indentable_block_comment(&lines(" single line ")));    // single-line
/// ```
pub fn is_indentable_block_comment(lines: &[&str]) -> bool {
    // The `*` of the `/*` opener attaches to the first line and the `*` of the
    // `*/` closer attaches to the last line, so the first line always qualifies
    // and an all-whitespace last line qualifies. Every other line must start
    // with `*`. (Pattern fails for single-line content → `false`.)
    let [_first, middle @ .., last] = lines else {
        return false; // fewer than 2 lines → not a multi-line indentable comment
    };
    for line in middle {
        if !line.trim_start().starts_with('*') {
            return false;
        }
    }
    // The last line qualifies when empty or `*`-prefixed.
    let last = last.trim_start();
    last.is_empty() || last.starts_with('*')
}

/// Calculate the visual width of a string, treating tabs as `tab_width` columns.
///
/// Uses grapheme cluster segmentation to match Prettier's width calculation:
/// - Multi-codepoint graphemes (emoji sequences, skin tones, ZWJ) = 2 columns
/// - Single codepoint: uses unicode-width (CJK = 2, regular = 1, zero-width = 0)
/// - Tabs = `tab_width` columns
///
/// # Example
/// ```
/// use tsv_lang::printing::visual_width;
///
/// assert_eq!(visual_width("hello", 2), 5);
/// assert_eq!(visual_width("\thello", 2), 7); // tab (2) + "hello" (5)
/// assert_eq!(visual_width("\thello", 4), 9); // tab (4) + "hello" (5)
/// assert_eq!(visual_width("⭐", 2), 2);      // emoji = 2 columns
/// assert_eq!(visual_width("中文", 2), 4);    // CJK = 2 columns each
/// assert_eq!(visual_width("👋🏽", 2), 2);    // emoji + skin tone = 2 (grapheme)
/// assert_eq!(visual_width("👨‍👩‍👧", 2), 2);  // ZWJ family = 2 (grapheme)
/// ```
#[inline]
pub fn visual_width(s: &str, tab_width: usize) -> usize {
    if s.is_ascii() {
        // Fast path: each ASCII byte is 1 column, tabs are tab_width columns.
        #[allow(clippy::naive_bytecount)]
        let tab_count = s.as_bytes().iter().filter(|&&b| b == b'\t').count();
        return s.len() + tab_count * (tab_width - 1);
    }
    visual_width_mixed(s, tab_width)
}

/// Width of a string containing non-ASCII: byte-count maximal ASCII runs,
/// grapheme-walk only the non-ASCII stretches. Cluster-identical to walking
/// every grapheme (one non-ASCII char must not change how the ASCII majority
/// is measured), which pins three boundary constraints the code can't show:
///
/// - An ASCII run followed by a non-ASCII char hands its LAST char to the
///   grapheme walker — that char may start a cluster crossing the boundary
///   (combining mark on an ASCII base `e\u{0301}`, keycap `1\u{FE0F}\u{20E3}`,
///   and ASCII+ZWJ, the one such cluster whose width (emoji rule: 2) differs
///   from the sum of its chars' widths, so it must be walked whole).
/// - The walker advances whole clusters and returns to byte counting only at
///   a cluster boundary, so a cluster that absorbs a *following* ASCII char
///   (Prepend, e.g. `\u{0600}1`) is consumed there and never double-counted.
///   Every switch position is a true cluster boundary of the full string,
///   except a CRLF split by a run boundary — the only ASCII-ASCII cluster —
///   which is width-preserving (both chars are width 0 on both paths).
/// - Run bytes use grapheme-path char semantics — printable 1, tab
///   `tab_width`, control/DEL 0 — NOT the pure-ASCII fast path's byte count
///   (which keeps its historical controls-count-as-1 behavior).
fn visual_width_mixed(s: &str, tab_width: usize) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut width = 0usize;
    let mut i = 0usize;
    while i < len {
        if bytes[i].is_ascii() {
            // Single pass: accumulate the run's width while finding its end.
            while i < len && bytes[i].is_ascii() {
                width += ascii_char_width(bytes[i], tab_width);
                i += 1;
            }
            if i == len {
                return width;
            }
            // Non-ASCII follows: un-count the run's last char and hand it to
            // the grapheme walker (it may start a boundary-crossing cluster).
            i -= 1;
            width -= ascii_char_width(bytes[i], tab_width);
        }
        for g in s[i..].graphemes(true) {
            width += grapheme_width(g, tab_width);
            i += g.len();
            if i < len && bytes[i].is_ascii() {
                break;
            }
        }
    }
    width
}

/// Width of one ASCII char: printable 1, tab `tab_width`, control/DEL 0.
/// Must agree with [`grapheme_width`] on a single-char ASCII cluster (there
/// `'\t'` is special-cased and `char::width` yields 1 for printables, `None`→0
/// for controls) — `visual_width_mixed`'s run counting relies on the two being
/// interchangeable, and the parity tests enforce it.
#[inline]
const fn ascii_char_width(b: u8, tab_width: usize) -> usize {
    if b == b'\t' {
        tab_width
    } else if b < 0x20 || b == 0x7f {
        0
    } else {
        1
    }
}

/// Calculate width of a single grapheme cluster.
#[inline]
fn grapheme_width(g: &str, tab_width: usize) -> usize {
    let mut chars = g.chars();
    let Some(first) = chars.next() else {
        return 0;
    };

    // Single-char grapheme: use unicode-width
    if chars.next().is_none() {
        return if first == '\t' {
            tab_width
        } else {
            first.width().unwrap_or(0)
        };
    }

    // Multi-char grapheme: check if it's an emoji sequence
    // Emoji with skin tones or ZWJ sequences = 2
    // Non-emoji (base + combining marks) = sum of char widths
    if g.chars().any(is_emoji_modifier) {
        2
    } else {
        // Sum widths - combining marks are 0
        g.chars().filter_map(UnicodeWidthChar::width).sum()
    }
}

/// Check if char is an emoji modifier (triggers width 2 for grapheme).
/// Only checks for modifiers that would make summed width incorrect.
#[inline]
fn is_emoji_modifier(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0x1F3FB
            ..=0x1F3FF | // Skin tone modifiers
        0x200D // ZWJ (zero-width joiner)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_quotes_uses_preferred() {
        let result = format_string_literal("hello", '"');
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_switches_to_minimize_escaping() {
        // Has single quote - switch to double
        let result = format_string_literal("it's", '\'');
        assert_eq!(result, r#""it's""#);

        // Has double quote - stay single
        let result = format_string_literal(r#"say "hi""#, '\'');
        assert_eq!(result, r#"'say "hi"'"#);
    }

    #[test]
    fn test_preserves_escape_sequences() {
        let result = format_string_literal(r"\u0041\n\t", '"');
        assert_eq!(result, r"'\u0041\n\t'");
    }

    #[test]
    fn test_swaps_quote_escaping_when_changing_quotes() {
        // Original: "it\'s" with single quote
        // After: "it's" with double quote (unescape the single quote)
        let result = format_string_literal(r"it\'s", '\'');
        assert_eq!(result, r#""it's""#);
    }

    #[test]
    fn test_already_optimal_quote() {
        // Already using single quotes, no change needed
        let result = format_string_literal("hello", '\'');
        assert_eq!(result, "'hello'");
    }

    #[test]
    fn test_many_quotes_chooses_less_frequent() {
        // 3 double quotes vs 1 single quote - choose single (minimize escaping)
        // Original (with double quotes): "a "b" "c" "d" e's"
        // After switching to single: 'a "b" "c" "d" e\'s' (single quote gets escaped)
        let content = r#"a "b" "c" "d" e's"#;
        let result = format_string_literal(content, '"');
        // Expected: single quote wrapper, double quotes unescaped, single quote escaped
        assert_eq!(result, "'a \"b\" \"c\" \"d\" e\\'s'");
    }

    #[test]
    fn test_visual_width_ascii_fast_path() {
        // Pure ASCII - hits fast path
        assert_eq!(visual_width("hello", 2), 5);
        assert_eq!(visual_width("hello world", 2), 11);
        assert_eq!(visual_width("", 2), 0);
        assert_eq!(visual_width(" ", 2), 1);
    }

    #[test]
    fn test_visual_width_ascii_tabs() {
        // Tabs in ASCII strings
        assert_eq!(visual_width("\t", 2), 2);
        assert_eq!(visual_width("\t", 4), 4);
        assert_eq!(visual_width("\thello", 2), 7);
        assert_eq!(visual_width("\thello", 4), 9);
        assert_eq!(visual_width("\t\t", 2), 4);
        assert_eq!(visual_width("a\tb", 2), 4);
    }

    #[test]
    fn test_visual_width_unicode_path() {
        // Non-ASCII - uses Unicode grapheme path
        assert_eq!(visual_width("⭐", 2), 2);
        assert_eq!(visual_width("中文", 2), 4);
        assert_eq!(visual_width("👋🏽", 2), 2);
        assert_eq!(visual_width("👨\u{200d}👩\u{200d}👧", 2), 2);
        // Mixed ASCII + non-ASCII
        assert_eq!(visual_width("hi⭐", 2), 4);
    }

    #[test]
    fn test_visual_width_combining_and_zero_width() {
        // base 'e' + combining acute accent (U+0301, width 0) = one grapheme, width 1.
        // Exercises the non-emoji multi-char branch (sum of char widths).
        assert_eq!(visual_width("e\u{0301}", 2), 1);
        // zero-width space contributes 0
        assert_eq!(visual_width("a\u{200B}b", 2), 2);
        // lone combining mark: must not panic, width 0
        assert_eq!(visual_width("\u{0301}", 2), 0);
    }

    /// The pre-hybrid implementation: walk every grapheme cluster. The hybrid
    /// `visual_width_mixed` must be value-identical to this on every input.
    fn visual_width_reference(s: &str, tab_width: usize) -> usize {
        s.graphemes(true)
            .map(|g| grapheme_width(g, tab_width))
            .sum()
    }

    #[test]
    fn test_visual_width_mixed_matches_reference_exhaustive() {
        // Chars chosen to hit every boundary rule: ASCII printable/control/
        // tab/CR/LF/DEL, combining mark (Extend), ZWJ + pictographic + skin
        // tone (the emoji-modifier rule), variation selector + keycap,
        // Prepend (U+0600 absorbs a following char), regional-indicator pair
        // (GB12 pairing), CJK/wide, zero-width space.
        const POOL: &[char] = &[
            'a',
            '1',
            ' ',
            '\t',
            '\r',
            '\n',
            '\u{7f}',
            '\u{1}',
            '\u{0301}',
            'é',
            '中',
            '⭐',
            '🙂',
            '\u{1F3FD}',
            '\u{200D}',
            '\u{FE0F}',
            '\u{20E3}',
            '\u{0600}',
            '\u{200B}',
            '\u{1F1FA}',
            '\u{1F1F8}',
        ];
        let mut s = String::new();
        for &a in POOL {
            for &b in POOL {
                for &c in POOL {
                    s.clear();
                    s.push(a);
                    s.push(b);
                    s.push(c);
                    for tw in [2usize, 4] {
                        assert_eq!(
                            visual_width_mixed(&s, tw),
                            visual_width_reference(&s, tw),
                            "triple {s:?} tab_width {tw}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_visual_width_mixed_matches_reference_targeted() {
        // Longer shapes the triple product can't reach: long ASCII runs with
        // sparse non-ASCII (the case the hybrid optimizes), cluster chains at
        // run boundaries, and multi-switch alternation.
        for s in [
            "a long ascii prefix with a trailing accent e\u{0301} and more ascii after",
            "/** JSDoc with one arrow → in the middle of a long comment line */",
            "1\u{FE0F}\u{20E3}x",
            "x\u{200D}\u{1F642}y",
            "\u{0600}12ab",
            "ab\r\né\r\ncd",
            "\té\ta\t中\t",
            "🇺🇸🇺🇸🇺a🇺🇸",
            "e\u{0301}\u{0301}a\u{0301}",
            "中中中 spaces 中中中",
            "🙂🏽\u{200D}🙂a🙂\u{200D}",
            "trailing run then unicode é",
            "é leading unicode then run",
            "é",
            "aé",
            "éa",
        ] {
            for tw in [2usize, 4] {
                assert_eq!(
                    visual_width_mixed(s, tw),
                    visual_width_reference(s, tw),
                    "input {s:?} tab_width {tw}"
                );
            }
        }
    }

    #[test]
    fn test_is_same_line_invalid_positions() {
        // Out-of-order and out-of-bounds positions are not "same line" (documented).
        assert!(!is_same_line("ab", 5, 1));
        assert!(!is_same_line("ab", 0, 99));
    }

    #[test]
    fn test_has_blank_line_between_invalid_positions() {
        assert!(!has_blank_line_between("a\n\nb", 5, 1));
        assert!(!has_blank_line_between("a\n\nb", 0, 99));
    }

    #[test]
    fn test_has_newline_between_invalid_positions() {
        assert!(!has_newline_between("{\nx", 5, 1));
        assert!(!has_newline_between("{\nx", 0, 99));
    }

    #[test]
    fn test_spans_on_same_line_overlap_and_reversed() {
        let source = "abcdefgh";
        let a = Span::new(0, 5);
        let b = Span::new(3, 8); // overlaps `a`
        assert!(spans_on_same_line(source, a, b));
        // Argument order must not matter.
        assert!(spans_on_same_line(source, b, a));
    }

    #[test]
    fn test_line_break_fns_slow_fast_agree() {
        // "a\n\nb\nc": newlines at byte offsets 1, 2, 4.
        let source = "a\n\nb\nc";
        let breaks = build_line_breaks(source);
        assert_eq!(breaks, vec![1, 2, 4]);
        for (p, c) in [(0u32, 1u32), (1, 4), (0, 6), (3, 5), (1, 3), (4, 6)] {
            assert_eq!(
                is_same_line(source, p, c),
                is_same_line_fast(&breaks, p, c),
                "is_same_line {p},{c}"
            );
            assert_eq!(
                has_blank_line_between(source, p, c),
                has_blank_line_between_fast(&breaks, p, c),
                "has_blank_line_between {p},{c}"
            );
            assert_eq!(
                has_newline_between(source, p, c),
                has_newline_between_fast(&breaks, p, c),
                "has_newline_between {p},{c}"
            );
        }
    }
}
