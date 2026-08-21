// Shared printing utilities for printers
//
// This module provides common printing logic used across language printers
// (TypeScript, CSS, Svelte) to eliminate code duplication.

use crate::escapes::swap_quote_escaping;
use crate::swar::{splat, zero_lanes};
use std::borrow::Cow;
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
    // One fused byte pass counting both quote kinds. Both quotes are ASCII and
    // UTF-8 continuation bytes are >= 0x80, so a byte compare cannot match
    // inside a multi-byte sequence. String contents are typically short, where
    // a per-pattern searcher setup dominates a plain counting loop; the
    // branchless sums auto-vectorize for long contents.
    let mut single_count = 0usize;
    let mut double_count = 0usize;
    for &b in raw_content.as_bytes() {
        single_count += usize::from(b == b'\'');
        double_count += usize::from(b == b'"');
    }
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

    // Check if there's a line terminator between the positions
    !contains_line_terminator(&source[prev_end..curr_start])
}

/// The byte length of the ECMAScript `LineTerminatorSequence` beginning at
/// `bytes[i]`, or `None` if none begins there.
///
/// The set is `<LF>`, `<CR>`, `<LS>` (U+2028) and `<PS>` (U+2029), with
/// `<CR><LF>` counting as **one** sequence (ECMAScript §12.3, and acorn's
/// `lineBreakG`). Every line question in this module goes through this one
/// predicate rather than testing `'\n'` inline, because a narrower class here is
/// not a cosmetic difference: the lexers end a `//` comment at *every* one of
/// these terminators, so a printer that believes only `\n` ends a line places
/// the comment and the token after it on one output line — and the emitted `//`
/// then swallows that token, losing code. A wider class is equally unsound (it
/// would fabricate line breaks inside string and template bodies), so this must
/// stay exactly the ECMAScript set.
///
/// CSS's own terminator set differs (`<LF>`, `<CR>`, `<FF>` — CSS Syntax §3.3);
/// `tsv_css` shares this table deliberately, since the two sets agree on
/// everything CSS source realistically contains and the alternative is a second
/// class to keep right.
#[inline]
fn line_terminator_len(bytes: &[u8], i: usize) -> Option<usize> {
    match bytes[i] {
        b'\n' => Some(1),
        b'\r' => Some(if bytes.get(i + 1) == Some(&b'\n') {
            2
        } else {
            1
        }),
        // U+2028 / U+2029 are `e2 80 a8` / `e2 80 a9` — the only 0xE2 leads that
        // are terminators, so the two continuation bytes must both match.
        0xE2 if bytes.get(i + 1) == Some(&0x80)
            && matches!(bytes.get(i + 2), Some(0xA8 | 0xA9)) =>
        {
            Some(3)
        }
        _ => None,
    }
}

/// Split `text` into lines on the ECMAScript line-terminator class
/// ([`line_terminator_len`]) — `str::lines()` with the right class.
///
/// `str::lines()` splits on `\n` alone (stripping a trailing `\r` from the line
/// it yields), so a source whose terminators are lone `<CR>` / `<LS>` / `<PS>`
/// reads back as a **single** line. That is fine for tsv's own output, which is
/// always LF, and wrong for anything scanning an author's *input* — a blank line
/// the author spelled `\r\r` disappears, and a tool comparing input against
/// output then reports the surviving blank as one the formatter invented.
///
/// Exported for exactly those input-scanning consumers (`tsv_debug`'s
/// blank-fabrication audit); a second copy of the class is the drift this whole
/// module exists to prevent.
pub fn ecmascript_lines(text: &str) -> impl Iterator<Item = &str> {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    let mut done = false;
    core::iter::from_fn(move || {
        // Matches `str::lines()`: no trailing empty line for a trailing
        // terminator, and no items at all for an empty string.
        if done || pos == bytes.len() {
            return None;
        }
        let mut i = pos;
        while i < bytes.len() {
            if let Some(len) = line_terminator_len(bytes, i) {
                let line = &text[pos..i];
                pos = i + len;
                done = pos == bytes.len();
                return Some(line);
            }
            i += 1;
        }
        let line = &text[pos..];
        done = true;
        Some(line)
    })
}

/// Fold every **carriage return** in `source` to LF, so everything downstream sees the
/// LF-only text each of tsv's languages is defined over. Borrowed unchanged when there is
/// none — the overwhelming majority of documents — for one `memchr` and the caller's
/// `&str` back.
///
/// Call this on a source string **before parsing it to FORMAT**, and never before parsing it
/// for the wire AST: the fold shifts byte offsets, and `parse`'s offsets are a drop-in
/// contract with acorn / Svelte / `parseCss` over the author's own bytes. Every
/// parse-then-format entry point does it — `tsv_ts` / `tsv_svelte`'s `format_str`, the CLI's
/// `format_source` (CSS's only one, having no `format_str` of its own), each binding's format
/// export, and `canonicalize_js` — so no printer ever sees a `<CR>`.
/// It is also where prettier answers the same question (`normalizeEndOfLine`, in
/// `normalizeInputAndOptions`, ahead of the parse).
///
/// **Every language tsv formats folds the CR itself, before it tokenizes.** HTML
/// preprocesses its input stream so that "there are never any U+000D CR characters in the
/// input to the tokenization stage"; CSS Syntax §3.3 filters `<CR>`, `<FF>` and `<CR><LF>`
/// to a single `<LF>` on the way in. ECMAScript has no input-stream pass — `<CR>` is a real
/// `LineTerminator` in its grammar — but it folds at the only place a `<CR>` can reach a
/// *value*: "`<CR><LF>` and `<CR>` |LineTerminatorSequence|s are normalized to `<LF>` for
/// both TV and TRV" (a raw `<CR>` cannot appear in a `StringLiteral` or a
/// `RegularExpressionLiteral` at all). So this changes bytes without changing meaning at
/// every position it reaches.
///
/// **Why the input rather than the finished output.** The printer asks *where are the
/// lines?* in several places that split on `'\n'` alone: [`crate::Comment::multiline`] at
/// parse, [`is_indentable_block_comment`] at doc-build, and the
/// per-line emitters under them. A fold applied to the finished string leaves every one of
/// those disagreeing with the output about where the lines *are* — a lone-`<CR>` document's
/// block comment reads as a single line, rides out verbatim, and the fold then splits it, so
/// a second pass re-indents what the first left alone and idempotence fails. Folding first
/// makes all of them right by construction, and is one answer rather than one per reader.
///
/// **CR only** — not the whole [`line_terminator_len`] class. U+2028 / U+2029 are
/// terminators to ECMAScript but ordinary characters to HTML and CSS text, both formatters
/// keep them where the author put them, and ECMAScript's own TRV keeps each as itself
/// (`<LS>` → U+2028), so folding one would change what a template renders.
///
/// Idempotent: its own output holds no `<CR>` to fold.
#[must_use]
pub fn normalize_carriage_returns(source: &str) -> Cow<'_, str> {
    let Some(first) = source.find('\r') else {
        return Cow::Borrowed(source);
    };
    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..first]);
    let mut rest = &source[first..];
    while let Some(i) = rest.find('\r') {
        out.push_str(&rest[..i]);
        out.push('\n');
        let after = &rest[i + 1..];
        // A CRLF pair is ONE LineTerminatorSequence: consume the `\n` so it does not
        // become a second.
        rest = after.strip_prefix('\n').unwrap_or(after);
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// The line `position` sits on, as `(line_start, line_end, line_number)` — bounds in bytes,
/// number 1-indexed — over the ECMAScript terminator class ([`line_terminator_len`]).
///
/// The answer for a lone byte offset with no line table to hand: the diagnostic snippet in
/// [`crate::error`]. The printer's hot path asks the same question against a prebuilt table
/// instead ([`is_same_line_fast`] and its siblings, over [`build_line_breaks_into`]), which
/// is why this one walks rather than bisects — it runs once, on a source that failed to
/// parse.
///
/// Splitting on `'\n'` alone is what this exists to prevent: it makes a lone-`<CR>` or
/// `<LS>`-separated source ONE line, so the number is 1 whatever the position, and the
/// excerpt carries raw `<CR>`s that a terminal renders by overwriting the line it just
/// printed — hiding the very text a caret points into.
pub(crate) fn line_bounds_at(source: &str, position: usize) -> (usize, usize, usize) {
    let bytes = source.as_bytes();
    let mut line_start = 0;
    let mut line_number = 1;
    let mut i = 0;
    while i < position {
        let candidate = next_line_terminator_candidate(bytes, i);
        if candidate >= position {
            break;
        }
        i = candidate;
        match line_terminator_len(bytes, i) {
            // A sequence that ENDS at or before `position` opens the line we want.
            Some(len) => {
                if i + len > position {
                    // `position` sits INSIDE the sequence — a byte offset between a `<CR>`
                    // and its `<LF>`. The line it belongs to is the one this sequence ends.
                    return (line_start, i, line_number);
                }
                line_number += 1;
                line_start = i + len;
                i += len;
            }
            // A `0xE2` lead that is not `<LS>` / `<PS>`.
            None => i += 1,
        }
    }

    // The line ENDS at the terminator, so only its start is wanted here.
    let line_end =
        next_line_terminator(bytes, position.max(line_start)).map_or(bytes.len(), |(at, _)| at);
    (line_start, line_end, line_number)
}

/// Whether `text` holds any ECMAScript line terminator ([`line_terminator_len`]).
#[inline]
fn contains_line_terminator(text: &str) -> bool {
    let bytes = text.as_bytes();
    // `\n` and `\r` are the overwhelmingly common cases and `memchr`-free
    // `iter().position()` over them auto-vectorizes; the 0xE2 lead is checked in
    // the same pass since a separate scan would cost a second walk.
    (0..bytes.len()).any(|i| line_terminator_len(bytes, i).is_some())
}

/// The number of ECMAScript line terminators in `text`, counting `<CR><LF>` once.
fn count_line_terminators(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        match line_terminator_len(bytes, i) {
            Some(len) => {
                count += 1;
                i += len;
            }
            None => i += 1,
        }
    }
    count
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

    // Check if there are 2+ line terminators (blank line) between the positions
    count_line_terminators(&source[prev_end..curr_start]) >= 2
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
    let bytes = between.as_bytes();
    let mut found_first_newline = false;
    let mut line_start = 0;

    let mut i = 0;
    while i < bytes.len() {
        let Some(len) = line_terminator_len(bytes, i) else {
            i += 1;
            continue;
        };
        if found_first_newline {
            // Check if the line between previous terminator and this one is blank
            let line = &between[line_start..i];
            if line.bytes().all(|b| b == b' ' || b == b'\t') {
                return true;
            }
        }
        found_first_newline = true;
        i += len;
        line_start = i;
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

    contains_line_terminator(&source[start..end])
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
    build_line_breaks_bytes(source.as_bytes(), breaks);
}

/// [`build_line_breaks_into`]'s walk, over bytes and without the capacity reserve — the
/// shape the exhaustive equivalence test grades against its byte-at-a-time reference.
fn build_line_breaks_bytes(bytes: &[u8], breaks: &mut Vec<u32>) {
    let mut i = 0;
    while i < bytes.len() {
        i = next_line_terminator_candidate(bytes, i);
        if i >= bytes.len() {
            break;
        }
        match line_terminator_len(bytes, i) {
            // The recorded offset is the sequence's LAST byte, which is what the
            // LF-only builder recorded for both `\n` and `\r\n`. Every consumer
            // (`is_same_line_fast`, `has_blank_line_between_fast`) reads the table
            // as "one entry per line ending", so a multi-byte sequence must push
            // exactly once — two entries for a `\r\n` would read as a blank line.
            Some(len) => {
                breaks.push((i + len - 1) as u32);
                i += len;
            }
            // A `0xE2` lead that is not `<LS>` / `<PS>` — the candidate scan's only
            // false positive, and the reason it is a CANDIDATE scan.
            None => i += 1,
        }
    }
}

/// Index of the first byte at or after `from` that could BEGIN a line terminator
/// sequence — `\n`, `\r`, or the `0xE2` lead of `<LS>` / `<PS>` — or `bytes.len()`.
///
/// The same word-at-a-time shape, and the same reason, as `location`'s
/// `next_ecmascript_terminator`: terminators are sparse (~1 per 30–40 source bytes), so a
/// per-byte compare spends nearly all of its work confirming misses, and this table is
/// built once over the whole source in every `format_in`. The third needle is what
/// `location`'s does not need — that one runs inside a run already proven ASCII, where no
/// `<LS>` / `<PS>` can occur; this one runs over the raw source, so it must not skip the
/// `0xE2` lead. `line_terminator_len` then classifies the hit, since most `0xE2` bytes
/// begin some other character.
///
/// `from_le_bytes` puts byte 0 in the low lane, so the lowest set bit is the earliest
/// match, and OR-ing the three masks preserves [`crate::swar::zero_lanes`]'s lowest-lane
/// guarantee: a spurious lane in any one mask is preceded by a genuine one in that same
/// mask.
#[inline]
fn next_line_terminator_candidate(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        let hits = zero_lanes(w ^ splat(b'\n'))
            | zero_lanes(w ^ splat(b'\r'))
            | zero_lanes(w ^ splat(LINE_SEPARATOR_LEAD));
        if hits != 0 {
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r' | LINE_SEPARATOR_LEAD) {
        i += 1;
    }
    i
}

/// The UTF-8 lead byte of `<LS>` (U+2028) and `<PS>` (U+2029) — `E2 80 A8/A9`, the only
/// multi-byte line terminators, spelled once for the scan and its scalar tail.
const LINE_SEPARATOR_LEAD: u8 = 0xE2;

/// The first ECMAScript line terminator at or after `from`, as `(start, len)` — `None` if the
/// rest of `bytes` holds none.
///
/// [`next_line_terminator_candidate`] plus the confirmation step each of its callers owes it:
/// the scan stops on any `0xE2` lead, only `<LS>` / `<PS>` are terminators, so a candidate
/// [`line_terminator_len`] declines has to be stepped over and the scan resumed. Open-coding
/// that retry is how a class question comes to be spelled *almost* right, so "where is the next
/// line break" has one spelling.
///
/// [`build_line_breaks_bytes`] keeps its own walk deliberately: it wants *every* terminator
/// rather than the next one, and its loop shape is what the exhaustive equivalence test grades.
#[inline]
fn next_line_terminator(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    loop {
        i = next_line_terminator_candidate(bytes, i);
        if i >= bytes.len() {
            return None;
        }
        if let Some(len) = line_terminator_len(bytes, i) {
            return Some((i, len));
        }
        // A `0xE2` lead that is not `<LS>` / `<PS>`.
        i += 1;
    }
}

/// Dedent a multi-line block comment's content the way Svelte's acorn wrapper does.
///
/// Svelte's `onComment` (`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`) takes
/// the indentation of the line the comment OPENS on and removes one copy of it from the front
/// of every line of the comment's content, so the wire `value` reads the same however deeply
/// the comment was nested. Mirroring it *is* the specification here: the parse product is a
/// drop-in contract with Svelte, so a class read any wider or narrower than `onComment`'s is a
/// wire divergence.
///
/// ⚠️ **The two steps use DIFFERENT line-terminator classes, deliberately** — because
/// `onComment` does:
///
/// - **finding the comment's line start** is `while (a > 0 && source[a - 1] !== '\n') a -= 1`:
///   `\n` and nothing else, so a `<CR>` / `<LS>` / `<PS>` between the real line start and the
///   comment is ordinary text and the indentation is still the *line's* own;
/// - **stripping it off each line** is `value.replace(new RegExp('^' + indentation, 'gm'), '')`,
///   and an `m`-mode `^` matches at the start of input and after every ECMAScript terminator
///   ([`line_terminator_len`]) — `\n`, `\r`, `<LS>` and `<PS>` alike.
///
/// So do not unify the two onto one predicate. Reading the walk-back with the full class finds
/// a line start Svelte has not got — dedenting by whatever `[ \t]` trails the terminator rather
/// than by the line's real indent — and splitting the content on `'\n'` alone misses line starts
/// Svelte does have. Both were live wire divergences, in both directions.
///
/// `<CR><LF>` needs no special case in the strip: the `m`-mode `^` matches after the `<CR>` as
/// well as after the `<LF>`, but the position after a `<CR>` begins with that `<LF>`, which a
/// non-empty `[ \t]` indentation can never match — so one boundary or two is the same answer,
/// and [`line_terminator_len`]'s pairing is free to take it as one.
///
/// The gate above this — a BLOCK comment whose content holds a `\n`, and only those
/// ([`crate::Comment::content_is_multiline`]) — and the `[ \t]` indentation class are Svelte's
/// too. All five spellings of both steps are pinned by
/// `tests/comment_dedent_line_terminators.rs`, whose module doc says why a fixture cannot
/// carry them.
///
/// # Examples
///
/// ```
/// use tsv_lang::printing::strip_comment_indentation;
///
/// // The comment opens at byte 1, on a line indented by one tab, so one tab comes off the
/// // front of each of its lines.
/// let source = "\t/* a\n\tb */";
/// assert_eq!(strip_comment_indentation(source, " a\n\tb ", 1), " a\nb ");
///
/// // A `<LS>` inside the content opens a line just as a `\n` does.
/// assert_eq!(
///     strip_comment_indentation(source, " a\u{2028}\tb ", 1),
///     " a\u{2028}b "
/// );
/// ```
pub fn strip_comment_indentation(source: &str, content: &str, comment_start: u32) -> String {
    let comment_start = comment_start as usize;
    let bytes = source.as_bytes();

    // The line the comment opens on — `\n` and nothing else, per the walk-back above.
    let mut line_start = comment_start;
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    // The `[ \t]` run that opens that line.
    let mut indentation_end = line_start;
    while matches!(bytes.get(indentation_end), Some(b' ' | b'\t')) {
        indentation_end += 1;
    }
    let indentation = &source[line_start..indentation_end];
    if indentation.is_empty() {
        return content.to_string();
    }

    // Drop one copy of it wherever an `m`-mode `^` matches: the content's start, and the
    // position after every line terminator.
    let content_bytes = content.as_bytes();
    let mut result = String::with_capacity(content.len());
    let mut pos = 0;
    loop {
        let body_start = if content[pos..].starts_with(indentation) {
            pos + indentation.len()
        } else {
            pos
        };

        // Copy the rest of the line, its terminator included; the next `^` sits at its end.
        let line_end = next_line_terminator(content_bytes, body_start)
            .map_or(content.len(), |(at, len)| at + len);
        result.push_str(&content[body_start..line_end]);

        if line_end == content.len() {
            return result;
        }
        pos = line_end;
    }
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
/// `lines` iterates the comment body *without* the `/*` / `*/` delimiters,
/// split on `'\n'` (typically `content.split('\n')` fed directly) — no line
/// buffer is materialized, so classification never heap-allocates. Returns
/// `false` for single-line content. Mirrors prettier's `isIndentableBlockComment`.
///
/// # Example
/// ```
/// use tsv_lang::printing::is_indentable_block_comment;
///
/// let lines = |s: &'static str| s.split('\n');
/// assert!(is_indentable_block_comment(lines("*\n * text\n ")));     // /** … */
/// assert!(is_indentable_block_comment(lines("\n * text\n ")));      // /* * … */
/// assert!(is_indentable_block_comment(lines("*\n *\n * text\n "))); // blank `*` line
/// assert!(!is_indentable_block_comment(lines(" a\n   b "))); // a line lacks `*`
/// assert!(!is_indentable_block_comment(lines(" single line ")));    // single-line
/// ```
pub fn is_indentable_block_comment<'s>(mut lines: impl Iterator<Item = &'s str>) -> bool {
    // The `*` of the `/*` opener attaches to the first line and the `*` of the
    // `*/` closer attaches to the last line, so the first line always qualifies
    // and an all-whitespace last line qualifies. Every other line must start
    // with `*`.
    if lines.next().is_none() {
        return false;
    }
    // Lag one line behind so the final line gets the last-line rule.
    let Some(mut prev) = lines.next() else {
        return false; // fewer than 2 lines → not a multi-line indentable comment
    };
    for next in lines {
        // A successor exists, so `prev` is a middle line: it must be `*`-prefixed.
        if !prev.trim_start().starts_with('*') {
            return false;
        }
        prev = next;
    }
    // The last line qualifies when empty or `*`-prefixed.
    let last = prev.trim_start();
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
    fn optimal_string_quote_matches_searcher_reference_exhaustively() {
        // No corpus can grade a counting bug here: a miscount only changes
        // output when it flips the quote choice, so this exhaustive equivalence
        // check against the two-searcher shape is the load-bearing gate for the
        // fused byte count.
        fn reference(raw_content: &str) -> char {
            let single_count = raw_content.matches('\'').count();
            let double_count = raw_content.matches('"').count();
            if double_count < single_count {
                '"'
            } else {
                '\''
            }
        }
        // Alphabet covers each arm: both quote kinds, plain ASCII, a control
        // char, a two-byte and a three-byte UTF-8 sequence (continuation bytes
        // must never read as a quote byte), and a backslash.
        const ALPHABET: [char; 7] = ['\'', '"', 'a', '\n', 'é', '✓', '\\'];
        let mut cases = vec![String::new()];
        for c1 in ALPHABET {
            cases.push(c1.to_string());
            for c2 in ALPHABET {
                cases.push(format!("{c1}{c2}"));
                for c3 in ALPHABET {
                    cases.push(format!("{c1}{c2}{c3}"));
                }
            }
        }
        for s in &cases {
            assert_eq!(optimal_string_quote(s), reference(s), "content {s:?}");
        }
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

    /// The four ECMAScript `LineTerminatorSequence`s, as `(source form, byte length)`.
    const TERMINATORS: [(&str, usize); 5] = [
        ("\n", 1),
        ("\r", 1),
        ("\r\n", 2),
        ("\u{2028}", 3),
        ("\u{2029}", 3),
    ];

    #[test]
    fn test_every_line_terminator_ends_a_line() {
        // The whole class must answer alike: the LF-only reading put a `//`
        // comment and the token after a `<CR>` / `<LS>` / `<PS>` on one output
        // line, where the emitted comment swallowed that token.
        for (term, _) in TERMINATORS {
            let source = format!("a{term}b");
            let b = (1 + term.len()) as u32;
            assert!(!is_same_line(&source, 1, b), "is_same_line across {term:?}");
            assert!(
                has_newline_between(&source, 1, b),
                "has_newline_between across {term:?}"
            );

            let breaks = build_line_breaks(&source);
            assert!(
                !is_same_line_fast(&breaks, 1, b),
                "is_same_line_fast across {term:?}"
            );
        }
    }

    #[test]
    fn test_line_break_table_records_each_terminator_once() {
        // One entry per line ending, at the sequence's LAST byte. A `\r\n`
        // recorded twice would read as a blank line to every consumer.
        for (term, len) in TERMINATORS {
            let source = format!("a{term}b");
            assert_eq!(
                build_line_breaks(&source),
                vec![len as u32],
                "table for {term:?}"
            );
        }
    }

    #[test]
    fn test_blank_line_needs_two_terminators_of_any_kind() {
        for (term, len) in TERMINATORS {
            let one = format!("a{term}b");
            let two = format!("a{term}{term}b");
            let end_one = (1 + term.len()) as u32;
            let end_two = (1 + 2 * term.len()) as u32;

            assert!(!has_blank_line_between(&one, 1, end_one), "one {term:?}");
            assert!(has_blank_line_between(&two, 1, end_two), "two {term:?}");
            assert!(
                !has_blank_line_between_strict(&one, 1, end_one),
                "strict one {term:?}"
            );
            assert!(
                has_blank_line_between_strict(&two, 1, end_two),
                "strict two {term:?}"
            );

            let breaks_two = build_line_breaks(&two);
            assert_eq!(breaks_two.len(), 2, "two {term:?} = two entries");
            assert!(
                has_blank_line_between_fast(&breaks_two, 1, end_two),
                "fast two {term:?}"
            );

            // `<CR><LF>` is the trap: one sequence, two bytes, never a blank line.
            let _ = len;
        }
    }

    #[test]
    fn test_lone_cr_and_ls_are_not_blank_line_filler() {
        // A `\r` inside a "blank" line can only be a terminator in its own right,
        // so the filler test never sees one — `a\r\rb` is a blank line, not a
        // single CRLF-ish break with `\r` padding.
        assert!(has_blank_line_between_strict("a\r\rb", 1, 3));
        assert!(!has_blank_line_between_strict("a\r\nb", 1, 3));
        // Real filler (spaces / tabs) still makes the line blank.
        assert!(has_blank_line_between_strict("a\n \t\nb", 1, 5));
        assert!(!has_blank_line_between_strict("a\n x \nb", 1, 6));
    }

    #[test]
    fn test_ecmascript_lines_matches_str_lines_on_lf_input() {
        // The whole point is a WIDER class, so the narrow reference must still
        // agree everywhere it is correct — otherwise the widening also changed
        // the LF behavior every existing consumer depends on.
        for s in [
            "",
            "a",
            "a\n",
            "a\nb",
            "a\n\nb",
            "\n",
            "\n\n",
            "\na",
            "a\n\n\n",
            "a\r\nb\r\n",
        ] {
            let got: Vec<&str> = ecmascript_lines(s).collect();
            let want: Vec<&str> = s.lines().collect();
            assert_eq!(got, want, "ecmascript_lines vs str::lines on {s:?}");
        }
    }

    #[test]
    fn test_ecmascript_lines_splits_the_terminators_str_lines_misses() {
        // `str::lines()` reads each of these as ONE line; that is the blindness
        // that made the fabrication audit report an authored blank as invented.
        for (term, _) in TERMINATORS {
            let s = format!("a{term}{term}b");
            assert_eq!(
                ecmascript_lines(&s).collect::<Vec<_>>(),
                vec!["a", "", "b"],
                "blank run spelled with {term:?}"
            );
        }
    }

    /// The byte-at-a-time shape [`build_line_breaks_bytes`]'s word-at-a-time scan replaced,
    /// kept as its arithmetic oracle.
    ///
    /// **No corpus can grade the replacement.** A word-at-a-time rewrite fails on *where
    /// the pattern lands relative to the stride*, which a corpus samples arbitrarily — and
    /// real source contains no `\r` at all, so its whole CRLF arm is corpus-dead. See
    /// `docs/performance.md` §"The same rule covers scans".
    fn reference_line_breaks(bytes: &[u8]) -> Vec<u32> {
        let mut breaks = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match line_terminator_len(bytes, i) {
                Some(len) => {
                    breaks.push((i + len - 1) as u32);
                    i += len;
                }
                None => i += 1,
            }
        }
        breaks
    }

    /// [`next_line_terminator_candidate`] graded DIRECTLY, at every length and alignment.
    ///
    /// Its caller cannot grade it: `build_line_breaks_bytes` steps a byte on a
    /// non-terminator, so a scan that gave up at the last short chunk would degrade into
    /// the caller's own per-byte walk and produce the same table. Deleting the scalar tail
    /// therefore passes the table-level test — which is what "an oracle you have never seen
    /// fail proves nothing" means in practice, one level up from the usual reading.
    #[test]
    fn swar_candidate_scan_matches_the_scalar_reference_exhaustive() {
        const ALPHABET: [u8; 8] = [b'\n', b'\r', 0xE2, 0x80, 0xA8, 0xA9, b'a', 0x7f];
        fn reference(bytes: &[u8], from: usize) -> usize {
            let mut i = from;
            while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r' | 0xE2) {
                i += 1;
            }
            i
        }
        let mut buf = Vec::new();
        for align in 0..=16usize {
            for len in 0..=4usize {
                for mut code in 0..8usize.pow(len as u32) {
                    buf.clear();
                    buf.resize(align, b'x');
                    for _ in 0..len {
                        buf.push(ALPHABET[code % 8]);
                        code /= 8;
                    }
                    for from in 0..=buf.len() {
                        assert_eq!(
                            next_line_terminator_candidate(&buf, from),
                            reference(&buf, from),
                            "align {align}, from {from}, bytes {buf:02x?}"
                        );
                    }
                }
            }
        }
    }

    /// Every string of length 0–4 over an alphabet covering each arm, at every alignment
    /// across the word stride. The alphabet holds both `<LS>`/`<PS>` continuation bytes and
    /// a bare `0xE2` lead, so the candidate scan's one false positive is exercised in every
    /// position; the inputs are raw bytes (the scan reads bytes, never chars), which lets a
    /// truncated sequence sit at the very end of the slice.
    #[test]
    fn swar_line_break_scan_matches_the_scalar_reference_exhaustive() {
        const ALPHABET: [u8; 8] = [b'\n', b'\r', 0xE2, 0x80, 0xA8, 0xA9, b'a', 0x7f];
        let mut buf = Vec::new();
        let mut actual = Vec::new();
        for align in 0..=16usize {
            for len in 0..=4usize {
                for mut code in 0..8usize.pow(len as u32) {
                    buf.clear();
                    buf.resize(align, b'x');
                    for _ in 0..len {
                        buf.push(ALPHABET[code % 8]);
                        code /= 8;
                    }
                    actual.clear();
                    build_line_breaks_bytes(&buf, &mut actual);
                    assert_eq!(
                        actual,
                        reference_line_breaks(&buf),
                        "align {align}, bytes {buf:02x?}"
                    );
                }
            }
        }
    }

    /// The same equivalence on inputs long enough to run the word loop many times over,
    /// with the terminators sparse the way real source has them.
    #[test]
    fn swar_line_break_scan_matches_the_scalar_reference_on_long_sources() {
        for period in [7usize, 8, 9, 16, 31, 33] {
            for terminator in ["\n", "\r\n", "\r", "\u{2028}", "\u{2029}"] {
                let mut src = String::new();
                for line in 0..40 {
                    src.push_str(&"x".repeat(period + line % 3));
                    src.push_str(terminator);
                }
                // A bare `0xE2` lead that is NOT a terminator, mid-source.
                src.push('\u{2603}');
                src.push('\n');
                let mut actual = Vec::new();
                build_line_breaks_bytes(src.as_bytes(), &mut actual);
                assert_eq!(
                    actual,
                    reference_line_breaks(src.as_bytes()),
                    "period {period}, terminator {terminator:?}"
                );
            }
        }
    }

    /// Every spelling of a carriage return folds to LF, and a CRLF pair stays ONE terminator.
    #[test]
    fn carriage_returns_normalize_to_lf() {
        assert_eq!(normalize_carriage_returns("a\r\nb"), "a\nb");
        assert_eq!(normalize_carriage_returns("a\rb"), "a\nb");
        assert_eq!(normalize_carriage_returns("a\r\n\r\nb"), "a\n\nb");
        assert_eq!(normalize_carriage_returns("a\r\rb"), "a\n\nb");
        // `\n\r` is two terminators, not a pair — only `\r\n` is one.
        assert_eq!(normalize_carriage_returns("a\n\rb"), "a\n\nb");
        assert_eq!(normalize_carriage_returns("\r"), "\n");
        assert_eq!(normalize_carriage_returns("\r\n"), "\n");
    }

    /// A CR-free string comes back BORROWED — the fold runs ahead of every format, so the
    /// common document must not pay an allocation for it — and folding is idempotent.
    #[test]
    fn carriage_return_normalization_borrows_without_one_and_is_idempotent() {
        assert!(matches!(
            normalize_carriage_returns("a\nb\n"),
            Cow::Borrowed("a\nb\n")
        ));
        assert!(matches!(normalize_carriage_returns(""), Cow::Borrowed("")));
        let once = normalize_carriage_returns("a\r\nb\rc").into_owned();
        assert!(matches!(
            normalize_carriage_returns(&once),
            Cow::Borrowed(_)
        ));
        assert_eq!(once, "a\nb\nc");
    }

    /// U+2028 / U+2029 are terminators to ECMAScript and ordinary characters to HTML and CSS
    /// text. Both formatters keep them where the author put them, and ECMAScript's own TRV
    /// keeps each as itself, so this fold must not reach them even though
    /// `line_terminator_len` counts them.
    #[test]
    fn carriage_return_normalization_leaves_line_and_paragraph_separators_alone() {
        assert_eq!(normalize_carriage_returns("a\u{2028}b"), "a\u{2028}b");
        assert_eq!(normalize_carriage_returns("a\u{2029}b"), "a\u{2029}b");
        assert_eq!(normalize_carriage_returns("a\u{2028}\r\nb"), "a\u{2028}\nb");
    }

    #[test]
    fn test_a_bare_0xe2_lead_is_not_a_terminator() {
        // `<LS>`/`<PS>` are the only 0xE2 leads that terminate a line; every other
        // three-byte character starting 0xE2 (here U+2010 HYPHEN, `e2 80 90`)
        // must not, or the class fabricates breaks inside ordinary text.
        let source = "a\u{2010}b";
        assert!(is_same_line(source, 1, 4));
        assert!(build_line_breaks(source).is_empty());
    }

    #[test]
    fn test_strip_comment_indentation_line_start_is_newline_only() {
        // The comment's own line is found by `onComment`'s `\n`-only walk-back, so a `<CR>` /
        // `<LS>` / `<PS>` ahead of it is ordinary text and the indentation is still the line's.
        // Reading the whole class here instead takes the `\t\t` AFTER the terminator for the
        // indent — a wider one than the line's, which over-dedents on the wire.
        for (term, _) in TERMINATORS {
            let source = format!("x{term}\t\t/* a\n\t\tb */");
            let comment_start = (1 + term.len() + 2) as u32;
            let stripped = strip_comment_indentation(&source, " a\n\t\tb ", comment_start);
            if term == "\n" || term == "\r\n" {
                // A line really does start after these two, and it opens with the `\t\t`.
                assert_eq!(stripped, " a\nb ", "line start after {term:?}");
            } else {
                // `x` opens the line, so there is no indentation to strip at all.
                assert_eq!(stripped, " a\n\t\tb ", "no line start at {term:?}");
            }
        }
    }

    #[test]
    fn test_strip_comment_indentation_strips_after_every_terminator() {
        // The other half is the opposite class: the strip is `^` under the `m` flag, whose
        // line starts are the whole terminator set. Splitting the content on `'\n'` alone
        // leaves the indent standing after a `<CR>` / `<LS>` / `<PS>`, under-dedenting.
        for (term, _) in TERMINATORS {
            assert_eq!(
                strip_comment_indentation("\t/* x */", &format!(" a\n\tb{term}\tc "), 1),
                format!(" a\nb{term}c "),
                "indent stripped after {term:?}"
            );
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
