// Pure source span-math helpers for comment handling.
//
// These scan the raw source bytes to locate delimiters (commas, the assertion
// `>`), the last comma in a range, blank-line breaks, and the end position
// including trailing same-line comments — skipping over comments and strings so
// glyphs inside them aren't mistaken for the real token.

use super::Printer;
use tsv_lang::source_scan::{TriviaProfile, find_char, find_char_skipping_comments};

impl<'a> Printer<'a> {
    /// Find the position of the next comma delimiter after the given position
    ///
    /// Used to distinguish trailing comments (before comma) from leading comments (after comma)
    /// in arrays and objects. Skips over comments and strings to find the actual delimiter comma.
    ///
    /// Returns None if no comma found.
    ///
    /// Example: `[A /* , */ , B]` - finds the second comma, not the one in the comment
    pub(crate) fn find_comma_after(&self, pos: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        find_char(source, pos as usize, source.len(), b',', TriviaProfile::JS).map(|i| i as u32)
    }

    /// `find_comma_after` bounded to `[pos, end)` — stops scanning at `end`
    /// instead of running to the next comma anywhere in the rest of the source.
    pub(crate) fn find_comma_in_range(&self, pos: u32, end: u32) -> Option<u32> {
        let source = self.source.as_bytes();
        find_char(source, pos as usize, end as usize, b',', TriviaProfile::JS).map(|i| i as u32)
    }

    /// Find an angle-bracket type assertion's closing `>` in `[start, end)`,
    /// skipping any `>` that sits inside a comment or string (`<T /* > */>x`).
    ///
    /// `start` is the type's end, `end` the asserted expression's start, so the
    /// first bare `>` between them is the cast's close. Returns `end` as a safe
    /// fallback if none is found (an impossible shape for a valid assertion) —
    /// that routes any in-range comments to the before-`>` side rather than
    /// dropping them.
    pub(crate) fn find_assertion_close_angle(&self, start: u32, end: u32) -> u32 {
        let source = self.source.as_bytes();
        find_char(
            source,
            start as usize,
            end as usize,
            b'>',
            TriviaProfile::JS,
        )
        .map_or(end, |i| i as u32)
    }

    /// Find the position of the LAST comma in `[start, end)`, or `None`.
    ///
    /// Walks forward via `find_comma_in_range`, so it correctly skips commas
    /// inside strings and comments. Used to anchor comments emitted past the
    /// last separator in trailing-elision arrays (e.g. `[, , ,/* c */]`).
    pub(crate) fn find_last_comma_before(&self, start: u32, end: u32) -> Option<u32> {
        let mut last = None;
        let mut pos = start;
        while let Some(c) = self.find_comma_in_range(pos, end) {
            last = Some(c);
            pos = c + 1;
        }
        last
    }

    /// Check for a blank line after the first comma in `(prev_end, upper)`,
    /// accounting for stripped grouping parens.
    ///
    /// The **array/tuple** blank rule — prettier's `isLineAfterElementEmpty`, which advances
    /// to the comma before measuring. Its counterpart for params, call arguments, and object
    /// properties is [`Self::is_next_line_empty`], which measures from the element's end; see
    /// that doc for the table of where the two disagree, and
    /// [`BlankRule`](super::BlankRule) for the enum that makes a list name which one it takes.
    ///
    /// If no comma is found before `upper`, the check starts at `prev_end`.
    /// Callers must pass `prev_end <= upper`.
    pub(crate) fn has_blank_line_after_comma(&self, prev_end: u32, upper: u32) -> bool {
        let after_comma = self
            .find_comma_in_range(prev_end, upper)
            .map_or(prev_end, |c| c + 1);
        // The scan counts raw newlines, so it must not span a comment's bytes — including
        // one this caller does not emit (an owned annotation leading the next element).
        // See `blank_scan_start`.
        let check_start = self.blank_scan_start(after_comma, upper);
        let check_end = super::calls::skip_stripped_open_paren(self.source, check_start, upper);
        self.has_blank_line_between(check_start, check_end)
    }

    /// Whether the line on which the element ending at `from` ends is followed by a **blank
    /// line** — the faithful port of prettier's `isNextLineEmpty`.
    ///
    /// This is the list-separator blank question for **params, call arguments, and object
    /// properties**, where prettier emits a `hardline` (which forces the list to break) at a
    /// blank. It deliberately differs from [`Self::has_blank_line_after_comma`], which is the
    /// **array** question: prettier's array helper (`isLineAfterElementEmpty`) advances to the
    /// comma *first* and measures from there, and arrays emit a `softline` that never forces a
    /// break. Two different questions in prettier, so two here.
    ///
    /// The distinction is exactly where the comma sits relative to the blank:
    ///
    /// | authoring | this predicate | after-comma |
    /// | --- | --- | --- |
    /// | `a,⏎⏎b` | `true` | `true` |
    /// | `a⏎⏎, b` | `true` | `false` |
    /// | `a⏎,⏎⏎b` | **`false`** | `true` |
    ///
    /// The third row is the one worth stating: a blank *after* a comma the author pushed onto
    /// its own line does **not** count, because the blank no longer begins on the line the
    /// element ended. Prettier collapses `f(a⏎,⏎⏎b)` to `f(a, b)`, and so does this.
    ///
    /// Mirrors prettier's step order: skip same-line trailing/inline comments, skip the
    /// `,; \t` run to end of line, require the very next byte to be the line break, consume
    /// exactly one, then look for a second before any non-whitespace. Bounded by `upper` (the
    /// next element's start), so it never reads past its own gap.
    pub(crate) fn is_next_line_empty(&self, from: u32, upper: u32) -> bool {
        // A direct `self.source` newline scan, so it must be gated on the canonical
        // flag (see the `Printer::canonical` doc): the canonical reprint empties the
        // layout line-break table so table-based blank reads collapse to "no blank",
        // but a raw source scan would still see the authored blank on pass 1 and force
        // expansion — breaking authoring-independence (`f(a,\n\nb)` must canonicalize
        // to `f(a, b)`) and, across passes, idempotence.
        if self.canonical {
            return false;
        }
        let bytes = self.source.as_bytes();
        let end = (upper as usize).min(bytes.len());
        // `skipInlineComment` / `skipTrailingComment`: a comment on the element's own line is
        // trivia, whichever side of the comma it sits on (`a /* c */, b` and `a, /* c */ b`).
        let mut pos = (self.find_end_with_trailing_comments(from) as usize).min(end);
        // `skipToLineEnd = skip(",; \t")` — the separator itself is trivia here, which is the
        // whole reason a pre-comma blank is seen.
        while pos < end && matches!(bytes[pos], b',' | b';' | b' ' | b'\t') {
            pos += 1;
        }
        // `skipNewline`: consume exactly one line terminator. Landing on anything else means
        // content follows on this line, so there is no empty next line.
        let after_first = match bytes.get(pos) {
            Some(b'\n') => pos + 1,
            Some(b'\r') if bytes.get(pos + 1) == Some(&b'\n') => pos + 2,
            Some(b'\r') => pos + 1,
            _ => return false,
        };
        // `hasNewline`: a second terminator before the next non-whitespace makes the line blank.
        bytes[after_first..end]
            .iter()
            .find(|b| !matches!(b, b' ' | b'\t'))
            .is_some_and(|b| matches!(b, b'\n' | b'\r'))
    }

    /// Get the search start position for leading comments on list elements
    ///
    /// For the first element, returns `prev_end` (search starts after opening delimiter).
    /// For subsequent elements, returns position after the comma, or `prev_end` if no comma found.
    ///
    /// This ensures that comments after a comma are treated as leading on the next element,
    /// not trailing on the previous element.
    pub(crate) fn leading_comment_search_start(&self, prev_end: u32, is_first: bool) -> u32 {
        if is_first {
            prev_end
        } else {
            self.find_comma_after(prev_end)
                .map_or(prev_end, |pos| pos + 1)
        }
    }

    /// **in source**: where a blank-line scan running *up to* `node_start` must **stop** —
    /// at the first comment physically in `[prev_end, node_start)`, else at `node_start`.
    ///
    /// `has_blank_line_between*` is a raw newline count over a byte range: it cannot tell
    /// a comment's own newlines from an author's blank line. So the scan must never span a
    /// comment's bytes — and "a comment" here means **every** comment in the gap, not just
    /// the ones this caller emits. An owned comment is printed by the node its token
    /// begins, but its bytes are still in the file; a scan that skipped it would read a
    /// multi-line annotation as a blank line the author never wrote.
    pub(in crate::printer) fn blank_scan_end(&self, prev_end: u32, node_start: u32) -> u32 {
        self.comments_in_source_between(prev_end, node_start)
            .next()
            .map_or(node_start, |c| c.span.start)
    }

    /// **in source**: where a blank-line scan running *up to* `end` must **start** — past
    /// the last comment physically in `[start, end)`, else at `start`.
    ///
    /// The mirror of [`Self::blank_scan_end`], for the callers that measure the gap
    /// *after* a comment run rather than before it (array element boundaries, the
    /// inter-argument gap). Same rule, same reason: the scan must not span comment bytes.
    /// Clamped to `[start, end]`.
    pub(in crate::printer) fn blank_scan_start(&self, start: u32, end: u32) -> u32 {
        self.comments_in_source_between(start, end)
            .map(|c| c.span.end)
            .max()
            .map_or(start, |e| e.clamp(start, end))
    }

    /// Find the end position including any trailing same-line comments
    ///
    /// Used to correctly detect blank lines - need to check from after trailing
    /// comments, not just after the statement.
    pub(in crate::printer) fn find_end_with_trailing_comments(&self, after_pos: u32) -> u32 {
        let mut end = after_pos;
        // Track the "current line" reference — follows multi-line block comments
        // to their closing */ line (same logic as build_trailing_same_line_comment_docs)
        let mut line_ref = after_pos;

        for comment in tsv_lang::comments_in_source_after(self.comments, after_pos) {
            if self.is_same_line(line_ref, comment.span.start) {
                end = comment.span.end;
                // Follow multi-line block comments to their closing line
                if comment.is_block && !self.is_same_line(comment.span.start, comment.span.end) {
                    line_ref = comment.span.end;
                }
            } else {
                break;
            }
        }
        end
    }

    /// Find the comma position between two adjacent list elements,
    /// skipping over any comments in between.
    #[allow(clippy::expect_used)]
    pub(crate) fn find_list_comma(&self, elem_end: u32, next_start: u32) -> u32 {
        find_char_skipping_comments(
            self.source.as_bytes(),
            elem_end as usize,
            next_start as usize,
            b',',
        )
        .expect("comma must exist between list elements") as u32
    }
}
