// ValueParser - Same-source recursive parser for CSS values
//
// Maintains ONE source string throughout entire parse tree to avoid position drift.
// Tracks (start, end) ranges within that source instead of creating substrings.

use crate::ast::internal::CssValue;
use crate::parser::value::cursor::ValueCursor;
use crate::parser::value::lists::{ValueSeparator, classify_separators};
use crate::parser::value::scan::value_skip_table;
use crate::whitespace::is_css_whitespace;
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;

/// [`ValueParser::fast_scan`]'s skip table. Beyond the structural set it must see the
/// comment introducer (`/`), and both separators it classifies on — the comma and
/// ASCII whitespace.
///
/// The exhaustiveness argument, which is what makes the skip byte-identical: a skipped
/// byte reaches no arm of the loop. It is not `/` (comment probe), not `\` (escape
/// probe), not a paren or quote (nesting), not `,` and not whitespace (the separators)
/// — and the one remaining arm, the in-quote close (`b == quote_char`), can't match it
/// either, because `quote_char` only ever holds `'` or `"`, both structural.
const FAST_SCAN_SKIP: [bool; 256] =
    value_skip_table!(|b| b == b'/' || b == b',' || b.is_ascii_whitespace());

/// Skip table for the comma-list split's delimiter, `|c| c == ','`.
const COMMA_SKIP: [bool; 256] = value_skip_table!(|b| b == b',');

/// Skip table for the whitespace-list split's delimiter, [`is_css_whitespace`] — which
/// is `char::is_ascii_whitespace`, so the table is total over it. Note this deliberately
/// does **not** mirror `char::is_whitespace`, whose ASCII half also includes U+000B
/// (vertical tab).
const CSS_WS_SKIP: [bool; 256] = value_skip_table!(|b| b.is_ascii_whitespace());

/// Position-tracking parser for CSS values
///
/// Maintains ONE source string throughout entire parse tree.
/// Tracks (start, end) ranges within that source.
/// All spans calculated relative to SAME source → no position drift.
///
/// # Example
/// ```ignore
/// // Internal API - not publicly accessible
/// use tsv_css::parser::value::parser::ValueParser;
/// use tsv_lang::Span;
///
/// let source = "red 01%, blue 02%";
/// let span = Span { start: 100, end: 117 };
/// let parser = ValueParser::new(source, span);
/// let value = parser.parse();
/// // All nested values have accurate spans pointing to correct bytes in source
/// ```
#[derive(Debug)]
pub(crate) struct ValueParser<'a> {
    /// Original value string (NEVER changes through recursion)
    source: &'a str,

    /// Parse range start in source (current level's slice start)
    start: usize,

    /// Parse range end in source (current level's slice end)
    end: usize,

    /// Base offset in full CSS document (for absolute span calculation)
    base_offset: u32,
}

/// Outcome of `ValueParser::fast_scan` — the single-pass classification of a value.
enum FastScan<'arena> {
    /// A top-level comma was found; the comma list is already split and parsed.
    Comma(&'arena [CssValue<'arena>]),
    /// No comma but a top-level whitespace run — a whitespace list.
    Whitespace,
    /// Neither a comma nor top-level whitespace — a single leaf value.
    Leaf,
    /// A `/* */` comment was found; take the comment-aware two-pass fallback.
    Comment,
}

/// ASCII bytes that `str::trim` (via `char::is_whitespace`) strips: the CSS
/// whitespace set plus U+000B (vertical tab), which `char::is_whitespace`
/// includes and `is_css_whitespace` does not. A conservative superset of what the
/// boundary trims themselves strip (CSS whitespace only, via
/// [`crate::escapes::trim_start_css`] / [`crate::escapes::trim_end_preserving_escape`],
/// which exclude U+000B). Used only to check — from a value range's boundary bytes —
/// whether it is already trimmed, so the fast path can skip a redundant trim; any
/// non-ASCII boundary byte conservatively takes the trimming path instead (where a
/// non-ASCII space is preserved, not dropped).
const fn is_trim_boundary_ws(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ')
}

impl<'a> ValueParser<'a> {
    /// Create parser from value string and its span in full CSS
    ///
    /// # Arguments
    /// * `source` - The value string to parse
    /// * `base_span` - The span of this value in the full CSS document
    ///
    /// # Example
    /// ```ignore
    /// let parser = ValueParser::new("red, blue", Span { start: 50, end: 59 });
    /// ```
    pub fn new(source: &'a str, base_span: Span) -> Self {
        Self {
            source,
            start: 0,
            end: source.len(),
            base_offset: base_span.start,
        }
    }

    /// Get current parse text (zero-cost slice)
    ///
    /// Returns a view into the source string for the current parse range.
    /// No allocation - just returns a slice reference.
    fn text(&self) -> &'a str {
        &self.source[self.start..self.end]
    }

    /// Get absolute span for current range
    ///
    /// Calculates the span in the full CSS document by adding base_offset
    /// to the current range positions.
    fn absolute_span(&self) -> Span {
        Span {
            start: self.base_offset + self.start as u32,
            end: self.base_offset + self.end as u32,
        }
    }

    /// Create sub-parser for a range (same source, different start/end)
    ///
    /// This is the key to avoiding position drift: we create a new parser
    /// that tracks a different range in the SAME source string, rather than
    /// creating a substring.
    ///
    /// # Arguments
    /// * `range_start` - Start offset relative to current start
    /// * `range_end` - End offset relative to current start
    ///
    /// # Returns
    /// New parser with same source but adjusted range
    fn sub_parser(&self, range_start: usize, range_end: usize) -> ValueParser<'a> {
        ValueParser {
            source: self.source,             // ✅ SAME source!
            start: self.start + range_start, // Offset into same source
            end: self.start + range_end,
            base_offset: self.base_offset, // Same base offset
        }
    }

    /// Calculate trimmed end position for a text range
    ///
    /// Removes trailing whitespace from a range and returns the adjusted end position —
    /// except whitespace a CSS escape owns (`var(--a, 50px\ )`), which stays part of the
    /// element (see `super::trim_end_preserving_escape`).
    ///
    /// # Arguments
    /// * `text` - The full text being parsed
    /// * `start` - Start position in text
    /// * `end` - End position in text (may include trailing whitespace)
    ///
    /// # Returns
    /// End position after removing trailing whitespace
    fn trimmed_end(&self, text: &str, start: usize, end: usize) -> usize {
        let slice = &text[start..end];
        start + crate::escapes::trim_end_preserving_escape(slice).len()
    }

    /// Main parse entry point
    ///
    /// Determines the value's structure and delegates. The parse range is trimmed
    /// at every level (both `ValueParser::new` entry points pass a trimmed string,
    /// and every `sub_parser` slice trims its bounds), so a value whose boundary
    /// bytes are ASCII and non-whitespace takes a single fused pass (`fast_scan`):
    /// a comma list is split and its elements built inline (no second `ValueCursor`
    /// walk), a whitespace list or single leaf dispatches to the same builders as
    /// before, and — because `text` is already trimmed — the leaf is built without
    /// re-trimming. A value with a `/* */` comment falls back to the original
    /// two-pass path — `classify_separators` is comment-*aware* while the
    /// `ValueCursor` split is comment-*blind*, and the two deliberately disagree on
    /// a comment between value tokens, so the single-string fast pass hands such a
    /// value off unchanged. A whitespace or non-ASCII boundary byte (rare, since the
    /// range is trimmed) also takes the two-pass path, which trims first —
    /// CSS-whitespace-only (`trim_start_css` / `trim_end_preserving_escape`), so an
    /// ASCII-whitespace boundary comes off (identical to what the fast path would
    /// produce) while a non-ASCII space (NBSP, em space) is kept as leaf content,
    /// never dropped.
    pub fn parse<'arena>(&self, arena: &'arena Bump) -> CssValue<'arena> {
        let text = self.text();
        let bytes = text.as_bytes();

        // Fast path: confirm `text` is already trimmed straight from its boundary
        // bytes (ASCII and non-whitespace ends ⇒ the ASCII trim is a no-op) and skip
        // the redundant trim the two-pass path runs. An ASCII-whitespace boundary
        // (`text` is trimmed in practice, so this is rare) or a non-ASCII boundary
        // byte falls through to the trimming path below: an ASCII-whitespace end is
        // trimmed there (same value), while a non-ASCII space (NBSP, em space) is
        // kept — it is CSS value content, not whitespace.
        if let (Some(&first), Some(&last)) = (bytes.first(), bytes.last())
            && first < 0x80
            && last < 0x80
            && !is_trim_boundary_ws(first)
            && !is_trim_boundary_ws(last)
        {
            match self.fast_scan(text, arena) {
                FastScan::Comma(values) => {
                    return CssValue::CommaSeparated {
                        values,
                        span: self.absolute_span(),
                    };
                }
                FastScan::Whitespace => return self.parse_space_separated(arena),
                FastScan::Leaf => return self.build_leaf(text, arena),
                // A `/* */` comment was found — defer to the comment-aware path.
                FastScan::Comment => {}
            }
        }

        // Fallback (comment present, a non-ASCII/whitespace boundary, or a
        // non-trimmed range): trim, then classify comment-aware and split with the
        // comment-blind `ValueCursor` — the original behaviour.
        let trimmed =
            crate::escapes::trim_end_preserving_escape(crate::escapes::trim_start_css(text));
        if trimmed.is_empty() {
            return CssValue::Identifier {
                span: self.absolute_span(),
            };
        }
        match classify_separators(trimmed) {
            ValueSeparator::Comma => self.parse_comma_separated(arena),
            ValueSeparator::Whitespace => self.parse_space_separated(arena),
            ValueSeparator::None => self.parse_single(arena),
        }
    }

    /// One fused pass over the (already-trimmed) value `text`, doing the work of
    /// `classify_separators` and the comma arm of `split_top_level` at once:
    ///
    /// - a top-level comma commits the value to a comma list, whose elements are
    ///   split and parsed inline here — skipping the second `ValueCursor` walk the
    ///   old comma path took;
    /// - otherwise a top-level whitespace run makes it a whitespace list and a bare
    ///   run makes it a single leaf, both handled by the existing builders, so the
    ///   fast pass only reports which;
    /// - a `/* */` comment outside quotes is reported so the caller takes the
    ///   comment-aware two-pass path (the comment-blind split here would diverge).
    ///
    /// Paren/quote nesting is tracked exactly as `classify_separators` and
    /// `ValueCursor` track it, so for a comment-free value the comma split is
    /// byte-for-byte identical to the old two-pass result.
    fn fast_scan<'arena>(&self, text: &str, arena: &'arena Bump) -> FastScan<'arena> {
        let bytes = text.as_bytes();
        let mut in_parens: u32 = 0;
        let mut in_quote = false;
        let mut quote_char = 0u8;
        let mut ws_seen = false;

        let mut values: BumpVec<'arena, CssValue<'arena>> = BumpVec::new_in(arena);
        let mut seg_start = 0usize; // start of the current comma segment
        let mut any_comma = false;
        let mut pushed = false; // any non-empty element emitted (for the leaf guard)

        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];

            // Most of a value's bytes are inert content, which no arm below can act
            // on (see `FAST_SCAN_SKIP`). One L1 load retires them ahead of the whole
            // comment/escape/nesting/separator branch chain.
            if FAST_SCAN_SKIP[b as usize] {
                i += 1;
                continue;
            }

            // A block comment outside quotes: the comment-blind split below would
            // treat the `,`/space inside it as separators, so hand the whole value
            // to the comment-aware fallback instead.
            if !in_quote && b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                return FastScan::Comment;
            }

            // Delimiter tests use the nesting level as of *before* this byte, then
            // the byte updates the nesting — the same order `ValueCursor` uses.
            //
            // An escape is OPAQUE: step over it whole (`crate::escapes::escape_len`), so
            // nothing inside it reads as structure. Every byte it can hide is one this
            // scanner would otherwise act on — an escaped paren (`url(  a\)b  )` would drop
            // the depth to 0 and expose the interior whitespace as a false top-level
            // separator, mis-splitting the opaque url-token); an escaped comma (`x\,y`)
            // would split one ident in two, which the comma path then rejoins with `", "`,
            // inserting a space *inside* the ident; an escaped space (`xxxxx\ yyyyy`) would
            // become a wrap point, and `\` before a newline is not a valid escape (§4.3.4),
            // so the output stops re-parsing. A hex escape's whitespace terminator is part
            // of the escape too (`\41 2px` is ONE ident, §4.3.7).
            //
            // Kept identical in the twin `ValueCursor` / `classify_separators` trackers
            // (the fused-pass invariant).
            if b == b'\\'
                && let Some(len) = crate::escapes::escape_len(text, i)
            {
                i += len;
                continue;
            }
            let top = in_parens == 0 && !in_quote;
            match b {
                b'\'' | b'"' if !in_quote => {
                    in_quote = true;
                    quote_char = b;
                }
                _ if in_quote && b == quote_char => in_quote = false,
                b'(' if !in_quote => in_parens += 1,
                b')' if !in_quote => in_parens = in_parens.saturating_sub(1),
                b',' if top => {
                    self.push_comma_segment(&mut values, text, seg_start, i, &mut pushed, arena);
                    any_comma = true;
                    seg_start = i + 1;
                }
                _ if top && b.is_ascii_whitespace() => ws_seen = true,
                _ => {}
            }

            i += 1;
        }

        if any_comma {
            // Final segment runs to EOF (`ve_raw == text.len()`), which arms the
            // leaf guard when it is the first non-empty element (a leading-comma
            // value like `,a b`, matching `split_top_level`).
            self.push_comma_segment(
                &mut values,
                text,
                seg_start,
                bytes.len(),
                &mut pushed,
                arena,
            );
            FastScan::Comma(values.into_bump_slice())
        } else if ws_seen {
            FastScan::Whitespace
        } else {
            FastScan::Leaf
        }
    }

    /// Emit one comma-list element for the raw segment `text[seg_start..seg_end]`,
    /// reproducing `split_top_level`'s per-element handling: skip leading and trim
    /// trailing whitespace, drop an empty element, and parse the first non-empty
    /// element that runs to EOF as a single leaf (the progress guard). `seg_end` is
    /// the `ve_raw` position — a comma index, or `text.len()` for the final segment.
    fn push_comma_segment<'arena>(
        &self,
        values: &mut BumpVec<'arena, CssValue<'arena>>,
        text: &str,
        seg_start: usize,
        seg_end: usize,
        pushed: &mut bool,
        arena: &'arena Bump,
    ) {
        let seg = &text[seg_start..seg_end];
        // Match `split_top_level`'s asymmetric trimming exactly: leading whitespace is
        // skipped ASCII-only (like `ValueCursor::skip_whitespace`), trailing is trimmed
        // CSS-whitespace-only and escape-aware (like `trimmed_end`'s
        // `trim_end_preserving_escape`). A non-ASCII space (e.g. NBSP) on either end
        // therefore stays part of the element — it is CSS value content, not a
        // separator — rather than being dropped (which, for a string element, would
        // delete the whole element).
        let after_lead = seg.trim_start_matches(|c: char| c.is_ascii_whitespace());
        let core = crate::escapes::trim_end_preserving_escape(after_lead);
        if core.is_empty() {
            return;
        }
        let value_start = seg_start + (seg.len() - after_lead.len());
        let value_end = value_start + core.len();
        let sub = self.sub_parser(value_start, value_end);
        // Same guard as `split_top_level`: a first non-empty element whose raw end
        // reaches EOF is parsed as a single leaf (the classify/cursor disagreement
        // safety, reachable comment-free only via leading delimiters).
        let node = if !*pushed && seg_end == text.len() {
            sub.parse_single(arena)
        } else {
            sub.parse(arena)
        };
        values.push(node);
        *pushed = true;
    }

    /// Parse comma-separated values: "a, b, c"
    fn parse_comma_separated<'arena>(&self, arena: &'arena Bump) -> CssValue<'arena> {
        CssValue::CommaSeparated {
            values: self.split_top_level(arena, |c| c == ',', &COMMA_SKIP),
            span: self.absolute_span(),
        }
    }

    /// Parse space-separated values: "a b c"
    fn parse_space_separated<'arena>(&self, arena: &'arena Bump) -> CssValue<'arena> {
        CssValue::List {
            values: self.split_top_level(arena, is_css_whitespace, &CSS_WS_SKIP),
            span: self.absolute_span(),
        }
    }

    /// Split the current range into top-level values at `is_delimiter`, parsing
    /// each recursively.
    ///
    /// Uses same-source recursion — every parsed value points to a range in the
    /// SAME source string, avoiding position drift. Leading/trailing whitespace
    /// around each element is trimmed and empty elements are dropped, so both
    /// the comma form (`a, b, c`) and the whitespace form (`a b c`, where runs
    /// collapse) fall out of the same loop. The delimiter is consumed after each
    /// element; for whitespace the next iteration's `skip_whitespace` absorbs the
    /// rest of the run.
    fn split_top_level<'arena, F>(
        &self,
        arena: &'arena Bump,
        is_delimiter: F,
        skip: &[bool; 256],
    ) -> &'arena [CssValue<'arena>]
    where
        F: Fn(char) -> bool,
    {
        let text = self.text();
        let mut cursor = ValueCursor::new(text);
        let mut values = BumpVec::new_in(arena);

        loop {
            cursor.skip_whitespace();
            if cursor.is_eof() {
                break;
            }

            let (value_start, value_end_raw) = cursor.consume_until(&is_delimiter, skip);
            let value_end = self.trimmed_end(text, value_start, value_end_raw);

            if value_end > value_start {
                // Non-empty value
                let sub_parser = self.sub_parser(value_start, value_end);
                // Progress guard: the cursor reached EOF without finding a
                // delimiter (`value_end_raw == text.len()`) and this is the only
                // element, so the whole range is a single value —
                // `classify_separators` and the comment-blind cursor disagreed
                // (an unbalanced paren/quote inside a comment). Re-`parse()`ing
                // the identical range would re-classify it the same way and
                // recurse forever, so parse it as a leaf instead.
                if values.is_empty() && value_end_raw == text.len() {
                    values.push(sub_parser.parse_single(arena));
                } else {
                    values.push(sub_parser.parse(arena)); // Recursive, but same source!
                }
            }

            cursor.set_position(value_end_raw);
            // Consume the delimiter that stopped the scan (a comma, or the first
            // char of a whitespace run); EOF leaves nothing to consume.
            if let Some(delimiter) = cursor.peek()
                && is_delimiter(delimiter)
            {
                cursor.advance(delimiter);
            }
        }

        values.into_bump_slice()
    }

    /// Build a leaf (single value) from already-trimmed `text`.
    ///
    /// The fast path passes `self.text()` directly (the range is trimmed), skipping
    /// the redundant `str::trim` that `parse_single` runs for the two-pass path.
    /// Identifier text is recovered from `span` at print time, so the fallback
    /// stores no copied string.
    fn build_leaf<'arena>(&self, text: &'a str, arena: &'arena Bump) -> CssValue<'arena> {
        let span = self.absolute_span();
        super::parse_single_value(text, span, arena).unwrap_or(CssValue::Identifier { span })
    }

    /// Parse single value (leaf node), trimming first.
    ///
    /// Used by the two-pass fallback, where the range may carry surrounding
    /// whitespace; the fast path calls `build_leaf` directly. The trim is
    /// CSS-whitespace-only and escape-aware (`trim_start_css` /
    /// `trim_end_preserving_escape`): a non-ASCII space (NBSP, em space) at either end
    /// is leaf content and stays, so the text handed to `build_leaf` still matches
    /// `span` (trimming it would desync the two and drop a string element — see the
    /// helper).
    fn parse_single<'arena>(&self, arena: &'arena Bump) -> CssValue<'arena> {
        let text = self.text();
        self.build_leaf(
            crate::escapes::trim_end_preserving_escape(crate::escapes::trim_start_css(text)),
            arena,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The **fused-pass invariant**: the three top-level trackers — `fast_scan`,
    /// `ValueCursor::consume_until`, and `classify_separators` — must agree on what is
    /// structure and what is the interior of an escape / quote / comment / paren.
    ///
    /// Each carries a comment asserting they are "kept identical"; nothing enforced it, and
    /// they had already drifted once (all three skipped only `\(` / `\)`, so an escaped
    /// comma or space read as a separator and tore an ident in half).
    ///
    /// This is a **regression net, not a proof**: it pins the invariant the three now hold,
    /// so the next edit to one of them fails here instead of silently in a corpus. It does
    /// not demonstrate the original bug — that was fixed before this test existed.
    #[test]
    fn twin_trackers_agree_on_top_level_structure() {
        // Each case pairs a shape with what the top level really contains, so a tracker
        // that mis-reads an escape's interior disagrees with the other two.
        let cases = [
            // (input, has_top_level_comma, has_top_level_whitespace)
            ("red", false, false),
            ("red blue", false, true),
            ("red, blue", true, true),
            ("rgba(1, 2, 3)", false, false),
            ("calc(1px + 2px)", false, false),
            // Escaped separators are CONTENT, not structure.
            (r"x\,y", false, false),
            (r"x\ y", false, false),
            (r"a\+b", false, false),
            (r"a\(b", false, false),
            (r"a\)b", false, false),
            // A hex escape swallows one whitespace terminator — still one token.
            (r"\41 2px", false, false),
            (r"\41 2px 3px", false, true),
            // An escaped backslash does NOT escape what follows it (even run).
            (r"a\\ b", false, true),
            (r"a\\,b", true, false),
            // Escapes next to real structure.
            (r"x\,y, z", true, true),
            (r"x\ y z", false, true),
            // Quotes, comments and parens still shield their interiors.
            ("'a, b'", false, false),
            ("url(a b)", false, false),
            // An escape at end-of-input has no payload to consume — it must not run past
            // the end or swallow a separator that is not there.
            (r"a\", false, false),
            (r"a\ ", false, false),
            // A `\r` terminator on a hex escape (CSS whitespace, unlike an escape payload).
            ("\\41\r2px", false, false),
            ("\\41\r2px 3px", false, true),
        ];

        for (text, want_comma, want_ws) in cases {
            let arena = Bump::new();
            let span = Span {
                start: 0,
                end: text.len() as u32,
            };
            let parser = ValueParser::new(text, span);

            // 1. `classify_separators` — the standalone classifier.
            let classified = classify_separators(text);
            let classify_comma = classified == ValueSeparator::Comma;
            let classify_ws = classified == ValueSeparator::Whitespace;
            assert_eq!(
                classify_comma, want_comma,
                "classify_separators comma verdict for {text:?}"
            );
            assert_eq!(
                classify_ws,
                want_ws && !want_comma,
                "classify_separators whitespace verdict for {text:?}"
            );

            // 2. `fast_scan` — the fused single pass. Its verdict must match.
            let fast_comma = matches!(parser.fast_scan(text, &arena), FastScan::Comma(_));
            let fast_ws = matches!(parser.fast_scan(text, &arena), FastScan::Whitespace);
            assert_eq!(
                fast_comma, classify_comma,
                "fast_scan disagrees with classify_separators on a comma in {text:?}"
            );
            assert_eq!(
                fast_ws, classify_ws,
                "fast_scan disagrees with classify_separators on whitespace in {text:?}"
            );

            // 3. `ValueCursor::consume_until` — the splitter's scanner. Stopping before
            // EOF means it found a top-level delimiter.
            let mut cursor = ValueCursor::new(text);
            let (_, comma_end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
            assert_eq!(
                comma_end < text.len(),
                want_comma,
                "ValueCursor disagrees on a top-level comma in {text:?}"
            );
            let mut cursor = ValueCursor::new(text);
            let (_, ws_end) = cursor.consume_until(is_css_whitespace, &CSS_WS_SKIP);
            assert_eq!(
                ws_end < text.len(),
                want_ws,
                "ValueCursor disagrees on top-level whitespace in {text:?}"
            );
        }

        // A comment is the ONE place the three deliberately diverge: `ValueCursor` is
        // comment-BLIND while `classify_separators` is comment-aware, so a `,` inside a
        // comment would split the cursor's scan. `fast_scan` is what makes that safe — it
        // detects the comment and bails to the comment-aware two-pass path, so the blind
        // cursor is never asked about such a value. Pin the bail, not a false agreement.
        for text in ["a/* , */b", "a /* , */ b", "red /* x */, blue"] {
            let arena = Bump::new();
            let span = Span {
                start: 0,
                end: text.len() as u32,
            };
            let parser = ValueParser::new(text, span);
            assert!(
                matches!(parser.fast_scan(text, &arena), FastScan::Comment),
                "fast_scan must bail to the comment-aware path for {text:?} — the blind \
                 `ValueCursor` may not be used on a comment-bearing value"
            );
        }
    }

    #[test]
    fn test_new_parser() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.source, "red, blue");
        assert_eq!(parser.start, 0);
        assert_eq!(parser.end, 9);
        assert_eq!(parser.base_offset, 100);
    }

    #[test]
    fn test_text() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.text(), "red, blue");
    }

    #[test]
    fn test_text_with_range() {
        let source = "red, blue";
        let parser = ValueParser {
            source,
            start: 5,
            end: 9,
            base_offset: 100,
        };

        assert_eq!(parser.text(), "blue");
    }

    #[test]
    fn test_absolute_span() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 100);
        assert_eq!(abs_span.end, 109);
    }

    #[test]
    fn test_absolute_span_with_range() {
        let source = "red, blue";
        let parser = ValueParser {
            source,
            start: 5, // "blue" starts at byte 5
            end: 9,   // "blue" ends at byte 9
            base_offset: 100,
        };

        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 105); // 100 + 5
        assert_eq!(abs_span.end, 109); // 100 + 9
    }

    #[test]
    fn test_sub_parser() {
        let source = "red, blue, green";
        let span = Span {
            start: 100,
            end: 116,
        };
        let parser = ValueParser::new(source, span);

        // Create sub-parser for "blue" (bytes 5-9 in source)
        let sub = parser.sub_parser(5, 9);

        assert_eq!(sub.source, "red, blue, green"); // Same source
        assert_eq!(sub.start, 5);
        assert_eq!(sub.end, 9);
        assert_eq!(sub.base_offset, 100); // Same base
        assert_eq!(sub.text(), "blue");

        let sub_span = sub.absolute_span();
        assert_eq!(sub_span.start, 105); // 100 + 5
        assert_eq!(sub_span.end, 109); // 100 + 9
    }

    #[test]
    fn test_nested_sub_parsers() {
        let source = "a, b, c";
        let span = Span { start: 0, end: 7 };
        let parser = ValueParser::new(source, span);

        // First level: "b, c" (bytes 3-7)
        let sub1 = parser.sub_parser(3, 7);
        assert_eq!(sub1.text(), "b, c");
        assert_eq!(sub1.absolute_span().start, 3);
        assert_eq!(sub1.absolute_span().end, 7);

        // Second level: "c" (bytes 6-7, but relative to sub1's start)
        // In sub1's coordinates: "c" is at position 3 (because sub1.start = 3)
        // So we want bytes 6-7 in original, which is 6-3=3 to 7-3=4 relative to sub1
        let sub2 = sub1.sub_parser(3, 4);
        assert_eq!(sub2.source, "a, b, c"); // Still same source!
        assert_eq!(sub2.text(), "c");
        assert_eq!(sub2.absolute_span().start, 6); // 0 + 3 + 3
        assert_eq!(sub2.absolute_span().end, 7); // 0 + 3 + 4
    }

    #[test]
    fn test_multiline_source() {
        let source = "val1,\n    val2";
        let span = Span {
            start: 100,
            end: 114,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.text(), "val1,\n    val2");
        assert_eq!(parser.source, "val1,\n    val2"); // Preserves whitespace
    }

    #[test]
    fn test_empty_range() {
        let source = "test";
        let parser = ValueParser {
            source,
            start: 2,
            end: 2, // Empty range
            base_offset: 0,
        };

        assert_eq!(parser.text(), "");
        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 2);
        assert_eq!(abs_span.end, 2);
    }

    // Tests: Parsing Methods

    #[test]
    fn test_parse_single_identifier() {
        let source = "auto";
        let span = Span { start: 0, end: 4 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::Identifier { .. }));
        if let CssValue::Identifier { span } = value {
            assert_eq!(span.extract(source), "auto");
        }
    }

    #[test]
    fn test_parse_comma_separated_simple() {
        let source = "red, blue, green";
        let span = Span {
            start: 100,
            end: 116,
        };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 3);
        }
    }

    #[test]
    fn test_parse_space_separated_simple() {
        let source = "10px 20px 30px";
        let span = Span { start: 0, end: 14 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::List { .. }));
        if let CssValue::List { values, .. } = value {
            assert_eq!(values.len(), 3);
        }
    }

    #[test]
    fn test_parse_multiline_comma_separated() {
        // Simulate box-shadow with multiline formatting
        let source = "0 1px rgba(0, 0, 0, 0.1),\n    0 2px rgba(0, 0, 0, 0.2)";
        let span = Span {
            start: 100,
            end: 154,
        };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);
            // Verify both values parsed correctly
            assert!(matches!(values[0], CssValue::List { .. }));
            assert!(matches!(values[1], CssValue::List { .. }));
        }
    }

    #[test]
    fn test_parse_nested_function_in_list() {
        // box-shadow value with rgba function
        let source = "0 2px 4px rgba(0, 0, 0, 0.1)";
        let span = Span { start: 0, end: 28 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::List { .. }));
        if let CssValue::List { values, .. } = value {
            assert_eq!(values.len(), 4); // "0", "2px", "4px", "rgba(...)"
            // Last value should be a color (rgba is recognized as color function)
            assert!(matches!(values[3], CssValue::Color { .. }));
        }
    }

    #[test]
    fn test_parse_preserves_leading_zeros_in_function() {
        // This is the key test - leading zeros should be preserved
        let source = "linear-gradient(90deg, red 01%, blue 02%)";
        let span = Span { start: 0, end: 42 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::Function { .. }));
        if let CssValue::Function { name, args, .. } = value {
            assert_eq!(name, "linear-gradient");
            // Arguments should have accurate spans pointing to original source
            assert_eq!(args.len(), 3); // "90deg", "red 01%", "blue 02%"
        }
    }

    #[test]
    fn test_parse_handles_empty_values() {
        // Edge case: double comma (empty value between)
        let source = "a,,c";
        let span = Span { start: 0, end: 4 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            // Empty values should be skipped
            assert_eq!(values.len(), 2);
        }
    }

    #[test]
    fn test_parse_handles_trailing_whitespace() {
        let source = "red  ,  blue  ";
        let span = Span { start: 0, end: 14 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);
            // Spans should exclude trailing whitespace
        }
    }

    #[test]
    fn test_parse_single_value_with_whitespace() {
        let source = "  auto  ";
        let span = Span { start: 0, end: 8 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::Identifier { .. }));
        if let CssValue::Identifier { span } = value {
            assert_eq!(span.extract(source).trim(), "auto");
        }
    }

    #[test]
    fn test_trimmed_end_helper() {
        let source = "red  ,  blue";
        let span = Span { start: 0, end: 12 };
        let parser = ValueParser::new(source, span);

        // Test trimming "red  " (positions 0-5, trimmed should be 0-3)
        let trimmed = parser.trimmed_end(source, 0, 5);
        assert_eq!(trimmed, 3);

        // Test trimming "  blue" (no trailing space)
        let trimmed = parser.trimmed_end(source, 8, 12);
        assert_eq!(trimmed, 12);
    }

    #[test]
    fn test_nested_sub_parsers_with_parse() {
        // Test same-source recursion through actual parsing
        let source = "a, b, c";
        let span = Span { start: 0, end: 7 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));

        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 3);

            // Each value should have correct span
            for (i, val) in values.iter().enumerate() {
                let val_span = val.span();
                // Verify spans are in correct order and non-overlapping
                if i > 0 {
                    let prev_span = values[i - 1].span();
                    assert!(val_span.start > prev_span.end);
                }
            }
        }
    }

    #[test]
    fn test_multibyte_non_ascii_value_is_single_leaf() {
        // U+4E20 encodes a 0xA0 byte that a per-byte `as char` cast would alias
        // to NBSP, classifying the lone token as a whitespace list and recursing
        // on the identical range forever. It must parse as a single leaf.
        let source = "丠";
        let span = Span {
            start: 0,
            end: source.len() as u32,
        };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::Identifier { .. }));
    }

    #[test]
    fn test_comment_unbalanced_paren_terminates() {
        // `classify_separators` is comment-aware and sees the top-level comma;
        // the comment-blind cursor sees the `(` inside the comment, opens a paren
        // it never closes, and can't reach the comma. The progress guard parses
        // the range as a single value instead of recursing forever.
        let source = "/* ( */ a, b";
        let span = Span {
            start: 0,
            end: source.len() as u32,
        };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena); // must terminate (no stack overflow)
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            // The guard collapsed the unsplittable range to one leaf.
            assert_eq!(values.len(), 1);
            assert!(matches!(values[0], CssValue::Identifier { .. }));
        }
    }

    #[test]
    fn test_complex_multiline_box_shadow() {
        // Real-world test case from fixture
        let source = "0 2px 4px rgba(0, 0, 0, 0.1),\n    0 4px 8px rgba(0, 0, 0, 0.2)";
        let span = Span { start: 0, end: 63 };
        let parser = ValueParser::new(source, span);

        let arena = Bump::new();
        let value = parser.parse(&arena);
        assert!(matches!(value, CssValue::CommaSeparated { .. }));

        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);

            // First value: "0 2px 4px rgba(0, 0, 0, 0.1)"
            assert!(matches!(values[0], CssValue::List { .. }));
            if let CssValue::List { values: parts, .. } = &values[0] {
                assert_eq!(parts.len(), 4);
                // Last part should be a color (rgba recognized as color)
                assert!(matches!(parts[3], CssValue::Color { .. }));
            }

            // Second value: "0 4px 8px rgba(0, 0, 0, 0.2)"
            assert!(matches!(values[1], CssValue::List { .. }));
        }
    }
}
