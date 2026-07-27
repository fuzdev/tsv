use std::cell::Cell;

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

impl ByteToCharMap {
    /// Build a byte-to-UTF-16-code-unit offset map from source text
    ///
    /// For ASCII-only sources, returns an empty map (fast path).
    pub fn new(source: &str) -> Self {
        if source.is_ascii() {
            return Self::identity();
        }
        let mut no_lines = Vec::new();
        build_map(source, &mut no_lines, LineRule::None)
    }

    /// The identity map: every byte offset translates to itself.
    ///
    /// Passing this to a `LocationMapper` selects byte-space emission — the
    /// mode `tsv_svelte`'s island-skeleton pass requires (a comment-bearing
    /// island's skeleton is emitted in byte space so the comment-attach spans
    /// line up; the final fused emit uses the real map).
    pub const fn identity() -> Self {
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
    pub fn byte_to_char(&self, byte_offset: u32) -> u32 {
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
    pub fn has_multibyte(&self) -> bool {
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
    const HIGH_BITS: u64 = 0x8080_8080_8080_8080;
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let hits = u64::from_le_bytes(*chunk) & HIGH_BITS;
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
/// Returns `Err(Outgrown)` — leaving `deltas` and `line_starts` correct and
/// complete up to that point, for the wide continuation to pick up — if the
/// next character's deltas would not fit `T`.
fn build_deltas<T: DeltaElem>(
    source: &str,
    deltas: &mut Vec<T>,
    line_starts: &mut Vec<usize>,
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
                LineRule::Lf => ascii_lf_line_starts_into(&bytes[i..run_end], i, line_starts),
                LineRule::Ecmascript => {
                    ascii_ecmascript_line_starts_into(&bytes[i..run_end], i, line_starts);
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
                    line_starts.push(i + 3);
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
/// multibyte characters are sparse. `line_starts` must already hold its leading
/// `0` unless `line_rule` is `None`.
fn build_map(source: &str, line_starts: &mut Vec<usize>, line_rule: LineRule) -> ByteToCharMap {
    let mut narrow: Vec<u8> = Vec::with_capacity(source.len() + 1);
    let Err(outgrown) = build_deltas::<u8>(source, &mut narrow, line_starts, 0, 0, line_rule)
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
        line_starts,
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
/// - with a real map (`ByteToCharMap::new(source)`), `pos` and
///   `pos_and_position` emit final UTF-16 code-unit offsets and char-based
///   columns directly — no post-conversion translation walk;
/// - with `ByteToCharMap::identity()`, both are exact byte-space passthrough —
///   the mode `tsv_svelte`'s island-skeleton pass requires (comment-attach
///   spans line up in byte space).
///
/// The fused column math is the delta-0 case of `translate_column`'s
/// delta-preserving rule: `char_col = map(offset) − map(line_start)`. It is
/// byte-identical to running the byte-space conversion plus the translation
/// walk because every conversion site derives `loc` from the same span it
/// writes into `start`/`end`.
#[derive(Clone, Copy, Debug)]
pub struct LocationMapper<'a> {
    pub tracker: &'a LocationTracker,
    pub map: &'a ByteToCharMap,
}

impl<'a> LocationMapper<'a> {
    /// A byte-space passthrough mapper over `tracker` (identity map).
    pub fn identity(tracker: &'a LocationTracker) -> Self {
        static IDENTITY: ByteToCharMap = ByteToCharMap::identity();
        Self {
            tracker,
            map: &IDENTITY,
        }
    }

    /// Translate an emitted byte offset (UTF-16 code units with a real map,
    /// identity in byte-space mode).
    #[inline]
    pub fn pos(&self, byte_offset: u32) -> u32 {
        self.map.byte_to_char(byte_offset)
    }

    /// The emitted offset (`pos`) plus its `Position`, in one translation —
    /// the per-endpoint form direct wire emitters use (calling `pos` and
    /// deriving the `Position` separately would translate `byte_offset`
    /// through the map twice on the multibyte path).
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
}

#[derive(Debug)]
pub struct LocationTracker {
    line_starts: Vec<usize>,
    /// 1-entry line-range cache for `get_line_column` / `line_start_byte`.
    /// Wire-JSON emission is a DFS with high line locality, so successive
    /// offset lookups usually fall in the last-resolved line's `[line_start,
    /// next_line_start)` range and skip the O(log n) binary search on
    /// `line_starts`. Holds `(line_idx, line_start, next_line_start)`; the
    /// initial `(0, 0, 0)` never matches (`offset < 0` is false), so the first
    /// lookup fills it. Interior mutability behind `&self` (the tracker is
    /// threaded by shared reference through the single-threaded convert path).
    line_cache: Cell<(usize, usize, usize)>,
}

impl LocationTracker {
    /// Build a tracker from precomputed line starts, seeding an empty
    /// line-range cache. The single constructor helper every `new*` routes
    /// through so the cache field stays in one place.
    #[inline]
    fn with_line_starts(line_starts: Vec<usize>) -> Self {
        Self {
            line_starts,
            line_cache: Cell::new((0, 0, 0)),
        }
    }

    /// Line starts at LF only — Svelte's `locate-character` convention, used
    /// for Svelte template and CSS locations.
    ///
    /// Production callers use the fused `new_with_map`; this survives as its
    /// differential test oracle (the "byte-identical to `new` +
    /// `ByteToCharMap::new`" contract).
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        Self::with_line_starts(line_starts)
    }

    /// Line starts per the ECMAScript LineTerminator set (LF, CR, CRLF,
    /// U+2028, U+2029) — acorn's rule, applied everywhere including inside
    /// string literals. Used for standalone TypeScript locations.
    ///
    /// Production callers use the fused `new_ecmascript_with_map`; this
    /// survives as its differential test oracle.
    pub fn new_ecmascript(source: &str) -> Self {
        if source.is_ascii() {
            return Self::with_line_starts(ascii_ecmascript_line_starts(source.as_bytes()));
        }
        let mut line_starts = vec![0];
        let mut chars = source.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            match ch {
                '\n' | '\u{2028}' | '\u{2029}' => line_starts.push(i + ch.len_utf8()),
                '\r' => {
                    // CRLF counts as a single line terminator
                    if let Some(&(j, '\n')) = chars.peek() {
                        chars.next();
                        line_starts.push(j + 1);
                    } else {
                        line_starts.push(i + 1);
                    }
                }
                _ => {}
            }
        }
        Self::with_line_starts(line_starts)
    }

    /// Build the ECMAScript-rule tracker and the byte→UTF-16 map in one
    /// source scan.
    ///
    /// The pair `convert_ast_json_string` needs per call — built separately
    /// they cost two full `char_indices` passes over the source; fused they
    /// cost one (plus the shared `is_ascii` pre-check, which selects a
    /// byte-level line scan + identity map on the common all-ASCII path).
    /// Byte-identical to `new_ecmascript(source)` + `ByteToCharMap::new(source)`.
    pub fn new_ecmascript_with_map(source: &str) -> (Self, ByteToCharMap) {
        if source.is_ascii() {
            return (
                Self::with_line_starts(ascii_ecmascript_line_starts(source.as_bytes())),
                ByteToCharMap::identity(),
            );
        }

        let mut line_starts = vec![0];
        let map = build_map(source, &mut line_starts, LineRule::Ecmascript);
        (Self::with_line_starts(line_starts), map)
    }

    /// Build the LF-only tracker (Svelte's `locate-character` convention — only
    /// `\n` starts a line; CR/U+2028/U+2029 do not) and the byte→UTF-16 map in
    /// one source scan. The Svelte sibling of `new_ecmascript_with_map`, for the
    /// wire-JSON writer's fused char-space emission over the Svelte spine.
    /// Byte-identical to `new(source)` + `ByteToCharMap::new(source)`.
    pub fn new_with_map(source: &str) -> (Self, ByteToCharMap) {
        if source.is_ascii() {
            return (
                Self::with_line_starts(ascii_lf_line_starts(source.as_bytes())),
                ByteToCharMap::identity(),
            );
        }

        let mut line_starts = vec![0];
        let map = build_map(source, &mut line_starts, LineRule::Lf);
        (Self::with_line_starts(line_starts), map)
    }

    /// A line-data-free tracker: only the byte→char `map` half of a
    /// `LocationMapper` is populated (`ByteToCharMap`), the tracker carries no
    /// `line_starts` scan. For the `no-locations` wire path: every line/column
    /// field is gated off, so the writer's line-table readers
    /// (`pos_and_position()` / `get_line_column()`, which back only `loc` /
    /// `name_loc` / column output) are all skipped behind the same `emit_loc`
    /// flag — leaving `LocationMapper::pos()` (byte→UTF-16 offset) as the sole
    /// live consumer. So the O(n) line scan the fused `new_ecmascript_with_map` /
    /// `new_with_map` do is pure dead work here. The stub `line_starts` (`[0]`)
    /// keeps `get_line_column` non-panicking if ever reached; the `map` is
    /// byte-identical to the fused constructors' map — line rules only affect
    /// `line_starts`, which this skips — so `start`/`end` offsets are unchanged.
    pub fn new_map_only(source: &str) -> (Self, ByteToCharMap) {
        (Self::with_line_starts(vec![0]), ByteToCharMap::new(source))
    }

    /// Resolve `offset` to `(line_idx, line_start)`, consulting the 1-entry
    /// line-range cache first and filling it via binary search on a miss.
    /// Byte-identical to the bare `binary_search` + `saturating_sub` both
    /// callers used before — the cache is a pure memo keyed on the line's
    /// half-open byte range.
    #[inline]
    fn resolve_line(&self, offset: usize) -> (usize, usize) {
        let (line_idx, line_start, next_line_start) = self.line_cache.get();
        if line_start <= offset && offset < next_line_start {
            return (line_idx, line_start);
        }
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx, // Exact match - this offset is at the start of a line
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        // Last line has no upper bound; a sentinel keeps it a permanent hit.
        let next_line_start = self
            .line_starts
            .get(line_idx + 1)
            .copied()
            .unwrap_or(usize::MAX);
        self.line_cache.set((line_idx, line_start, next_line_start));
        (line_idx, line_start)
    }

    pub fn get_line_column(&self, offset: usize) -> (usize, usize) {
        let (line_idx, line_start) = self.resolve_line(offset);
        (line_idx + 1, offset - line_start) // Lines are 1-indexed
    }

    /// Get the byte offset of the start of the line containing the given byte offset
    ///
    /// Used to compute character-based columns: `char_column = byte_to_char(offset) - byte_to_char(line_start)`.
    pub fn line_start_byte(&self, offset: usize) -> usize {
        self.resolve_line(offset).1
    }
}

/// LF-only line starts for ASCII-only source (Svelte's `locate-character`
/// convention: only `\n` starts a line — no CR/CRLF fusing).
fn ascii_lf_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut line_starts = vec![0];
    ascii_lf_line_starts_into(bytes, 0, &mut line_starts);
    line_starts
}

/// ECMAScript-rule line starts for ASCII-only source: no U+2028/U+2029
/// possible, so line terminators are single bytes with CRLF fusing.
fn ascii_ecmascript_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut line_starts = vec![0];
    ascii_ecmascript_line_starts_into(bytes, 0, &mut line_starts);
    line_starts
}

/// Broadcast `b` to every lane of a `u64` word — the SWAR needle. Written as a
/// multiply rather than a byte array so it carries no endianness.
#[inline]
const fn splat(b: u8) -> u64 {
    b as u64 * 0x0101_0101_0101_0101
}

/// Lane mask of the zero bytes in `v`: the high bit of lane `k` is set if
/// `v`'s byte `k` is zero. The classic `has_zero` kernel.
///
/// ⚠️ **Only the LOWEST set lane is guaranteed genuine**, and every caller here
/// relies on exactly that. A zero byte borrows into the next lane, which can
/// flag lane `k+1` spuriously — but a spurious flag at `k+1` requires lane `k`
/// to have been zero, and a zero lane always flags itself, so the lowest flagged
/// lane is always a real match. Read this mask with `trailing_zeros`, never with
/// a popcount or a highest-bit scan.
#[inline]
const fn zero_lanes(v: u64) -> u64 {
    const LOW_BITS: u64 = 0x0101_0101_0101_0101;
    const HIGH_BITS: u64 = 0x8080_8080_8080_8080;
    v.wrapping_sub(LOW_BITS) & !v & HIGH_BITS
}

/// Index of the first `\n` at or after `from`, or `bytes.len()`.
///
/// The line-scan sibling of [`next_non_ascii`], and the same word-at-a-time
/// shape for the same reason: line terminators are sparse (~1 per 30–40 source
/// bytes), so a per-byte compare spends nearly all of its work confirming
/// misses. `from_le_bytes` puts byte 0 in the low lane, so the lowest set bit is
/// the earliest match.
#[inline]
fn next_lf(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let hits = zero_lanes(u64::from_le_bytes(*chunk) ^ splat(b'\n'));
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

/// Index of the first `\n` **or** `\r` at or after `from`, or `bytes.len()`.
///
/// Both needles are tested against the same loaded word, so the source is read
/// **once** — two independent single-needle passes would double the memory
/// traffic to save one `xor`/`sub`/`andn` triple per word, the wrong trade on a
/// multi-megabyte source.
#[inline]
fn next_ecmascript_terminator(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        // OR-ing two masks preserves the lowest-lane guarantee: a spurious lane
        // in either mask is preceded by a genuine one in that same mask.
        let hits = zero_lanes(w ^ splat(b'\n')) | zero_lanes(w ^ splat(b'\r'));
        if hits != 0 {
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
        i += 1;
    }
    i
}

/// Append the LF-only line starts of an ASCII run to `line_starts`, offset by
/// the run's `base` position in the source.
///
/// The multibyte builder splits the source into ASCII runs at its multibyte
/// characters, so each run's line scan is exactly the all-ASCII one — shared
/// with the fast path rather than re-derived. A run boundary can never split a
/// line terminator: every terminator this rule recognizes is one ASCII byte.
fn ascii_lf_line_starts_into(bytes: &[u8], base: usize, line_starts: &mut Vec<usize>) {
    let mut i = next_lf(bytes, 0);
    while i < bytes.len() {
        line_starts.push(base + i + 1);
        i = next_lf(bytes, i + 1);
    }
}

/// Append the ECMAScript-rule line starts of an ASCII run to `line_starts`,
/// offset by the run's `base` position in the source.
///
/// A CRLF pair can never straddle a run boundary — a run only ends at a
/// non-ASCII byte, which `\n` is not — so a `\r` at the end of a run is a lone
/// CR, exactly as this scan reads it.
///
/// The rule collapses to one shape once the scan lands *on* a terminator: `\n`
/// and a lone `\r` both start the next line at `i + 1`, and CRLF is the same
/// after stepping `i` onto its `\n`. So the only per-byte work left is the CRLF
/// test, run once per line instead of once per byte.
fn ascii_ecmascript_line_starts_into(bytes: &[u8], base: usize, line_starts: &mut Vec<usize>) {
    let mut i = next_ecmascript_terminator(bytes, 0);
    while i < bytes.len() {
        // CRLF counts as a single line terminator.
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
        }
        line_starts.push(base + i + 1);
        i = next_ecmascript_terminator(bytes, i + 1);
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

        let plain = ByteToCharMap::new(source);
        let (ecma_tracker, ecma_map) = LocationTracker::new_ecmascript_with_map(source);
        let (lf_tracker, lf_map) = LocationTracker::new_with_map(source);
        let (_, map_only) = LocationTracker::new_map_only(source);

        let maps = [
            (&plain, "new"),
            (&ecma_map, "new_ecmascript_with_map"),
            (&lf_map, "new_with_map"),
            (&map_only, "new_map_only"),
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

    #[test]
    fn test_delta_map_widens_at_the_u8_boundary() {
        // Each 'é' grows the delta by exactly 1, so the source length in chars
        // *is* the final delta — the narrow/wide boundary lands exactly at 255.
        for (chars, expect_wide) in [(254, false), (255, false), (256, true), (700, true)] {
            let source = "é".repeat(chars);
            let map = ByteToCharMap::new(&source);
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
        let map = ByteToCharMap::new(&source);
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
                matches!(ByteToCharMap::new(&source).deltas, Deltas::Wide(_)),
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
    /// (`test_swar_line_scans_match_scalar_reference_exhaustive`), and by
    /// nothing else.
    fn reference_lf_line_starts(bytes: &[u8], base: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                out.push(base + i + 1);
            }
        }
        out
    }

    fn reference_ecmascript_line_starts(bytes: &[u8], base: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => out.push(base + i + 1),
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    out.push(base + i + 1);
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
                // tested inside a word, straddling two, and in the scalar tail.
                for prefix in 0..17usize {
                    let mut bytes = vec![b'x'; prefix];
                    bytes.extend_from_slice(&word);
                    bytes.extend_from_slice(b"yy");
                    let label = format!("{bytes:?}");

                    let mut lf = Vec::new();
                    ascii_lf_line_starts_into(&bytes, 100, &mut lf);
                    assert_eq!(lf, reference_lf_line_starts(&bytes, 100), "lf {label}");

                    let mut es = Vec::new();
                    ascii_ecmascript_line_starts_into(&bytes, 100, &mut es);
                    assert_eq!(
                        es,
                        reference_ecmascript_line_starts(&bytes, 100),
                        "ecmascript {label}"
                    );
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
            let mut lf = Vec::new();
            ascii_lf_line_starts_into(&bytes, 0, &mut lf);
            assert_eq!(lf, reference_lf_line_starts(&bytes, 0), "lf case {case}");

            let mut es = Vec::new();
            ascii_ecmascript_line_starts_into(&bytes, 0, &mut es);
            assert_eq!(
                es,
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
        let m = ByteToCharMap::new("abc");
        assert!(!m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(2), 2);
        // On the ASCII fast path the input is returned unchanged, even past the end.
        assert_eq!(m.byte_to_char(99), 99);
    }

    #[test]
    fn test_byte_to_char_bmp_multibyte() {
        // "é=x": é is 2 UTF-8 bytes but 1 UTF-16 code unit, so '=' is unit 1, 'x' unit 2.
        let m = ByteToCharMap::new("é=x");
        assert!(m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(2), 1); // '=' at byte 2
        assert_eq!(m.byte_to_char(3), 2); // 'x' at byte 3
    }

    #[test]
    fn test_byte_to_char_astral_surrogate_pair() {
        // "😀x": the emoji is 4 UTF-8 bytes and 2 UTF-16 code units (surrogate pair),
        // so 'x' at byte 4 is UTF-16 unit 2.
        let m = ByteToCharMap::new("😀x");
        assert!(m.has_multibyte());
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(4), 2); // 'x'
        assert_eq!(m.byte_to_char(5), 3); // end-of-string sentinel
    }

    #[test]
    fn test_byte_to_char_adjacent_multibyte() {
        // "日本x": 日 = bytes 0..3 / unit 0, 本 = bytes 3..6 / unit 1, x = byte 6 / unit 2.
        let m = ByteToCharMap::new("日本x");
        assert_eq!(m.byte_to_char(0), 0);
        assert_eq!(m.byte_to_char(3), 1); // second char's start, no ASCII gap
        assert_eq!(m.byte_to_char(4), 1); // interior of 本
        assert_eq!(m.byte_to_char(6), 2); // 'x'
        assert_eq!(m.byte_to_char(7), 3); // end-of-string sentinel
    }

    #[test]
    fn test_byte_to_char_past_end_is_identity() {
        // Offsets past the end translate to themselves, even on a multibyte map.
        let m = ByteToCharMap::new("é");
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
        let m = LocationMapper::identity(&tracker);
        assert_eq!(m.pos(8), 8);
        let (pos, p) = m.pos_and_position(8); // 'c'
        assert_eq!(pos, 8);
        assert_eq!((p.line, p.column), (2, 4)); // byte column
    }

    #[test]
    fn test_location_mapper_fused_char_columns() {
        let source = "aé\nbé c";
        let tracker = LocationTracker::new_ecmascript(source);
        let map = ByteToCharMap::new(source);
        let m = LocationMapper {
            tracker: &tracker,
            map: &map,
        };
        assert_eq!(m.pos(8), 6); // 'c' in UTF-16 code units
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
        let m = ByteToCharMap::new("a😀b");
        assert_eq!(m.byte_to_char(0), 0); // 'a'
        assert_eq!(m.byte_to_char(1), 1); // emoji start
        assert_eq!(m.byte_to_char(2), 1); // interior byte → emoji start
        assert_eq!(m.byte_to_char(5), 3); // 'b' (emoji consumed 2 units)
    }
}
