//! What acorn SAW in the text ahead of one embedded parse.
//!
//! Svelte hands acorn a different string at every embedded parse, and for four of them that
//! string is **manufactured**: the bytes ahead of the region are rewritten, and a synthetic
//! token may stand where they end. Two answers in the wire read that preparation rather than
//! the document:
//!
//! - the **line class** an acorn-owned `loc` was counted under — whether the terminators ahead
//!   of the region survived the rewrite ([`AcornPrefix::counts_ecmascript_lines`], the axis
//!   `tsv_ts::AcornSeed` seeds a parse's first line from);
//! - the **indentation** `onComment` dedents a multi-line block comment by, which is the one
//!   the *manufactured* line opens with ([`AcornPrefix::line_indentation`], read by
//!   [`printing::strip_comment_indentation`]).
//!
//! One value answers both, so the two cannot disagree about what a given parse was handed. It
//! lives here rather than beside the seed in `tsv_ts` because the comment dedent —
//! `onComment`'s mirror — already does, and a fact two crates read is this crate's.
//!
//! **What crosses the crate boundary is the VALUE, not the mechanics.** `tsv_svelte`'s parser
//! states a preparation ([`AcornPrefix::manufactured`] / [`AcornPrefix::DOCUMENT`]) and
//! `tsv_ts` reads its one bit ([`AcornPrefix::counts_ecmascript_lines`]); the walk-back and
//! the run measurement are `pub(crate)`, because they answer in `onComment`'s coordinate
//! space and only [`printing::strip_comment_indentation`] knows to ask them together.
//!
//! [`printing::strip_comment_indentation`]: crate::printing::strip_comment_indentation

use std::borrow::Cow;

use crate::whitespace::is_js_whitespace;

/// How Svelte prepared the text ahead of one embedded parse — one variant per preparation its
/// parser performs, plus the raw template.
///
/// The sites, all under `svelte/packages/svelte/src/compiler/phases/1-parse/`:
///
/// | variant | reader | the string acorn got |
/// | --- | --- | --- |
/// | [`Document`](Self::Document) | `read_expression`, `parse_statement_at` | `parser.template`, untouched |
/// | [`Blanked`](Self::Blanked) | `read/script.js` | `slice(0, start).replace(/[^\n]/g, ' ') + data` |
/// | [`BlankedThenParen`](Self::BlankedThenParen) | `read/context.js` `read_pattern` | the same, **minus its first space**, then `(pattern = 1)` |
/// | [`BlankedThenAs`](Self::BlankedThenAs) | `read/context.js` `read_type_annotation` | the same, then `_ as ` over the five bytes it covers |
/// | [`WhitespaceKept`](Self::WhitespaceKept) | `state/tag.js`, the `{#snippet}` head | `slice(0, params_start).replace(/\S/g, ' ')` — only the NON-whitespace is blanked |
///
/// The distinctions are not cosmetic. `Blanked` and its two siblings erase every terminator
/// but `\n`, so acorn counted Svelte's own lines, where `WhitespaceKept` keeps all of them and
/// acorn counted the ECMAScript class exactly as for the raw template — and each of the two
/// synthetic tokens ends the manufactured run at a place the document has no byte for, which
/// is what the comment dedent measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcornPrefixText {
    /// acorn was handed the document's own bytes. Every standalone parse is this, and so is
    /// every Svelte island read out of the raw template.
    Document,
    /// Every non-`\n` byte ahead of the region became a space, and the region's own bytes
    /// follow it directly.
    Blanked,
    /// The same, with the prefix's **first space removed** and a `(` standing at its end —
    /// `read_pattern`'s `(pattern = 1)` wrapper. Removing that one space is what keeps the
    /// pattern's columns where the document put them.
    BlankedThenParen,
    /// The same as [`Blanked`](Self::Blanked), with `_ as ` standing over the five bytes at
    /// the prefix's end — `read_type_annotation`'s trick for making a type annotation into an
    /// expression acorn will parse.
    BlankedThenAs,
    /// Every non-**whitespace** byte ahead of the region became a space; the author's own
    /// whitespace bytes are still standing.
    WhitespaceKept,
}

impl AcornPrefixText {
    /// `read_type_annotation`'s `const insert = '_ as '` — the synthetic token
    /// [`BlankedThenAs`](Self::BlankedThenAs) stands at the end of its prefix, and the only
    /// one that OVERWRITES document bytes rather than being spliced between them.
    ///
    /// **One spelling, because two places measure the same five bytes from opposite ends**:
    /// `tsv_svelte`'s parser subtracts this length from the colon to place the region's
    /// `origin`, and `AcornPrefix::synthetic_insert_range` rebuilds the window forward from
    /// that same `origin`. A second copy would let the two cover different bytes, and the
    /// only symptom would be a comment dedented against a line acorn never measured.
    pub const AS_INSERT: &'static str = "_ as ";

    /// Whether a synthetic token stands where this preparation's prefix ends, so no run of
    /// source bytes can continue through it.
    #[inline]
    const fn ends_in_synthetic_token(self) -> bool {
        matches!(self, Self::BlankedThenParen | Self::BlankedThenAs)
    }
}

/// One parse's preparation and where it ends — [`AcornPrefixText`] plus the offset the
/// manufactured bytes run out at (the parse's `origin`, which is where Svelte's own slicing
/// put the boundary).
///
/// The pair travels together because neither half answers anything alone: a kind with no
/// boundary cannot say which bytes it covers, and a boundary with no kind cannot say what
/// stands at it. [`DOCUMENT`](Self::DOCUMENT) is the identity — the state every standalone
/// parse and every raw-template island is in — and carries no boundary at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcornPrefix {
    text: AcornPrefixText,
    /// One past the last manufactured byte. Unread under [`AcornPrefixText::Document`].
    end: u32,
}

impl AcornPrefix {
    /// acorn read the document's own bytes — no manufacture, nothing to model.
    ///
    /// The **only** spelling of that state, which is why neither this type nor
    /// [`AcornPrefixText`] carries a `Default`: a second way to say it is a second thing to
    /// keep in step with [`manufactured`](Self::manufactured)'s refusal to produce it.
    pub const DOCUMENT: Self = Self {
        text: AcornPrefixText::Document,
        end: 0,
    };

    /// A manufactured prefix of `text` running out at `end`.
    ///
    /// Debug-asserts against [`AcornPrefixText::Document`], which has no boundary:
    /// [`DOCUMENT`](Self::DOCUMENT) is the only spelling of that state, so a caller arriving
    /// here with it has lost track of which parse it is describing.
    #[must_use]
    pub fn manufactured(text: AcornPrefixText, end: u32) -> Self {
        debug_assert!(
            text != AcornPrefixText::Document,
            "`Document` carries no boundary — spell it `AcornPrefix::DOCUMENT`"
        );
        Self { text, end }
    }

    /// The start of the line the comment at `comment_start` opens on — `onComment`'s
    /// `while (a > 0 && source[a - 1] !== '\n') a -= 1`, over the string acorn was handed.
    ///
    /// ⚠️ **A preparation that OVERWRITES bytes can swallow the author's newline**, and then
    /// acorn's line opens further back than the document's does. `read_type_annotation`'s
    /// `_ as ` is the one that can: it stands over the five bytes ending at the parse's
    /// `origin`, so `{#each xs as x⏎\t: /* … */ T}` — a newline the author wrote between a
    /// binding and its colon — is erased before acorn sees it, and the annotation's comment
    /// is measured from the *binding's* line. The same five bytes are why that region needs a
    /// line seed at all (`tsv_ts::AcornSeed`), so this is one fact read at a second place.
    ///
    /// The blanking preparations cannot do it: `[^\n]` and `\S` both leave every `\n`
    /// standing, and `read_pattern`'s wrapper deletes a *space* and inserts a `(`.
    #[must_use]
    pub(crate) fn line_start(self, source: &str, comment_start: usize) -> usize {
        let bytes = source.as_bytes();
        let insert = self.synthetic_insert_range();
        let mut at = comment_start;
        while at > 0 {
            let before = at - 1;
            // A `\n` a synthetic token stands over is not a line start, because it is not in
            // the string acorn was reading.
            if bytes[before] == b'\n' && !insert.contains(&before) {
                break;
            }
            at = before;
        }
        at
    }

    /// The document bytes a synthetic token stands over — empty for every preparation but
    /// [`BlankedThenAs`](AcornPrefixText::BlankedThenAs), whose `_ as ` is the only insert
    /// that *replaces* text rather than being spliced between it.
    #[inline]
    fn synthetic_insert_range(self) -> std::ops::Range<usize> {
        if self.text == AcornPrefixText::BlankedThenAs {
            let end = self.end as usize;
            end..end + AcornPrefixText::AS_INSERT.len()
        } else {
            0..0
        }
    }

    /// Whether acorn counted the **ECMAScript** terminator class over this prefix rather than
    /// Svelte's `\n`-only one.
    ///
    /// True for the raw template (acorn saw every terminator the author wrote) and for the
    /// `{#snippet}` head, whose prelude blanks only the non-whitespace so every terminator
    /// survived. False for the three blanked preparations, which leave `\n` standing alone.
    #[inline]
    #[must_use]
    pub const fn counts_ecmascript_lines(self) -> bool {
        matches!(
            self.text,
            AcornPrefixText::Document | AcornPrefixText::WhitespaceKept
        )
    }

    /// The `[ \t]` run acorn saw at `line_start` — `onComment`'s
    /// `while (/[ \t]/.test(source[b])) b += 1`, over the string acorn was actually handed.
    ///
    /// `line_start` is a **document** offset, and the walk-back that produced it is sound in
    /// either coordinate space: `\n` survives every preparation, so the line is the same one
    /// on both sides. Borrowed for the raw template (the overwhelmingly common case, and the
    /// only one a standalone parse can reach); owned for the three preparations that put
    /// bytes there the document does not hold.
    ///
    /// ⚠️ **The answer is a LENGTH, and the length is counted in UTF-16 code units** — never
    /// in bytes. The blanking is `String.replace`, which substitutes one space per matched
    /// **code unit**, so a prefix holding a non-ASCII character becomes a run SHORTER than
    /// its byte span ([`blanked_width`]). The two agree on all of ASCII, which is why a
    /// byte count survives every fixture and every corpus: what it takes is one non-ASCII
    /// character between the line's start and the manufacture — a word of prose on the line
    /// that opens a `<script>`, a Unicode identifier in a `{#snippet}` head — and then this
    /// run is one space per extra byte too long, and the dedent it drives strips nothing
    /// where Svelte strips the line.
    #[must_use]
    pub(crate) fn line_indentation<'s>(self, source: &'s str, line_start: usize) -> Cow<'s, str> {
        let bytes = source.as_bytes();
        let run_from = |at: usize| {
            let mut end = at;
            while matches!(bytes.get(end), Some(b' ' | b'\t')) {
                end += 1;
            }
            end
        };
        let manufactured_end = self.end as usize;
        // Where the document's own bytes take over — past the blanked prefix AND past any
        // synthetic token standing over document text. The two are the same offset at every
        // preparation but `BlankedThenAs`, whose `_ as ` OVERWRITES the five bytes it covers:
        // a line opening exactly at that insert opens on its `_`, so the run acorn saw is
        // empty where the document's is whatever `[ \t]` the author put under the insert.
        let document_bytes_from = manufactured_end.max(self.synthetic_insert_range().end);
        if self.text == AcornPrefixText::Document || line_start >= document_bytes_from {
            // Past the manufacture the region's own bytes are standing, so this is the
            // document's own run either way.
            return Cow::Borrowed(&source[line_start..run_from(line_start)]);
        }

        let mut run = String::new();
        if self.text == AcornPrefixText::WhitespaceKept {
            // The author's whitespace survived; everything else became a space. A whitespace
            // character that is not ` ` or `\t` ends the run just as it would in the document.
            for ch in source[line_start..manufactured_end].chars() {
                match ch {
                    ' ' | '\t' => run.push(ch),
                    _ if is_js_whitespace(ch) => return Cow::Owned(run),
                    // One space per code unit: see the length note above.
                    _ => run.extend(std::iter::repeat_n(' ', ch.len_utf16())),
                }
            }
        } else {
            // `[^\n]` became a space, and no `\n` can sit between a line's start and a
            // position on that same line — so the whole span is spaces. Empty when the line
            // opens AT a synthetic insert: it is past the blanking already, so nothing of the
            // prefix is on it and the insert's own first byte ends the run at zero.
            //
            // The span cannot run backwards: the early return took every `line_start` at or
            // past the manufacture, and a `\n` inside a synthetic insert cannot open a line
            // (see `line_start`), so nothing is left that opens between the two.
            debug_assert!(
                line_start <= manufactured_end,
                "a line opening inside the manufacture at {line_start} has no blanked run"
            );
            let mut width = blanked_width(&source[line_start..manufactured_end]);
            if self.text == AcornPrefixText::BlankedThenParen
                && paren_space_fell_here(source, line_start)
            {
                width -= 1;
            }
            run.extend(std::iter::repeat_n(' ', width));
        }

        if !self.text.ends_in_synthetic_token() {
            // Nothing stands between the prefix and the region, so the run carries on into the
            // region's own leading whitespace.
            run.push_str(&source[manufactured_end..run_from(manufactured_end)]);
        }
        Cow::Owned(run)
    }
}

/// How many spaces a blanking substitution lays down over `blanked` — one per **UTF-16 code
/// unit**, which is what `String.replace(/[^\n]/g, ' ')` counts.
///
/// A JS regex without the `u` flag matches one code unit at a time, so each is replaced by one
/// space: a BMP character becomes one space where its UTF-8 form is two or three bytes, and an
/// astral one becomes two (its surrogate pair) where its UTF-8 form is four. The manufactured
/// string is therefore SHORTER than the document span it stands over whenever that span is not
/// pure ASCII, and this run's length is the whole answer — it is what the dedent strips.
#[inline]
fn blanked_width(blanked: &str) -> usize {
    // `len()` is the answer for the ASCII prefix that essentially every document has; the walk
    // only runs when it isn't.
    if blanked.is_ascii() {
        blanked.len()
    } else {
        blanked.chars().map(char::len_utf16).sum()
    }
}

/// Whether `read_pattern`'s one removed space fell on the line starting at `line_start`.
///
/// `space_with_newline` drops the **first** space of the blanked prefix, which is the first
/// non-`\n` byte of the whole template — so it lies on this line exactly when nothing but line
/// terminators precedes it. A free fn rather than a method because the answer is a fact about
/// the document alone: no field of the prefix reaches it, and only the ONE caller that has
/// already established [`BlankedThenParen`](AcornPrefixText::BlankedThenParen) may ask.
///
/// The scan reads as a walk over the whole prefix and is not one: `all` stops at the first
/// byte that is not `\n`, which on every real document is byte 0.
///
/// ⚠️ **`indexOf` returning `-1` is a Svelte bug this deliberately does not model.** With no
/// space in the blanked prefix at all, `slice(0, -1) + slice(first_space + 1)` *duplicates*
/// the prefix instead of shortening it. It takes a prefix that is entirely line terminators,
/// and this reader cannot see one: every caller of `read_pattern`'s destructuring arm
/// (`{#each}`, `{@const}`, `{#await … then}`, `{:then}`, `{:catch}`) has its own tag text
/// ahead of it, so the prefix always holds a non-`\n` byte. Were that to change, the answer
/// here would be a *widening*, not this predicate's inverse.
#[inline]
fn paren_space_fell_here(source: &str, line_start: usize) -> bool {
    source.as_bytes()[..line_start].iter().all(|&b| b == b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_prefix_reads_the_source() {
        let source = "\t\t/* a\n\t\tb */";
        assert_eq!(AcornPrefix::DOCUMENT.line_indentation(source, 0), "\t\t");
    }

    #[test]
    fn a_blanked_prefix_is_spaces_the_document_never_held() {
        // `\t<script>` — nine bytes acorn saw as nine spaces, then the content's own `/*`.
        let source = "\t<script>/* a\n\t b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::Blanked, 9);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(9));
    }

    #[test]
    fn a_blanked_run_continues_into_the_regions_own_whitespace() {
        // `<script>  /*` — eight blanked bytes and then two real spaces, all one run.
        let source = "<script>  /* a\n          b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::Blanked, 8);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(10));
    }

    #[test]
    fn a_synthetic_token_ends_the_run() {
        // `_ as ` stands over the five bytes ending at the colon, so the run is the blanked
        // span alone — never the region's own leading whitespace behind the insert.
        let source = "\t{@const x:  /* a\n\t b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 6);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(6));
    }

    #[test]
    fn a_blanked_run_is_one_space_per_code_unit_not_per_byte() {
        // `<p>café</p><script>` — 19 UTF-16 code units over 20 bytes, because `é` is one unit
        // and two bytes. `String.replace` lays down 19 spaces; a byte count lays down 20 and
        // dedents a line the author never indented that far.
        let source = "<p>café</p><script>/* a\n b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::Blanked, 20);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(19));

        // An astral character is a SURROGATE PAIR to the regex — two matches, two spaces,
        // over four bytes. `<p>𝔞</p><script>` is 17 units over 19 bytes.
        let source = "<p>\u{1d51e}</p><script>/* a\n b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::Blanked, 19);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(17));
    }

    #[test]
    fn the_snippet_prelude_blanks_per_code_unit_too() {
        // `{#snippet café` — the space survives `\S`, and the four-character name blanks to
        // FOUR spaces over its five bytes. 14 units, 15 bytes.
        let source = "{#snippet café(b = 1)}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::WhitespaceKept, 15);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(14));
    }

    #[test]
    fn a_line_opening_at_the_as_insert_reads_the_inserts_own_bytes() {
        // The author's `\n` sits one byte AHEAD of the insert window, so it survives and the
        // line opens exactly where `_ as ` begins. Under the insert are four spaces the
        // document holds and acorn never saw — the run is `_`'s, which is empty.
        let source = "{#each xs as x\n    : /* a\n    b */ T}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 15);
        assert_eq!(prefix.line_start(source, 21), 15);
        assert_eq!(prefix.line_indentation(source, 15), "");
        // The null control on the same shape: with the `\n` INSIDE the window the insert
        // swallows it, the line opens back on the binding's, and the run is the blanking's.
        let swallowed = "{#each xs as x\n\t: /* a\n\tb */ T}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 12);
        assert_eq!(prefix.line_start(swallowed, 18), 0);
        assert_eq!(prefix.line_indentation(swallowed, 0), " ".repeat(12));
    }

    #[test]
    fn the_paren_wrapper_drops_one_space_on_the_documents_first_line() {
        // Nothing but the pattern's own line ahead of it, so the removed space falls in this
        // run.
        let source = "{@const {a} = e}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenParen, 8);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(7));
        // A line below one that holds content keeps the full run: the space went missing up
        // there instead.
        let source = "{#if e}\n\t{@const {a} = e}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenParen, 17);
        assert_eq!(prefix.line_indentation(source, 8), " ".repeat(9));
    }

    #[test]
    fn the_snippet_prelude_keeps_the_authors_own_whitespace() {
        // `\t{#snippet s` — the tab survives, the rest becomes spaces.
        let source = "\t{#snippet s(a = 1)}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::WhitespaceKept, 12);
        assert_eq!(
            prefix.line_indentation(source, 0),
            format!("\t{}", " ".repeat(11))
        );
    }

    #[test]
    fn a_non_tab_whitespace_character_ends_the_snippet_run() {
        // U+00A0 is JS whitespace, so `\S` left it standing — and it is no more `[ \t]` than
        // it would be in the document.
        let source = "\u{a0}{#snippet s(a = 1)}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::WhitespaceKept, 13);
        assert_eq!(prefix.line_indentation(source, 0), "");
    }

    #[test]
    fn ecmascript_line_counting_follows_what_survived_the_blanking() {
        assert!(AcornPrefix::DOCUMENT.counts_ecmascript_lines());
        assert!(
            AcornPrefix::manufactured(AcornPrefixText::WhitespaceKept, 1).counts_ecmascript_lines()
        );
        for text in [
            AcornPrefixText::Blanked,
            AcornPrefixText::BlankedThenParen,
            AcornPrefixText::BlankedThenAs,
        ] {
            assert!(!AcornPrefix::manufactured(text, 1).counts_ecmascript_lines());
        }
    }
}
