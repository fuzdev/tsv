use crate::span::Span;

/// A position in source code (line and column)
///
/// Generic type without serialization - languages can wrap this in their own types
/// that include serde derives if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// A source location spanning from start to end position
///
/// Generic type without serialization - languages can wrap this in their own types
/// that include serde derives if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

/// Maps byte offsets to JS-compatible character offsets (UTF-16 code units)
///
/// Rust strings are byte-indexed, but JS (and Svelte/acorn) uses UTF-16
/// code unit indices. For ASCII-only sources, byte == char offset, so the map is empty.
/// For sources with multibyte UTF-8 characters, the map stores the UTF-16 code unit
/// offset for each byte position.
///
/// Characters in the BMP (U+0000-U+FFFF) count as 1 UTF-16 code unit.
/// Characters outside the BMP (U+10000+, e.g., most emoji) count as 2 (surrogate pair).
///
/// Only valid for byte positions at character boundaries (i.e., positions returned
/// by the parser, which always point to the start of a character).
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
            return Self {
                offsets: Vec::new(),
                has_multibyte: false,
            };
        }

        let mut offsets = vec![0u32; source.len() + 1];
        let mut utf16_idx = 0u32;
        for (byte_idx, ch) in source.char_indices() {
            offsets[byte_idx] = utf16_idx;
            // Characters outside BMP need 2 UTF-16 code units (surrogate pair)
            utf16_idx += ch.len_utf16() as u32;
        }
        offsets[source.len()] = utf16_idx;

        // Fill intermediate bytes (inside multibyte characters) with the
        // UTF-16 offset of the character they belong to. This handles
        // cases where a byte position falls in the middle of a multibyte char.
        let mut last = 0u32;
        for offset in &mut offsets {
            if *offset == 0 && last > 0 {
                *offset = last;
            } else {
                last = *offset;
            }
        }

        Self {
            offsets,
            has_multibyte: true,
        }
    }

    /// Convert a byte offset to a UTF-16 code unit offset
    ///
    /// For ASCII-only sources, returns the byte offset unchanged.
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

#[derive(Debug)]
pub struct LocationTracker {
    line_starts: Vec<usize>,
}

impl LocationTracker {
    /// Line starts at LF only — Svelte's `locate-character` convention, used
    /// for Svelte template and CSS locations.
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    /// Line starts per the ECMAScript LineTerminator set (LF, CR, CRLF,
    /// U+2028, U+2029) — acorn's rule, applied everywhere including inside
    /// string literals. Used for standalone TypeScript locations.
    pub fn new_ecmascript(source: &str) -> Self {
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
        Self { line_starts }
    }

    pub fn get_line_column(&self, offset: usize) -> (usize, usize) {
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx, // Exact match - this offset is at the start of a line
            Err(idx) => idx.saturating_sub(1),
        };

        let column = offset - self.line_starts[line_idx];
        (line_idx + 1, column) // Lines are 1-indexed
    }

    /// Convert a byte offset to a Position
    ///
    /// # Example
    /// ```
    /// use tsv_lang::{LocationTracker, Position};
    ///
    /// let source = "line1\nline2\nline3";
    /// let tracker = LocationTracker::new(source);
    ///
    /// let pos = tracker.offset_to_position(6); // Start of "line2"
    /// assert_eq!(pos.line, 2);
    /// assert_eq!(pos.column, 0);
    /// ```
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let (line, column) = self.get_line_column(offset);
        Position { line, column }
    }

    /// Convert a Span to a SourceLocation
    ///
    /// # Example
    /// ```
    /// use tsv_lang::{LocationTracker, Span};
    ///
    /// let source = "line1\nline2\nline3";
    /// let tracker = LocationTracker::new(source);
    ///
    /// let span = Span { start: 0, end: 5 }; // "line1"
    /// let loc = tracker.span_to_location(span);
    /// assert_eq!(loc.start.line, 1);
    /// assert_eq!(loc.start.column, 0);
    /// assert_eq!(loc.end.line, 1);
    /// assert_eq!(loc.end.column, 5);
    /// ```
    pub fn span_to_location(&self, span: Span) -> SourceLocation {
        let start = self.offset_to_position(span.start_usize());
        let end = self.offset_to_position(span.end_usize());
        SourceLocation { start, end }
    }

    /// Convert a Span to a SourceLocation with offset adjustment
    ///
    /// Useful for embedded content where AST has global positions but LocationTracker
    /// is created from a substring. The offset is subtracted from the span positions
    /// before conversion.
    ///
    /// # Example
    /// ```
    /// use tsv_lang::{LocationTracker, Span};
    ///
    /// // Full source: "<script>const x = 1;</script>"
    /// // LocationTracker created from: "const x = 1;"
    /// let embedded_source = "const x = 1;";
    /// let tracker = LocationTracker::new(embedded_source);
    /// let offset = 8; // Position where "const" starts in full source
    ///
    /// // Span from full source
    /// let span = Span { start: 8, end: 13 }; // "const" in full source
    ///
    /// // Convert with offset to get position in embedded source
    /// let loc = tracker.span_to_location_with_offset(span, offset);
    /// assert_eq!(loc.start.line, 1);
    /// assert_eq!(loc.start.column, 0); // "const" is at start of embedded source
    /// ```
    pub fn span_to_location_with_offset(&self, span: Span, offset: usize) -> SourceLocation {
        let adjusted_span = Span {
            start: span.start - offset as u32,
            end: span.end - offset as u32,
        };
        self.span_to_location(adjusted_span)
    }

    /// Get the byte offset of the start of the line containing the given byte offset
    ///
    /// Used to compute character-based columns: `char_column = byte_to_char(offset) - byte_to_char(line_start)`.
    pub fn line_start_byte(&self, offset: usize) -> usize {
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        self.line_starts[line_idx]
    }
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
