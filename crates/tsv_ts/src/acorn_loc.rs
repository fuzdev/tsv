//! Where acorn's line/column counter was seeded for one embedded parse.
//!
//! A `LocationTracker` answers "which line is this byte on?" for a whole source
//! under one rule. An acorn-owned `loc` is not that answer: acorn seeds its
//! counter **once per parse**, over whatever prefix the host prepared for that
//! parse, and skips the run between its own `lineStart` and the first byte it
//! lexes. [`AcornSeed`] carries that per-parse difference as three constants, so
//! one ECMAScript-rule table can serve every island in a document.
//!
//! This lives in `tsv_ts` rather than in `tsv_lang` because it is a fact about
//! **acorn**, the parser this crate is the drop-in replacement for — not about
//! source locations in general. `tsv_lang` supplies the tables and the
//! byte→UTF-16 map; nothing in it needs to know how acorn was seeded. The two
//! consumers are this crate's wire writer and `tsv_svelte`, which already
//! depends on it.
//!
//! Full model: docs/architecture.md §`loc` lines — two classes, one per acorn parse.

use tsv_lang::{LocationMapper, LocationTracker, Position};

/// Which line-terminator class acorn counted in the text **ahead of** one
/// embedded parse — the axis Svelte's per-call source preparation decides.
///
/// Svelte hands acorn a different string at every embedded parse, and the choice
/// of what it does to the bytes before the region is exactly this enum:
///
/// - [`Lf`](Self::Lf) — the prefix was blanked with `replace(/[^\n]/g, ' ')`, so
///   only its LFs survived and acorn's line count over it is Svelte's own
///   (`<script>` content, `read_pattern` destructures, `read_type_annotation`).
/// - [`Ecmascript`](Self::Ecmascript) — acorn got the raw template (every
///   `read_expression` island, the bare `{const …}` / `{let …}` statement, and the
///   snippet parameter list, whose `replace(/\S/g, ' ')` prelude keeps *all* whitespace),
///   so every ECMAScript terminator ahead of the region counted.
#[derive(Clone, Copy, Debug)]
pub enum PrefixLines {
    Lf,
    Ecmascript,
}

/// The line/column origin of one **acorn parse**, for re-seeding an
/// ECMAScript-rule tracker's answers onto it.
///
/// acorn seeds `curLine` / `lineStart` **once**, at construction, and only then
/// advances them over the ECMAScript terminators it lexes:
///
/// ```js
/// this.lineStart = input.lastIndexOf("\n", startPos - 1) + 1;  // LF only
/// this.curLine   = input.slice(0, this.lineStart).split(lineBreak).length;
/// ```
///
/// So an acorn-owned `loc` is *not* the ECMAScript tracker's answer: the tracker
/// counts every terminator in the source, while acorn counted only the ones in
/// the prefix Svelte let it see, and none at all in `[lineStart, startPos)` —
/// which it skips over entirely. This carries that difference as two constants
/// applied to the tracker's answer, plus the line they apply on.
///
/// [`NONE`](Self::NONE) is the identity, and is what every non-Svelte writer emits
/// under. A Svelte document reaches it too on essentially every real source — but
/// "the two line classes agree" is **not** the whole condition, because a parse
/// entered *behind* where it starts lexing (Svelte's `read_type_annotation`, whose
/// synthetic `_ as ` overwrites the bytes it covers) counts a line the author wrote
/// and acorn never saw, on a plain-LF source. See `tsv_svelte`'s `AcornLines`.
///
/// The three fields are `u32` for the same reason [`Span`](tsv_lang::Span) is:
/// each is bounded by the source length, which the parsers already refuse past
/// `u32::MAX`. That keeps the seed 12 bytes, so carrying two of them (a block
/// pattern's two parses) costs an `EmbedWriter` less than one `Position` would.
#[derive(Clone, Copy, Debug)]
pub struct AcornSeed {
    /// acorn's line number for the region's first line, or `0` when inactive.
    /// `0` never equals a real 1-based line, so the column rule is then inert.
    first_line: u32,
    /// Lines to subtract from the ECMAScript tracker's answer: the terminators
    /// it counted ahead of the region that acorn did not.
    line_delta: u32,
    /// Columns to add on `first_line` only: the distance from acorn's own
    /// `lineStart` out to the ECMAScript tracker's, in emitted units. Nonzero
    /// only when a non-LF terminator sits between the two.
    column_shift: u32,
}

impl AcornSeed {
    /// The identity seed: the tracker's answer, emitted as-is.
    pub const NONE: Self = Self {
        first_line: 0,
        line_delta: 0,
        column_shift: 0,
    };

    /// The seed for the acorn parse Svelte started at `origin` and that begins
    /// lexing real source bytes at `lex_start`.
    ///
    /// The two positions differ only where Svelte *inserts* synthetic text at the
    /// parse start — `read_type_annotation`'s `_ as `, which acorn lexes instead
    /// of the bytes it covers. Everywhere else they are the same position.
    ///
    /// `acorn` is the ECMAScript-rule mapper the re-seeded answers come from and
    /// `lf` the LF-rule tracker over that same source. Taking a *mapper* on the
    /// acorn side rather than a second bare tracker is what keeps the byte→UTF-16
    /// map this shifts columns in the very one those answers were emitted
    /// through — a line rule never affects the map, so there is only ever one.
    pub fn new(
        lf: &LocationTracker,
        acorn: LocationMapper<'_>,
        origin: u32,
        lex_start: u32,
        prefix: PrefixLines,
    ) -> Self {
        // acorn's `lineStart` at `origin` is the last LF at or before it, and its
        // `curLine` is that position's line under whichever rule the prefix left
        // standing — which is the same line as `origin`'s under both.
        let column_origin = lf.line_start_byte(origin as usize);
        let first_line = match prefix {
            PrefixLines::Lf => lf.get_line_column(origin as usize).0,
            PrefixLines::Ecmascript => acorn.tracker.get_line_column(column_origin).0,
        };
        // Both hit the tracker's 1-entry line cache, the second for free.
        let acorn_line = acorn.tracker.get_line_column(lex_start as usize).0;
        let acorn_line_start = acorn.tracker.line_start_byte(lex_start as usize);
        Self {
            first_line: first_line as u32,
            line_delta: (acorn_line - first_line) as u32,
            column_shift: acorn.pos(acorn_line_start as u32) - acorn.pos(column_origin as u32),
        }
    }

    /// Whether this seed leaves the tracker's answer alone — the question that
    /// decides whether a document needs the re-seeding route at all.
    ///
    /// `first_line` is not consulted: with no lines to subtract and no columns to
    /// add, the line it would have applied on is inert. So a computed seed over a
    /// document that needs no re-basing compares equal in *behaviour* to
    /// [`NONE`](Self::NONE) without having to equal it field-for-field.
    #[inline]
    pub const fn is_identity(self) -> bool {
        self.line_delta == 0 && self.column_shift == 0
    }

    /// acorn's position for one the ECMAScript tracker put at `pos`.
    ///
    /// The two halves are one call rather than two because the column rule reads
    /// the **re-seeded** line, not the tracker's: a caller holding a `line()` and
    /// a `column()` has to know to feed the first into the second, and feeding it
    /// the tracker's line instead is a silent off-by-a-column on exactly the
    /// documents this whole mechanism exists for.
    #[inline]
    pub fn position(self, pos: Position) -> Position {
        let line = pos.line - self.line_delta as usize;
        let column = if line == self.first_line as usize {
            pos.column + self.column_shift as usize
        } else {
            pos.column
        };
        Position { line, column }
    }
}
