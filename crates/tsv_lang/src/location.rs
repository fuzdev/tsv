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
/// For sources with multibyte UTF-8 characters, the map stores the UTF-16 code unit
/// offset for each byte position. (A sparse per-multibyte-char representation
/// with binary-search lookup was measured at +3% instructions on a
/// multibyte-dense corpus — the O(1) dense lookup wins; don't re-derive.)
///
/// Characters in the BMP (U+0000-U+FFFF) count as 1 UTF-16 code unit.
/// Characters outside the BMP (U+10000+, e.g., most emoji) count as 2 (surrogate pair).
///
/// Only valid for byte positions at character boundaries (i.e., positions returned
/// by the parser, which always point to the start of a character); a byte
/// position inside a multibyte character resolves to that character's start.
#[derive(Debug)]
pub struct ByteToCharMap {
    /// For each byte position, the corresponding UTF-16 code unit offset.
    /// Empty for ASCII-only sources (byte == char offset).
    offsets: Vec<u32>,
    has_multibyte: bool,
}

impl ByteToCharMap {
    /// Build a byte-to-UTF-16-code-unit offset map from source text
    ///
    /// For ASCII-only sources, returns an empty map (fast path).
    pub fn new(source: &str) -> Self {
        if source.is_ascii() {
            return Self::identity();
        }

        let mut offsets = vec![0u32; source.len() + 1];
        let mut utf16_idx = 0u32;
        for (byte_idx, ch) in source.char_indices() {
            // Every byte of the character gets the character's UTF-16 offset,
            // so a byte position inside a multibyte char resolves to that
            // char's start.
            for offset in &mut offsets[byte_idx..byte_idx + ch.len_utf8()] {
                *offset = utf16_idx;
            }
            // Characters outside BMP need 2 UTF-16 code units (surrogate pair)
            utf16_idx += ch.len_utf16() as u32;
        }
        offsets[source.len()] = utf16_idx;

        Self {
            offsets,
            has_multibyte: true,
        }
    }

    /// The identity map: every byte offset translates to itself.
    ///
    /// Passing this to a `LocationMapper` selects byte-space emission — the
    /// mode `tsv_svelte`'s island-skeleton pass requires (a comment-bearing
    /// island's skeleton is emitted in byte space so the comment-attach spans
    /// line up; the final fused emit uses the real map).
    pub const fn identity() -> Self {
        Self {
            offsets: Vec::new(),
            has_multibyte: false,
        }
    }

    /// Convert a byte offset to a UTF-16 code unit offset
    ///
    /// For ASCII-only sources, returns the byte offset unchanged. Offsets
    /// past the end of the source also translate to themselves.
    #[inline]
    pub fn byte_to_char(&self, byte_offset: u32) -> u32 {
        if !self.has_multibyte {
            return byte_offset;
        }
        self.offsets
            .get(byte_offset as usize)
            .copied()
            .unwrap_or(byte_offset)
    }

    /// Whether the source contains multibyte UTF-8 characters
    #[inline]
    pub fn has_multibyte(&self) -> bool {
        self.has_multibyte
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
        let mut offsets = vec![0u32; source.len() + 1];
        let mut utf16_idx = 0u32;
        let mut chars = source.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            let len_utf8 = ch.len_utf8();
            // Every byte of the character gets the character's UTF-16 offset,
            // so a byte position inside a multibyte char resolves to that
            // char's start.
            for offset in &mut offsets[i..i + len_utf8] {
                *offset = utf16_idx;
            }
            utf16_idx += ch.len_utf16() as u32;
            match ch {
                '\n' | '\u{2028}' | '\u{2029}' => line_starts.push(i + len_utf8),
                '\r' => {
                    // CRLF counts as a single line terminator; the consumed
                    // '\n' still fills its map slot and counts one UTF-16 unit.
                    if let Some(&(j, '\n')) = chars.peek() {
                        chars.next();
                        offsets[j] = utf16_idx;
                        utf16_idx += 1;
                        line_starts.push(j + 1);
                    } else {
                        line_starts.push(i + 1);
                    }
                }
                _ => {}
            }
        }
        offsets[source.len()] = utf16_idx;

        (
            Self::with_line_starts(line_starts),
            ByteToCharMap {
                offsets,
                has_multibyte: true,
            },
        )
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
        let mut offsets = vec![0u32; source.len() + 1];
        let mut utf16_idx = 0u32;
        for (i, ch) in source.char_indices() {
            // Every byte of the character gets the character's UTF-16 offset,
            // so a byte position inside a multibyte char resolves to that
            // char's start.
            for offset in &mut offsets[i..i + ch.len_utf8()] {
                *offset = utf16_idx;
            }
            utf16_idx += ch.len_utf16() as u32;
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        offsets[source.len()] = utf16_idx;

        (
            Self::with_line_starts(line_starts),
            ByteToCharMap {
                offsets,
                has_multibyte: true,
            },
        )
    }

    /// A line-data-free tracker: only the byte→char `map` half of a
    /// `LocationMapper` is populated (`ByteToCharMap`), the tracker carries no
    /// `line_starts` scan. For the `no-locations` wire path, where the writer
    /// calls only `LocationMapper::pos()` (byte→UTF-16 offset) and never
    /// `pos_and_position()`/`get_line_column` — so the O(n) line scan the fused
    /// `new_ecmascript_with_map` / `new_with_map` do is pure dead work there.
    /// The stub `line_starts` (`[0]`) keeps `get_line_column` non-panicking if
    /// ever called (it never is on this path). The `map` is byte-identical to
    /// the fused constructors' map — line rules only affect `line_starts`, which
    /// this skips — so `start`/`end` offsets are unchanged.
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
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    line_starts
}

/// ECMAScript-rule line starts for ASCII-only source: no U+2028/U+2029
/// possible, so line terminators are single bytes with CRLF fusing.
fn ascii_ecmascript_line_starts(bytes: &[u8]) -> Vec<usize> {
    let mut line_starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => line_starts.push(i + 1),
            b'\r' => {
                // CRLF counts as a single line terminator
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                line_starts.push(i + 1);
            }
            _ => {}
        }
        i += 1;
    }
    line_starts
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_new_ecmascript_with_map_matches_separate_builds() {
        // Mixed content: CRLF, lone CR, U+2028, multibyte inside and at line
        // boundaries, astral char — every branch of the fused scan.
        for source in [
            "abc",
            "a\r\nb\rc\nd",
            "aé\r\né😀\u{2028}x\ry\n中",
            "\u{2028}\r\n😀",
            "",
        ] {
            let (tracker, map) = LocationTracker::new_ecmascript_with_map(source);
            let expected_tracker = LocationTracker::new_ecmascript(source);
            let expected_map = ByteToCharMap::new(source);
            assert_eq!(
                tracker.line_starts, expected_tracker.line_starts,
                "line starts diverge on {source:?}"
            );
            assert_eq!(map.has_multibyte(), expected_map.has_multibyte());
            for b in 0..=(source.len() as u32 + 2) {
                assert_eq!(
                    map.byte_to_char(b),
                    expected_map.byte_to_char(b),
                    "map diverges at byte {b} on {source:?}"
                );
            }
        }
    }

    #[test]
    fn test_new_with_map_matches_separate_builds() {
        // LF-only: CR / U+2028 / U+2029 are NOT line starts; multibyte inside and
        // at line boundaries; astral char — the map half must match `new` + `new`.
        for source in [
            "abc",
            "a\r\nb\rc\nd",
            "aé\r\né😀\u{2028}x\ry\n中",
            "\u{2028}\r\n😀",
            "",
        ] {
            let (tracker, map) = LocationTracker::new_with_map(source);
            let expected_tracker = LocationTracker::new(source);
            let expected_map = ByteToCharMap::new(source);
            assert_eq!(
                tracker.line_starts, expected_tracker.line_starts,
                "LF-only line starts diverge on {source:?}"
            );
            assert_eq!(map.has_multibyte(), expected_map.has_multibyte());
            for b in 0..=(source.len() as u32 + 2) {
                assert_eq!(
                    map.byte_to_char(b),
                    expected_map.byte_to_char(b),
                    "map diverges at byte {b} on {source:?}"
                );
            }
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
