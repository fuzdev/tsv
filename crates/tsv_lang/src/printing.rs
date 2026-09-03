// Shared printing utilities for printers
//
// This module provides common printing logic used across language printers
// (TypeScript, CSS, Svelte) to eliminate code duplication.

use crate::acorn_prefix::AcornPrefix;
use crate::escapes::swap_quote_escaping;
use crate::swar::{high_bit_lanes, lanes_less_than, splat, zero_lanes, zero_or_high_lanes};
use crate::whitespace::is_js_whitespace;
use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// Choose the optimal surrounding quote for a string's raw content: the quote
/// that appears less often inside needs fewer escapes. Ties prefer single
/// quotes (hardcoded — matches prettier-plugin-svelte; tsv is non-configurable).
///
/// Exposed so a caller can cheaply decide whether [`format_string_literal`]
/// would change the quote (when this returns the original quote, the formatted
/// output equals the verbatim source literal — no allocation needed).
///
/// ⭐ A caller that has the literal's DOCUMENT and span wants
/// [`optimal_string_quote_in`] instead: same answer, read off the host's words rather
/// than a slice's, and it returns the width class of the same content for free.
#[inline]
pub fn optimal_string_quote(raw_content: &str) -> char {
    // Double quotes win only when they are STRICTLY rarer, so a content holding
    // no `'` at all takes the single-quote answer whatever its `"` count is —
    // which makes the question "is there a `'` here?", not "how many of each?".
    // That is one needle on the word-at-a-time rung ([`crate::swar::next_byte_of`],
    // whose own table puts `N` = 1 at 12 instructions per eight bytes and which
    // inlines here as 11) instead of a per-byte count, and it answers **99.4%** of
    // real calls: over 1,666 `.ts` files only 541 of 86,410 string contents hold a
    // `'` (7 of 11,987 on 1,695 `.svelte`, 41 of 4,299 on 638 `.css`). Both quotes
    // are ASCII and UTF-8 continuation bytes are >= 0x80, so a byte compare cannot
    // match inside a multi-byte sequence.
    let bytes = raw_content.as_bytes();
    if crate::swar::next_byte_of(bytes, 0, [b'\'']) == bytes.len() {
        return '\'';
    }
    quote_minimizing_escapes(bytes)
}

/// [`optimal_string_quote`]'s counting arm, reached only by a content that holds
/// at least one `'`.
///
/// Outlined because it is 0.6% of calls: the fused sums are the whole body of the
/// function otherwise, and inlining them into every string-literal site pays for a
/// path almost nothing takes. ⚠️ The sums do vectorize, and the vectorization is
/// **not** a reason to prefer them — `objdump` reads four bytes per iteration
/// through a byte-to-`u64` widening chain (`pcmpeqb`/`punpcklbw`/`pshuflw`/
/// `pshufd`/`pand`/`paddq`, once per needle), about 7.75 instructions a byte,
/// which is what put the scan above on the gate instead.
#[cold]
#[inline(never)]
fn quote_minimizing_escapes(bytes: &[u8]) -> char {
    let mut single_count = 0usize;
    let mut double_count = 0usize;
    for &b in bytes {
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

/// [`optimal_string_quote`] for a string literal's content **in its document**, with
/// the width question the printer asks next answered by the same pass:
/// `(optimal quote, plain)`, where `plain` is `true` when the content holds no byte
/// the width depends on — no `\t`, no `\n`, nothing at or above `0x80` — and so
/// measures one column a byte.
///
/// `from..end` is the CONTENT: the literal's span without its two quote delimiters.
/// Both delimiters are plain one-column ASCII, so `plain` describes the whole literal
/// span too — which is the span the caller emits.
///
/// ⭐ **The cheapest measure is one an earlier phase already took.** The quote choice
/// reads every content byte looking for a `'`, and 99.4% of contents hold none (541 of
/// 86,410 over 1,666 `.ts` files), so that walk visits the whole content — and whether
/// it saw a `\t`, a raw line terminator or a non-ASCII byte *is* the width answer, one
/// pass earlier in the same phase. Verbatim string literals are **54.0%** of the width
/// measures a TypeScript format run makes outside identifier names (84,771 of 157,073
/// on that corpus; 1.65 MB, mean 19.5 bytes), and this retires the second pass
/// `DocArena::source_span` would make over the same bytes for the price of one more
/// lane kernel per word.
///
/// ⭐ **And it reads the DOCUMENT's words, not the content's.** [`optimal_string_quote`]
/// takes a slice, so a content under eight bytes has no word to form and falls to the
/// scalar tail; this borrows the bytes on either side of the content and disbelieves
/// any hit past `end`.
///
/// ⚠️ Both answers are **exact**, not conservative, so a caller may state the width
/// claim two-sidedly — and must, because a wrong claim in either direction is a silent
/// width error (`DocArena::source_span_plain`). The rare arm, taken by a content
/// holding a `'` *or* a width-relevant byte, re-asks each question with the exact scan
/// that owns it rather than inferring one answer from the other.
#[inline]
pub fn optimal_string_quote_in(source: &str, from: usize, end: usize) -> (char, bool) {
    if next_width_relevant_or_single_quote_in(source.as_bytes(), from, end) == end {
        // No `'` — so single quotes need no escaping and take the tie-break — and no
        // byte the width depends on. Finding nothing is both answers at once.
        return ('\'', true);
    }
    optimal_string_quote_in_cold(source, from, end)
}

/// [`optimal_string_quote_in`]'s arm for a content that holds a `'`, a `\t`, a line
/// terminator or a non-ASCII byte — about **2.6%** of the string literals a
/// TypeScript format run prints.
///
/// Outlined so the two exact scans, and [`optimal_string_quote`]'s own counting arm
/// behind them, stay out of every string-literal call site. Which of the two bytes
/// fired is not recorded, so both questions are simply re-asked here; each is a scan
/// over a content the fast path has already shown to be rare.
#[cold]
#[inline(never)]
fn optimal_string_quote_in_cold(source: &str, from: usize, end: usize) -> (char, bool) {
    (
        optimal_string_quote(&source[from..end]),
        next_width_relevant_in(source.as_bytes(), from, end) == end,
    )
}

/// Format a string literal with optimal quote selection
///
/// Takes raw string content (with escape sequences preserved) and formats it
/// by choosing the optimal quote character to minimize escaping.
///
/// # Algorithm
///
/// 1. Pick the quote with [`optimal_string_quote`]
/// 2. That is the one appearing less frequently inside (minimize escaping)
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
/// parse-then-format entry point does it — each language crate's `format_str`, the CLI's
/// `format_source`, each binding's format export, and `canonicalize_js` — so no printer ever
/// sees a `<CR>`.
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
///
/// **The fold's pass also takes the folded document's line verdict** — whether every
/// terminator left in it is a `\n` ([`FoldedSource::lf_only`]) — because the one loose
/// needle that finds a `\r` (`\r` or any non-ASCII byte, [`classify_line_terminators`])
/// is the needle the verdict pass asks too ([`line_terminators_are_lf_only`]), and a
/// printer built on the fold ([`LineBreaks::of_folded`]) would otherwise walk every byte a
/// second time to re-ask it. The verdict is a fact about the FOLDED text: no `\r` remains
/// in it, so it is exactly "no U+2028 / U+2029 anywhere", and the fold moves neither.
#[must_use]
pub fn normalize_carriage_returns(source: &str) -> FoldedSource<'_> {
    let Terminators {
        first_cr,
        holds_separator,
    } = classify_line_terminators(source.as_bytes());
    let lf_only = !holds_separator;
    let Some(first) = first_cr else {
        debug_assert_eq!(lf_only, line_terminators_are_lf_only(source.as_bytes()));
        return FoldedSource {
            text: Cow::Borrowed(source),
            lf_only,
        };
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
    debug_assert_eq!(lf_only, line_terminators_are_lf_only(out.as_bytes()));
    FoldedSource {
        text: Cow::Owned(out),
        lf_only,
    }
}

/// A document with its `<CR>` fold applied ([`normalize_carriage_returns`]) and the line
/// verdict the fold's own pass took over the folded text — so a printer built on it
/// ([`LineBreaks::of_folded`]) takes the verdict from the pass that already ran instead
/// of walking the source again. The two travel as one value so the verdict can never be
/// read against a text it was not taken on.
#[derive(Debug)]
pub struct FoldedSource<'a> {
    text: Cow<'a, str>,
    lf_only: bool,
}

impl<'a> FoldedSource<'a> {
    /// The folded text — borrowed when the source held no `<CR>`.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether every line terminator in [`Self::text`] is a `\n` — the fact
    /// [`line_terminators_are_lf_only`] states over those bytes, here taken by the fold's
    /// pass (a `\r` cannot remain, so this is exactly "no U+2028 / U+2029 anywhere").
    pub fn lf_only(&self) -> bool {
        self.lf_only
    }

    /// The folded text alone, for a caller with no printer to hand the verdict to.
    pub fn into_text(self) -> Cow<'a, str> {
        self.text
    }
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
///
/// The 0xE2 lead rides the same pass as `\n` / `\r`, since a separate scan would
/// cost a second walk. ⚠️ It is a **scalar** pass — measured, against a comment
/// here that used to assert otherwise: inlined into [`is_same_line`] the loop is
/// ten instructions and four branches a byte and retires no vector instruction,
/// because the early exit makes the stride data-dependent. Both callers ask about
/// an inter-token gap, so the walk is a handful of bytes and the site is below the
/// sampling floor on the format *and* the wire board; the word-at-a-time form
/// ([`crate::swar::next_byte_of`], which the lexers' longer runs use) would be
/// paying its per-call setup for nothing here. Re-measure before changing it —
/// the reason this stays scalar is the gap LENGTH, not the codegen.
#[inline]
fn contains_line_terminator(text: &str) -> bool {
    let bytes = text.as_bytes();
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
// The three line questions asked of a BUILT table — a sorted `Vec<u32>` of the byte
// offsets of each terminator's last byte (`build_line_breaks_into`). No printer calls
// these directly any more: they are the search behind the `#[cold]` fallbacks of the scan
// forms below (`is_same_line_scan` and siblings, which read the source bytes and reach
// the table only past the scan cap or on a non-LF-only document) and the oracle the
// exhaustive test grades those scan forms against.

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

/// A document's line-break table, built on demand — and the document's verdict on it,
/// taken up front.
///
/// The printers ask the line-break table three questions (same line? a newline between?
/// a blank line between?), and since the scan forms of those questions
/// ([`is_same_line_scan`] and siblings) answer nearly every ask off the source bytes,
/// the table is read only as a fallback: past [`LINE_SCAN_CAP`] bytes, or on a document
/// holding a terminator that is not a `\n`. A census over 12 MB of TypeScript put the
/// fallback in 8 of 1,666 documents at the cap, none of them non-LF-only — so the table
/// no longer exists until a fallback asks for it. What is taken up front is the one fact
/// the scan forms need before they read a byte: whether every line terminator in the
/// document is a `\n` ([`line_terminators_are_lf_only`], one loose-needle pass with no
/// per-line work), the fact under which the table would be exactly the set of `\n`
/// positions.
///
/// The fill goes into `scratch`, the arena-parked table a multi-file driver hands each
/// document (`DocArena::take_line_breaks_scratch`); [`Self::into_scratch`] hands it back,
/// built or not, for parking.
pub struct LineBreaks<'s> {
    source: &'s [u8],
    lf_only: bool,
    table: OnceCell<Vec<u32>>,
    scratch: Cell<Vec<u32>>,
}

impl<'s> LineBreaks<'s> {
    /// Classify `source` (one pass) and park `scratch` — a logically empty table whose
    /// capacity is warm — for the fill a fallback may ask for.
    pub fn new(source: &'s str, scratch: Vec<u32>) -> Self {
        let bytes = source.as_bytes();
        LineBreaks {
            source: bytes,
            lf_only: line_terminators_are_lf_only(bytes),
            table: OnceCell::new(),
            scratch: Cell::new(scratch),
        }
    }

    /// [`Self::new`] with a fresh scratch — the one-document callers.
    pub fn of(source: &'s str) -> Self {
        Self::new(source, Vec::new())
    }

    /// [`Self::new`] over a folded document, taking the verdict the fold's own pass
    /// already took ([`FoldedSource::lf_only`]) instead of classifying the bytes again —
    /// the format entry points that fold ahead of the parse (the CLI, the bindings, each
    /// crate's `format_str`) build their table this way, so the document is walked once,
    /// not twice.
    pub fn of_folded(folded: &'s FoldedSource<'_>, scratch: Vec<u32>) -> Self {
        let bytes = folded.text().as_bytes();
        debug_assert_eq!(folded.lf_only(), line_terminators_are_lf_only(bytes));
        LineBreaks {
            source: bytes,
            lf_only: folded.lf_only(),
            table: OnceCell::new(),
            scratch: Cell::new(scratch),
        }
    }

    /// Whether every line terminator in the document is a `\n` (a `\r\n` counts: the
    /// byte the table records for it IS the `\n`; a bare `\r` or a U+2028 / U+2029 does
    /// not) — the document's verdict, taken once at construction.
    pub fn lf_only(&self) -> bool {
        self.lf_only
    }

    /// The handle the printers carry: this table with its verdict.
    pub fn table(&self) -> LineTable<'_> {
        LineTable {
            breaks: Some(self),
            lf_only: self.lf_only,
        }
    }

    /// The table itself — one entry per line terminator's LAST byte
    /// ([`build_line_breaks_into`]) — filled on the first call.
    ///
    /// Reached only from the cold fallbacks of the scan forms (and from the two-sided
    /// `debug_assert` in each of them, so a debug build fills every document's table and
    /// grades every ask against it).
    pub fn breaks(&self) -> &[u32] {
        self.table.get_or_init(|| {
            let mut breaks = self.scratch.take();
            breaks.clear();
            breaks.reserve(self.source.len() / 32);
            let lf_only = build_line_breaks_bytes(self.source, &mut breaks);
            // The builder's own verdict re-derives the up-front one; the exhaustive test
            // grades the two against each other at every alignment of every terminator.
            debug_assert_eq!(lf_only, self.lf_only);
            breaks
        })
    }

    /// The scratch back, for parking: the filled table when a fallback asked for it,
    /// else the untouched capacity.
    pub fn into_scratch(self) -> Vec<u32> {
        self.table
            .into_inner()
            .unwrap_or_else(|| self.scratch.into_inner())
    }
}

impl fmt::Debug for LineBreaks<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineBreaks")
            .field("bytes", &self.source.len())
            .field("lf_only", &self.lf_only)
            .field("built", &self.table.get().is_some())
            .finish()
    }
}

/// A document's line-break table with its verdict — the value the printers carry, so the
/// two cannot drift apart across the printers that hold it — or the ERASED table.
///
/// [`LineTable::EMPTY`] is the canonical reprint's erased layout table: no terminator
/// anywhere, whatever the source says, which is what lets the erasure ride through the
/// scan forms untouched (an erased table is answered before a byte is read).
#[derive(Clone, Copy, Debug)]
pub struct LineTable<'a> {
    /// `None` is the erased table.
    breaks: Option<&'a LineBreaks<'a>>,
    lf_only: bool,
}

impl LineTable<'_> {
    /// No line breaks at all — the erased layout table.
    pub const EMPTY: LineTable<'static> = LineTable {
        breaks: None,
        lf_only: true,
    };

    /// The table's entries, for grading: empty when erased, else built on demand. Reached
    /// only from the `debug_assert_eq!` in each public scan form (a release build
    /// typechecks that call and folds it away, so this is not `cfg`-gated).
    fn breaks_for_grading(&self) -> &[u32] {
        self.breaks.map_or(&[], LineBreaks::breaks)
    }
}

//
// The document's verdict — one pass, ahead of any table
//

/// [`line_terminators_are_lf_only`]'s loose lane test — `\r` or any non-ASCII byte, and
/// [`classify_line_terminators`]'s, which walks the same loop — is a superset of the exact
/// candidate class `{ \r, 0xE2 }` their word re-asks and tails answer,
/// proved here rather than trusted (a uniform word cannot borrow across lanes unless it is
/// itself a match, so `hits != 0` is exactly the byte test).
const _: () = {
    let mut b = 0u16;
    while b < 256 {
        let byte = b as u8;
        let loose = zero_or_high_lanes(u64::from_le_bytes([byte; 8]) ^ splat(b'\r'));
        assert!((loose != 0) == (byte == b'\r' || byte >= 0x80));
        let exact = zero_lanes(u64::from_le_bytes([byte; 8]) ^ splat(b'\r'))
            | zero_lanes(u64::from_le_bytes([byte; 8]) ^ splat(LINE_SEPARATOR_LEAD));
        assert!((exact != 0) == (byte == b'\r' || byte == LINE_SEPARATOR_LEAD));
        b += 1;
    }
};

/// Whether every line terminator in `bytes` is a `\n` — a `\r\n` included (the byte the
/// table records for it is the `\n`); a bare `\r` or a U+2028 / U+2029 anywhere, a string
/// literal or a comment body included, says no. The document-level fact the scan forms of
/// the three line questions are gated on, and the one thing a document pays for up front
/// now that its table is built on demand ([`LineBreaks`]) — on the entry points that fold
/// `<CR>` ahead of the parse, the fold's own pass takes it instead
/// ([`classify_line_terminators`], the same loop with a different cold re-ask), so this
/// runs only for a caller that never folds (`format_in` reached directly).
///
/// One pass with ONE loose needle: `\r` or any non-ASCII byte, three operations a word
/// ([`crate::swar::zero_or_high_lanes`]), and nearly every word of real source fires
/// neither — this is a streaming loop with no per-hit chain, unlike the table builder it
/// replaces, which re-entered its scan once per line to push each hit. A word that fires
/// is asked the exact two-needle question out of line ([`lf_only_in_word`]) and the loose
/// loop **resumes at the next word** — it never hands the rest of the document to an exact
/// loop the way `next_line_terminator_candidate` does. That handoff was built first and
/// measured: 917 of 1,666 files of a 12 MB TypeScript corpus hold a non-ASCII byte
/// somewhere (a `©` in a header, an em dash in a comment), and 64% of the corpus's bytes
/// sit after the first one, so the ~20-instruction exact loop ran over two thirds of the
/// corpus and the pass averaged ~16 instructions a word against this loop's 10. Only 0.7%
/// of the words hold a non-ASCII byte at all, and a re-ask per fired WORD (not per byte,
/// which is what makes the candidate scan's handoff pay there) leaves a CJK-dense document
/// at the exact loop's price and everything else at the loose loop's.
///
/// Byte-at-a-time is sound for the exact question (`line_terminator_len`): neither `\r`
/// nor `0xE2` is ever a UTF-8 continuation byte, so a candidate found at any offset is a
/// character boundary.
pub fn line_terminators_are_lf_only(bytes: &[u8]) -> bool {
    let mut i = 0;
    // Two words a step: with nothing firing, the bound check and the branch are the loop's
    // own overhead, and they amortize over sixteen bytes (measured against one word a
    // step: a tenth of a point of a TypeScript format run, on both corpora).
    // ⚠️ The sixteen bytes are claimed ONCE (`first_chunk::<16>`) and split with no second
    // check: spelled as `split_first_chunk::<8>` + `first_chunk::<8>` with the lone-word
    // arm inside the loop, LLVM kept both bound checks in the loop — 22 instructions per
    // sixteen bytes against 16 — and gave back 0.35 points of the lever.
    while let Some(chunk) = bytes[i..].first_chunk::<16>() {
        let (words, _) = chunk.as_chunks::<8>();
        let (a, b) = (u64::from_le_bytes(words[0]), u64::from_le_bytes(words[1]));
        if (zero_or_high_lanes(a ^ splat(b'\r')) | zero_or_high_lanes(b ^ splat(b'\r'))) != 0
            && !(lf_only_in_word(bytes, i, a) && lf_only_in_word(bytes, i + 8, b))
        {
            return false;
        }
        i += 16;
    }
    // The one word that may remain ahead of the tail.
    if let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        if zero_or_high_lanes(w ^ splat(b'\r')) != 0 && !lf_only_in_word(bytes, i, w) {
            return false;
        }
        i += 8;
    }
    lf_only_tail(bytes, i)
}

/// The exact question over the one word at `at` the loose test fired on: every `\r` and
/// every `0xE2` lane in it, asked [`lf_only_at`] (which may read the two bytes past the
/// lane — the `\n` of a `\r\n`, the tail of a U+2028 — into the next word). Out of line
/// so the loose loop's unit holds one loop and nothing else; it runs on a fraction of a
/// percent of the words.
#[cold]
#[inline(never)]
fn lf_only_in_word(bytes: &[u8], at: usize, w: u64) -> bool {
    let mut hits = zero_lanes(w ^ splat(b'\r')) | zero_lanes(w ^ splat(LINE_SEPARATOR_LEAD));
    while hits != 0 {
        let lane = (hits.trailing_zeros() / 8) as usize;
        if !lf_only_at(bytes, at + lane) {
            return false;
        }
        hits &= hits - 1;
    }
    true
}

/// The exact question at a candidate byte (a `\r` or a `0xE2`): does the terminator that
/// begins here, if one does, end in a `\n`? The builder's own rule
/// (`build_line_breaks_bytes` records each terminator's last byte and asks whether it is
/// a `\n`), spelled once for the word loop and its tail.
#[inline]
fn lf_only_at(bytes: &[u8], at: usize) -> bool {
    match line_terminator_len(bytes, at) {
        Some(len) => bytes[at + len - 1] == b'\n',
        None => true,
    }
}

/// [`line_terminators_are_lf_only`] over the fewer-than-eight bytes the word loop stops
/// short of, a byte at a time.
#[inline]
fn lf_only_tail(bytes: &[u8], from: usize) -> bool {
    (from..bytes.len())
        .all(|i| !matches!(bytes[i], b'\r' | LINE_SEPARATOR_LEAD) || lf_only_at(bytes, i))
}

/// What the `<CR>` fold's one pass learns about a document ([`classify_line_terminators`]):
/// where its first `\r` is, if it holds one — where the fold starts copying — and whether
/// a U+2028 / U+2029 is anywhere in it — the folded text's line verdict, negated.
struct Terminators {
    first_cr: Option<usize>,
    holds_separator: bool,
}

/// The `<CR>` fold's pass ([`normalize_carriage_returns`]): [`line_terminators_are_lf_only`]'s
/// loose loop — the same one needle, `\r` or any non-ASCII byte, two words a step, a
/// fired word re-asked out of line — recording the two facts the fold and the folded
/// document's verdict need, so the fold's up-front `find('\r')` (std's memchr, nine
/// instructions a word — a whole-source pass on every format entry point that folds, over
/// a corpus in which no file holds a `\r`) is gone and the verdict pass does not run a
/// second time behind it; the fold's per-line search from the first `\r` on is unchanged. Unlike the verdict pass it has
/// no early answer to return: it runs to the end, or until both facts are known.
///
/// The verdict is stated over the FOLDED text, which is why a `\r` is not asked whether
/// it ends in a `\n` here: after the fold every `\r` and `\r\n` IS a `\n`, and a U+2028 /
/// U+2029 — which the fold does not touch — is the only terminator that can remain
/// otherwise.
///
/// Outlined on purpose, like the verdict pass: inlined into the CLI's format function the
/// loop carried that function's register pressure — 19 instructions per sixteen bytes
/// against 17 here — and read 0.07 points less of a CLI run (measured on two corpora).
#[inline(never)]
fn classify_line_terminators(bytes: &[u8]) -> Terminators {
    let mut found = Terminators {
        first_cr: None,
        holds_separator: false,
    };
    let mut i = 0;
    // The loop is `line_terminators_are_lf_only`'s, spelled the same way for the same
    // reasons (two words a step; the sixteen bytes claimed once).
    while let Some(chunk) = bytes[i..].first_chunk::<16>() {
        let (words, _) = chunk.as_chunks::<8>();
        let (a, b) = (u64::from_le_bytes(words[0]), u64::from_le_bytes(words[1]));
        if (zero_or_high_lanes(a ^ splat(b'\r')) | zero_or_high_lanes(b ^ splat(b'\r'))) != 0
            && (classify_word(bytes, i, a, &mut found)
                || classify_word(bytes, i + 8, b, &mut found))
        {
            return found;
        }
        i += 16;
    }
    // The one word that may remain ahead of the tail.
    if let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        if zero_or_high_lanes(w ^ splat(b'\r')) != 0 && classify_word(bytes, i, w, &mut found) {
            return found;
        }
        i += 8;
    }
    for at in i..bytes.len() {
        if matches!(bytes[at], b'\r' | LINE_SEPARATOR_LEAD) && classify_at(bytes, at, &mut found) {
            break;
        }
    }
    found
}

/// [`classify_line_terminators`]'s exact question over the one word at `at` the loose
/// test fired on — every `\r` and every `0xE2` lane in it, asked [`classify_at`]. Out of
/// line for the reason [`lf_only_in_word`] is: the loose loop's unit holds one loop and
/// nothing else, and this runs on a fraction of a percent of the words. Returns whether
/// both facts are now known, so the pass can stop.
#[cold]
#[inline(never)]
fn classify_word(bytes: &[u8], at: usize, w: u64, found: &mut Terminators) -> bool {
    let mut hits = zero_lanes(w ^ splat(b'\r')) | zero_lanes(w ^ splat(LINE_SEPARATOR_LEAD));
    while hits != 0 {
        let lane = (hits.trailing_zeros() / 8) as usize;
        if classify_at(bytes, at + lane, found) {
            return true;
        }
        hits &= hits - 1;
    }
    false
}

/// Record what the candidate byte at `at` is — the first `\r` seen, a U+2028 / U+2029, or
/// (a `0xE2` that leads another character, or a lane the SWAR kernel flagged spuriously)
/// nothing. Returns whether both facts are now known.
#[inline]
fn classify_at(bytes: &[u8], at: usize, found: &mut Terminators) -> bool {
    match bytes[at] {
        b'\r' => {
            if found.first_cr.is_none() {
                found.first_cr = Some(at);
            }
        }
        LINE_SEPARATOR_LEAD => {
            if line_terminator_len(bytes, at).is_some() {
                found.holds_separator = true;
            }
        }
        _ => {}
    }
    found.first_cr.is_some() && found.holds_separator
}

//
// The same three questions answered by a bounded SCAN of the source, with the table
// as the fallback
//

/// How many bytes past `prev_end` the scan forms of the three line questions walk before
/// handing the question to the table.
///
/// The table's binary search is `log2(lines)` dependent load-and-select steps — seven to
/// eleven on real files — and the printers ask it ~400,000 times per 12 MB of TypeScript.
/// The first terminator after `prev_end` is nearly always the end of the line the
/// question was asked on: a census over that corpus puts the bytes a forward scan reads
/// at a mean of **5.5** for `is_same_line` (82.7% within eight, 99.7% within 64), 2.6 for
/// the blank-line check and 3.2 for the newline check — one host word answers almost every
/// ask. The cap keeps a pathological line (a minified document) from turning each ask into
/// a walk of the whole line: past it the search runs exactly as before — and since the
/// table is built only when a fallback asks ([`LineBreaks`]), the cap is also what decides
/// how many documents build one at all. At 64 the fallback reached 241 of 1,666 documents
/// of that corpus, holding 29% of its bytes; at 128, 8 documents and 0.4% of the bytes;
/// at 256, none. The asks between 64 and 128 bytes are ~630 a pass — a few thousand
/// instructions against a builder run over 3.5 MB.
const LINE_SCAN_CAP: usize = 128;

/// [`next_lf_in`]'s and [`next_lf`]'s lane test and the byte compare their scalar tails run
/// must agree on every byte value — proved here rather than trusted (a uniform word cannot
/// borrow across lanes unless it is itself a match, so `hits != 0` is exactly the byte test).
const _: () = {
    let mut b = 0u16;
    while b < 256 {
        let byte = b as u8;
        let hits = zero_lanes(u64::from_le_bytes([byte; 8]) ^ splat(b'\n'));
        assert!((hits != 0) == (byte == b'\n'));
        b += 1;
    }
};

#[inline]
fn next_lf_in(bytes: &[u8], from: usize, end: usize) -> usize {
    debug_assert!(from <= end && end <= bytes.len());
    let mut i = from;
    while i < end {
        let Some(chunk) = bytes[i..].first_chunk::<8>() else {
            // Within eight bytes of the HOST's end — the one place no word is readable.
            while i < end && bytes[i] != b'\n' {
                i += 1;
            }
            return i;
        };
        let hits = zero_lanes(u64::from_le_bytes(*chunk) ^ splat(b'\n'));
        if hits != 0 {
            let at = i + (hits.trailing_zeros() / 8) as usize;
            return if at < end { at } else { end };
        }
        i += 8;
    }
    end
}

/// How many `\n` bytes lie in `[prev_end, curr_start)` — the count an LF-only line-break
/// table would give, read off the bytes instead: `Some(n)` with `n` saturated at `want`,
/// or `None` when the walk hit `cap` bytes past `prev_end` before the answer was known.
///
/// Reaching `curr_start` (or the host's end, past which the table holds nothing) settles
/// the count; only a walk stopped by the cap has no answer.
#[inline]
fn lf_count_in(
    bytes: &[u8],
    prev_end: usize,
    curr_start: usize,
    cap: usize,
    want: u8,
) -> Option<u8> {
    let complete_at = curr_start.min(bytes.len());
    let stop = complete_at.min(prev_end.saturating_add(cap));
    let mut found = 0u8;
    let mut i = prev_end;
    while i < stop {
        i = next_lf_in(bytes, i, stop);
        if i >= stop {
            break;
        }
        found += 1;
        if found == want {
            return Some(found);
        }
        i += 1;
    }
    (stop == complete_at).then_some(found)
}

/// [`is_same_line_fast`], answered by a bounded scan of `bytes` — the source the table
/// belongs to — with the table search as the fallback past [`LINE_SCAN_CAP`].
///
/// `table` carries the document's verdict ([`LineBreaks::lf_only`]): when it holds, the
/// table is the set of `\n` positions and a one-needle scan reads the same answer off the
/// bytes; when it does not (a bare `\r`, a U+2028 / U+2029 — none of which the format
/// path's CR fold leaves in a real document), the search runs as before, over a table
/// built on that first ask. Same answer as the table form at every position (the
/// exhaustive test beside it grades every position of every terminator shape at every
/// cap), and **an erased table is authoritative**: the canonical reprint erases the
/// layout table to erase authoring intent, and a scan that re-read the source would put
/// it back.
#[inline]
pub fn is_same_line_scan(
    bytes: &[u8],
    table: LineTable<'_>,
    prev_end: u32,
    curr_start: u32,
) -> bool {
    let answer = is_same_line_scan_capped(bytes, table, prev_end, curr_start, LINE_SCAN_CAP);
    debug_assert_eq!(
        answer,
        is_same_line_fast(table.breaks_for_grading(), prev_end, curr_start)
    );
    answer
}

/// [`has_blank_line_between_fast`], answered by a bounded scan of `bytes` — see
/// [`is_same_line_scan`] for the contract.
#[inline]
pub fn has_blank_line_between_scan(
    bytes: &[u8],
    table: LineTable<'_>,
    prev_end: u32,
    curr_start: u32,
) -> bool {
    let answer =
        has_blank_line_between_scan_capped(bytes, table, prev_end, curr_start, LINE_SCAN_CAP);
    debug_assert_eq!(
        answer,
        has_blank_line_between_fast(table.breaks_for_grading(), prev_end, curr_start)
    );
    answer
}

/// [`has_newline_between_fast`], answered by a bounded scan of `bytes` — see
/// [`is_same_line_scan`] for the contract.
#[inline]
pub fn has_newline_between_scan(bytes: &[u8], table: LineTable<'_>, start: u32, end: u32) -> bool {
    let answer = has_newline_between_scan_capped(bytes, table, start, end, LINE_SCAN_CAP);
    debug_assert_eq!(
        answer,
        has_newline_between_fast(table.breaks_for_grading(), start, end)
    );
    answer
}

// The table searches, outlined and cold, so the scan forms' hot unit holds ONE loop — and
// the table's fill lives behind them: the first of these a document reaches is what builds
// its table.
//
// ⚠️ Measured, not stylistic. With the searches inline, LLVM outlined each scan form as
// one ~120-instruction unit carrying both fallback searches: a seven-register prologue,
// the cap passed on the stack, and two 64-bit constants re-materialized INSIDE the word
// loop — ~60 instructions an ask, exactly what the search it replaced cost, and the
// lever read as a null (+0.036% of a TypeScript format run). The fallbacks run on a few
// hundred asks a pass (the cap) and on documents the format path never produces (a bare
// `\r`, a U+2028); they belong out of line.

#[cold]
#[inline(never)]
fn is_same_line_table(breaks: &LineBreaks<'_>, prev_end: u32, curr_start: u32) -> bool {
    is_same_line_fast(breaks.breaks(), prev_end, curr_start)
}

#[cold]
#[inline(never)]
fn has_blank_line_between_table(breaks: &LineBreaks<'_>, prev_end: u32, curr_start: u32) -> bool {
    has_blank_line_between_fast(breaks.breaks(), prev_end, curr_start)
}

#[cold]
#[inline(never)]
fn has_newline_between_table(breaks: &LineBreaks<'_>, start: u32, end: u32) -> bool {
    has_newline_between_fast(breaks.breaks(), start, end)
}

// `inline(always)` on the three `_capped` bodies, and it is a measured constraint, not a
// preference: each has ONE caller in a release build (its cap-free public form), and
// under plain `#[inline]` LLVM still outlined the blank-line body as a 148-instruction
// unit with a seven-register prologue and the cap passed on the stack — ~60 instructions
// an ask over 142,000 asks, the search's own price — while inlining the other two. The
// public form is what inlines into the printers; this layer exists only so the exhaustive
// test can grade every cap, and it must not cost a call. (A `const CAP` generic in its
// place, one instantiation per public form, was measured at +0.047 points of the lever on
// the TypeScript cell — the generic re-decided LLVM's inlining at the callers.)
#[expect(clippy::inline_always)]
#[inline(always)]
fn is_same_line_scan_capped(
    bytes: &[u8],
    table: LineTable<'_>,
    prev_end: u32,
    curr_start: u32,
    cap: usize,
) -> bool {
    if prev_end == curr_start {
        return true;
    }
    if prev_end > curr_start {
        return false;
    }
    // An erased table is authoritative (the canonical reprint's layout).
    let Some(breaks) = table.breaks else {
        return true;
    };
    if !table.lf_only {
        return is_same_line_table(breaks, prev_end, curr_start);
    }
    match lf_count_in(bytes, prev_end as usize, curr_start as usize, cap, 1) {
        Some(found) => found == 0,
        None => is_same_line_table(breaks, prev_end, curr_start),
    }
}

#[expect(clippy::inline_always)]
#[inline(always)]
fn has_blank_line_between_scan_capped(
    bytes: &[u8],
    table: LineTable<'_>,
    prev_end: u32,
    curr_start: u32,
    cap: usize,
) -> bool {
    if prev_end >= curr_start {
        return false;
    }
    let Some(breaks) = table.breaks else {
        return false;
    };
    if !table.lf_only {
        return has_blank_line_between_table(breaks, prev_end, curr_start);
    }
    match lf_count_in(bytes, prev_end as usize, curr_start as usize, cap, 2) {
        Some(found) => found == 2,
        None => has_blank_line_between_table(breaks, prev_end, curr_start),
    }
}

#[expect(clippy::inline_always)]
#[inline(always)]
fn has_newline_between_scan_capped(
    bytes: &[u8],
    table: LineTable<'_>,
    start: u32,
    end: u32,
    cap: usize,
) -> bool {
    if start >= end {
        return false;
    }
    let Some(breaks) = table.breaks else {
        return false;
    };
    if !table.lf_only {
        return has_newline_between_table(breaks, start, end);
    }
    match lf_count_in(bytes, start as usize, end as usize, cap, 1) {
        Some(found) => found == 1,
        None => has_newline_between_table(breaks, start, end),
    }
}

/// Whether every entry of a line-break table is a `\n` byte of `bytes` — the condition
/// under which the table is exactly the set of `\n` positions, so a one-needle scan of
/// the bytes answers what the table answers. The builder reports it for free
/// ([`build_line_breaks_into`]); this is the reference the test grades that report
/// against. A `\r\n` qualifies (its recorded byte IS the `\n`); a bare `\r` or a
/// U+2028 / U+2029 does not.
pub fn line_breaks_are_lf_only(bytes: &[u8], line_breaks: &[u32]) -> bool {
    line_breaks
        .iter()
        .all(|&p| bytes.get(p as usize) == Some(&b'\n'))
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

/// Like [`build_line_breaks`], filling a caller-provided (empty) table — the fill behind
/// [`LineBreaks::breaks`], into the arena-parked scratch (`DocArena::take_line_breaks_scratch`),
/// which runs only when a line question falls back to the table.
///
/// Returns whether the table is **LF-only** — every recorded byte is a `\n`, so the
/// table is exactly the set of `\n` positions. The document's verdict is taken ahead of
/// any table by [`line_terminators_are_lf_only`]; this is the builder's own re-derivation
/// of it, on the branch it already takes per line, graded against it wherever the table
/// is built ([`line_breaks_are_lf_only`] is the same fact read off the finished table).
pub fn build_line_breaks_into(source: &str, breaks: &mut Vec<u32>) -> bool {
    // Pre-size to ~one newline per 32 bytes (average code lines run ~25–40
    // bytes), so typical files fill in one allocation instead of the doubling
    // chain (a no-op once the parked table is warm). Capacity-only — never
    // affects the recorded values.
    breaks.reserve(source.len() / 32);
    build_line_breaks_bytes(source.as_bytes(), breaks)
}

/// [`build_line_breaks_into`]'s walk, over bytes and without the capacity reserve — the
/// shape the exhaustive equivalence test grades against its byte-at-a-time reference.
///
/// ⛔ **It re-enters [`next_line_terminator_candidate`] per line ON PURPOSE, and the obvious
/// repair was measured and refused.** That entry point answers "where is the NEXT candidate",
/// so this walk re-loads and re-masks the words holding each hit, once per line. Draining a
/// block's mask in place instead — answering every flagged lane before leaving the block —
/// removes exactly that, and it does show up: `instructions:u` **−0.191%** on the largest
/// corpus and **−0.302%** on a short-line one. It then **lost on cycles** (+0.29 points
/// against the null over twelve binaries and three pooled replicates, wall +0.21), for the
/// reason stated on the scan itself: this walk is latency-bound on the per-hit chain, and a
/// drain adds a compare and a `max` between the byte's classification and the next block's
/// address. It also taxed a 62%-non-ASCII document by +0.094%, because the exact-needle
/// fallback then routes per block rather than per word. **The per-line re-entry is the
/// cheaper shape; leave it.**
fn build_line_breaks_bytes(bytes: &[u8], breaks: &mut Vec<u32>) -> bool {
    let mut lf_only = true;
    let mut i = 0;
    while i < bytes.len() {
        i = next_line_terminator_candidate(bytes, i);
        if i >= bytes.len() {
            break;
        }
        // The `\n` arm first and alone: it is nearly every hit, and it is the one arm
        // that says nothing about `lf_only` — the flag is a fact about the RARE arms, so
        // it is written only there (a per-line `&=` on this arm measured as a per-line
        // cost on every short-line corpus, for a value that never changes here).
        if bytes[i] == b'\n' {
            breaks.push(i as u32);
            i += 1;
            continue;
        }
        match line_terminator_len(bytes, i) {
            // The recorded offset is the sequence's LAST byte, which is what the
            // LF-only builder recorded for both `\n` and `\r\n`. Every consumer
            // (`is_same_line_fast`, `has_blank_line_between_fast`) reads the table
            // as "one entry per line ending", so a multi-byte sequence must push
            // exactly once — two entries for a `\r\n` would read as a blank line.
            Some(len) => {
                let last = i + len - 1;
                // The recorded byte is the `\n` for a `\r\n`; a bare `\r` or a
                // `<LS>` / `<PS>` puts something else in the table.
                lf_only &= bytes[last] == b'\n';
                breaks.push(last as u32);
                i += len;
            }
            // A `0xE2` lead that is not `<LS>` / `<PS>` — the candidate scan's only
            // false positive, and the reason it is a CANDIDATE scan.
            None => i += 1,
        }
    }
    lf_only
}

/// Index of the first byte at or after `from` that could BEGIN a line terminator
/// sequence — `\n`, `\r`, or the `0xE2` lead of `<LS>` / `<PS>` — or `bytes.len()`.
///
/// The same word-at-a-time shape, and the same reason, as `location`'s
/// `next_ecmascript_terminator`: terminators are sparse (~1 per 30–40 source bytes), so a
/// per-byte compare spends nearly all of its work confirming misses, and this table is
/// built once over the whole source in every `format_in`. The `0xE2` lead is what
/// `location`'s does not look for — that one runs inside a run already proven ASCII, where
/// no `<LS>` / `<PS>` can occur; this one runs over the raw source, so it must not skip it.
/// `line_terminator_len` then classifies the hit, since most `0xE2` bytes begin some other
/// character.
///
/// ⭐ **The steady-state word test asks a WIDER question than the answer, because the
/// wider one is cheaper: `\n`, `\r`, or any non-ASCII byte.**
/// [`crate::swar::zero_or_high_lanes`] is [`crate::swar::zero_lanes`] with its `& !v` term
/// dropped, and that term is the only thing that excluded the non-ASCII lanes — so two
/// loose needles cost seven operations where three exact ones cost fourteen, and `<LS>` /
/// `<PS>`'s `0xE2` lead is inside the loose class for free. The word that fires is then
/// asked the exact question, so **the function's own class is unchanged** and no caller
/// sees the difference.
///
/// ⚠️ **The exact re-ask is what makes a non-ASCII-dense document affordable, and it is
/// not optional.** Returning the loose candidate to the caller instead reads `-0.354%` on
/// real source and **+20%** on a document that is 98% non-ASCII, because every such byte
/// becomes a hit the caller classifies and steps over one at a time. Re-asking keeps the
/// stride at eight bytes through a run of CJK or emoji, and it costs real source nothing:
/// a hit is only re-asked when its own word holds a non-ASCII byte at all.
///
/// `from_le_bytes` puts byte 0 in the low lane, so the lowest set bit is the earliest
/// match, and OR-ing masks preserves the kernels' lowest-lane guarantee: a spurious lane in
/// any one mask is preceded by a genuine one in that same mask. The loose mask is a
/// superset of the exact one lane for lane — every `\n`, `\r` and `0xE2` is `\n`, `\r` or
/// non-ASCII — so the exact answer can never sit BELOW the loose lane the byte test
/// rejected.
///
/// ⛔⛔ **Do not widen this loop. Eight bytes a word is where it stops paying, and that has
/// been measured twice.** Two words per bound check and per branch, and a variant that
/// drains every flagged lane before leaving a sixteen-byte block, each read `instructions:u`
/// **−0.166…−0.191%** across seven corpora at a per-side spread of 0.001% — and the blocked
/// one also removed **2.4% of the whole program's branch misses**, which is essentially this
/// function's entire share of them. **Both lost on cycles**: twelve binaries with three
/// pooled replicates, +0.45 and +0.29 points against the null, wall agreeing at +0.52 and
/// +0.21, every replicate positive. L1d misses and frontend stalls flat; IPC fell with the
/// instruction count.
///
/// The reason is the shape of the work, not the spelling of the fix: **the scan is
/// latency-bound on its per-hit chain, not throughput-bound on this loop.** That chain is
/// `trailing_zeros` → the hit offset → the load that classifies the byte → the sequence
/// length → the next hop's start address, and the machine already runs this loop several
/// iterations ahead of it. A wider block only halves bookkeeping that was being hidden, and
/// it *lengthens* the chain — a select over which word fired lands ahead of the
/// `trailing_zeros`. **byte → word** converts (see [`next_lf`]); **word → wider word** does
/// not. A short-line document makes it worse again: a hop shorter than the block still pays
/// for the whole block.
#[inline]
fn next_line_terminator_candidate(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        let loose = zero_or_high_lanes(w ^ splat(b'\n')) | zero_or_high_lanes(w ^ splat(b'\r'));
        if loose != 0 {
            if high_bit_lanes(w) == 0 {
                // An all-ASCII word, which is what nearly every hit is: with no lane at or
                // above `0x80` the loose mask IS the exact one, lane for lane, because the
                // `& !v` term `zero_or_high_lanes` drops is all-ones exactly there.
                return i + (loose.trailing_zeros() / 8) as usize;
            }
            // The word holds a non-ASCII byte, so the hit may be the loose class's own
            // false positive — and a non-ASCII byte rarely comes alone. Hand the rest of
            // this hop to the exact scan rather than re-asking word by word.
            return exact_line_terminator_candidate(bytes, i);
        }
        i += 8;
    }
    line_terminator_candidate_tail(bytes, i)
}

/// [`next_line_terminator_candidate`]'s answer, computed with three EXACT needles — the
/// scan the loose one approximates from above, and the fallback it hands a non-ASCII
/// stretch to.
///
/// Same class, same result, ~20 instructions a word against the loose loop's 15. It exists
/// because the loose loop's false positives are exactly the non-ASCII bytes, which come in
/// runs: a `<LS>`-free stretch of CJK or emoji would otherwise leave the loose loop once
/// per byte. Handing it the whole rest of the hop keeps that document at the cost it had
/// before the loose loop existed, and real source reaches this call once per few hundred
/// words.
///
/// ⚠️⚠️ **`#[inline(never)]` here IS the lever, not a refinement of it.** Marked
/// `#[inline]` instead, this body is folded into [`next_line_terminator_candidate`] — and
/// that grown body then loses ITS `#[inline]`, so the whole scan is emitted out of line and
/// every hop pays a call. The measurement collapses from **`-0.300%`** to **`-0.003%`**:
/// the lever is gone, not reduced. **The tell is free and it is the counter-intuitive
/// one** — inlining more source made `tsv`'s `.text` 160 bytes SMALLER (2,895,893 against
/// 2,896,053), because three inlined copies of the scan collapsing into one outweighs the
/// body this attribute adds. Check `objcopy -O binary --only-section=.text` before
/// believing any spelling of this pair.
#[cold]
#[inline(never)]
fn exact_line_terminator_candidate(bytes: &[u8], from: usize) -> usize {
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
    line_terminator_candidate_tail(bytes, i)
}

/// The candidate class asked a byte at a time, over the fewer-than-eight bytes both word
/// loops above stop short of.
///
/// One spelling, because it is the class ITSELF and the two loops reach it by different
/// routes — and because a scan whose tail disagrees with its word loop by one arm is the
/// bug no corpus finds: the tail runs only over a buffer's last seven bytes.
#[inline]
fn line_terminator_candidate_tail(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r' | LINE_SEPARATOR_LEAD) {
        i += 1;
    }
    i
}

/// The UTF-8 lead byte of `<LS>` (U+2028) and `<PS>` (U+2029) — `E2 80 A8/A9`, the only
/// multi-byte line terminators, spelled once for the scan and its scalar tail.
const LINE_SEPARATOR_LEAD: u8 = 0xE2;

/// Index of the first `\n` at or after `from` in `bytes`, or `bytes.len()`.
///
/// The LF-only member of this module's scan family — the question a *body split* asks,
/// where `\r` and `<LS>` / `<PS>` are ordinary content rather than line ends. Every
/// parse-then-format entry point folds `\r` away before it parses
/// ([`normalize_carriage_returns`]), and the doc pool's multi-line text is joined with
/// `\n` by construction, so a body's lines are its `\n`-separated runs and nothing else.
///
/// It exists because `str::split('\n')` is not this scan. `core`'s `CharSearcher` restarts
/// `memchr` at every line — an alignment offset, an unaligned prefix walked a byte at a
/// time and a sub-word tail walked the same way — and then re-verifies each hit against the
/// needle re-encoded as UTF-8, all for a one-byte ASCII needle. A block-comment body
/// averages ten lines and forty-five bytes a line, so that setup is paid ten times over a
/// stretch one continuous word loop crosses; the byte-at-a-time prefix and tail alone are
/// about half of what the search retires.
///
/// Reach for it where a body is split into lines on a hot path. A cold site splitting a
/// handful of lines gains nothing worth an inlined copy of this loop, and `str::split('\n')`
/// stays perfectly good there.
#[inline]
pub fn next_lf(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        let hits = zero_lanes(w ^ splat(b'\n'));
        if hits != 0 {
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

/// The class [`next_width_relevant`] scans for, asked of ONE byte.
///
/// One spelling, because the word loop and its scalar tail reach the same class by
/// different routes — and a scan whose tail disagrees with its word loop by one arm is
/// the bug no corpus finds: the tail runs only over a slice's last seven bytes.
///
/// Public for the same reason: a caller that SKIPS the scan (see
/// [`crate::doc::arena::DocArena::source_span_plain`]) owes a debug-build assertion that its
/// claim holds, and that assertion must ask this class rather than restate it.
#[inline]
pub const fn is_width_relevant(b: u8) -> bool {
    b == b'\n' || b == b'\t' || b >= 0x80
}

/// The word loop's lane test and [`is_width_relevant`] must answer identically for every
/// byte value — proved here rather than trusted, because a scan whose tail disagrees with
/// its word loop by one arm is the bug no corpus finds (the tail runs only over a
/// region's last seven bytes, and a width error changes nothing but a fits verdict).
///
/// A uniform word makes the two directly comparable: with every lane holding the same
/// byte, no lane can borrow from a neighbour unless it is itself a match, so `hits != 0`
/// is exactly "this byte is in the class". The mixed-word behaviour — the lowest set lane
/// is the genuine one — is what the runtime alignment sweep in `tests` grades.
const _: () = {
    let mut b = 0u16;
    while b < 256 {
        let byte = b as u8;
        let w = u64::from_le_bytes([byte; 8]);
        let hits = zero_or_high_lanes(w ^ splat(b'\n')) | zero_or_high_lanes(w ^ splat(b'\t'));
        assert!((hits != 0) == is_width_relevant(byte));
        b += 1;
    }
};

/// Index of the first byte in `bytes[from..end]` that is **not** a plain one-column ASCII
/// character — a `\t` (it is `tab_width` columns), a `\n` (it ends the line), or any
/// non-ASCII byte (its width needs the grapheme walk) — or `end`.
///
/// The question a width measure actually asks. A region with no such byte has a width
/// equal to its byte count, so the scan that finds none has *finished* the measurement —
/// no accumulator, no per-byte add. That is why this is a scan and not a fold: the
/// per-byte width sum it replaced cost **13 instructions a byte** (a load, two compares,
/// a width select, the add and the loop) where this costs **~1.9**, and the sum was
/// discarded in the same breath by the caller that only wanted `len`.
///
/// ⭐ **`bytes` is the HOST buffer and `[from, end)` is the region measured, and the gap
/// between the two is the point.** The word loop reads eight bytes *of the host*, so a
/// region shorter than eight bytes still gets one word test instead of falling to the
/// scalar walk — and the scalar walk is where this scan is expensive (**9 instructions a
/// byte** against the word rung's ~1.9, so a seven-byte region used to cost more than a
/// sixteen-byte one). Most regions asked about here are short: 61% of the document spans
/// a TypeScript format run measures are eight bytes or fewer, and only **72** of 640,428
/// sit within eight bytes of the document's end, where no word is readable.
///
/// The bytes past `end` are read and then **not believed**. `zero_or_high_lanes` never
/// misses a genuine match, so lanes below the lowest set one hold no class byte, and the
/// lowest set lane is itself genuine. A first hit at or past `end` therefore *proves* the
/// region holds none, and a hit before `end` is real. Reading the mask with
/// [`u64::trailing_zeros`] alone — which the note below requires anyway — is exactly what
/// that argument needs.
///
/// ⭐ **The steady-state word test asks a WIDER question than the answer, because the
/// wider one is cheaper — and here the wider question IS the answer.**
/// [`crate::swar::zero_or_high_lanes`] flags a lane that is zero *or* at or above `0x80`,
/// so `zero_or_high_lanes(w ^ splat(b'\n'))` flags `\n` **and every non-ASCII byte**, for
/// the price of the `\n` alone. Two of them are the whole class. Unlike
/// [`next_line_terminator_candidate`], where the non-ASCII lanes are the loose mask's
/// false positives and must be re-asked exactly, here they are wanted, so there is no
/// exact re-ask and no non-ASCII-dense degradation to guard against: a hit is a hit.
///
/// `from_le_bytes` puts byte 0 in the low lane, so the lowest set bit is the earliest
/// match, and OR-ing the two masks preserves the kernels' lowest-lane guarantee (a
/// spurious lane in either mask is preceded by a genuine one in that same mask). Read the
/// result with `trailing_zeros` only.
#[inline]
pub(crate) fn next_width_relevant_in(bytes: &[u8], from: usize, end: usize) -> usize {
    debug_assert!(from <= end && end <= bytes.len());
    let mut i = from;
    while i < end {
        let Some(chunk) = bytes[i..].first_chunk::<8>() else {
            // Within eight bytes of the HOST's end — the one place no word is
            // readable, and so the only place the scalar class test still runs.
            while i < end && !is_width_relevant(bytes[i]) {
                i += 1;
            }
            return i;
        };
        let w = u64::from_le_bytes(*chunk);
        let hits = zero_or_high_lanes(w ^ splat(b'\n')) | zero_or_high_lanes(w ^ splat(b'\t'));
        if hits != 0 {
            let at = i + (hits.trailing_zeros() / 8) as usize;
            return if at < end { at } else { end };
        }
        i += 8;
    }
    end
}

/// [`next_width_relevant_in`] over a whole buffer — the form for a slice that has no
/// host to borrow trailing bytes from (a pool-stored string, a `MultilineText` line).
#[inline]
pub(crate) fn next_width_relevant(bytes: &[u8], from: usize) -> usize {
    next_width_relevant_in(bytes, from, bytes.len())
}

/// Is `b` a **printable ASCII** byte — `0x20..=0x7e`?
///
/// The class [`next_non_printable_ascii`] scans for, asked of one byte, and the
/// only class [`visual_width_mixed`] can count without looking at the byte at all:
/// every member is exactly one column, so a stretch of them is as wide as it is
/// long. Its complement is the three things that are not — a `\t`
/// (`tab_width` columns), a control or `DEL` (**zero** columns, and this is the
/// arm that differs from [`visual_width`]'s pure-ASCII fast path on purpose), and
/// a non-ASCII byte (the grapheme walk's).
///
/// ⚠️ **Not [`is_width_relevant`]'s complement**, and the gap is the whole reason
/// this class is spelled separately: that one admits every control but `\t` and
/// `\n` as a one-column byte, because the caller it serves is a *span of source*,
/// where a control cannot appear. Here the caller counts [`ascii_char_width`],
/// which gives a control **0**, so a scan over the wider class would silently
/// over-count every string holding one. The [`ascii_char_width`] agreement is
/// proved below for every ASCII byte — the value that function is ever asked
/// about — and the non-ASCII half is the lane test's, in the same block.
#[inline]
const fn is_printable_ascii(b: u8) -> bool {
    b >= 0x20 && b < 0x7f
}

/// [`next_non_printable_ascii`]'s word test and [`is_printable_ascii`] must answer
/// identically for every byte value, and [`is_printable_ascii`] must be exactly the
/// bytes [`ascii_char_width`] gives a width of one — proved here rather than trusted.
///
/// The corpus cannot grade either claim: a 1,666-file TypeScript corpus holds
/// **7** tabs and **zero** other controls inside the half-megabyte of ASCII runs
/// this scan replaces, so a wrong class is a silent width error no real document
/// would reveal. A uniform word makes the lane test directly comparable — with
/// every lane holding the same byte no lane can borrow from a neighbour unless it
/// is itself a match — and the mixed-word behaviour is graded by the runtime
/// alignment sweep in `tests`.
const _: () = {
    let mut b = 0u16;
    while b < 256 {
        let byte = b as u8;
        let w = u64::from_le_bytes([byte; 8]);
        let hits = lanes_less_than(w, 0x20) | zero_or_high_lanes(w ^ splat(0x7f));
        assert!((hits == 0) == is_printable_ascii(byte));
        // [`ascii_char_width`] is only ever asked about an ASCII byte, so the
        // agreement is pinned there; the non-ASCII half is the lane test above,
        // which must flag every one of those bytes for the run to end on it. The
        // tab width is irrelevant to the equivalence — any value but 1 shows a
        // `\t` failing the class, which is the arm this exists to pin.
        assert!(byte >= 0x80 || is_printable_ascii(byte) == (ascii_char_width(byte, 2) == 1));
        b += 1;
    }
};

/// Index of the first byte at or after `from` that is **not** printable ASCII, or
/// `bytes.len()` when none is.
///
/// [`visual_width_mixed`]'s run counter: the bytes it steps over are one column
/// each, so the scan that finds none has *finished* measuring them — `stop - i`
/// is their width, with no accumulator and no per-byte select.
///
/// The per-byte fold it replaced cost **16 instructions a byte** against this word
/// rung's ~2, over **524,232** bytes a TypeScript format pass. `objdump`, in full —
/// the fold owes a width for every ASCII byte and leaves only on a non-ASCII one,
/// so the three-way select is branchless and stays in the loop body:
///
/// ```text
/// movzbl / test / js                                    the ASCII test and the exit
/// cmp $0x20 / setae / cmp $0x7f / setne / and /
///   cmp $0x9 / movzbl / cmove                           the width
/// add / inc / mov / cmp / jne                           the accumulate and the loop
/// ```
///
/// ⭐ **The class is two kernels, and the second one is free.** The controls come
/// from [`lanes_less_than`] at `0x20`; `zero_or_high_lanes(w ^ splat(0x7f))` is
/// `DEL` **and every non-ASCII byte** for the price of the `DEL` alone — and
/// non-ASCII is exactly where the run has to stop anyway, so the loose kernel's
/// usual false positives are the wanted answer here, the same way they are in
/// [`next_width_relevant_in`].
///
/// ⚠️ Both kernels borrow, so only the **lowest** set lane is genuine, and the OR
/// preserves that (a spurious lane in either mask is preceded by a genuine one in
/// the same mask). Read with [`u64::trailing_zeros`] only.
///
/// Unlike [`next_width_relevant_in`] this takes no host buffer to borrow trailing
/// bytes from: its caller holds a `&str`, not a span of a document. The scalar
/// tail therefore runs within eight bytes of the string's end — affordable
/// because the runs are long (a mean printable stretch of **31.8** bytes, with
/// 496 KB of the 524 KB in stretches of 16 or more).
#[inline]
fn next_non_printable_ascii(bytes: &[u8], from: usize) -> usize {
    let len = bytes.len();
    let mut i = from;
    while i < len {
        let Some(chunk) = bytes[i..].first_chunk::<8>() else {
            while i < len && is_printable_ascii(bytes[i]) {
                i += 1;
            }
            return i;
        };
        let w = u64::from_le_bytes(*chunk);
        let hits = lanes_less_than(w, 0x20) | zero_or_high_lanes(w ^ splat(0x7f));
        if hits != 0 {
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    len
}

/// [`is_width_relevant`] widened by the one byte a printed string literal also has to
/// know about: a `'`.
///
/// The two questions such a literal asks — which quote does it print with, and is its
/// width its byte length — read the same content bytes, and neither wants a position.
/// The quote is single unless a `'` is in there ([`optimal_string_quote`]); the width
/// is the byte count unless a width-relevant byte is. So finding no byte of the union
/// *is* both answers.
#[inline]
const fn is_width_relevant_or_single_quote(b: u8) -> bool {
    is_width_relevant(b) || b == b'\''
}

/// The union word test and [`is_width_relevant_or_single_quote`] must answer
/// identically for every byte value — the same proof the width class's own word loop
/// carries above, and for the same reason: the tail runs only over a region's last
/// seven bytes, so a class its word loop disagrees with is the bug no corpus finds.
const _: () = {
    let mut b = 0u16;
    while b < 256 {
        let byte = b as u8;
        let w = u64::from_le_bytes([byte; 8]);
        let hits = zero_or_high_lanes(w ^ splat(b'\n'))
            | zero_or_high_lanes(w ^ splat(b'\t'))
            | zero_or_high_lanes(w ^ splat(b'\''));
        assert!((hits != 0) == is_width_relevant_or_single_quote(byte));
        b += 1;
    }
};

/// [`next_width_relevant_in`] with a `'` added to its class — the one pass
/// [`optimal_string_quote_in`] answers both of its questions from.
///
/// ⭐ The third needle is **one more lane kernel**, not a second pass over the bytes:
/// `'` is ASCII, so `w ^ splat(b'\'')` is zero exactly at a quote and at or above
/// `0x80` exactly at a non-ASCII byte — which the width class already wanted. As in
/// [`next_width_relevant_in`], the loose kernel's non-ASCII lanes are not false
/// positives here but part of the answer, so there is no exact re-ask.
///
/// Same host-buffer contract as [`next_width_relevant_in`] — `bytes` is the whole
/// document and `[from, end)` the region asked about, so a region under eight bytes
/// still gets one word test — and the same reading rule: `trailing_zeros` only, a hit
/// at or past `end` proving the region holds none.
///
/// ⚠️ Unlike the sibling, whose caller uses the returned POSITION, the only caller here
/// tests `== end` — so clamping a hit past `end` back to `end` is what keeps the fast
/// arm REACHABLE, not what keeps the answer sound. It is load-bearing anyway: a
/// double-quoted literal's own closing delimiter is not in the class, so the word
/// straddling `end` routinely fires on the `\n` ending the statement, and returning
/// that position unclamped would send an ordinary literal down the cold arm.
#[inline]
fn next_width_relevant_or_single_quote_in(bytes: &[u8], from: usize, end: usize) -> usize {
    debug_assert!(from <= end && end <= bytes.len());
    let mut i = from;
    while i < end {
        let Some(chunk) = bytes[i..].first_chunk::<8>() else {
            // Within eight bytes of the HOST's end — the one place no word is
            // readable, and so the only place the scalar class test still runs.
            while i < end && !is_width_relevant_or_single_quote(bytes[i]) {
                i += 1;
            }
            return i;
        };
        let w = u64::from_le_bytes(*chunk);
        let hits = zero_or_high_lanes(w ^ splat(b'\n'))
            | zero_or_high_lanes(w ^ splat(b'\t'))
            | zero_or_high_lanes(w ^ splat(b'\''));
        if hits != 0 {
            let at = i + (hits.trailing_zeros() / 8) as usize;
            return if at < end { at } else { end };
        }
        i += 8;
    }
    end
}

/// `s.split('\n')` over [`next_lf`] — the same sequence, scanned a word at a time.
///
/// # Example
/// ```
/// use tsv_lang::printing::split_lf;
///
/// assert_eq!(split_lf("a\nb").collect::<Vec<_>>(), ["a", "b"]);
/// assert_eq!(split_lf("a\n").collect::<Vec<_>>(), ["a", ""]); // trailing empty line
/// assert_eq!(split_lf("").collect::<Vec<_>>(), [""]);          // one empty line
/// ```
#[inline]
pub fn split_lf(s: &str) -> SplitLf<'_> {
    SplitLf { rest: Some(s) }
}

/// [`split_lf`]'s iterator.
#[derive(Debug, Clone)]
pub struct SplitLf<'a> {
    /// The un-yielded remainder. `Some("")` and `None` are different states, and that
    /// distinction is the whole trailing-line rule: `"a\n"` yields a final empty line
    /// where `"a"` does not.
    rest: Option<&'a str>,
}

impl<'a> Iterator for SplitLf<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        let rest = self.rest?;
        let end = next_lf(rest.as_bytes(), 0);
        if end == rest.len() {
            self.rest = None;
            Some(rest)
        } else {
            self.rest = Some(&rest[end + 1..]);
            Some(&rest[..end])
        }
    }
}

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
/// ⚠️ **`source` is the document, but the indentation is the one acorn SAW** — and for four of
/// Svelte's readers those are different strings. `onComment` measures the line out of whatever
/// `parser.template` slice-and-splice its caller handed acorn, which may be a blanked prefix,
/// a `(pattern = 1)` wrapper or a `_ as ` insert; `prefix` is that fact, and
/// [`AcornPrefix::line_indentation`] is what reads it. Measuring the document instead strips
/// the author's own tab off a `value` Svelte leaves whole. [`AcornPrefix::DOCUMENT`] is the
/// identity, and is what every standalone parse passes.
///
/// The gate above this — a BLOCK comment whose content holds a `\n`, and only those
/// ([`crate::Comment::content_is_multiline`]) — and the `[ \t]` indentation class are Svelte's
/// too. All five spellings of both steps are pinned by
/// `tests/comment_dedent_line_terminators.rs`, and the five preparations by
/// `tests/comment_dedent_manufactured_source.rs`; each module doc says why a fixture cannot
/// carry what it holds.
///
/// # Examples
///
/// ```
/// use tsv_lang::{AcornPrefix, AcornPrefixText, printing::strip_comment_indentation};
///
/// // The comment opens at byte 2, on a line indented by two tabs, so two tabs come off the
/// // front of each of its lines.
/// let source = "\t\t/* a\n\t\tb */";
/// let doc = AcornPrefix::DOCUMENT;
/// assert_eq!(strip_comment_indentation(source, " a\n\t\tb ", 2, doc), " a\nb ");
///
/// // A `<LS>` inside the content opens a line just as a `\n` does.
/// assert_eq!(
///     strip_comment_indentation(source, " a\u{2028}\t\tb ", 2, doc),
///     " a\u{2028}b "
/// );
///
/// // Under a blanked prefix acorn saw two SPACES where those tabs are, so the tabs are not
/// // the indentation and the content rides out whole.
/// let blanked = AcornPrefix::manufactured(AcornPrefixText::Blanked, 2);
/// assert_eq!(strip_comment_indentation(source, " a\n\t\tb ", 2, blanked), " a\n\t\tb ");
/// ```
pub fn strip_comment_indentation(
    source: &str,
    content: &str,
    comment_start: u32,
    prefix: AcornPrefix,
) -> String {
    // The line the comment opens on — `\n` and nothing else, per the walk-back above, and
    // read out of what acorn SAW: a preparation that overwrites the author's newline opens
    // the line further back than the document does ([`AcornPrefix::line_start`]).
    let line_start = prefix.line_start(source, comment_start as usize);

    // The `[ \t]` run that opens that line, as acorn saw it.
    let indentation = prefix.line_indentation(source, line_start);
    if indentation.is_empty() {
        return content.to_string();
    }

    // Drop one copy of it wherever an `m`-mode `^` matches: the content's start, and the
    // position after every line terminator.
    let content_bytes = content.as_bytes();
    let mut result = String::with_capacity(content.len());
    let mut pos = 0;
    loop {
        let body_start = if content[pos..].starts_with(&*indentation) {
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
/// split on `'\n'` (typically [`split_lf`] fed directly) — no line
/// buffer is materialized, so classification never heap-allocates. Returns
/// `false` for single-line content. Mirrors prettier's `isIndentableBlockComment`.
///
/// ⚠️ The leading trim is [`is_js_whitespace`], because prettier's is
/// `line.trimStart()[0] === "*"` — `String.prototype.trimStart`, the JS `\s` class. Rust's
/// `str::trim_start` is `White_Space`, which disagrees at exactly the two witnesses and so
/// flipped the CLASSIFICATION in both directions: a `<ZWNBSP>*`-prefixed line reads as
/// `*`-aligned to prettier (which reindents the comment) but not to Rust, and a
/// `<NEL>*`-prefixed line the other way — and the two answers print through entirely
/// different emitters (reindented vs preserved verbatim), so the whole comment moves.
///
/// # Example
/// ```
/// use tsv_lang::printing::{is_indentable_block_comment, split_lf};
///
/// let lines = |s: &'static str| split_lf(s);
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
        if !prev.trim_start_matches(is_js_whitespace).starts_with('*') {
            return false;
        }
        prev = next;
    }
    // The last line qualifies when empty or `*`-prefixed.
    let last = prev.trim_start_matches(is_js_whitespace);
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
        #[expect(clippy::naive_bytecount)]
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
///
/// ⭐ **The run is counted by SEARCH, not by fold.** Every printable ASCII byte
/// is one column, so a stretch of them is as wide as it is long and
/// [`next_non_printable_ascii`] measures it by finding where it ends; only the
/// byte that ended it needs a width of its own. That byte is rare — a
/// 1,666-file TypeScript corpus stops **16,502** times over **524,232** run
/// bytes, i.e. about once per run, and **7** of those stops are a tab and none
/// is another control.
pub(crate) fn visual_width_mixed(s: &str, tab_width: usize) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut width = 0usize;
    let mut i = 0usize;
    while i < len {
        if bytes[i].is_ascii() {
            // The run is every ASCII byte from here, counted a printable stretch
            // at a time. `i` advances at least once before the `break` (the byte
            // that opened the run is ASCII: either the scan steps over it or the
            // `ascii_char_width` arm does), so the step back below is in bounds.
            loop {
                let stop = next_non_printable_ascii(bytes, i);
                width += stop - i;
                i = stop;
                if i == len {
                    return width;
                }
                let b = bytes[i];
                if !b.is_ascii() {
                    break;
                }
                // A `\t` or a control/DEL: still in the run, but not one column.
                width += ascii_char_width(b, tab_width);
                i += 1;
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
    fn optimal_string_quote_matches_reference_across_the_word_boundary() {
        // The exhaustive test above tops out at three characters, so it grades
        // only the tail compare chain: [`crate::swar::next_byte_of`]'s word loop
        // engages at EIGHT bytes, and the `'` gate is what decides whether the
        // counting arm runs at all. This sweeps contents long enough to cross
        // several words with each quote at every byte alignment, which is where a
        // lane-mask bug would live.
        fn reference(raw_content: &str) -> char {
            let single_count = raw_content.matches('\'').count();
            let double_count = raw_content.matches('"').count();
            if double_count < single_count {
                '"'
            } else {
                '\''
            }
        }
        for len in 0..=40usize {
            // No `'` anywhere, `"` at every stride — the gate's own claim, and it
            // is asserted against the CONSTANT rather than only against the
            // reference, so a gate and a reference that were wrong together would
            // still fail here.
            for stride in 1..=len.max(1) {
                let s: String = (0..len)
                    .map(|i| if i % stride == 0 { '"' } else { 'a' })
                    .collect();
                assert_eq!(optimal_string_quote(&s), '\'', "no-single content {s:?}");
                assert_eq!(optimal_string_quote(&s), reference(&s), "content {s:?}");
            }
            // A single `'` at every alignment, against a `"` at every other one —
            // both sides of the strict-inequality tie-break, word-crossing.
            for single_at in 0..len {
                for double_at in 0..len {
                    if double_at == single_at {
                        continue;
                    }
                    let mut s: Vec<u8> = vec![b'a'; len];
                    s[single_at] = b'\'';
                    s[double_at] = b'"';
                    let s = String::from_utf8(s).unwrap();
                    assert_eq!(optimal_string_quote(&s), reference(&s), "content {s:?}");
                }
                // And the same alignment with no `"` at all, so the counting arm
                // returns the OTHER quote and a gate that swallowed it is caught.
                let mut s: Vec<u8> = vec![b'a'; len];
                s[single_at] = b'\'';
                let s = String::from_utf8(s).unwrap();
                assert_eq!(optimal_string_quote(&s), '"', "single-only content {s:?}");
            }
        }
        // Multi-byte fillers pushed past the word boundary: a continuation byte
        // must never read as a quote lane. The quote goes both AFTER the fillers
        // and BETWEEN them, so a word holds a needle beside a continuation byte
        // rather than only past the last one.
        for pad in 0..12usize {
            for quotes in ["", "'", "\"", "'\"", "'''\""] {
                let filler = "é✓".repeat(pad);
                for s in [
                    format!("{filler}{quotes}"),
                    format!("{quotes}{filler}"),
                    format!("{filler}{quotes}{filler}"),
                ] {
                    assert_eq!(optimal_string_quote(&s), reference(&s), "content {s:?}");
                }
            }
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

    /// [`next_lf`] graded against a byte-at-a-time reference, at every length and
    /// alignment across the word stride.
    ///
    /// Its callers cannot grade its tail: [`split_lf`] and the comment builder both step a
    /// byte past each hit and re-enter, so a scan that gave up at the last short chunk
    /// would degrade into the callers' own walk and yield the same lines. The alphabet
    /// holds a non-ASCII lead and a continuation byte because the scan reads BYTES — a
    /// truncated UTF-8 sequence can sit at the very end of the slice, and `0x0A` never
    /// appears inside one.
    #[test]
    fn lf_scan_matches_the_scalar_reference_exhaustive() {
        const ALPHABET: [u8; 5] = [b'\n', b'\r', 0xE2, 0xA8, b'a'];
        fn reference(bytes: &[u8], from: usize) -> usize {
            let mut i = from;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            i
        }
        let mut buf = Vec::new();
        for align in 0..=16usize {
            for len in 0..=4usize {
                for mut code in 0..5usize.pow(len as u32) {
                    buf.clear();
                    buf.resize(align, b'x');
                    for _ in 0..len {
                        buf.push(ALPHABET[code % 5]);
                        code /= 5;
                    }
                    for from in 0..=buf.len() {
                        assert_eq!(
                            next_lf(&buf, from),
                            reference(&buf, from),
                            "align {align}, from {from}, bytes {buf:02x?}"
                        );
                    }
                }
            }
        }
    }

    /// [`next_width_relevant`] against a scalar scan of the same class, over every
    /// alignment of a hit within and across words.
    ///
    /// ⚠️ The **fillers** are the point. `zero_or_high_lanes` borrows across lanes, so a
    /// lane's flag is only trustworthy as the lowest set one; `0x00` and `0x08` sit under
    /// both needles and drive that borrow, and `0x7f` / `0x80` straddle the axis a borrow
    /// must not cross. ⚠️ And a **non-ASCII hit is a needle here, not a false positive** —
    /// the opposite of [`next_line_terminator_candidate`]'s loose class — so `0x80`,
    /// `0xc3` and `0xff` are graded as hits rather than as bytes to step over.
    #[test]
    fn next_width_relevant_matches_a_scalar_scan() {
        fn scalar(bytes: &[u8], from: usize) -> usize {
            let mut i = from;
            while i < bytes.len() && !is_width_relevant(bytes[i]) {
                i += 1;
            }
            i
        }
        // Fillers that exercise the borrow chain across lanes (zero, the
        // sub-needle 0x08, plain ASCII, the 0x7f/0x80 axis a borrow must not
        // cross) and every byte of the class as the hit.
        let filler = [0x00u8, 0x08, b'a', 0x7f, 0x80, 0xff];
        let hits = [b'\n', b'\t', 0x80u8, 0xc3, 0xff];
        for &f in &filler {
            for len in 0..40usize {
                for at in 0..=len {
                    for &h in &hits {
                        let mut v = vec![f; len];
                        if at < len {
                            v[at] = h;
                        }
                        for from in 0..=len {
                            assert_eq!(
                                next_width_relevant(&v, from),
                                scalar(&v, from),
                                "filler {f:#x}, hit {h:#x}, len {len}, at {at}, from {from}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// [`next_non_printable_ascii`] against a scalar scan of the same class, over
    /// every alignment of a hit within and across words.
    ///
    /// ⚠️ The **fillers** are the point, exactly as in the sweep above: both
    /// kernels borrow across lanes, so a lane's flag is trustworthy only as the
    /// lowest set one. `0x00` and `0x1f` drive `lanes_less_than`'s borrow chain,
    /// `0x7f` and `0x80` straddle the axis a borrow must not cross, and `0x20` is
    /// the class boundary on the other side. Every byte of the class is graded as
    /// the hit — including a non-ASCII one, which is a needle here and not a false
    /// positive.
    #[test]
    fn next_non_printable_ascii_matches_a_scalar_scan() {
        fn scalar(bytes: &[u8], from: usize) -> usize {
            let mut i = from;
            while i < bytes.len() && is_printable_ascii(bytes[i]) {
                i += 1;
            }
            i
        }
        let filler = [b'a', b' ', 0x20u8, 0x7e];
        let hits = [0x00u8, 0x09, 0x0a, 0x1f, 0x7f, 0x80, 0xc3, 0xff];
        for &f in &filler {
            for len in 0..40usize {
                for at in 0..=len {
                    for &h in &hits {
                        let mut v = vec![f; len];
                        if at < len {
                            v[at] = h;
                        }
                        for from in 0..=len {
                            assert_eq!(
                                next_non_printable_ascii(&v, from),
                                scalar(&v, from),
                                "filler {f:#x}, hit {h:#x}, len {len}, at {at}, from {from}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// [`visual_width_mixed`] against its reference over the shapes **no corpus
    /// holds**: a `\t` or a control at every alignment of a long ASCII run that
    /// also carries a non-ASCII byte.
    ///
    /// ⚠️ This is the grader the run-counting scan actually needs. A 1,666-file
    /// TypeScript corpus puts **7** tabs and **zero** other controls inside the
    /// 524,232 ASCII-run bytes `visual_width_mixed` measures, and the exhaustive
    /// triple product above never forms a word — so dropping a needle from
    /// [`is_printable_ascii`]'s class would be caught by nothing else, and it is a
    /// silent width error (it moves a fits verdict and no other observable).
    #[test]
    fn visual_width_mixed_matches_reference_with_specials_at_every_alignment() {
        // One of each arm the run counter must not fold away, plus the two
        // non-ASCII leads that end a run.
        const SPECIALS: [char; 6] = ['\t', '\u{0}', '\u{1f}', '\u{7f}', '\r', '\n'];
        for &special in &SPECIALS {
            for len in 0..24usize {
                for at in 0..len {
                    for tail in ["", "é", "中", "🙂", "e\u{0301}"] {
                        let mut s: String = "a".repeat(len);
                        s.replace_range(at..=at, &special.to_string());
                        s.push_str(tail);
                        for tw in [2usize, 4] {
                            assert_eq!(
                                visual_width_mixed(&s, tw),
                                visual_width_reference(&s, tw),
                                "special {special:?}, len {len}, at {at}, tail {tail:?}, tw {tw}"
                            );
                        }
                        // And with the non-ASCII LEADING, so the run the scan
                        // walks starts mid-string rather than at byte 0.
                        let lead = format!("{tail}{s}");
                        for tw in [2usize, 4] {
                            assert_eq!(
                                visual_width_mixed(&lead, tw),
                                visual_width_reference(&lead, tw),
                                "lead {special:?}, len {len}, at {at}, tail {tail:?}, tw {tw}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// [`split_lf`] is `str::split('\n')` — the oracle is the thing it replaces, and the
    /// only interesting cases are the empty-line ones the `Option<&str>` state exists for.
    ///
    /// Strings rather than raw bytes here, since that is the type the iterator hands back;
    /// the multi-byte characters make a boundary bug in the slicing visible.
    #[test]
    fn split_lf_matches_str_split_exhaustive() {
        const ALPHABET: [&str; 5] = ["\n", "a", "\r", "字", "🙂"];
        let mut s = String::new();
        for align in 0..=9usize {
            for len in 0..=4usize {
                for mut code in 0..5usize.pow(len as u32) {
                    s.clear();
                    for _ in 0..align {
                        s.push('x');
                    }
                    for _ in 0..len {
                        s.push_str(ALPHABET[code % 5]);
                        code /= 5;
                    }
                    assert_eq!(
                        split_lf(&s).collect::<Vec<_>>(),
                        s.split('\n').collect::<Vec<_>>(),
                        "align {align}, s {s:?}"
                    );
                }
            }
        }
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

    /// The same equivalence over inputs whose non-ASCII bytes fill WHOLE WORDS, which is
    /// the only shape that iterates `exact_line_terminator_candidate`'s loop: the alphabet
    /// tests above pad with ASCII, so their non-ASCII bytes never span eight bytes and the
    /// fallback always answers from the first word it is handed.
    ///
    /// Every run length across the stride, with a terminator placed at every offset inside
    /// the run and immediately after it — a `<LS>` inside a CJK run is exactly the case the
    /// loose scan cannot see and the fallback exists for.
    #[test]
    fn swar_line_break_scan_matches_the_scalar_reference_through_non_ascii_runs() {
        for run in 0..24usize {
            for terminator in ["\n", "\r\n", "\r", "\u{2028}", "\u{2029}", "\u{2603}"] {
                for cut in 0..=run {
                    let mut src = String::from("const x = 'start");
                    src.push_str(&"中".repeat(cut));
                    src.push_str(terminator);
                    src.push_str(&"中".repeat(run - cut));
                    src.push_str("end';\nlet y = 1;\n");
                    let mut actual = Vec::new();
                    build_line_breaks_bytes(src.as_bytes(), &mut actual);
                    assert_eq!(
                        actual,
                        reference_line_breaks(src.as_bytes()),
                        "run {run}, cut {cut}, terminator {terminator:?}"
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
        assert_eq!(normalize_carriage_returns("a\r\nb").text(), "a\nb");
        assert_eq!(normalize_carriage_returns("a\rb").text(), "a\nb");
        assert_eq!(normalize_carriage_returns("a\r\n\r\nb").text(), "a\n\nb");
        assert_eq!(normalize_carriage_returns("a\r\rb").text(), "a\n\nb");
        // `\n\r` is two terminators, not a pair — only `\r\n` is one.
        assert_eq!(normalize_carriage_returns("a\n\rb").text(), "a\n\nb");
        assert_eq!(normalize_carriage_returns("\r").text(), "\n");
        assert_eq!(normalize_carriage_returns("\r\n").text(), "\n");
    }

    /// A CR-free string comes back BORROWED — the fold runs ahead of every format, so the
    /// common document must not pay an allocation for it — and folding is idempotent.
    #[test]
    fn carriage_return_normalization_borrows_without_one_and_is_idempotent() {
        assert!(matches!(
            normalize_carriage_returns("a\nb\n").into_text(),
            Cow::Borrowed("a\nb\n")
        ));
        assert!(matches!(
            normalize_carriage_returns("").into_text(),
            Cow::Borrowed("")
        ));
        let once = normalize_carriage_returns("a\r\nb\rc")
            .into_text()
            .into_owned();
        assert!(matches!(
            normalize_carriage_returns(&once).into_text(),
            Cow::Borrowed(_)
        ));
        assert_eq!(once, "a\nb\nc");
    }

    /// The verdict the fold's pass takes is the verdict over the FOLDED text: a `\r` or a
    /// `\r\n` is a `\n` on the other side of the fold, so only a U+2028 / U+2029 can say
    /// no — and the fold does not move those. (The exhaustive test below grades the same
    /// claim at every alignment of every terminator shape; these are the shapes by name.)
    #[test]
    fn the_fold_takes_the_folded_texts_line_verdict() {
        assert!(normalize_carriage_returns("a\nb").lf_only());
        assert!(normalize_carriage_returns("a\r\nb").lf_only());
        assert!(normalize_carriage_returns("a\rb").lf_only());
        assert!(normalize_carriage_returns("a\u{2000}b\u{e9}").lf_only());
        assert!(!normalize_carriage_returns("a\u{2028}b").lf_only());
        assert!(!normalize_carriage_returns("a\u{2029}b").lf_only());
        assert!(!normalize_carriage_returns("a\u{2028}\r\nb").lf_only());
        assert!(!normalize_carriage_returns("a\r\n\u{2028}b").lf_only());
        // Both facts known early: the pass may stop, and the first `\r` is still the FIRST.
        let folded = normalize_carriage_returns("\r\u{2028}x\ry\r\nz");
        assert_eq!(folded.text(), "\n\u{2028}x\ny\nz");
        assert!(!folded.lf_only());
    }

    /// U+2028 / U+2029 are terminators to ECMAScript and ordinary characters to HTML and CSS
    /// text. Both formatters keep them where the author put them, and ECMAScript's own TRV
    /// keeps each as itself, so this fold must not reach them even though
    /// `line_terminator_len` counts them.
    #[test]
    fn carriage_return_normalization_leaves_line_and_paragraph_separators_alone() {
        assert_eq!(
            normalize_carriage_returns("a\u{2028}b").text(),
            "a\u{2028}b"
        );
        assert_eq!(
            normalize_carriage_returns("a\u{2029}b").text(),
            "a\u{2029}b"
        );
        assert_eq!(
            normalize_carriage_returns("a\u{2028}\r\nb").text(),
            "a\u{2028}\nb"
        );
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
            let stripped = strip_comment_indentation(
                &source,
                " a\n\t\tb ",
                comment_start,
                AcornPrefix::DOCUMENT,
            );
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
                strip_comment_indentation(
                    "\t/* x */",
                    &format!(" a\n\tb{term}\tc "),
                    1,
                    AcornPrefix::DOCUMENT
                ),
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

    /// The scan forms answer exactly as the table forms at EVERY byte position — inside
    /// a multi-byte terminator included — for every terminator shape at every alignment,
    /// and at every cap (so both the walk and the table fallback are graded, at the
    /// boundary where a terminator straddles the cap too); the document's up-front
    /// verdict ([`line_terminators_are_lf_only`]) agrees with the builder's and with the
    /// finished table at every alignment of every terminator, the word loop and its
    /// handoff included; and the table built on demand is the eager builder's. No corpus
    /// can grade this: a wrong answer moves a blank line or a trailing-comment
    /// classification only on a document that holds the shape, and the shapes that matter
    /// (a `\r` the format path folds away, a U+2028) appear in none.
    #[test]
    fn line_break_scan_fns_agree_with_the_table_at_every_position_and_cap() {
        // `\u{2000}` is an `0xE2`-led character that is NOT a terminator (the candidate
        // scan's false positive); `é` a non-ASCII byte with a different lead.
        let pieces = ["a", "\n", "\r", "\u{2028}", "\u{2029}", "\u{2000}", "é"];
        let caps = [0usize, 1, 2, 3, 4, 7, 8, 9, 16, 64];
        let mut checked = 0usize;
        let mut lf_only_documents = 0usize;
        let mut source = String::new();
        let mut padded = String::new();
        // Every sequence of up to five pieces (7^5 = 16,807 documents).
        for len in 0..=5 {
            let mut counters = vec![0usize; len];
            loop {
                source.clear();
                for &k in &counters {
                    source.push_str(pieces[k]);
                }
                let bytes = source.as_bytes();
                let mut breaks = Vec::new();
                let lf_only = build_line_breaks_into(&source, &mut breaks);
                // The builder's verdict is the table's own fact, re-derived — and the
                // up-front verdict is the same fact, at every alignment the word loop can
                // meet a piece at (a prefix of 0..=17 plain bytes puts every piece in
                // every lane of a word and in the tail, with and without a non-ASCII byte
                // ahead of it).
                assert_eq!(
                    lf_only,
                    line_breaks_are_lf_only(bytes, &breaks),
                    "lf_only {source:?}"
                );
                for pad in 0..=17 {
                    padded.clear();
                    for _ in 0..pad {
                        padded.push('x');
                    }
                    padded.push_str(&source);
                    assert_eq!(
                        line_terminators_are_lf_only(padded.as_bytes()),
                        lf_only,
                        "verdict {padded:?}"
                    );
                    let mut ahead = String::from("é");
                    ahead.push_str(&padded);
                    assert_eq!(
                        line_terminators_are_lf_only(ahead.as_bytes()),
                        lf_only,
                        "verdict with a non-ASCII byte ahead {ahead:?}"
                    );
                    // The fold's own pass states the folded text's verdict — the up-front
                    // verdict over the bytes it returns — at the same alignments.
                    for document in [&padded, &ahead] {
                        let folded = normalize_carriage_returns(document);
                        assert_eq!(
                            folded.lf_only(),
                            line_terminators_are_lf_only(folded.text().as_bytes()),
                            "fold verdict {document:?}"
                        );
                        assert!(!folded.text().contains('\r'), "fold left a CR {document:?}");
                        assert_eq!(
                            matches!(folded.into_text(), Cow::Borrowed(_)),
                            !document.contains('\r'),
                            "fold borrowed/copied wrongly {document:?}"
                        );
                    }
                }
                if lf_only {
                    lf_only_documents += 1;
                }
                let lazy = LineBreaks::new(&source, Vec::new());
                assert_eq!(lazy.lf_only(), lf_only, "{source:?}");
                let table = lazy.table();
                // Every position pair, out-of-range ones included (the table forms take
                // any `u32`, so the scan forms must too).
                for p in 0..=bytes.len() + 2 {
                    for c in p.saturating_sub(1)..=bytes.len() + 2 {
                        let (p, c) = (p as u32, c as u32);
                        for &cap in &caps {
                            assert_eq!(
                                is_same_line_scan_capped(bytes, table, p, c, cap),
                                is_same_line_fast(&breaks, p, c),
                                "is_same_line {source:?} {p},{c} cap {cap}"
                            );
                            assert_eq!(
                                has_blank_line_between_scan_capped(bytes, table, p, c, cap),
                                has_blank_line_between_fast(&breaks, p, c),
                                "has_blank_line_between {source:?} {p},{c} cap {cap}"
                            );
                            assert_eq!(
                                has_newline_between_scan_capped(bytes, table, p, c, cap),
                                has_newline_between_fast(&breaks, p, c),
                                "has_newline_between {source:?} {p},{c} cap {cap}"
                            );
                            checked += 1;
                        }
                    }
                }
                // The table built on demand is the eager builder's, and it comes back
                // out for parking.
                assert_eq!(lazy.breaks(), &breaks[..], "{source:?}");
                assert_eq!(lazy.into_scratch(), breaks, "{source:?}");
                // Advance the odometer.
                let mut i = 0;
                loop {
                    if i == len {
                        break;
                    }
                    counters[i] += 1;
                    if counters[i] < pieces.len() {
                        break;
                    }
                    counters[i] = 0;
                    i += 1;
                }
                if i == len {
                    break;
                }
            }
        }
        assert!(checked > 1_000_000, "{checked}");
        // Both arms were reached: the LF-only scan and the table fallback.
        assert!(lf_only_documents > 1_000, "{lf_only_documents}");
        assert!(
            checked > lf_only_documents * 10,
            "{checked} {lf_only_documents}"
        );

        // An erased table is authoritative (the canonical reprint's erased layout), even
        // over a source full of terminators — under either verdict.
        let bytes = b"a\n\nb\n\nc";
        for lf_only in [true, false] {
            let erased = LineTable {
                breaks: None,
                lf_only,
            };
            assert!(is_same_line_scan(bytes, erased, 0, 7));
            assert!(!has_blank_line_between_scan(bytes, erased, 0, 7));
            assert!(!has_newline_between_scan(bytes, erased, 0, 7));
        }
        assert!(is_same_line_scan(bytes, LineTable::EMPTY, 0, 7));

        // The cap hands a long line to the table — built on that first ask, and not
        // before — and a `\n` straddling the cap is still counted once.
        let long = format!("{}\n\n{}", "x".repeat(200), "y".repeat(200));
        let mut breaks = Vec::new();
        assert!(build_line_breaks_into(&long, &mut breaks));
        let lazy = LineBreaks::of(&long);
        assert!(lazy.lf_only());
        assert!(lazy.table.get().is_none());
        // (The `_capped` forms: the public ones grade every ask against the table in a
        // debug build, which builds it.)
        assert!(is_same_line_scan_capped(
            long.as_bytes(),
            lazy.table(),
            0,
            3,
            LINE_SCAN_CAP
        ));
        assert!(
            lazy.table.get().is_none(),
            "answered within the cap: no table"
        );
        assert!(!is_same_line_scan_capped(
            long.as_bytes(),
            lazy.table(),
            0,
            250,
            LINE_SCAN_CAP
        ));
        assert!(
            lazy.table.get().is_some(),
            "past the cap: the table was built"
        );
        let table = lazy.table();
        for p in 0..long.len() as u32 {
            for c in [p, p + 1, 150, 201, 202, 250, long.len() as u32] {
                assert_eq!(
                    is_same_line_scan(long.as_bytes(), table, p, c),
                    is_same_line_fast(&breaks, p, c)
                );
                assert_eq!(
                    has_blank_line_between_scan(long.as_bytes(), table, p, c),
                    has_blank_line_between_fast(&breaks, p, c)
                );
            }
        }

        // A `\r\n` document is LF-only (the recorded byte is the `\n`); a bare `\r` and
        // a U+2028 are not — by the builder and by the up-front verdict alike.
        let mut breaks = Vec::new();
        assert!(build_line_breaks_into("a\r\nb\r\n", &mut breaks));
        assert_eq!(breaks, vec![2, 5]);
        assert!(LineBreaks::of("a\r\nb\r\n").lf_only());
        assert_eq!(LineBreaks::of("a\r\nb\r\n").breaks(), &[2, 5]);
        assert!(!build_line_breaks_into("a\rb", &mut Vec::new()));
        assert!(!LineBreaks::of("a\rb").lf_only());
        assert!(!build_line_breaks_into("a\u{2028}b\n", &mut Vec::new()));
        assert!(!LineBreaks::of("a\u{2028}b\n").lf_only());
        // A non-LF-only document's asks all go to the table, built on the first.
        let ls = LineBreaks::of("a\u{2028}b\nc");
        assert!(ls.table.get().is_none());
        assert!(!is_same_line_scan_capped(
            ls.source,
            ls.table(),
            0,
            5,
            LINE_SCAN_CAP
        ));
        assert!(ls.table.get().is_some());
    }
}
