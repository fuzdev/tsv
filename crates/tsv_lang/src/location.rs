use std::cell::Cell;

use crate::sizing::estimated_line_starts_capacity;
use crate::swar::{ascii_has_byte_below, ascii_zero_lanes, high_bit_lanes, splat};

/// A position in source code (line and column)
///
/// Generic type without serialization - languages can wrap this in their own types
/// that include serde derives if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Maps byte offsets to JS-compatible character offsets (UTF-16 code units)
///
/// Rust strings are byte-indexed, but JS (and Svelte/acorn) uses UTF-16
/// code unit indices. For ASCII-only sources, byte == char offset, so the map is empty.
/// For sources with multibyte UTF-8 characters, the map stores, for each byte
/// position, the **delta** `byte − utf16` rather than the absolute UTF-16 offset:
/// the delta is what a multibyte character actually contributes, it is
/// non-decreasing across the source, and it stays small enough to hold in a
/// `u8` on any source whose multibyte characters are sparse — which is the
/// common case (a real TS corpus runs ~1 non-ASCII character per 1000 bytes,
/// yet ~90% of its files contain at least one). Narrowing the table from `u32`
/// to `u8` cuts its construction write traffic and its cache footprint 4×; a
/// source dense enough to outgrow `u8` widens back to `u32`.
///
/// Lookup stays a single O(1) indexed load either way (`pos = byte −
/// delta[byte]`). (A sparse per-multibyte-char representation with
/// binary-search *lookup* was measured at +3% instructions on a
/// multibyte-dense corpus — the O(1) dense lookup wins; don't re-derive. That
/// result is about lookup only, and the delta narrowing leaves it intact.)
///
/// Characters in the BMP (U+0000-U+FFFF) count as 1 UTF-16 code unit.
/// Characters outside the BMP (U+10000+, e.g., most emoji) count as 2 (surrogate pair).
///
/// Only valid for byte positions at character boundaries (i.e., positions returned
/// by the parser, which always point to the start of a character); a byte
/// position inside a multibyte character resolves to that character's start.
#[derive(Debug)]
pub struct ByteToCharMap {
    deltas: Deltas,
}

/// The `byte − utf16` delta table, one entry per source byte plus an
/// end-of-source sentinel. `Identity` carries no table at all (ASCII-only
/// source, or the byte-space passthrough mode).
#[derive(Debug)]
enum Deltas {
    Identity,
    Narrow(Vec<u8>),
    Wide(Vec<u32>),
}

/// Whether a byte-order mark at byte 0 occupies a wire position — the one place the
/// canonical parsers disagree about what string their offsets index.
///
/// acorn **counts** it: U+FEFF is whitespace to its tokenizer, so a BOM-led `.ts` file's
/// first statement starts at offset 1, column 1, and `Program.start` stays 0. Svelte's
/// `parse` and `parseCss` **strip** it before parsing (`remove_bom` in the compiler's
/// entry points), so every offset they emit indexes the BOM-less string — one UTF-16 unit
/// lower than the author's file, and one column lower on line 1. Each wire follows its own
/// oracle, so the writer names which reading its map takes; a map built `Elided` resolves
/// the BOM's three bytes to position 0 and every later byte one unit lower than `Counted`
/// would. A U+FEFF anywhere but byte 0 is an ordinary character under both.
///
/// A parameter rather than a default because a silent choice is the failure mode in both
/// directions: the TS wire *must* count it to stay a drop-in for acorn, and the Svelte
/// and CSS wires *must not*, and nothing in a fixture without a BOM can tell the two apart.
/// The parsers themselves are untouched — their spans index the author's bytes, and only
/// the emitted position moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadingBom {
    /// The BOM is one UTF-16 unit at position 0 (acorn).
    Counted,
    /// The BOM occupies no position; offsets index the BOM-less string (Svelte, `parseCss`).
    Elided,
}

/// U+FEFF: a byte-order mark at a file's byte 0, and the character ZERO WIDTH NO-BREAK
/// SPACE anywhere else.
pub const BOM: char = '\u{FEFF}';

/// The byte length of the byte-order mark `source` begins with — [`BOM`]'s three UTF-8
/// bytes, or 0 when there is none. Every lexer starts past it: the parsers skip a leading
/// BOM, and how the wire then counts it is [`LeadingBom`]'s question.
#[inline]
#[must_use]
pub fn leading_bom_len(source: &str) -> usize {
    if source.starts_with(BOM) {
        BOM.len_utf8()
    } else {
        0
    }
}

impl ByteToCharMap {
    /// Build a byte-to-UTF-16-code-unit offset map from source text
    ///
    /// For ASCII-only sources, returns an empty map (fast path).
    pub(crate) fn new(source: &str, bom: LeadingBom) -> Self {
        if source.is_ascii() {
            return Self::identity();
        }
        build_map(source, &mut LineScan::map_only(), LineRule::None, bom)
    }

    /// The identity map: every byte offset translates to itself — what `new`
    /// returns for an all-ASCII source, where a byte offset is its UTF-16 offset.
    pub(crate) const fn identity() -> Self {
        Self {
            deltas: Deltas::Identity,
        }
    }

    /// Convert a byte offset to a UTF-16 code unit offset
    ///
    /// For ASCII-only sources, returns the byte offset unchanged. Offsets
    /// past the end of the source also translate to themselves (a missing
    /// entry is a zero delta).
    #[inline]
    pub(crate) fn byte_to_char(&self, byte_offset: u32) -> u32 {
        match &self.deltas {
            Deltas::Identity => byte_offset,
            Deltas::Narrow(deltas) => {
                byte_offset - u32::from(deltas.get(byte_offset as usize).copied().unwrap_or(0))
            }
            Deltas::Wide(deltas) => {
                byte_offset - deltas.get(byte_offset as usize).copied().unwrap_or(0)
            }
        }
    }

    /// Whether the source contains multibyte UTF-8 characters
    #[inline]
    pub(crate) fn has_multibyte(&self) -> bool {
        !matches!(self.deltas, Deltas::Identity)
    }
}

/// Which line-terminator rule the fused map builder applies. `None` skips line
/// starts entirely — the map-only path.
///
/// A runtime parameter rather than a `const` one: it is consulted once per
/// *run* (the stretch between two multibyte characters), never per byte, so
/// specializing on it buys nothing measurable and costs three extra
/// monomorphizations of the builder in every shipped artifact.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LineRule {
    None,
    Lf,
    Ecmascript,
}

/// What a line-rule scan produces: the line starts.
///
/// The constructors carry the leading-`0` precondition the builders rely on, so
/// it is a type state rather than a doc comment — which is why there is no
/// `Default`: the derived one would seed no line 1 while collecting starts, the
/// single state the builders cannot produce a correct table from.
struct LineScan {
    starts: Vec<u32>,
}

impl LineScan {
    /// A scan that collects line starts over a source of `source_len` bytes,
    /// seeded with line 1's — byte 0 — at the table's estimated capacity.
    fn lines(source_len: usize) -> Self {
        Self {
            starts: seeded_line_starts(source_len),
        }
    }

    /// A scan that collects no line data at all ([`LineRule::None`]).
    fn map_only() -> Self {
        Self { starts: Vec::new() }
    }

    /// An unseeded scan, for the unit tests that grade a single ASCII *run*
    /// against a reference scan of that same run — where a line-1 start the
    /// reference never emits would be the only difference.
    ///
    /// Deliberately spelled as `map_only`'s alias rather than as its own literal:
    /// the two are the same VALUE and differ only in what the caller is claiming,
    /// and writing the fields twice would let one drift while both names still
    /// compiled.
    #[cfg(test)]
    fn bare() -> Self {
        Self::map_only()
    }
}

/// A line-start table holding line 1's start (byte 0), reserved at
/// `estimated_line_starts_capacity` for a `source_len`-byte source — every
/// scanning constructor's seed, so no table grows from a single entry by
/// doubling.
fn seeded_line_starts(source_len: usize) -> Vec<u32> {
    let mut starts = Vec::with_capacity(estimated_line_starts_capacity(source_len));
    starts.push(0);
    starts
}

/// One delta-table element width. `u8` is what a sparse-multibyte source needs;
/// `u32` is the widening fallback for a source whose deltas outgrow it.
trait DeltaElem: Copy {
    /// The largest delta this element can hold. The `u32` arm's check folds
    /// away — a delta never exceeds the source length.
    const MAX_DELTA: u32;
    fn from_delta(delta: u32) -> Self;
    fn into_deltas(deltas: Vec<Self>) -> Deltas;
}

impl DeltaElem for u8 {
    const MAX_DELTA: u32 = Self::MAX as u32;
    #[inline]
    fn from_delta(delta: u32) -> Self {
        delta as Self
    }
    fn into_deltas(deltas: Vec<Self>) -> Deltas {
        Deltas::Narrow(deltas)
    }
}

impl DeltaElem for u32 {
    const MAX_DELTA: u32 = Self::MAX;
    #[inline]
    fn from_delta(delta: u32) -> Self {
        delta
    }
    fn into_deltas(deltas: Vec<Self>) -> Deltas {
        Deltas::Wide(deltas)
    }
}

/// Where a narrow build stopped because the next character's deltas would not
/// fit — the resume point for the wide continuation.
struct Outgrown {
    /// Byte index of the character that didn't fit; also the number of table
    /// entries already written, since the scan writes exactly one per byte.
    at: usize,
    /// The running delta as of `at`.
    delta: u32,
}

/// Index of the first non-ASCII byte at or after `from`, or `bytes.len()`.
///
/// Word-at-a-time: the multibyte characters this splits the source on are
/// sparse, so the scan between them is the bulk of construction.
#[inline]
fn next_non_ascii(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let hits = high_bit_lanes(u64::from_le_bytes(*chunk));
        if hits != 0 {
            // Little-endian: the lowest set bit is the high bit of the
            // earliest non-ASCII byte in the word.
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < bytes.len() && bytes[i] < 0x80 {
        i += 1;
    }
    i
}

/// Fill the `byte − utf16` delta table (and, per `line_rule`, the line starts),
/// resuming at byte `from` with running delta `delta`.
///
/// The scan walks *runs*: everything between two multibyte characters is a
/// constant delta (one fill) and pure ASCII (so the line scan is the same
/// byte-level one the all-ASCII fast path uses, shared verbatim rather than
/// re-derived). Only the multibyte characters themselves are handled per byte.
///
/// Returns `Err(Outgrown)` — leaving `deltas` and `lines` correct and complete
/// up to that point, for the wide continuation to pick up — if the next
/// character's deltas would not fit `T`.
fn build_deltas<T: DeltaElem>(
    source: &str,
    deltas: &mut Vec<T>,
    lines: &mut LineScan,
    from: usize,
    mut delta: u32,
    line_rule: LineRule,
) -> Result<(), Outgrown> {
    let bytes = source.as_bytes();
    let mut i = from;

    while i < bytes.len() {
        let run_end = next_non_ascii(bytes, i);
        if run_end > i {
            match line_rule {
                LineRule::Lf => ascii_lf_line_starts_into(&bytes[i..run_end], i, lines),
                LineRule::Ecmascript => {
                    ascii_ecmascript_line_starts_into(&bytes[i..run_end], i, lines);
                }
                LineRule::None => {}
            }
            // The whole ASCII run shares the running delta.
            deltas.resize(run_end, T::from_delta(delta));
            i = run_end;
            if i == bytes.len() {
                break;
            }
        }

        // A multibyte character starts at `i`. Each of its bytes gets the
        // delta that resolves it to the character's own start, so a byte
        // position inside the character reads back as that start.
        let lead = bytes[i];
        let len = utf8_len(lead);
        // The character's last byte carries `delta + len - 1`, the widest value
        // it writes — and the delta it leaves behind is no wider than that.
        if T::MAX_DELTA - delta < len as u32 - 1 {
            return Err(Outgrown { at: i, delta });
        }
        match len {
            2 => {
                deltas.push(T::from_delta(delta));
                deltas.push(T::from_delta(delta + 1));
                delta += 1;
                i += 2;
            }
            3 => {
                deltas.push(T::from_delta(delta));
                deltas.push(T::from_delta(delta + 1));
                deltas.push(T::from_delta(delta + 2));
                delta += 2;
                // U+2028 / U+2029 are line terminators under the ECMAScript
                // rule only, and are the sole multibyte ones (E2 80 A8/A9).
                if line_rule == LineRule::Ecmascript
                    && lead == 0xE2
                    && bytes[i + 1] == 0x80
                    && matches!(bytes[i + 2], 0xA8 | 0xA9)
                {
                    lines.starts.push((i + 3) as u32);
                }
                i += 3;
            }
            _ => {
                // Astral: 4 bytes, 2 UTF-16 code units (surrogate pair).
                deltas.push(T::from_delta(delta));
                deltas.push(T::from_delta(delta + 1));
                deltas.push(T::from_delta(delta + 2));
                deltas.push(T::from_delta(delta + 3));
                delta += 2;
                i += 4;
            }
        }
    }

    // The trailing run plus the end-of-source sentinel.
    deltas.resize(bytes.len() + 1, T::from_delta(delta));
    Ok(())
}

/// UTF-8 length from a lead byte (only ever called on one, so the 2-byte arm
/// covers the whole `0x80..0xE0` range).
#[inline]
fn utf8_len(lead: u8) -> usize {
    if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

/// Build the delta table for a source known to contain a multibyte character,
/// narrowing to `u8` when the source's deltas fit — which is every source whose
/// multibyte characters are sparse. `lines` must match `line_rule`:
/// [`LineScan::lines`] for the two collecting rules, [`LineScan::map_only`] for
/// [`LineRule::None`].
///
/// An elided leading BOM is the one character whose deltas the scan does not derive from
/// its width: its three bytes resolve to position 0 and it leaves a running delta of 3
/// (`byte − utf16` with the unit it would have counted for gone), so every later byte
/// reads one unit lower. It holds no line terminator, so the line scan can start behind it.
fn build_map(
    source: &str,
    lines: &mut LineScan,
    line_rule: LineRule,
    bom: LeadingBom,
) -> ByteToCharMap {
    let mut narrow: Vec<u8> = Vec::with_capacity(source.len() + 1);
    let bom_len = match bom {
        LeadingBom::Elided => leading_bom_len(source),
        LeadingBom::Counted => 0,
    };
    if bom_len > 0 {
        narrow.extend_from_slice(&[0, 1, 2]);
    }
    // An elided BOM's three bytes resolve to position 0: a running delta of its length.
    let (from, delta) = (bom_len, bom_len as u32);
    let Err(outgrown) = build_deltas::<u8>(source, &mut narrow, lines, from, delta, line_rule)
    else {
        return ByteToCharMap {
            deltas: u8::into_deltas(narrow),
        };
    };

    // A multibyte-dense source (hundreds of non-ASCII characters) whose running
    // delta outgrew `u8`. Widen what the narrow scan already produced and
    // continue from where it stopped — the source is scanned once either way.
    let mut wide: Vec<u32> = Vec::with_capacity(source.len() + 1);
    wide.extend(narrow.iter().copied().map(u32::from));
    drop(narrow);
    debug_assert_eq!(wide.len(), outgrown.at, "widened prefix must be complete");
    // Infallible: `u32::MAX_DELTA` is unreachable — a delta never exceeds the
    // source length, and spans are `u32` throughout.
    let _ = build_deltas::<u32>(
        source,
        &mut wide,
        lines,
        outgrown.at,
        outgrown.delta,
        line_rule,
    );
    ByteToCharMap {
        deltas: u32::into_deltas(wide),
    }
}

/// A `LocationTracker` paired with a `ByteToCharMap`: converts byte spans to
/// emitted positions in one step.
///
/// The wire-JSON writers thread this instead of a bare tracker so position
/// emission and byte→UTF-16 translation fuse into one pass:
///
/// - with a real map (`ByteToCharMap::new(source, bom)`), `pos_and_position`
///   and the span forms emit final UTF-16 code-unit offsets and char-based
///   columns directly — no post-conversion translation walk;
/// - with `ByteToCharMap::identity()` (an all-ASCII source, where a byte offset
///   and a UTF-16 offset coincide), both are exact byte-space passthrough.
///
/// The fused column is `char_col = map(offset) − map(line_start)`: the UTF-16
/// distance from the line's start, measured on the same emitted offsets the
/// node's `start`/`end` carry.
#[derive(Clone, Copy, Debug)]
pub struct LocationMapper<'a> {
    pub tracker: &'a LocationTracker,
    pub map: &'a ByteToCharMap,
}

impl LocationMapper<'_> {
    /// The emitted offset (UTF-16 code units) plus its `Position`, in one translation —
    /// the per-endpoint form direct wire emitters use (translating the offset with
    /// `WirePositions::pos` and deriving the `Position` separately would put
    /// `byte_offset` through the map twice on the multibyte path).
    #[inline]
    pub fn pos_and_position(&self, byte_offset: u32) -> (u32, Position) {
        let (line, byte_column) = self.tracker.get_line_column(byte_offset as usize);
        if self.map.has_multibyte() {
            let pos = self.map.byte_to_char(byte_offset);
            let line_start = byte_offset as usize - byte_column;
            let column = (pos - self.map.byte_to_char(line_start as u32)) as usize;
            (pos, Position { line, column })
        } else {
            // Byte-space passthrough: the map is identity, so the emitted
            // offset is the byte offset itself.
            (
                byte_offset,
                Position {
                    line,
                    column: byte_column,
                },
            )
        }
    }

    /// Both endpoints of a span, each as `pos_and_position` would return it —
    /// the form every wire emitter wants, since a node header writes `start`
    /// and `end` together.
    ///
    /// Resolves the two lines through [`LocationTracker::resolve_span`], which
    /// searches `end` forward from `start`'s line instead of from scratch and
    /// keeps the line cache parked on the descending line. Byte-identical to
    /// `pos_and_position(start)` + `pos_and_position(end)`.
    ///
    /// `inline(always)`: plain `#[inline]` was declined here — it outlined as
    /// a 3.4% standalone symbol — and the writer's node header then paid
    /// for an opaque call mid-staged-run forcing every following
    /// append to re-load the stage cursor. Forcing it turned the whole change
    /// from **+0.14%** instructions to **−1.72%**, with cycles moving
    /// alongside. Four call sites, so the code-size exposure is bounded (the
    /// `parse` WASM bundle moved +0.16%, and `format` — which builds without
    /// the `json` feature — stayed flat at +21 B, confirming the growth is
    /// confined to the convert path).
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn span_positions(&self, start: u32, end: u32) -> ((u32, Position), (u32, Position)) {
        let ((start_line, start_line_byte), (end_line, end_line_byte)) =
            self.tracker.resolve_span(start as usize, end as usize);
        if self.map.has_multibyte() {
            let start_pos = self.map.byte_to_char(start);
            let end_pos = self.map.byte_to_char(end);
            let start_column = (start_pos - self.map.byte_to_char(start_line_byte as u32)) as usize;
            let end_column = (end_pos - self.map.byte_to_char(end_line_byte as u32)) as usize;
            (
                (
                    start_pos,
                    Position {
                        line: start_line + 1,
                        column: start_column,
                    },
                ),
                (
                    end_pos,
                    Position {
                        line: end_line + 1,
                        column: end_column,
                    },
                ),
            )
        } else {
            // Byte-space passthrough: the map is identity, so the emitted
            // offsets are the byte offsets and columns are byte columns.
            (
                (
                    start,
                    Position {
                        line: start_line + 1,
                        column: start as usize - start_line_byte,
                    },
                ),
                (
                    end,
                    Position {
                        line: end_line + 1,
                        column: end as usize - end_line_byte,
                    },
                ),
            )
        }
    }

    /// [`LocationMapper::span_positions`] for a span that lies on **one line** — which
    /// the caller guarantees — resolving only `start`'s line: `end` shares it, and its
    /// column is `start`'s plus the span's width in emitted units, since both columns
    /// are measured from the same line start in the same space. Byte-identical to
    /// `span_positions` on such a span (debug builds check it).
    ///
    /// For a name: every name span stops at whitespace, so it never crosses a line
    /// under a line rule whose terminators are all whitespace. `inline(always)`, as
    /// `span_positions` is, so the line cache's hit stays inline at the caller.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn one_line_span_positions(
        &self,
        start: u32,
        end: u32,
    ) -> ((u32, Position), (u32, Position)) {
        let (line_idx, line_byte) = self.tracker.resolve_line(start as usize);
        let line = line_idx + 1;
        let positions = if self.map.has_multibyte() {
            let start_pos = self.map.byte_to_char(start);
            let end_pos = self.map.byte_to_char(end);
            let column = (start_pos - self.map.byte_to_char(line_byte as u32)) as usize;
            (
                (start_pos, Position { line, column }),
                (
                    end_pos,
                    Position {
                        line,
                        column: column + (end_pos - start_pos) as usize,
                    },
                ),
            )
        } else {
            (
                (
                    start,
                    Position {
                        line,
                        column: start as usize - line_byte,
                    },
                ),
                (
                    end,
                    Position {
                        line,
                        column: end as usize - line_byte,
                    },
                ),
            )
        };
        debug_assert_eq!(
            positions,
            self.span_positions(start, end),
            "a one-line span must lie on one line"
        );
        positions
    }
}

/// What a wire writer emits positions from: the byte→UTF-16 map every `start` / `end`
/// needs, and — only when the wire carries `loc` — the document's line table. Built by
/// [`WireTables::positions`], never directly, so the line table is always the one built
/// alongside the map.
///
/// The one definition of `loc` every writer implements: an object with numeric
/// `start` / `end` gets `loc.start` / `loc.end` at the line (1-based) and column
/// (0-based, UTF-16 code units) of those same emitted offsets, under the document's
/// line table — [`LocationTracker::new_ecmascript_with_map`] for a TypeScript
/// document, [`LocationTracker::new_with_map`] for a Svelte document (every island
/// in it included) and a CSS one. No line table is the span-only wire.
///
/// The line table exists only under the `locations` feature. Without it [`Self::lines`]
/// is a constant `None`, so every `loc` emitter in the writers is dead code the
/// optimizer removes — what keeps the bindings, which ship the span-only wire alone,
/// from carrying the loc writer they never run.
#[derive(Clone, Copy, Debug)]
pub struct WirePositions<'a> {
    map: &'a ByteToCharMap,
    #[cfg(feature = "locations")]
    lines: Option<&'a LocationTracker>,
}

impl<'a> WirePositions<'a> {
    /// Byte offset → emitted (UTF-16 code unit) offset.
    #[inline]
    #[must_use]
    pub fn pos(&self, byte_offset: u32) -> u32 {
        self.map.byte_to_char(byte_offset)
    }

    /// The line/column mapper, or `None` on the span-only wire.
    #[inline]
    #[must_use]
    pub fn lines(&self) -> Option<LocationMapper<'a>> {
        #[cfg(feature = "locations")]
        {
            self.lines.map(|tracker| LocationMapper {
                tracker,
                map: self.map,
            })
        }
        #[cfg(not(feature = "locations"))]
        {
            None
        }
    }

    /// Whether this wire carries `loc`.
    #[inline]
    #[must_use]
    pub const fn has_locations(&self) -> bool {
        #[cfg(feature = "locations")]
        {
            self.lines.is_some()
        }
        #[cfg(not(feature = "locations"))]
        {
            false
        }
    }
}

/// The tables one document's wire is written from, owned: the byte→UTF-16 map every
/// wire needs and — only for the loc-bearing wire — the line table built alongside it
/// in the same source scan. [`Self::positions`] lends both to a writer.
#[derive(Debug)]
pub struct WireTables {
    map: ByteToCharMap,
    #[cfg(feature = "locations")]
    lines: Option<LocationTracker>,
}

impl WireTables {
    /// A TypeScript document's tables: ECMAScript's line terminators
    /// ([`LocationTracker::new_ecmascript_with_map`]) when `locations` is set, the map
    /// alone otherwise.
    #[must_use]
    pub fn ecmascript(source: &str, bom: LeadingBom, locations: bool) -> Self {
        #[cfg(feature = "locations")]
        if locations {
            let (lines, map) = LocationTracker::new_ecmascript_with_map(source, bom);
            return Self {
                map,
                lines: Some(lines),
            };
        }
        Self::span_only(source, bom, locations)
    }

    /// A Svelte or CSS document's tables: `\n` alone starts a line
    /// ([`LocationTracker::new_with_map`]) when `locations` is set, the map alone
    /// otherwise.
    #[must_use]
    pub fn lf(source: &str, bom: LeadingBom, locations: bool) -> Self {
        #[cfg(feature = "locations")]
        if locations {
            let (lines, map) = LocationTracker::new_with_map(source, bom);
            return Self {
                map,
                lines: Some(lines),
            };
        }
        Self::span_only(source, bom, locations)
    }

    /// The map alone. Without the `locations` feature a set `locations` cannot reach
    /// here — each language crate's loc-bearing entry point is gated on the same feature
    /// — so the request is asserted rather than silently dropped.
    fn span_only(source: &str, bom: LeadingBom, locations: bool) -> Self {
        debug_assert!(
            cfg!(feature = "locations") || !locations,
            "the loc-bearing wire needs tsv_lang's `locations` feature"
        );
        Self {
            map: ByteToCharMap::new(source, bom),
            #[cfg(feature = "locations")]
            lines: None,
        }
    }

    /// The positions a writer emits from, borrowing both tables.
    #[inline]
    #[must_use]
    pub fn positions(&self) -> WirePositions<'_> {
        WirePositions {
            map: &self.map,
            #[cfg(feature = "locations")]
            lines: self.lines.as_ref(),
        }
    }
}

#[derive(Debug)]
pub struct LocationTracker {
    /// Byte offset of each line's first byte, ascending, `[0]` always present.
    ///
    /// `u32`, not `usize`: a source offset is already `u32`-bounded (`Span`,
    /// and the 4 GB file-size limit `ParseError::FileTooLarge` enforces), so
    /// the wide element bought nothing and cost the search half its cache
    /// residency. The searches hold their needle in `u32` too, so nothing
    /// widens per probe.
    line_starts: Vec<u32>,
    /// 1-entry line-range cache for `get_line_column` and the span resolvers.
    /// Wire-JSON emission is a DFS with high line locality, so successive
    /// offset lookups usually fall in the last-resolved line's `[line_start,
    /// next_line_start)` range and skip the O(log n) binary search on
    /// `line_starts`. Holds `(line_idx, line_start, next_line_start)`; the
    /// initial `(0, 0, 0)` never matches (`offset < 0` is false), so the first
    /// lookup fills it. Interior mutability behind `&self` (the tracker is
    /// threaded by shared reference through the single-threaded convert path).
    line_cache: Cell<(usize, usize, usize)>,
    /// A second line-range entry, read and written only by
    /// `resolve_line_after`: the line the last *searched* trailing endpoint
    /// landed on. Nested nodes end together (`});` closes a call, its
    /// argument and that argument's body on one line), so a span that leaves
    /// its start's line very often ends on the line the previous such span
    /// ended on, and the entry answers it without the forward search.
    ///
    /// Same `(line_idx, line_start, next_line_start)` triple as `line_cache`,
    /// and the same kind of memo — an entry is only ever a row of this
    /// tracker's own immutable `line_starts`, and a read uses it only when
    /// the offset lies inside its range. The initial `(0, usize::MAX, 0)`
    /// never matches.
    end_cache: Cell<(usize, usize, usize)>,
}

impl LocationTracker {
    /// Build a tracker from precomputed line starts, seeding both line-range
    /// caches empty. The single constructor helper every `new*` routes
    /// through so the cache fields stay in one place.
    #[inline]
    fn with_line_starts(line_starts: Vec<u32>) -> Self {
        Self {
            line_starts,
            line_cache: Cell::new((0, 0, 0)),
            end_cache: Cell::new((0, usize::MAX, 0)),
        }
    }

    /// Line starts at LF only, by walking characters — the differential test
    /// oracle for the fused [`new_with_map`](Self::new_with_map).
    #[cfg(test)]
    fn new(source: &str) -> Self {
        let mut line_starts = seeded_line_starts(source.len());
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self::with_line_starts(line_starts)
    }

    /// Line starts per the ECMAScript LineTerminator set (LF, CR, CRLF,
    /// U+2028, U+2029), by walking characters — the differential test oracle
    /// for the fused [`new_ecmascript_with_map`](Self::new_ecmascript_with_map).
    #[cfg(test)]
    fn new_ecmascript(source: &str) -> Self {
        let mut line_starts = seeded_line_starts(source.len());
        let mut chars = source.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            match ch {
                '\n' | '\u{2028}' | '\u{2029}' => line_starts.push((i + ch.len_utf8()) as u32),
                '\r' => {
                    // CRLF counts as a single line terminator
                    if let Some(&(j, '\n')) = chars.peek() {
                        chars.next();
                        line_starts.push((j + 1) as u32);
                    } else {
                        line_starts.push((i + 1) as u32);
                    }
                }
                _ => {}
            }
        }
        Self::with_line_starts(line_starts)
    }

    /// Build the ECMAScript-rule tracker (LF, CR, CRLF, U+2028, U+2029 —
    /// acorn's rule, applied everywhere including inside string literals) and
    /// the byte→UTF-16 map in one source scan: the line table of a TypeScript
    /// document.
    ///
    /// Built separately the pair costs two full `char_indices` passes over the
    /// source; fused it costs one (plus the shared `is_ascii` pre-check, which
    /// selects a byte-level line scan + identity map on the common all-ASCII
    /// path). Byte-identical to the character-walking oracle +
    /// `ByteToCharMap::new(source, bom)`.
    pub fn new_ecmascript_with_map(source: &str, bom: LeadingBom) -> (Self, ByteToCharMap) {
        if source.is_ascii() {
            return (
                Self::with_line_starts(ascii_ecmascript_line_starts(source.as_bytes())),
                ByteToCharMap::identity(),
            );
        }

        let mut lines = LineScan::lines(source.len());
        let map = build_map(source, &mut lines, LineRule::Ecmascript, bom);
        (Self::with_line_starts(lines.starts), map)
    }

    /// Build the LF-only tracker (Svelte's `locate-character` convention — only
    /// `\n` starts a line; CR/U+2028/U+2029 do not) and the byte→UTF-16 map in
    /// one source scan: the line table of a Svelte document — every island in
    /// it included — and of a standalone CSS document. The sibling of
    /// `new_ecmascript_with_map`; byte-identical to the character-walking oracle
    /// + `ByteToCharMap::new(source, bom)`.
    pub fn new_with_map(source: &str, bom: LeadingBom) -> (Self, ByteToCharMap) {
        let mut lines = LineScan::lines(source.len());
        let map = if source.is_ascii() {
            ascii_lf_line_starts_into(source.as_bytes(), 0, &mut lines);
            ByteToCharMap::identity()
        } else {
            build_map(source, &mut lines, LineRule::Lf, bom)
        };
        (Self::with_line_starts(lines.starts), map)
    }

    /// Resolve `offset` to `(line_idx, line_start)`, consulting the 1-entry
    /// line-range cache first and filling it via search on a miss.
    /// Byte-identical to the bare `binary_search` + `saturating_sub` both
    /// callers used before — the cache is a pure memo keyed on the line's
    /// half-open byte range.
    ///
    /// ⚠️ The cache hit path reads **no memory**: `line_cache` is a `Cell` of
    /// three scalars the optimizer keeps in registers, so a hit is two
    /// compares. Any "fast path" that indexes `line_starts` instead is
    /// *slower* than a hit, not faster — measured, see `resolve_line_after`.
    #[inline]
    fn resolve_line(&self, offset: usize) -> (usize, usize) {
        let (line_idx, line_start, next_line_start) = self.line_cache.get();
        if line_start <= offset && offset < next_line_start {
            return (line_idx, line_start);
        }
        // A miss that is still *ahead* of the cached line — the writer walks
        // the AST in pre-order, so this is the common miss — searches forward
        // from the cached index rather than bisecting the whole table.
        let found = if offset >= line_start {
            self.search_line_from(line_idx, offset)
        } else {
            self.search_line_all(offset)
        };
        self.fill_cache(found)
    }

    /// Record `line_idx` in the 1-entry cache and return `(line_idx,
    /// line_start)` — the shared tail of every cache-filling resolution.
    #[inline]
    fn fill_cache(&self, line_idx: usize) -> (usize, usize) {
        let line_start = self.line_starts[line_idx] as usize;
        // Last line has no upper bound; a sentinel keeps it a permanent hit.
        let next_line_start = self
            .line_starts
            .get(line_idx + 1)
            .map_or(usize::MAX, |&s| s as usize);
        self.line_cache.set((line_idx, line_start, next_line_start));
        (line_idx, line_start)
    }

    /// The search needle: `offset` narrowed to the table's element width.
    ///
    /// Total, not a debug-only assumption. Every line start fits `u32` (the
    /// 4 GB file-size limit), so an `offset` past `u32::MAX` is at or after
    /// every line start and saturating to `u32::MAX` selects the same last
    /// line the un-narrowed compare would.
    #[inline]
    fn needle(offset: usize) -> u32 {
        offset.min(u32::MAX as usize) as u32
    }

    /// The unbiased search: the greatest `idx` with `line_starts[idx] <=
    /// offset`, over the whole table. `line_starts[0]` is 0 and offsets are
    /// non-negative, so the answer always exists.
    #[inline]
    fn search_line_all(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&Self::needle(offset)) {
            Ok(idx) => idx, // Exact match - this offset is at the start of a line
            Err(idx) => idx.saturating_sub(1),
        }
    }

    /// The same answer as [`Self::search_line_all`], given the lower bound
    /// `line_starts[from] <= offset`: gallop forward from `from` until the
    /// step overshoots, then bisect the bracketed window.
    ///
    /// The point is locality, not asymptotics. `line_starts` for a real file
    /// is a few KB — L1-resident — so a bisection is not miss-bound but
    /// *latency*-bound: ~9 dependent load-compare-cmov steps that cannot
    /// pipeline. A gallop from a nearby index resolves the ordinary
    /// "next line or two" case in one or two steps.
    fn search_line_from(&self, from: usize, offset: usize) -> usize {
        let starts = &self.line_starts;
        let n = starts.len();
        let needle = Self::needle(offset);
        // Gallop: double the step until it overshoots, keeping `lo` at the
        // last probe known to be `<= offset`. `hi` ends at the first probe
        // known to be `> offset`, or at `n`.
        let mut lo = from;
        let mut step = 1;
        let hi = loop {
            let probe = from + step;
            if probe >= n {
                break n;
            }
            if starts[probe] > needle {
                break probe;
            }
            lo = probe;
            step *= 2;
        };
        lo + starts[lo + 1..hi].partition_point(|&s| s <= needle)
    }

    pub(crate) fn get_line_column(&self, offset: usize) -> (usize, usize) {
        let (line_idx, line_start) = self.resolve_line(offset);
        (line_idx + 1, offset - line_start) // Lines are 1-indexed
    }

    /// The byte offset of the start of the line containing `offset` — the
    /// cache tests' second probe of `resolve_line`.
    #[cfg(test)]
    fn line_start_byte(&self, offset: usize) -> usize {
        self.resolve_line(offset).1
    }

    /// Resolve the trailing endpoint of a span, given that the leading one
    /// resolved to line `from` — so `line_starts[from] <= offset` and the
    /// answer is `from` or later.
    ///
    /// Layered deliberately, and the order is the whole point:
    ///
    /// 1. **the cache**, which the caller's `start` resolution just filled
    ///    with `from`'s range. A same-line span — 88.5% of them on the TS
    ///    corpus — is therefore two *register* compares. Do not "optimize"
    ///    this into a direct `offset < line_starts[from + 1]` test: that
    ///    reads memory where a hit reads none, and measured **+0.78%
    ///    instructions** for exactly that reason.
    /// 2. **the end cache**, for the genuine multi-line span: the line the
    ///    last searched trailing endpoint landed on, which is this one's
    ///    whenever nested nodes close together.
    /// 3. **a forward gallop** from `from`, which then fills the end cache.
    ///
    /// It never writes `line_cache`, which is what keeps that one parked on
    /// the *descending* line: the writer emits a node's `start` (line N), its
    /// `end` (line N+k), then its first child's `start` (back to line N). The
    /// end cache is this function's alone, so filling it disturbs nothing.
    ///
    /// Returns exactly what `resolve_line(offset)` would; the caller owes the
    /// precondition, which `resolve_span` discharges from `end >= start`. An
    /// end-cache hit does not lean on it: the entry's range holds `offset`,
    /// which names the line whatever `from` is.
    #[inline]
    fn resolve_line_after(&self, from: usize, offset: usize) -> (usize, usize) {
        let (line_idx, line_start, next_line_start) = self.line_cache.get();
        if line_start <= offset && offset < next_line_start {
            return (line_idx, line_start);
        }
        let (end_idx, end_start, end_next) = self.end_cache.get();
        if end_start <= offset && offset < end_next {
            debug_assert_eq!(
                end_idx,
                self.search_line_all(offset),
                "an end-cache hit must name the line a search would"
            );
            return (end_idx, end_start);
        }
        let idx = self.search_line_from(from, offset);
        let start = self.line_starts[idx] as usize;
        // Last line has no upper bound, as in `fill_cache`.
        let next = self
            .line_starts
            .get(idx + 1)
            .map_or(usize::MAX, |&s| s as usize);
        self.end_cache.set((idx, start, next));
        (idx, start)
    }

    /// Resolve both endpoints of a span in one call, as
    /// `((start_line_idx, start_line_start), (end_line_idx, end_line_start))`.
    ///
    /// `end >= start` always, so `end`'s line is at or after `start`'s and the
    /// trailing endpoint is searched forward from the leading one's index
    /// rather than bisected from scratch — see [`Self::resolve_line_after`]
    /// for the layering that makes that pay. On the wire-JSON writer's access
    /// pattern this removes **54%** of all bisections (82.0k → 38.0k per pass
    /// over the fuz_app TS corpus), and the ones left are the genuinely
    /// backward lookups.
    ///
    /// Byte-identical to `resolve_line(start)` + `resolve_line(end)`; both caches
    /// are pure memos, so which lines they hold is unobservable.
    #[inline]
    fn resolve_span(&self, start: usize, end: usize) -> ((usize, usize), (usize, usize)) {
        let start_line = self.resolve_line(start);
        let end_line = self.resolve_line_after(start_line.0, end);
        (start_line, end_line)
    }
}

/// ECMAScript-rule line starts for ASCII-only source: no U+2028/U+2029
/// possible, so line terminators are single bytes with CRLF fusing.
fn ascii_ecmascript_line_starts(bytes: &[u8]) -> Vec<u32> {
    let mut lines = LineScan::lines(bytes.len());
    ascii_ecmascript_line_starts_into(bytes, 0, &mut lines);
    lines.starts
}

/// The line-terminator lanes of one word: `(lf, cr)`, the `\n` lanes and the
/// `\r` lanes, each **exact per lane** over any bytes at all.
///
/// Exact is what lets the scans below take every terminator a word holds from
/// the one load — clear the lowest set lane and continue — rather than
/// re-searching from the byte after each one: lines run ~30–40 bytes, so a
/// search per line re-enters its loop, reloads the word it just read, and pays
/// its entry and exit once a line, where a stride over the source pays them
/// once a run. Both needles ride the same loaded word, so the source is read
/// **once**. The LF-only scan shares the hop and drops the `\r` lanes: a word
/// whose only terminator is a lone `\r` costs it one empty stop, and a CRLF
/// pair almost always shares its word with the LF the scan wanted anyway.
///
/// The kernel beneath, [`ascii_zero_lanes`], is exact only over an ASCII word —
/// a non-ASCII lane (post-XOR at or above `0x81`, every high byte but the
/// needle's own `0x8a` / `0x8d`) carries into the lane above it — so a word
/// holding a non-ASCII byte takes [`terminator_lanes_any`] instead. The scans'
/// callers hand them ASCII runs, so that arm is cold, but the exactness is this
/// function's, not its callers': one high-bit test per flagged word (the hop
/// tests only the words its loose class has flagged) buys a scan that is exact
/// over any byte string rather than one whose release build silently draws
/// phantom and missing lines from a slice that is not the run it expected.
///
/// `from_le_bytes` puts byte 0 in the low lane, so lane `k` is byte `k`.
#[inline]
fn terminator_lanes(word: u64) -> (u64, u64) {
    if high_bit_lanes(word) != 0 {
        return terminator_lanes_any(word);
    }
    (
        ascii_zero_lanes(word ^ splat(b'\n')),
        ascii_zero_lanes(word ^ splat(b'\r')),
    )
}

/// [`terminator_lanes`] for a word holding a non-ASCII byte: clear every lane's
/// high bit so the word is ASCII and [`ascii_zero_lanes`] exact again, then drop
/// the lanes that were non-ASCII — a `0x8a` masked to `0x0a` would otherwise
/// read as a `\n`.
#[cold]
#[inline(never)]
fn terminator_lanes_any(word: u64) -> (u64, u64) {
    let high = high_bit_lanes(word);
    let ascii = word ^ high;
    (
        ascii_zero_lanes(ascii ^ splat(b'\n')) & !high,
        ascii_zero_lanes(ascii ^ splat(b'\r')) & !high,
    )
}

/// The lanes of `cr` holding a **lone** CR — one whose next byte is not `\n` —
/// given the same word's exact `lf` lanes and `next`, the byte after the word.
///
/// Lane `k + 1`'s flag shifted down one lane is lane `k`'s "followed by LF", so
/// `cr & !(lf >> 8)` answers every lane but the last from the word alone. Lane
/// 7's next byte lies outside the word — the next word's first, or nothing at
/// the end of the run (a run only ends at a non-ASCII byte or the source's end,
/// and neither is `\n`) — so the shift reads it as "not LF" and `next` corrects
/// it.
#[inline]
fn lone_cr_lanes(lf: u64, cr: u64, next: Option<&u8>) -> u64 {
    let lone = cr & !(lf >> 8);
    if next == Some(&b'\n') {
        lone & !(0x80 << 56)
    } else {
        lone
    }
}

/// The next of `words` at or after index `*k` holding a line terminator, as
/// `(byte offset, lf, cr)` ([`terminator_lanes`]), leaving `*k` one past it; or
/// `None` once the words run out.
///
/// The hop between terminators — four or five words a line — asks the LOOSE
/// class first: a byte below `0x0e` ([`ascii_has_byte_below`], one subtract and
/// one test a word) holds both terminators and, of what real source carries,
/// only the tab besides — which sits beside a line break in the indentation that
/// follows it, so it rarely costs a word of its own. Only a word the loose test
/// flags pays for the two exact masks.
///
/// The loose test can never skip a terminator word, whatever bytes it holds: off
/// ASCII it errs only toward a spurious hit (a lane at or above `0x8e` flags),
/// never a miss, because the lowest lane below the bound always flags — see
/// [`ascii_has_byte_below`]. A spurious hit costs the exact masks, which find
/// nothing, and the hop moves on.
///
/// Its own loop rather than a branch inside the caller's: spelled as one loop
/// over every word with the push inside it, the push's state stays live beside
/// the word test's constants and LLVM rematerializes the constants once a
/// word; as a loop of its own, the hop holds them in registers.
#[inline]
fn next_terminator_word(words: &[[u8; 8]], k: &mut usize) -> Option<(usize, u64, u64)> {
    let mut i = *k;
    while i < words.len() {
        let word = u64::from_le_bytes(words[i]);
        i += 1;
        if ascii_has_byte_below(word, b'\r' + 1) {
            let (lf, cr) = terminator_lanes(word);
            if lf | cr != 0 {
                *k = i;
                return Some((8 * (i - 1), lf, cr));
            }
        }
    }
    *k = i;
    None
}

/// Push the line start after each flagged lane of `lanes`, in ascending order;
/// `after` is the start after lane 0 — the word's source position plus one.
#[inline]
fn push_lane_starts(starts: &mut Vec<u32>, mut lanes: u64, after: usize) {
    while lanes != 0 {
        starts.push((after + (lanes.trailing_zeros() / 8) as usize) as u32);
        lanes &= lanes - 1;
    }
}

/// Append the LF-only line starts of an ASCII run to `lines`, offset by the
/// run's `base` position in the source.
///
/// The multibyte builder splits the source into ASCII runs at its multibyte
/// characters, so each run's line scan is exactly the all-ASCII one — shared
/// with the fast path rather than re-derived. A run boundary can never split a
/// terminator this scan reads: the LFs it collects are one ASCII byte.
///
/// It shares [`terminator_lanes`] with its ECMAScript sibling and reads only the
/// `\n` lanes; its starts are exact over any bytes, though its callers hand it
/// ASCII runs.
fn ascii_lf_line_starts_into(bytes: &[u8], base: usize, lines: &mut LineScan) {
    let (words, tail) = bytes.as_chunks::<8>();
    let mut k = 0;
    while let Some((at, lf, _)) = next_terminator_word(words, &mut k) {
        push_lane_starts(&mut lines.starts, lf, base + at + 1);
    }
    if tail.is_empty() {
        return;
    }
    if let Some(last) = bytes.last_chunk::<8>() {
        // The tail is the high lanes of the run's LAST word, whose low lanes the
        // loop has already read: take that word and keep only the fresh lanes.
        let (lf, _) = terminator_lanes(u64::from_le_bytes(*last));
        let fresh = !0u64 << (8 * (8 - tail.len()));
        push_lane_starts(&mut lines.starts, lf & fresh, base + bytes.len() - 7);
    } else {
        // A run shorter than one word.
        for (k, &b) in tail.iter().enumerate() {
            if b == b'\n' {
                lines.starts.push((base + k + 1) as u32);
            }
        }
    }
}

/// Append the ECMAScript-rule line starts of an ASCII run to `lines`, offset by
/// the run's `base` position in the source. It takes the same [`LineScan`] as
/// its LF sibling so the builder's two arms read alike.
///
/// A CRLF pair can never straddle a run boundary — a run only ends at a
/// non-ASCII byte, which `\n` is not — so a `\r` at the end of a run is a lone
/// CR, exactly as this scan reads it.
///
/// A line starts after every `\n` and after every lone `\r`; a CR followed by
/// its LF starts nothing of its own, since the LF's start is the pair's. So a
/// word's starts are its LF lanes plus its lone-CR lanes, and the CR arm runs
/// only in a word that holds a `\r` at all.
///
/// Exact over any bytes ([`terminator_lanes`]) for the two terminators it reads,
/// `\n` and `\r`, but it does NOT see `<LS>` / `<PS>` (U+2028 / U+2029): those
/// are three-byte characters, so its callers hand it ASCII runs and the
/// multibyte builder handles the two between them.
fn ascii_ecmascript_line_starts_into(bytes: &[u8], base: usize, lines: &mut LineScan) {
    let (words, tail) = bytes.as_chunks::<8>();
    let mut k = 0;
    while let Some((at, lf, cr)) = next_terminator_word(words, &mut k) {
        let starts = if cr == 0 {
            lf
        } else {
            lf | lone_cr_lanes(lf, cr, bytes.get(at + 8))
        };
        push_lane_starts(&mut lines.starts, starts, base + at + 1);
    }
    if tail.is_empty() {
        return;
    }
    if let Some(last) = bytes.last_chunk::<8>() {
        // The tail is the high lanes of the run's LAST word, whose low lanes the
        // loop has already read: take that word and keep only the fresh lanes.
        let (lf, cr) = terminator_lanes(u64::from_le_bytes(*last));
        let fresh = !0u64 << (8 * (8 - tail.len()));
        let starts = (lf | lone_cr_lanes(lf, cr, None)) & fresh;
        push_lane_starts(&mut lines.starts, starts, base + bytes.len() - 7);
    } else {
        // A run shorter than one word.
        for (k, &b) in tail.iter().enumerate() {
            let starts_line = match b {
                b'\n' => true,
                b'\r' => tail.get(k + 1) != Some(&b'\n'),
                _ => false,
            };
            if starts_line {
                lines.starts.push((base + k + 1) as u32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dense absolute-`u32` table the delta representation replaced, kept
    /// as this module's arithmetic oracle.
    ///
    /// **No corpus can grade this translation.** A byte→UTF-16 error only
    /// changes emitted output at a position the wire actually carries, on a
    /// file that actually has a multibyte character before it — so a wrong
    /// table survives fixture, format-diff, and wire-diff gates. The delta
    /// table is therefore graded against this shape *exhaustively*
    /// (`test_delta_map_matches_dense_reference_exhaustive`), and by nothing
    /// else.
    fn reference_offsets(source: &str) -> Vec<u32> {
        let mut offsets = vec![0u32; source.len() + 1];
        let mut utf16_idx = 0u32;
        for (byte_idx, ch) in source.char_indices() {
            for offset in &mut offsets[byte_idx..byte_idx + ch.len_utf8()] {
                *offset = utf16_idx;
            }
            utf16_idx += ch.len_utf16() as u32;
        }
        offsets[source.len()] = utf16_idx;
        offsets
    }

    /// Every map constructor, at every offset in and past the source.
    fn assert_map_matches_reference(source: &str, label: &str) {
        // One table per source — a per-offset rebuild is quadratic.
        let reference = reference_offsets(source);
        let reference_byte_to_char = |byte_offset: u32| -> u32 {
            if source.is_ascii() {
                return byte_offset;
            }
            reference
                .get(byte_offset as usize)
                .copied()
                .unwrap_or(byte_offset)
        };

        let plain = ByteToCharMap::new(source, LeadingBom::Counted);
        let (ecma_tracker, ecma_map) =
            LocationTracker::new_ecmascript_with_map(source, LeadingBom::Counted);
        let (lf_tracker, lf_map) = LocationTracker::new_with_map(source, LeadingBom::Counted);

        let maps = [
            (&plain, "new"),
            (&ecma_map, "new_ecmascript_with_map"),
            (&lf_map, "new_with_map"),
        ];

        for b in 0..=(source.len() as u32 + 2) {
            let expected = reference_byte_to_char(b);
            for (map, which) in maps {
                assert_eq!(
                    map.byte_to_char(b),
                    expected,
                    "{which} diverges at byte {b} on {label} {source:?}"
                );
            }
        }

        // An ASCII-only source must carry no table at all — the flag every
        // `LocationMapper` column branch reads.
        for (map, which) in maps {
            assert_eq!(
                map.has_multibyte(),
                !source.is_ascii(),
                "{which} has_multibyte wrong on {label} {source:?}"
            );
        }

        // The line halves of the fused constructors, against the char-walking
        // trackers they must stay byte-identical to.
        assert_eq!(
            ecma_tracker.line_starts,
            LocationTracker::new_ecmascript(source).line_starts,
            "ECMAScript line starts diverge on {label} {source:?}"
        );
        assert_eq!(
            lf_tracker.line_starts,
            LocationTracker::new(source).line_starts,
            "LF line starts diverge on {label} {source:?}"
        );
    }

    #[test]
    fn test_delta_map_matches_dense_reference_exhaustive() {
        // An alphabet covering every arm of the builder: plain ASCII, both
        // ASCII line terminators (so CR, LF, and CRLF pairs all occur), a
        // 2-byte char, a 3-byte char, both multibyte line terminators, and an
        // astral char (the 4-byte / surrogate-pair arm). Exhaustive over every
        // string of length 0..=3 — enough to place any two arms adjacent, on
        // either side of a run boundary, and at the start/end of the source.
        const ALPHABET: [char; 8] = ['a', '\n', '\r', 'é', '中', '\u{2028}', '\u{2029}', '😀'];

        let mut source = String::new();
        assert_map_matches_reference(&source, "len 0");
        for &a in &ALPHABET {
            for &b in &ALPHABET {
                for &c in &ALPHABET {
                    for len in 1..=3 {
                        source.clear();
                        source.push(a);
                        if len > 1 {
                            source.push(b);
                        }
                        if len > 2 {
                            source.push(c);
                        }
                        assert_map_matches_reference(&source, "exhaustive");
                    }
                }
            }
        }
    }

    /// An elided leading BOM resolves its three bytes to position 0 and every later byte
    /// one unit below the counted reading — on every constructor, in both element
    /// widths, and with a line-1 column that starts at 0. Counted stays the reference.
    /// No fixture can grade the column arithmetic on its own: the Svelte pin sees it only
    /// through one `name_loc`, so the map states it here.
    #[test]
    fn test_elided_leading_bom_shifts_every_later_position_by_one() {
        let bom = '\u{feff}';
        for tail in ["<div>x</div>", "a\né\n中", &"é".repeat(300)] {
            let source = format!("{bom}{tail}");
            let counted = ByteToCharMap::new(&source, LeadingBom::Counted);
            let (lf_tracker, lf_map) = LocationTracker::new_with_map(&source, LeadingBom::Elided);
            let (_, ecma_map) =
                LocationTracker::new_ecmascript_with_map(&source, LeadingBom::Elided);
            let plain = ByteToCharMap::new(&source, LeadingBom::Elided);
            for (map, which) in [
                (&plain, "new"),
                (&lf_map, "new_with_map"),
                (&ecma_map, "new_ecmascript_with_map"),
            ] {
                for b in 0..3u32 {
                    assert_eq!(map.byte_to_char(b), 0, "{which}: BOM byte {b} on {tail:?}");
                }
                // Up to the end-of-source sentinel; a position past it translates to
                // itself under both readings (a missing entry is a zero delta).
                for b in 3..=(source.len() as u32) {
                    assert_eq!(
                        map.byte_to_char(b) + 1,
                        counted.byte_to_char(b),
                        "{which}: byte {b} on {tail:?}"
                    );
                }
            }
            // The column on line 1 reads through the same map, so it starts at 0 where
            // Counted puts the first character at column 1.
            let mapper = LocationMapper {
                tracker: &lf_tracker,
                map: &lf_map,
            };
            let (pos, position) = mapper.pos_and_position(3);
            assert_eq!((pos, position.line, position.column), (0, 1, 0), "{tail:?}");
        }

        // A U+FEFF anywhere but byte 0 is an ordinary character under both readings,
        // and a BOM-less source builds the same table either way.
        for source in ["a\u{feff}b", "é", "abc"] {
            let counted = ByteToCharMap::new(source, LeadingBom::Counted);
            let elided = ByteToCharMap::new(source, LeadingBom::Elided);
            for b in 0..=(source.len() as u32 + 1) {
                assert_eq!(
                    counted.byte_to_char(b),
                    elided.byte_to_char(b),
                    "{source:?}@{b}"
                );
            }
        }
    }

    #[test]
    fn test_delta_map_widens_at_the_u8_boundary() {
        // Each 'é' grows the delta by exactly 1, so the source length in chars
        // *is* the final delta — the narrow/wide boundary lands exactly at 255.
        for (chars, expect_wide) in [(254, false), (255, false), (256, true), (700, true)] {
            let source = "é".repeat(chars);
            let map = ByteToCharMap::new(&source, LeadingBom::Counted);
            assert_eq!(
                matches!(map.deltas, Deltas::Wide(_)),
                expect_wide,
                "wrong element width at {chars} chars"
            );
            assert_map_matches_reference(&source, "u8 boundary");
        }
    }

    #[test]
    fn test_delta_map_wide_path_covers_every_arm() {
        // The widening rebuild re-runs the whole scan, including the line
        // rules — drive it with a source long enough to overflow `u8` that
        // still contains every arm (a 3-byte char grows the delta by 2, an
        // astral char by 2).
        let source = "中a\r\n😀\u{2028}é\u{2029}x\r".repeat(60);
        let map = ByteToCharMap::new(&source, LeadingBom::Counted);
        assert!(matches!(map.deltas, Deltas::Wide(_)), "expected wide table");
        assert_map_matches_reference(&source, "wide, all arms");
    }

    #[test]
    fn test_delta_map_matches_dense_reference_on_long_mixed_sources() {
        // The exhaustive test's strings are too short to reach the
        // word-at-a-time run scan or a multi-run fill. These are long enough
        // for both, at three multibyte densities: sparse (the real-corpus
        // shape — long ASCII runs between characters, narrow table), mixed,
        // and dense (wide table). Deterministic LCG, so a failure reproduces.
        const ALPHABET: [char; 8] = ['a', ' ', '\n', '\r', 'é', '中', '\u{2028}', '😀'];
        for (multibyte_in, expect_wide) in [(1000u32, false), (64, true), (8, true), (2, true)] {
            let mut state = 0x2545_F491_4F6C_DD1Du64;
            let mut source = String::new();
            while source.len() < 40_000 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let roll = (state >> 33) as u32;
                if roll.is_multiple_of(multibyte_in) {
                    source.push(ALPHABET[4 + (roll % 4) as usize]);
                } else {
                    source.push(ALPHABET[(roll % 4) as usize]);
                }
            }
            assert_eq!(
                matches!(
                    ByteToCharMap::new(&source, LeadingBom::Counted).deltas,
                    Deltas::Wide(_)
                ),
                expect_wide,
                "wrong element width at 1-in-{multibyte_in} density"
            );
            assert_map_matches_reference(&source, "long mixed");
        }
    }

    #[test]
    fn test_next_non_ascii_finds_the_first_high_byte() {
        // Every alignment within and past the word-at-a-time stride.
        for prefix in 0..24usize {
            let mut source = "a".repeat(prefix);
            source.push('é');
            source.push_str("bcd");
            let bytes = source.as_bytes();
            assert_eq!(next_non_ascii(bytes, 0), prefix, "prefix {prefix}");
            // A scan starting past the character finds no further high byte.
            assert_eq!(
                next_non_ascii(bytes, prefix + 2),
                bytes.len(),
                "tail scan, prefix {prefix}"
            );
        }
        assert_eq!(next_non_ascii(b"", 0), 0);
        assert_eq!(next_non_ascii(b"abc", 0), 3);
    }

    /// The byte-at-a-time shapes the SWAR scans replaced, kept as this
    /// module's line-scan oracle.
    ///
    /// **No corpus can grade a scan.** A missed or spurious line start moves a
    /// `loc.line`/`loc.column` that still parses as valid JSON, so a wrong scan
    /// survives the fixture suite, the format diff, and a byte-identity wire
    /// diff over thousands of files unless some file happens to exercise the
    /// exact word alignment that breaks. The SWAR scans are therefore graded
    /// against these shapes *exhaustively over every alignment*
    /// (`test_swar_line_scans_match_scalar_reference_exhaustive`), over every
    /// terminator arrangement within a word
    /// (`test_swar_line_scans_match_scalar_reference_on_dense_words`), and over
    /// bytes that are not ASCII
    /// (`test_swar_line_scans_match_scalar_reference_on_non_ascii_bytes`).
    fn reference_lf_line_starts(bytes: &[u8], base: usize) -> Vec<u32> {
        let mut out = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                out.push((base + i + 1) as u32);
            }
        }
        out
    }

    fn reference_ecmascript_line_starts(bytes: &[u8], base: usize) -> Vec<u32> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => out.push((base + i + 1) as u32),
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    out.push((base + i + 1) as u32);
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    #[test]
    fn test_swar_line_scans_match_scalar_reference_exhaustive() {
        // Exhaustive over every string of length 0..=4 on an alphabet that
        // covers each arm and each adjacency the SWAR kernel can confuse: both
        // terminators, the CRLF pair in both orders, a byte whose lane borrows
        // (`\0`), and an ordinary byte. Four positions over five symbols is
        // every terminator pattern the word kernel can see within a lane group.
        const ALPHABET: [u8; 5] = [b'\n', b'\r', b'\0', b'a', 0x7f];
        for len in 0..=4usize {
            let mut word = vec![0u8; len];
            let total = ALPHABET.len().pow(len as u32);
            for n in 0..total {
                let mut rest = n;
                for slot in word.iter_mut() {
                    *slot = ALPHABET[rest % ALPHABET.len()];
                    rest /= ALPHABET.len();
                }
                // Every alignment across the 8-byte stride, so a pattern is
                // tested inside a word, straddling two, and in the run's last
                // word — read overlapped with the one before and masked to its
                // fresh lanes — or, in a run under eight bytes, the byte loop.
                for prefix in 0..17usize {
                    let mut bytes = vec![b'x'; prefix];
                    bytes.extend_from_slice(&word);
                    bytes.extend_from_slice(b"yy");
                    let label = format!("{bytes:?}");

                    let mut lf = LineScan::bare();
                    ascii_lf_line_starts_into(&bytes, 100, &mut lf);
                    assert_eq!(
                        lf.starts,
                        reference_lf_line_starts(&bytes, 100),
                        "lf {label}"
                    );

                    let mut es = LineScan::bare();
                    ascii_ecmascript_line_starts_into(&bytes, 100, &mut es);
                    assert_eq!(
                        es.starts,
                        reference_ecmascript_line_starts(&bytes, 100),
                        "ecmascript {label}"
                    );
                }
            }
        }
    }

    /// Every whole word over the terminators and their borrow partners, at every
    /// alignment, against both references.
    ///
    /// The scans take EVERY flagged lane of a word, so they rest on the lane
    /// kernel being exact per lane, not merely at its lowest lane — which the
    /// four-byte patterns above cannot fully grade: a word can hold eight
    /// terminators. The alphabet is the two terminators plus `0x0b` and `0x0c` —
    /// each needle with its low bit flipped, exactly the byte a borrowing has-zero
    /// kernel flags spuriously in the lane after a genuine match, so a scan built
    /// on one fails here (and on the non-ASCII test's `0x0b` / `0x0c` pieces), and
    /// both below the hop's loose `0x0e` bound, so they also drive the loose test's
    /// false hits. The suffixes put an LF, an ordinary byte, or the end of the run
    /// behind the word's last lane, which is where a CRLF pair straddles two words
    /// and a CR's LF lies outside the word that holds it; the prefixes place the
    /// word at every offset, splitting it across two words and across the word
    /// loop and the overlapped, masked read of the run's last word.
    #[test]
    fn test_swar_line_scans_match_scalar_reference_on_dense_words() {
        const ALPHABET: [u8; 4] = [b'\n', b'\r', 0x0b, 0x0c];
        let mut word = [0u8; 8];
        for n in 0..ALPHABET.len().pow(8) {
            let mut rest = n;
            for slot in &mut word {
                *slot = ALPHABET[rest % ALPHABET.len()];
                rest /= ALPHABET.len();
            }
            for prefix in 0..8usize {
                for suffix in [&b""[..], b"\n", b"y"] {
                    let mut bytes = vec![b'x'; prefix];
                    bytes.extend_from_slice(&word);
                    bytes.extend_from_slice(suffix);

                    let mut lf = LineScan::bare();
                    ascii_lf_line_starts_into(&bytes, 7, &mut lf);
                    let lf_reference = reference_lf_line_starts(&bytes, 7);
                    assert_eq!(lf.starts, lf_reference, "lf {bytes:?}");

                    let mut es = LineScan::bare();
                    ascii_ecmascript_line_starts_into(&bytes, 7, &mut es);
                    let es_reference = reference_ecmascript_line_starts(&bytes, 7);
                    assert_eq!(es.starts, es_reference, "ecmascript {bytes:?}");
                }
            }
        }
    }

    /// Both scans against the references over bytes that are NOT ASCII: the scans
    /// are exact over any bytes (`terminator_lanes`), not only over the ASCII runs
    /// their callers hand them.
    ///
    /// Two families. Every high byte `0x80..=0xff` at every lane of a 24-byte run
    /// (whole words) and a 21-byte one (ending in the overlapped last word), over
    /// an LF, a CR and an ordinary background, and again with a `\n` or `\r`
    /// directly on either side of it — the placements where the raw ASCII kernel
    /// fails both ways. XORed with the needle, every high byte but one lands at
    /// or above `0x81`, wraps to read as that needle (a phantom line) and carries
    /// into the lane above, clearing a genuine terminator there (a missed line).
    /// The one that does not is `0x8a` for `\n` and `0x8d` for `\r`, which XOR
    /// to exactly `0x80` and, absent a carry in, stay unflagged; they are instead
    /// the lanes `terminator_lanes_any`'s high-bit clear turns into `\n` / `\r`,
    /// which its `& !high` removes. And every ordered pair of real UTF-8
    /// characters and terminator spellings at every offset, each followed by the
    /// end of the run, an LF, an ordinary byte or a multibyte character — a
    /// multibyte sequence against a terminator at every lane, including the
    /// U+2028 / U+2029 the scans do not count and the references do not either.
    /// The pieces also hold `\x0b` and `\x0c`, the bytes that XOR with `\n` and
    /// `\r` to `0x01`: directly above a terminator in a word that also holds a
    /// high byte, they are the lanes a borrowing kernel (`swar::zero_lanes`) in
    /// the non-ASCII arm would flag spuriously.
    ///
    /// Deleting the high-bit gate in `terminator_lanes` fails this test and no
    /// other in the module, as do dropping the `& !high` that takes the
    /// non-ASCII lanes back out of `terminator_lanes_any`'s masks and swapping
    /// the borrowing `zero_lanes` in for `ascii_zero_lanes` there.
    #[test]
    fn test_swar_line_scans_match_scalar_reference_on_non_ascii_bytes() {
        fn assert_scans_match(bytes: &[u8]) {
            let mut lf = LineScan::bare();
            ascii_lf_line_starts_into(bytes, 3, &mut lf);
            let lf_reference = reference_lf_line_starts(bytes, 3);
            assert_eq!(lf.starts, lf_reference, "lf {bytes:x?}");

            let mut es = LineScan::bare();
            ascii_ecmascript_line_starts_into(bytes, 3, &mut es);
            let es_reference = reference_ecmascript_line_starts(bytes, 3);
            assert_eq!(es.starts, es_reference, "ecmascript {bytes:x?}");
        }

        for high in 0x80..=0xffu8 {
            for len in [24usize, 21] {
                for pos in 0..len {
                    for fill in [b'x', b'\n', b'\r'] {
                        let mut bytes = vec![fill; len];
                        bytes[pos] = high;
                        assert_scans_match(&bytes);
                    }
                    for terminator in [b'\n', b'\r'] {
                        for neighbour in [pos.wrapping_sub(1), pos + 1] {
                            if neighbour < len {
                                let mut bytes = vec![b'x'; len];
                                bytes[pos] = high;
                                bytes[neighbour] = terminator;
                                assert_scans_match(&bytes);
                            }
                        }
                    }
                }
            }
        }

        const PIECES: [&str; 13] = [
            "\n", "\r", "\r\n", "\n\r", "\x0b", "\x0c", "\u{85}", "é", "\u{7ff}", "中", "\u{2028}",
            "\u{2029}", "😀",
        ];
        for first in PIECES {
            for second in PIECES {
                for prefix in 0..16usize {
                    for suffix in ["", "\n", "y", "é"] {
                        let mut bytes = vec![b'x'; prefix];
                        bytes.extend_from_slice(first.as_bytes());
                        bytes.extend_from_slice(second.as_bytes());
                        bytes.extend_from_slice(suffix.as_bytes());
                        assert_scans_match(&bytes);
                    }
                }
            }
        }
    }

    #[test]
    fn test_swar_line_scans_match_scalar_reference_on_long_sources() {
        // A long source exercises the word loop for many iterations, and a
        // deterministic LCG mixes terminator densities the structured cases
        // above fix by construction.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for case in 0..64 {
            let mut bytes = Vec::with_capacity(4096);
            for _ in 0..4096 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                // Terminator density sweeps from dense to sparse across cases.
                bytes.push(match (state >> 33) % (case + 2) {
                    0 => b'\n',
                    1 => b'\r',
                    _ => b'a',
                });
            }
            let mut lf = LineScan::bare();
            ascii_lf_line_starts_into(&bytes, 0, &mut lf);
            assert_eq!(
                lf.starts,
                reference_lf_line_starts(&bytes, 0),
                "lf case {case}"
            );

            let mut es = LineScan::bare();
            ascii_ecmascript_line_starts_into(&bytes, 0, &mut es);
            assert_eq!(
                es.starts,
                reference_ecmascript_line_starts(&bytes, 0),
                "ecmascript case {case}"
            );
        }
    }

    #[test]
    fn test_new_counts_lf_only() {
        // Svelte's locate-character convention: CR, U+2028, and U+2029 are not
        // line starts, only LF
        let tracker = LocationTracker::new("a\rb\u{2028}c\nd");
        assert_eq!(tracker.get_line_column(0), (1, 0)); // a
        assert_eq!(tracker.get_line_column(2), (1, 2)); // b
        assert_eq!(tracker.get_line_column(6), (1, 6)); // c (U+2028 is 3 bytes)
        assert_eq!(tracker.get_line_column(8), (2, 0)); // d
    }

    #[test]
    fn test_new_ecmascript_lf() {
        let tracker = LocationTracker::new_ecmascript("a\nb\nc");
        assert_eq!(tracker.get_line_column(0), (1, 0)); // a
        assert_eq!(tracker.get_line_column(2), (2, 0)); // b
        assert_eq!(tracker.get_line_column(4), (3, 0)); // c
    }

    #[test]
    fn test_new_ecmascript_crlf_is_one_terminator() {
        let tracker = LocationTracker::new_ecmascript("a\r\nb\r\nc");
        assert_eq!(tracker.get_line_column(0), (1, 0)); // a
        assert_eq!(tracker.get_line_column(1), (1, 1)); // \r
        assert_eq!(tracker.get_line_column(3), (2, 0)); // b
        assert_eq!(tracker.get_line_column(6), (3, 0)); // c
    }

    #[test]
    fn test_new_ecmascript_lone_cr() {
        let tracker = LocationTracker::new_ecmascript("a\rb\rc");
        assert_eq!(tracker.get_line_column(2), (2, 0)); // b
        assert_eq!(tracker.get_line_column(4), (3, 0)); // c
    }

    #[test]
    fn test_new_ecmascript_cr_at_eof() {
        let tracker = LocationTracker::new_ecmascript("a\r");
        assert_eq!(tracker.get_line_column(2), (2, 0)); // EOF on line 2
    }

    #[test]
    fn test_new_ecmascript_unicode_separators() {
        // U+2028 and U+2029 are 3-byte UTF-8 sequences
        let tracker = LocationTracker::new_ecmascript("a\u{2028}b\u{2029}c");
        assert_eq!(tracker.get_line_column(0), (1, 0)); // a
        assert_eq!(tracker.get_line_column(4), (2, 0)); // b
        assert_eq!(tracker.get_line_column(8), (3, 0)); // c
    }

    #[test]
    fn test_new_ecmascript_cr_then_separator() {
        // \r followed by U+2028 is two terminators (only \r\n fuses)
        let tracker = LocationTracker::new_ecmascript("a\r\u{2028}b");
        assert_eq!(tracker.get_line_column(5), (3, 0)); // b
    }

    #[test]
    fn test_line_range_cache_is_order_independent() {
        // The 1-entry line-range cache must be a pure memo: the same offset
        // resolves identically regardless of prior lookups. A fresh tracker per
        // reference query keeps the cache permanently cold, so it is the pure
        // binary-search oracle. Includes an empty line ("\n\n") and a final
        // no-newline line to stress the boundary (`offset == next_line_start`
        // must miss) and last-line (unbounded) cases.
        let src = "ab\ncde\n\nfghi\nj";
        let n = src.len();
        let warm = LocationTracker::new_ecmascript(src);
        let cold = |off: usize| LocationTracker::new_ecmascript(src).get_line_column(off);
        let cold_lsb = |off: usize| LocationTracker::new_ecmascript(src).line_start_byte(off);

        // Forward, then backward on the SAME warm tracker (the backward sweep is
        // where the cache would go wrong if the range check were unsound), then
        // worst-locality interleaved jumps.
        for off in 0..=n {
            assert_eq!(warm.get_line_column(off), cold(off), "forward @{off}");
        }
        for off in (0..=n).rev() {
            assert_eq!(warm.get_line_column(off), cold(off), "backward @{off}");
        }
        for &off in &[n, 0, 3, 7, 0, n, 6, 8, 2, n, 13] {
            assert_eq!(warm.get_line_column(off), cold(off), "jump @{off}");
            assert_eq!(warm.line_start_byte(off), cold_lsb(off), "jump lsb @{off}");
        }
    }

    #[test]
    fn test_byte_to_char_ascii_identity() {
        let m = ByteToCharMap::new("abc", LeadingBom::Counted);
        assert!(!m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(2), 2);
        // On the ASCII fast path the input is returned unchanged, even past the end.
        assert_eq!(m.byte_to_char(99), 99);
    }

    #[test]
    fn test_byte_to_char_bmp_multibyte() {
        // "é=x": é is 2 UTF-8 bytes but 1 UTF-16 code unit, so '=' is unit 1, 'x' unit 2.
        let m = ByteToCharMap::new("é=x", LeadingBom::Counted);
        assert!(m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(2), 1); // '=' at byte 2
        assert_eq!(m.byte_to_char(3), 2); // 'x' at byte 3
    }

    #[test]
    fn test_byte_to_char_astral_surrogate_pair() {
        // "😀x": the emoji is 4 UTF-8 bytes and 2 UTF-16 code units (surrogate pair),
        // so 'x' at byte 4 is UTF-16 unit 2.
        let m = ByteToCharMap::new("😀x", LeadingBom::Counted);
        assert!(m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(4), 2); // 'x'
        assert_eq!(m.byte_to_char(5), 3); // end-of-string sentinel
    }

    #[test]
    fn test_byte_to_char_adjacent_multibyte() {
        // "日本x": 日 = bytes 0..3 / unit 0, 本 = bytes 3..6 / unit 1, x = byte 6 / unit 2.
        let m = ByteToCharMap::new("日本x", LeadingBom::Counted);
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(3), 1); // second char's start, no ASCII gap
        assert_eq!(m.byte_to_char(4), 1); // interior of 本
        assert_eq!(m.byte_to_char(6), 2); // 'x'
        assert_eq!(m.byte_to_char(7), 3); // end-of-string sentinel
    }

    #[test]
    fn test_byte_to_char_past_end_is_identity() {
        // Offsets past the end translate to themselves, even on a multibyte map.
        let m = ByteToCharMap::new("é", LeadingBom::Counted);
        assert_eq!(m.byte_to_char(2), 1); // end sentinel: 1 UTF-16 unit
        assert_eq!(m.byte_to_char(3), 3); // past the end
        assert_eq!(m.byte_to_char(99), 99);
    }

    #[test]
    fn test_fused_constructors_match_separate_builds() {
        // Both fused constructors must stay byte-identical to the separate
        // `new_ecmascript`/`new` + `ByteToCharMap::new` builds they replaced —
        // which is exactly what `assert_map_matches_reference` asserts, on both
        // halves. Hand-picked mixed sources: CRLF, lone CR, U+2028, multibyte
        // inside and at line boundaries, astral, all-ASCII, and empty.
        for source in [
            "abc",
            "a\r\nb\rc\nd",
            "aé\r\né😀\u{2028}x\ry\n中",
            "\u{2028}\r\n😀",
            "",
        ] {
            assert_map_matches_reference(source, "mixed");
        }
    }

    #[test]
    fn test_location_mapper_identity_is_byte_space() {
        // bytes: a=0, é=1..3, \n=3, b=4, é=5..7, ' '=7, c=8
        let source = "aé\nbé c";
        let tracker = LocationTracker::new_ecmascript(source);
        let m = LocationMapper {
            tracker: &tracker,
            map: &ByteToCharMap::identity(),
        };
        assert_eq!(m.pos_and_position(8).0, 8);
        let (pos, p) = m.pos_and_position(8); // 'c'
        assert_eq!(pos, 8);
        assert_eq!((p.line, p.column), (2, 4)); // byte column
    }

    #[test]
    fn test_location_mapper_fused_char_columns() {
        let source = "aé\nbé c";
        let tracker = LocationTracker::new_ecmascript(source);
        let map = ByteToCharMap::new(source, LeadingBom::Counted);
        let m = LocationMapper {
            tracker: &tracker,
            map: &map,
        };
        assert_eq!(m.pos_and_position(8).0, 6); // 'c' in UTF-16 code units
        let (_, start) = m.pos_and_position(4); // "bé c" minus 'c'
        let (_, end) = m.pos_and_position(8);
        assert_eq!((start.line, start.column), (2, 0));
        assert_eq!((end.line, end.column), (2, 3)); // é is 1 UTF-16 unit
    }

    #[test]
    fn test_byte_to_char_interior_byte_fills_to_char_start() {
        // "a😀b": 'a'=unit 0, emoji=units 1-2 (bytes 1-4), 'b'=unit 3 (byte 5).
        // A byte offset *inside* the emoji fills to that char's UTF-16 start (1),
        // exercising the gap-fill loop's `last > 0` branch.
        let m = ByteToCharMap::new("a😀b", LeadingBom::Counted);
        assert_eq!(m.byte_to_char(0), 0); // 'a'
        assert_eq!(m.byte_to_char(1), 1); // emoji start
        assert_eq!(m.byte_to_char(2), 1); // interior byte → emoji start
        assert_eq!(m.byte_to_char(5), 3); // 'b' (emoji consumed 2 units)
    }

    /// `resolve_line_after` is a second search path to the answer
    /// `resolve_line` already computes, and no corpus can grade a search that
    /// returns a plausible wrong line — the wire stays valid JSON either way.
    /// So grade it exhaustively against the binary search, over every
    /// `(from, offset)` pair its precondition admits, on a line-length profile
    /// covering the empty line, the single-byte line, and a run long enough to
    /// make the gallop take several doublings.
    #[test]
    fn test_resolve_line_after_matches_binary_search() {
        let mut source = String::new();
        for len in [
            0usize, 1, 0, 0, 5, 1, 40, 0, 2, 3, 1, 1, 1, 7, 0, 1, 100, 1, 0, 4,
        ] {
            source.push_str(&"x".repeat(len));
            source.push('\n');
        }
        source.push_str("tail"); // last line, no terminator
        let tracker = LocationTracker::new_ecmascript(&source);
        let lines = tracker.line_starts.len();
        assert!(lines >= 20, "profile must exercise multi-step gallops");

        for offset in 0..=source.len() {
            let expected = tracker.resolve_line(offset);
            // Every `from` the precondition admits: `line_starts[from] <= offset`.
            for from in 0..lines {
                if tracker.line_starts[from] as usize > offset {
                    break;
                }
                assert_eq!(
                    tracker.resolve_line_after(from, offset),
                    expected,
                    "offset {offset} from line index {from}"
                );
            }
        }
    }

    /// The line a table names for `offset`, by a scan that shares nothing with
    /// the tracker's searches or caches: `(line_idx, line_start)`.
    fn reference_line(starts: &[u32], offset: usize) -> (usize, usize) {
        let idx = starts.iter().rposition(|&s| s as usize <= offset).unwrap();
        (idx, starts[idx] as usize)
    }

    /// The end cache is a second memo in front of `resolve_line_after`'s
    /// search, and a memo that answers a plausible wrong line is invisible to
    /// every corpus gate. So one *warm* tracker — both caches carrying
    /// whatever the previous spans left — is graded against a cache-free scan
    /// over span sequences shaped to move the two caches every way they can
    /// move: ascending, descending, repeated, nested spans closing together,
    /// spans ending on the last line and at EOF, and a seeded random walk.
    /// Both line classes, over a source holding every ECMAScript terminator.
    #[test]
    fn test_end_cache_matches_cache_free_scan() {
        fn assert_sequence(
            tracker: &LocationTracker,
            spans: impl IntoIterator<Item = (usize, usize)>,
            label: &str,
        ) {
            for (start, end) in spans {
                let primary_before = {
                    // the primary cache as `start`'s resolution leaves it
                    tracker.resolve_line(start);
                    tracker.line_cache.get()
                };
                assert_eq!(
                    tracker.resolve_span(start, end),
                    (
                        reference_line(&tracker.line_starts, start),
                        reference_line(&tracker.line_starts, end)
                    ),
                    "{label} [{start}, {end}]"
                );
                assert_eq!(
                    tracker.line_cache.get(),
                    primary_before,
                    "{label} [{start}, {end}]: the trailing endpoint moved the primary cache"
                );
            }
        }

        let mut long = String::new();
        for len in [
            0usize, 1, 0, 0, 5, 1, 40, 0, 2, 3, 1, 1, 1, 7, 0, 1, 100, 1, 0, 4,
        ] {
            long.push_str(&"x".repeat(len));
            long.push('\n');
        }
        long.push_str("tail");
        let sources = [
            long.as_str(),
            "a\rbb\r\nc\u{2028}dd\u{2029}\n\r\ne\r",
            "é\u{2028}\u{2028}😀\r\r\nz",
            "one line only",
            "\n\n\n",
            "",
        ];
        for source in sources {
            let n = source.len();
            let all: Vec<(usize, usize)> =
                (0..=n).flat_map(|s| (s..=n).map(move |e| (s, e))).collect();
            for (class, tracker) in [
                ("lf", LocationTracker::new(source)),
                ("ecmascript", LocationTracker::new_ecmascript(source)),
            ] {
                let label = format!("{class} {source:?}");
                // Ascending, then descending, on the same warm tracker.
                assert_sequence(&tracker, all.iter().copied(), &label);
                assert_sequence(&tracker, all.iter().rev().copied(), &label);
                // Each span twice running, then nested spans sharing an end
                // (the closing-together shape), outermost first.
                assert_sequence(&tracker, all.iter().flat_map(|&s| [s, s]), &label);
                for end in (0..=n).rev() {
                    assert_sequence(&tracker, (0..=end).map(|start| (start, end)), &label);
                }
                // Spans to EOF and to the last line's first byte, interleaved
                // with single-point lookups through the public readers.
                let last = *tracker.line_starts.last().unwrap() as usize;
                for start in 0..=n {
                    assert_sequence(&tracker, [(start, n), (start.min(last), last)], &label);
                    let (idx, line_start) = reference_line(&tracker.line_starts, start);
                    assert_eq!(
                        tracker.get_line_column(start),
                        (idx + 1, start - line_start)
                    );
                    assert_eq!(tracker.line_start_byte(n - start), {
                        reference_line(&tracker.line_starts, n - start).1
                    });
                }
                // A seeded random walk: no locality at all.
                let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
                let mut next = |bound: usize| {
                    seed = seed
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    (seed >> 33) as usize % (bound + 1)
                };
                let walk: Vec<(usize, usize)> = (0..4000)
                    .map(|_| {
                        let start = next(n);
                        (start, start + next(n - start))
                    })
                    .collect();
                assert_sequence(&tracker, walk, &label);
            }
        }
    }

    /// `span_positions` must be byte-identical to the two `pos_and_position`
    /// calls it replaces — including the multibyte column path, where the two
    /// endpoints translate through the map independently.
    #[test]
    fn test_span_positions_matches_two_pos_and_position() {
        for source in [
            "a\nbb\n\nccc\ndddd",
            "aé\nbé c\n\n😀x\ny",
            "one line only",
            "\n\n\n",
        ] {
            let tracker = LocationTracker::new_ecmascript(source);
            let map = ByteToCharMap::new(source, LeadingBom::Counted);
            let identity = ByteToCharMap::identity();
            for m in [
                LocationMapper {
                    tracker: &tracker,
                    map: &map,
                },
                LocationMapper {
                    tracker: &tracker,
                    map: &identity,
                },
            ] {
                for start in 0..=source.len() as u32 {
                    for end in start..=source.len() as u32 {
                        // Fresh trackers so neither side inherits the other's
                        // cache state — the memo must not be load-bearing.
                        let fresh = LocationTracker::new_ecmascript(source);
                        let f = LocationMapper {
                            tracker: &fresh,
                            map: m.map,
                        };
                        let expected = (f.pos_and_position(start), f.pos_and_position(end));
                        assert_eq!(
                            m.span_positions(start, end),
                            expected,
                            "{source:?} [{start}, {end}]"
                        );
                    }
                }
            }
        }
    }
}
