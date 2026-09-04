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

    /// Check for a blank line after the separator comma, accounting for stripped grouping
    /// parens.
    ///
    /// The **array/tuple** blank rule — prettier's `isLineAfterElementEmpty`, which advances
    /// to the comma before measuring. Its counterpart for params, call arguments, and object
    /// properties is [`Self::is_next_line_empty`], which measures from the element's end; see
    /// that doc for the table of where the two disagree, and
    /// [`BlankRule`](super::BlankRule) for the enum that makes a list name which one it takes.
    ///
    /// ⚠️ **The comma search and the blank scan take DIFFERENT upper bounds, and that is the
    /// rule rather than a redundancy.** `upper` is where the next slot's printed *content*
    /// begins — pulled back to a leading comment when the slot has one — and it bounds the
    /// **blank scan**, which is what keeps a comment run out of the measured range
    /// (`'a', // x⏎⏎/* c */ 'b'` measures the blank *between* them). `comma_bound` is the
    /// next element's own start and bounds the **comma search**, because the separator may
    /// sit *past* that content: an author who puts the comma below a comment
    /// (`[a⏎// c⏎, b]`) leaves it between the comment and the next element. Bounding the
    /// search at `upper` made that comma invisible, and the fallback then measured from the
    /// element's end — silently answering this list with the **object family's** rule, which
    /// is the one thing prettier's two helpers say an array must not do. It cost two bugs at
    /// once: a blank the author wrote ahead of such a comment was kept where prettier drops
    /// it, and — since the fallback range starts inside a stripped paren shell — the shell's
    /// own `)` line was read as an author blank and one was FABRICATED
    /// (`[(⏎x⏎)⏎// c⏎, y]`). Both forms are stable under both formatters, so only a prettier
    /// `compare` finds them.
    ///
    /// If no comma is found at all the gap has no separator, and the check falls back to the
    /// element's end. Callers must pass `prev_end <= upper <= comma_bound`.
    ///
    /// The scan stays the **table-only** count rather than the strict intervening-line one,
    /// and that is sound *here* rather than generally: once the separator is found the range
    /// opens at `comma + 1`, where the only re-emitted delimiter that can follow is the next
    /// element's own stripped `(` — which [`skip_stripped_open_paren`](super::calls::skip_stripped_open_paren)
    /// already caps the range at. A `)` cannot appear after a separator comma, so the reading
    /// that fabricates elsewhere has nothing to fabricate from.
    pub(crate) fn has_blank_line_after_comma(
        &self,
        prev_end: u32,
        upper: u32,
        comma_bound: u32,
    ) -> bool {
        // Past the comma the next printed content is whatever follows *it*, so a comma the
        // author pushed below the slot's leading comment re-derives its own bound rather
        // than measuring backwards into a range that ends before it.
        let (after_comma, blank_upper) = match self.find_comma_in_range(prev_end, comma_bound) {
            Some(comma) if comma >= upper => {
                (comma + 1, self.blank_scan_end(comma + 1, comma_bound))
            }
            Some(comma) => (comma + 1, upper),
            None => (prev_end, upper),
        };
        // The scan counts raw newlines, so it must not span a comment's bytes — including
        // one this caller does not emit (an owned annotation leading the next element).
        // See `blank_scan_start`.
        let check_start = self.blank_scan_start(after_comma, blank_upper);
        let check_end =
            super::calls::skip_stripped_open_paren(self.source, check_start, blank_upper);
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
    /// Prettier's `isPreviousLineEmpty` — is the line **directly above** `next_start`
    /// blank? — scanning backwards and stopping at `floor`.
    ///
    /// The trailing-run separator's question ([`Printer::push_trailing_run_separator`]), and
    /// NOT the same as "does the gap hold a blank line anywhere". The two part exactly where
    /// re-emitted structure sits between the author's blank and the comment: a comma the
    /// author pushed onto its own line (`a⏎⏎,⏎// c`) leaves a blank in the gap while the line
    /// above the comment is the comma's, so prettier writes no blank and a gap-wide scan
    /// writes one. `printTrailingComment` reads it off `locStart(comment)` for the same
    /// reason — the run is emitted against the comment, so the comment is what the blank has
    /// to be adjacent to.
    ///
    /// Byte-scanning, and therefore blind to U+2028 / U+2029 exactly as prettier's own
    /// helper is — this predicate exists to match it, so the narrower terminator class is
    /// the faithful one here (contrast the statement-gap count, which keeps the line-break
    /// table precisely because nothing upstream is byte-scanning it).
    pub(crate) fn previous_line_is_empty(&self, floor: u32, next_start: u32) -> bool {
        if self.canonical {
            return false;
        }
        let bytes = self.source.as_bytes();
        let floor = floor as usize;
        let mut pos = (next_start as usize).min(bytes.len());
        let back_over_spaces = |pos: &mut usize| {
            while *pos > floor && matches!(bytes[*pos - 1], b' ' | b'\t') {
                *pos -= 1;
            }
        };
        // The comment's own line, back to its start.
        back_over_spaces(&mut pos);
        // Consume exactly one line terminator; without one there is no previous line.
        match bytes.get(pos.wrapping_sub(1)) {
            Some(b'\n') if pos > floor => {
                pos -= 1;
                if pos > floor && bytes[pos - 1] == b'\r' {
                    pos -= 1;
                }
            }
            Some(b'\r') if pos > floor => pos -= 1,
            _ => return false,
        }
        // Whatever is left of the previous line: blank iff it holds nothing but spaces.
        back_over_spaces(&mut pos);
        pos > floor && matches!(bytes[pos - 1], b'\n' | b'\r')
    }

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

    /// [`Self::blank_scan_end`] from a comment's own end: where a blank-line scan running
    /// from `comment` up to `node_start` must stop — at its successor when that comment
    /// lies wholly before `node_start`, else at `node_start`.
    ///
    /// The same answer as `blank_scan_end(comment.span.end, node_start)`, taken from the
    /// comment's own index (`tsv_lang::comments_in_source_after_comment`) rather than from
    /// a position lookup that is a guaranteed hint miss. ⚠️ Every scan that starts at a
    /// comment's end goes through here, none through the position form.
    pub(in crate::printer) fn blank_scan_end_after(
        &self,
        comment: &tsv_lang::Comment,
        node_start: u32,
    ) -> u32 {
        tsv_lang::comments_in_source_after_comment(self.comments, comment)
            .first()
            .filter(|c| c.span.end <= node_start)
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
        // The comment-free window answers first: `after_pos` inside it puts the next
        // comment in source at the window's end, so when that comment is on a later line
        // — or there is none — nothing trails and the search is not needed. Asked at
        // every statement's end, and the window has this answer for 97 asks in 100 on a
        // real corpus (the ask that precedes it, the same gap's trailing run, drew the
        // window); the walk below then runs only for a comment actually on the line.
        if let Some(next_start) = self.comment_free_gap.next_comment_start(after_pos)
            && (next_start == u32::MAX || !self.is_same_line(after_pos, next_start))
        {
            return after_pos;
        }
        let mut end = after_pos;
        // Track the "current line" reference — follows multi-line block comments
        // to their closing */ line (same logic as build_trailing_same_line_comment_docs)
        let mut line_ref = after_pos;

        for comment in self.comments_in_source_after(after_pos) {
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
    #[expect(clippy::expect_used)]
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
