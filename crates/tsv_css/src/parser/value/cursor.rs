// ValueCursor - Position-tracking cursor for parsing CSS values
//
// Splits a CSS value string into delimiter-separated ranges (lists, function
// args) while respecting parenthesis and quote nesting, tracking byte position
// during parsing.

use crate::parser::value::scan::is_value_structural;
use crate::whitespace::is_css_whitespace;

/// Position-tracking cursor for parsing CSS values
///
/// Splits a CSS value string into delimiter-separated ranges (lists, function
/// args) while respecting parenthesis and quote nesting.
///
/// # Example
/// ```ignore
/// // Internal API - not publicly accessible
/// use tsv_css::parser::value::cursor::ValueCursor;
///
/// let source = "red 01%, blue 02%";
/// let mut cursor = ValueCursor::new(source);
///
/// // Parse first value
/// let start = cursor.skip_whitespace();
/// let (_, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
/// assert_eq!(&source[start..end], "red 01%");
/// ```
#[derive(Debug)]
pub(crate) struct ValueCursor<'a> {
    /// Original value string being parsed
    source: &'a str,

    /// Current byte position in source
    pos: usize,

    /// Parenthesis nesting depth (don't split inside function calls)
    paren_depth: u32,

    /// Quote state (don't split inside strings)
    in_quote: bool,

    /// Which quote character opened the current string
    quote_char: char,
}

impl<'a> ValueCursor<'a> {
    /// Create cursor from a value string
    ///
    /// # Arguments
    /// * `source` - The value string to parse (e.g., "red 01%, blue 02%")
    ///
    /// # Example
    /// ```ignore
    /// // Internal API - not publicly accessible
    /// use tsv_css::parser::value::cursor::ValueCursor;
    ///
    /// let cursor = ValueCursor::new("rgba(255, 0, 0, 1.0)");
    /// ```
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            paren_depth: 0,
            in_quote: false,
            quote_char: '\0',
        }
    }

    /// Peek at next character without advancing
    ///
    /// Returns `None` if at end of input.
    pub fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    /// Check if at end of input
    pub fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Skip whitespace, return new position
    ///
    /// Advances cursor past all whitespace characters and returns the
    /// new position (first non-whitespace character or EOF).
    pub fn skip_whitespace(&mut self) -> usize {
        let bytes = self.source.as_bytes();
        // CSS whitespace is ASCII-only (`is_css_whitespace` is `is_ascii_whitespace`;
        // NBSP and other Unicode whitespace are value content), so a non-ASCII byte
        // always stops the skip and no UTF-8 decode is needed — byte-identical to
        // the previous `peek`-per-char loop.
        while let Some(&b) = bytes.get(self.pos) {
            if b < 0x80 && is_css_whitespace(b as char) {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.pos
    }

    /// Consume characters until delimiter or EOF, tracking paren/quote nesting
    ///
    /// Returns the start and end positions of the consumed text.
    /// Respects nesting - delimiters inside parentheses or quotes are ignored.
    ///
    /// # Arguments
    /// * `is_delimiter` - Predicate to check if character is a delimiter
    /// * `skip` - The caller's [`value_skip_table!`](super::scan::value_skip_table) for
    ///   `is_delimiter`. It has to be the caller's, because `is_delimiter` is opaque
    ///   here — so the table and the predicate it mirrors are declared side by side at
    ///   the call site, and the `debug_assert` below re-derives every skip from the
    ///   predicate itself. A table that disagrees fails the test suite (which runs in
    ///   debug) rather than silently changing a split.
    ///
    /// # Returns
    /// `(start, end)` - Byte positions of consumed text (not including delimiter)
    ///
    /// # Example
    /// ```ignore
    /// let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
    /// // Parses "rgba(1, 2, 3)" correctly - commas inside parens don't stop parsing
    /// ```
    pub fn consume_until<F>(&mut self, is_delimiter: F, skip: &[bool; 256]) -> (usize, usize)
    where
        F: Fn(char) -> bool,
    {
        let start = self.pos;
        let bytes = self.source.as_bytes();
        let mut i = self.pos;

        // Walk bytes instead of `char_indices` to skip the per-char UTF-8 decode
        // on the ASCII-dominant common case. Every delimiter and every nesting
        // trigger (`(` `)` `'` `"`) is ASCII, so an ASCII byte is processed as its
        // char with no decode; a non-ASCII lead byte is decoded in full so the
        // predicate still sees the real char (some callers — e.g.
        // `char::is_whitespace` — treat non-ASCII chars as delimiters). Advancing
        // by the char width keeps `i` on a char boundary, so this is byte-for-byte
        // identical to the previous `char_indices` loop.
        while i < bytes.len() {
            let b = bytes[i];

            // Most of a value's bytes are inert content — not a delimiter, not an
            // escape, not a nesting or quote trigger. One L1 load retires them
            // before the decode, the escape probe, the predicate call, and
            // `update_state`'s match chain.
            if skip[b as usize] {
                debug_assert!(
                    b < 0x80 && !is_value_structural(b) && !is_delimiter(b as char),
                    "consume_until skip table disagrees with its delimiter predicate at {b:#04x}"
                );
                i += 1;
                continue;
            }

            let (ch, width) = if b < 0x80 {
                (b as char, 1)
            } else {
                // `i` is within bounds and on a char boundary, so `next()` is
                // always `Some`; the `else` is unreachable.
                let Some(c) = self.source[i..].chars().next() else {
                    break;
                };
                (c, c.len_utf8())
            };

            // An escaped paren (`\(` / `\)`) is a content code point (css-syntax §4.3.7), not a
            // nesting delimiter, so it must not change `paren_depth` — otherwise an escaped `)`
            // inside `url()` mis-drops the depth and exposes a false top-level separator. Skip
            // both (parens are ASCII, one byte each). Kept identical in the twin
            // `fast_scan` / `classify_separators` trackers.
            if ch == '\\' && matches!(bytes.get(i + 1), Some(b'(' | b')')) {
                i += 2;
                continue;
            }

            // Check delimiter BEFORE updating state (use current nesting level)
            if self.paren_depth == 0 && !self.in_quote && is_delimiter(ch) {
                return (start, i);
            }

            // Update state for next iteration
            self.update_state(ch);
            i += width;
        }

        // Reached EOF without finding delimiter
        (start, self.source.len())
    }

    /// Update nesting state for character
    ///
    /// Tracks parenthesis depth and quote state to know when we're inside
    /// nested structures (don't split on delimiters inside these).
    fn update_state(&mut self, ch: char) {
        match ch {
            '(' if !self.in_quote => self.paren_depth += 1,
            ')' if !self.in_quote => self.paren_depth = self.paren_depth.saturating_sub(1),
            '\'' | '"' if !self.in_quote => {
                self.in_quote = true;
                self.quote_char = ch;
            }
            c if self.in_quote && c == self.quote_char => {
                self.in_quote = false;
            }
            _ => {}
        }
    }

    /// Advance position past character
    ///
    /// Used to skip past delimiters after parsing a value.
    ///
    /// # Arguments
    /// * `ch` - Character to advance past (used for UTF-8 length calculation)
    pub fn advance(&mut self, ch: char) {
        self.pos += ch.len_utf8();
    }

    /// Set position (for manual navigation)
    ///
    /// # Safety
    /// Caller must ensure position is at a UTF-8 character boundary.
    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos;
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::value::scan::value_skip_table;

    /// Skip table for `|c| c == ','`, the comma delimiter these tests drive.
    const COMMA_SKIP: [bool; 256] = value_skip_table!(|b| b == b',');

    /// Skip table for `char::is_whitespace`, whose ASCII half is CSS whitespace plus
    /// U+000B (vertical tab). No production delimiter uses it — it is here precisely to
    /// exercise `consume_until` against a predicate that also treats *non*-ASCII chars
    /// as delimiters, proving the table's ASCII-only rule keeps them on the decode path.
    const UNICODE_WS_SKIP: [bool; 256] =
        value_skip_table!(|b| b.is_ascii_whitespace() || b == 0x0B);

    use super::*;

    #[test]
    fn test_new_cursor() {
        let source = "red 01%, blue 02%";
        let cursor = ValueCursor::new(source);

        assert_eq!(cursor.pos, 0);
        assert_eq!(cursor.paren_depth, 0);
        assert!(!cursor.in_quote);
    }

    #[test]
    fn test_peek() {
        let source = "abc";
        let cursor = ValueCursor::new(source);

        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_peek_eof() {
        let source = "";
        let cursor = ValueCursor::new(source);

        assert_eq!(cursor.peek(), None);
    }

    #[test]
    fn test_is_eof() {
        let source = "a";
        let mut cursor = ValueCursor::new(source);

        assert!(!cursor.is_eof());
        cursor.pos = 1;
        assert!(cursor.is_eof());
    }

    #[test]
    fn test_skip_whitespace() {
        let source = "   abc";
        let mut cursor = ValueCursor::new(source);

        let pos = cursor.skip_whitespace();
        assert_eq!(pos, 3);
        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_skip_whitespace_no_whitespace() {
        let source = "abc";
        let mut cursor = ValueCursor::new(source);

        let pos = cursor.skip_whitespace();
        assert_eq!(pos, 0);
        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_consume_until_simple() {
        let source = "red, blue";
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert_eq!(&source[start..end], "red");
    }

    #[test]
    fn test_consume_until_respects_parens() {
        let source = "rgba(1, 2, 3), blue";
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 13);
        assert_eq!(&source[start..end], "rgba(1, 2, 3)");
    }

    #[test]
    fn test_consume_until_respects_quotes() {
        let source = r#""foo, bar", baz"#;
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 10);
        assert_eq!(&source[start..end], r#""foo, bar""#);
    }

    #[test]
    fn test_consume_until_eof() {
        let source = "red blue";
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 8);
        assert_eq!(&source[start..end], "red blue");
    }

    #[test]
    fn test_advance() {
        let source = "abc";
        let mut cursor = ValueCursor::new(source);

        cursor.advance('a');
        assert_eq!(cursor.pos, 1);
        assert_eq!(cursor.peek(), Some('b'));
    }

    #[test]
    fn test_utf8_multibyte_char() {
        // '€' is 3 bytes in UTF-8
        let source = "€100";
        let mut cursor = ValueCursor::new(source);

        assert_eq!(cursor.peek(), Some('€'));
        cursor.advance('€');
        assert_eq!(cursor.pos, 3); // Advanced 3 bytes
        assert_eq!(cursor.peek(), Some('1'));
    }

    #[test]
    fn test_nested_parens() {
        let source = "calc(10px + calc(5px + 2px)), blue";
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 28);
        assert_eq!(&source[start..end], "calc(10px + calc(5px + 2px))");
    }

    #[test]
    fn test_mixed_quotes() {
        let source = r#"'foo "bar" baz', "qux 'quux'""#;
        let mut cursor = ValueCursor::new(source);

        let (start, end) = cursor.consume_until(|c| c == ',', &COMMA_SKIP);
        assert_eq!(start, 0);
        assert_eq!(end, 15);
        assert_eq!(&source[start..end], r#"'foo "bar" baz'"#);
    }

    #[test]
    fn test_set_position() {
        let source = "abc";
        let mut cursor = ValueCursor::new(source);

        cursor.set_position(2);
        assert_eq!(cursor.pos, 2);
        assert_eq!(cursor.peek(), Some('c'));
    }

    #[test]
    fn test_empty_string() {
        let source = "";
        let cursor = ValueCursor::new(source);

        assert!(cursor.is_eof());
        assert_eq!(cursor.peek(), None);
    }

    #[test]
    fn test_update_state_parens() {
        let source = "((()))";
        let mut cursor = ValueCursor::new(source);

        assert_eq!(cursor.paren_depth, 0);
        cursor.update_state('(');
        assert_eq!(cursor.paren_depth, 1);
        cursor.update_state('(');
        assert_eq!(cursor.paren_depth, 2);
        cursor.update_state(')');
        assert_eq!(cursor.paren_depth, 1);
        cursor.update_state(')');
        assert_eq!(cursor.paren_depth, 0);
    }

    #[test]
    fn test_update_state_quotes() {
        let source = r#""test""#;
        let mut cursor = ValueCursor::new(source);

        assert!(!cursor.in_quote);
        cursor.update_state('"');
        assert!(cursor.in_quote);
        assert_eq!(cursor.quote_char, '"');
        cursor.update_state('"');
        assert!(!cursor.in_quote);
    }

    #[test]
    fn test_parens_inside_quotes_ignored() {
        // Parens inside strings must not affect paren_depth
        let source = r#""(" b"#;
        let mut cursor = ValueCursor::new(source);

        // consume_until whitespace should split at the space between "(" and b
        let (start, end) = cursor.consume_until(char::is_whitespace, &UNICODE_WS_SKIP);
        assert_eq!(&source[start..end], r#""(""#);
        assert_eq!(cursor.paren_depth, 0);
    }

    #[test]
    fn test_update_state_parens_inside_quotes() {
        let source = r#""(""#;
        let mut cursor = ValueCursor::new(source);

        cursor.update_state('"');
        assert!(cursor.in_quote);
        cursor.update_state('(');
        assert_eq!(cursor.paren_depth, 0); // Must stay 0 — inside quotes
        cursor.update_state('"');
        assert!(!cursor.in_quote);
    }

    #[test]
    fn test_saturating_sub_parens() {
        // Test that paren_depth doesn't go negative
        let source = ")";
        let mut cursor = ValueCursor::new(source);

        cursor.update_state(')');
        assert_eq!(cursor.paren_depth, 0); // Should stay at 0, not underflow
    }
}
