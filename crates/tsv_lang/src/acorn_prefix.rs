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
/// | [`BlankedThenAs`](Self::BlankedThenAs) | `read/context.js` `read_type_annotation` | the same, then `_ as ` over the five UTF-16 code units ending at the colon |
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
    /// The same as [`Blanked`](Self::Blanked), with `_ as ` standing over the five UTF-16
    /// code units ending at the colon — `read_type_annotation`'s trick for making a type
    /// annotation into an expression acorn will parse. The manufacture runs out one past the
    /// colon, where acorn starts lexing the document again.
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
    /// Its length is counted in **UTF-16 code units** — Svelte's `parser.index -
    /// insert.length` indexes a JS string — so it covers five units, not five bytes, of the
    /// document: [`as_insert_origin`](Self::as_insert_origin) is the one place that walks
    /// them back from the colon, for the parser's `origin` and for the dedent's window alike.
    /// The literal is ASCII, so its `len()` is that unit count.
    pub const AS_INSERT: &'static str = "_ as ";

    /// Where `read_type_annotation`'s `_ as ` begins, as a **byte offset**: the character
    /// holding the first of the five UTF-16 code units that end at `lex_start` (one past the
    /// annotation's colon).
    ///
    /// Svelte counts the window in code units, so behind a non-ASCII binding it reaches more
    /// than five bytes back — and it can open **between the halves of a surrogate pair**
    /// (`𝑎é⏎\t:`), a position with no byte offset at all. The answer is then the astral
    /// character's own start: its high half stayed in the blanked prefix and its low half is
    /// the insert's `_`, and neither half is a line terminator, so every question asked of
    /// this offset — which line the parse was entered on, which `\n`s the insert swallowed —
    /// has the same answer at the character's start. (The one question that tells the halves
    /// apart, how many units the blanked prefix spans, is measured from the colon instead;
    /// see `AcornPrefix::line_indentation`.)
    ///
    /// Walks bytes, never a `str` slice, so no `lex_start` can make it panic: a UTF-8 lead
    /// byte opens a character, a four-byte lead is two units, and a continuation byte is none.
    ///
    /// `None` when fewer than five units precede `lex_start` — no window fits, so this is not
    /// a block binding's annotation at all (every head that reaches one spends more than five
    /// units before its colon; the shortest, `{@const x:`, spends nine). The caller decides
    /// what an impossible window means rather than being handed a quietly short one.
    #[must_use]
    pub fn as_insert_origin(source: &str, lex_start: usize) -> Option<usize> {
        let bytes = source.as_bytes();
        let mut units = 0;
        let mut at = lex_start.min(bytes.len());
        while at > 0 {
            at -= 1;
            units += utf16_units_of_byte(bytes[at]);
            if units >= Self::AS_INSERT.len() {
                return Some(at);
            }
        }
        None
    }

    /// Whether a synthetic token stands where this preparation's prefix ends, so no run of
    /// source bytes can continue through it.
    #[inline]
    const fn ends_in_synthetic_token(self) -> bool {
        matches!(self, Self::BlankedThenParen | Self::BlankedThenAs)
    }
}

/// One parse's preparation and where it ends — [`AcornPrefixText`] plus the offset the
/// manufactured bytes run out at (where Svelte's own slicing put the boundary: the parse's
/// `origin`, or one past the colon for the `_ as ` that overwrites up to there).
///
/// The pair travels together because neither half answers anything alone: a kind with no
/// boundary cannot say which bytes it covers, and a boundary with no kind cannot say what
/// stands at it. [`DOCUMENT`](Self::DOCUMENT) is the identity — the state every standalone
/// parse and every raw-template island is in — and carries no boundary at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcornPrefix {
    text: AcornPrefixText,
    /// One past the last manufactured byte — the parse's `origin` for the preparations that
    /// splice between document bytes, and one past the colon for
    /// [`BlankedThenAs`](AcornPrefixText::BlankedThenAs), whose `_ as ` overwrites the
    /// document up to there. Unread under [`AcornPrefixText::Document`].
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
    /// `_ as ` is the one that can: it stands over the five code units ending at the colon,
    /// so `{#each xs as x⏎\t: /* … */ T}` — a newline in the four units before the colon —
    /// is erased before acorn sees it, and the annotation's comment is measured from the line
    /// the window opens on. The same five units are why that region needs a line seed at all
    /// (`tsv_ts::AcornSeed`), so this is one fact read at a second place.
    ///
    /// The blanking preparations cannot do it: `[^\n]` and `\S` both leave every `\n`
    /// standing, and `read_pattern`'s wrapper deletes a *space* and inserts a `(`.
    #[must_use]
    pub(crate) fn line_start(self, source: &str, comment_start: usize) -> usize {
        let bytes = source.as_bytes();
        let insert = self.synthetic_insert_range(source);
        let mut at = comment_start.min(bytes.len());
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
    /// that *replaces* text rather than being spliced between it: the five code units ending
    /// one short of `end` ([`AcornPrefixText::as_insert_origin`]).
    #[inline]
    fn synthetic_insert_range(self, source: &str) -> std::ops::Range<usize> {
        if self.text == AcornPrefixText::BlankedThenAs {
            let end = self.end as usize;
            // No full window is no insert a `\n` could be under: an offset no parse produces.
            AcornPrefixText::as_insert_origin(source, end).unwrap_or(end)..end
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
        // Every offset below is read through `bytes` or `str::get`, never an indexing slice
        // of `source`: a caller's offset that is not a character boundary answers, it does
        // not panic.
        let ascii_run = |at: usize| {
            let mut end = at;
            while matches!(bytes.get(end), Some(b' ' | b'\t')) {
                end += 1;
            }
            // `[ \t]` bytes only, so the slice is valid wherever it starts.
            source.get(at..end).unwrap_or("")
        };
        // Where the document's own bytes take over, past the blanked prefix and any synthetic
        // token standing over document text — `end` for every preparation, including
        // `BlankedThenAs`, whose manufacture runs out one past the colon.
        let manufactured_end = (self.end as usize).min(bytes.len());
        if self.text == AcornPrefixText::Document || line_start >= manufactured_end {
            // Past the manufacture the region's own bytes are standing, so this is the
            // document's own run either way.
            return Cow::Borrowed(ascii_run(line_start));
        }

        let mut run = String::new();
        if self.text == AcornPrefixText::WhitespaceKept {
            // The author's whitespace survived; everything else became a space. A whitespace
            // character that is not ` ` or `\t` ends the run just as it would in the document.
            for ch in source
                .get(line_start..manufactured_end)
                .unwrap_or("")
                .chars()
            {
                match ch {
                    ' ' | '\t' => run.push(ch),
                    _ if is_js_whitespace(ch) => return Cow::Owned(run),
                    // One space per code unit: see the length note above.
                    _ => run.extend(std::iter::repeat_n(' ', ch.len_utf16())),
                }
            }
        } else {
            // `[^\n]` became a space, and no `\n` can sit between a line's start and a
            // position on that same line — so the whole span is spaces.
            let mut width = blanked_width(&bytes[line_start..manufactured_end]);
            match self.text {
                // The `_ as ` overwrote the last five of those units (the colon's included),
                // so the blanking reached only the ones before them. Counted back from the
                // colon, never forward from `as_insert_origin`: a window opening between a
                // surrogate pair's halves blanked the high one, which no byte offset can say.
                // Empty when the line opens AT the insert — the `\n` one unit short of the
                // window survived — since the insert's `_` then ends the run at zero. A line
                // cannot open INSIDE the window, since a `\n` there is the one the insert
                // swallowed (see `line_start`); the subtraction saturates rather than asserts
                // so an offset no parse produces still answers.
                AcornPrefixText::BlankedThenAs => {
                    width = width.saturating_sub(AcornPrefixText::AS_INSERT.len());
                }
                AcornPrefixText::BlankedThenParen if paren_space_fell_here(source, line_start) => {
                    width -= 1;
                }
                _ => {}
            }
            run.extend(std::iter::repeat_n(' ', width));
        }

        if !self.text.ends_in_synthetic_token() {
            // Nothing stands between the prefix and the region, so the run carries on into the
            // region's own leading whitespace.
            run.push_str(ascii_run(manufactured_end));
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
///
/// Counted over BYTES ([`utf16_units_of_byte`]), so a span whose ends are not character
/// boundaries is still an answer rather than a panic.
#[inline]
fn blanked_width(blanked: &[u8]) -> usize {
    // `len()` is the answer for the ASCII prefix that essentially every document has; the walk
    // only runs when it isn't.
    if blanked.is_ascii() {
        blanked.len()
    } else {
        blanked.iter().map(|&b| utf16_units_of_byte(b)).sum()
    }
}

/// The UTF-16 code units the UTF-8 byte `b` opens: one for a single-byte character or the lead
/// of a two- or three-byte one, two for the lead of a four-byte (astral) one, and none for a
/// continuation byte — so summing over a character's bytes gives its `len_utf16`.
#[inline]
const fn utf16_units_of_byte(b: u8) -> usize {
    if b & 0xC0 == 0x80 {
        0
    } else if b >= 0xF0 {
        2
    } else {
        1
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
        // `_ as ` stands over the five units ending at the colon, so the run is the blanked
        // span alone — never the region's own leading whitespace behind the insert.
        let source = "\t{@const x:  /* a\n\t b */";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 11);
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
        // The author's `\n` sits one unit AHEAD of the insert window, so it survives and the
        // line opens exactly where `_ as ` begins. Under the insert are four spaces the
        // document holds and acorn never saw — the run is `_`'s, which is empty.
        let source = "{#each xs as x\n    : /* a\n    b */ T}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 20);
        assert_eq!(prefix.line_start(source, 21), 15);
        assert_eq!(prefix.line_indentation(source, 15), "");
        // The null control on the same shape: with the `\n` INSIDE the window the insert
        // swallows it, the line opens back on the binding's, and the run is the blanking's.
        let swallowed = "{#each xs as x\n\t: /* a\n\tb */ T}";
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, 17);
        assert_eq!(prefix.line_start(swallowed, 18), 0);
        assert_eq!(prefix.line_indentation(swallowed, 0), " ".repeat(12));
    }

    #[test]
    fn the_as_insert_window_is_five_code_units() {
        // `as⏎éé:` — the window is `\n`, `é`, `é` and the colon's four units back reach the
        // `s`: five units over seven bytes. The `\n` is swallowed.
        let source = "{#each xs as\néé: /* a\n b */ T}";
        let lex_start = source.find(':').unwrap_or_default() + 1;
        assert_eq!(
            AcornPrefixText::as_insert_origin(source, lex_start),
            Some(11)
        );
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, lex_start as u32);
        assert_eq!(prefix.line_start(source, lex_start + 1), 0);
        // `{#each xs a` blanked: eleven units, the `_ as ` over the rest.
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(11));

        // `as⏎éééé:` — four units of binding fill the window, and the `\n` survives.
        let source = "{#each xs as\néééé: /* a\n b */ T}";
        let lex_start = source.find(':').unwrap_or_default() + 1;
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, lex_start as u32);
        assert_eq!(prefix.line_start(source, lex_start + 1), 13);
        assert_eq!(prefix.line_indentation(source, 13), "");
    }

    #[test]
    fn an_as_insert_window_can_open_between_a_surrogate_pair() {
        // `𝑎é⏎\t:` — four units back from the colon is the LOW half of `𝑎`. The origin is the
        // character's start; its high half is one blanked space, so `{#each xs as 𝑎` blanks
        // to fourteen units, not fifteen and not thirteen.
        let source = "{#each xs as \u{1d44e}é\n\t: /* a\n b */ T}";
        let lex_start = source.find(':').unwrap_or_default() + 1;
        assert_eq!(
            AcornPrefixText::as_insert_origin(source, lex_start),
            Some(13)
        );
        // fewer than five units ahead: no window
        assert_eq!(AcornPrefixText::as_insert_origin("\u{1d44e}é:", 7), None);
        let prefix = AcornPrefix::manufactured(AcornPrefixText::BlankedThenAs, lex_start as u32);
        assert_eq!(prefix.line_start(source, lex_start + 1), 0);
        assert_eq!(prefix.line_indentation(source, 0), " ".repeat(14));
    }

    #[test]
    fn offsets_off_a_character_boundary_answer_rather_than_panic() {
        // Every offset lands inside `é` (bytes 1..3); none may slice the `str` there.
        let source = "xé: /* a\n b */";
        for end in 0..=source.len() + 1 {
            let _ = AcornPrefixText::as_insert_origin(source, end);
            for text in [
                AcornPrefixText::Blanked,
                AcornPrefixText::BlankedThenParen,
                AcornPrefixText::BlankedThenAs,
                AcornPrefixText::WhitespaceKept,
            ] {
                let prefix = AcornPrefix::manufactured(text, end as u32);
                for at in 0..=source.len() + 1 {
                    let _ = prefix.line_start(source, at);
                    let _ = prefix.line_indentation(source, at.min(source.len()));
                }
            }
        }
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
