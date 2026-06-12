// ValueCursor - Position-tracking cursor for parsing CSS values
//
// Enables accurate span calculation for nested values (lists, function args)
// by tracking byte position during parsing. This allows the printer to extract
// from source and preserve formatting quirks like leading zeros.

use tsv_lang::Span;

/// Position-tracking cursor for parsing CSS values
///
/// Enables accurate span calculation for nested values (lists, function args)
/// by tracking byte position during parsing.
///
/// # Example
/// ```ignore
/// // Internal API - not publicly accessible
/// use tsv_css::parser::value::cursor::ValueCursor;
/// use tsv_lang::Span;
///
/// let source = "red 01%, blue 02%";
/// let parent_span = Span { start: 100, end: 116 };
/// let mut cursor = ValueCursor::new(source, parent_span);
///
/// // Parse first value
/// let start = cursor.skip_whitespace();
/// let (_, end) = cursor.consume_until(|c| c == ',');
/// let (text, span) = cursor.extract_trimmed(start, end);
/// assert_eq!(text, "red 01%");
/// assert_eq!(span.start, 100);
/// assert_eq!(span.end, 107);
/// ```
#[derive(Debug)]
pub(crate) struct ValueCursor<'a> {
    /// Original value string being parsed
    source: &'a str,

    /// Current byte position in source
    pos: usize,

    /// Base span offset (parent value's start position in full CSS)
    base_offset: u32,

    /// Parenthesis nesting depth (don't split inside function calls)
    paren_depth: u32,

    /// Quote state (don't split inside strings)
    in_quote: bool,

    /// Which quote character opened the current string
    quote_char: char,
}

impl<'a> ValueCursor<'a> {
    /// Create cursor from value string and parent span
    ///
    /// # Arguments
    /// * `source` - The value string to parse (e.g., "red 01%, blue 02%")
    /// * `base_span` - Parent value's span in the full CSS document
    ///
    /// # Example
    /// ```ignore
    /// // Internal API - not publicly accessible
    /// use tsv_css::parser::value::cursor::ValueCursor;
    /// use tsv_lang::Span;
    ///
    /// let source = "rgba(255, 0, 0, 1.0)";
    /// let span = Span { start: 50, end: 70 };
    /// let cursor = ValueCursor::new(source, span);
    /// ```
    pub fn new(source: &'a str, base_span: Span) -> Self {
        Self {
            source,
            pos: 0,
            base_offset: base_span.start,
            paren_depth: 0,
            in_quote: false,
            quote_char: '\0',
        }
    }

    /// Get current absolute span for a given byte range
    ///
    /// Converts relative positions in the value string to absolute positions
    /// in the full CSS document.
    ///
    /// # Arguments
    /// * `start` - Start byte position in source string
    /// * `end` - End byte position in source string
    ///
    /// # Returns
    /// Absolute span in the full CSS document
    #[allow(dead_code)]
    pub fn span(&self, start: usize, end: usize) -> Span {
        Span {
            start: self.base_offset + start as u32,
            end: self.base_offset + end as u32,
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
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
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
    ///
    /// # Returns
    /// `(start, end)` - Byte positions of consumed text (not including delimiter)
    ///
    /// # Example
    /// ```ignore
    /// let (start, end) = cursor.consume_until(|c| c == ',');
    /// // Parses "rgba(1, 2, 3)" correctly - commas inside parens don't stop parsing
    /// ```
    pub fn consume_until<F>(&mut self, is_delimiter: F) -> (usize, usize)
    where
        F: Fn(char) -> bool,
    {
        let start = self.pos;

        // Iterate using char_indices for UTF-8 safety
        // byte_offset is relative to self.source[self.pos..]
        for (byte_offset, ch) in self.source[self.pos..].char_indices() {
            // Check delimiter BEFORE updating state (use current nesting level)
            if self.paren_depth == 0 && !self.in_quote && is_delimiter(ch) {
                let end = self.pos + byte_offset;
                return (start, end);
            }

            // Update state for next iteration
            self.update_state(ch);
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

    /// Extract substring with accurate span (untrimmed)
    ///
    /// Returns the raw text and its absolute span.
    ///
    /// # Arguments
    /// * `start` - Start byte position in source string
    /// * `end` - End byte position in source string
    ///
    /// # Returns
    /// `(text, span)` - Raw text slice and absolute span
    #[allow(dead_code)]
    pub fn extract(&self, start: usize, end: usize) -> (&'a str, Span) {
        let text = &self.source[start..end];
        let span = self.span(start, end);
        (text, span)
    }

    /// Extract substring with accurate span (trimmed)
    ///
    /// Returns the trimmed text and an adjusted span that points to the
    /// trimmed content (excludes leading/trailing whitespace).
    ///
    /// # Arguments
    /// * `start` - Start byte position in source string
    /// * `end` - End byte position in source string
    ///
    /// # Returns
    /// `(trimmed_text, adjusted_span)` - Trimmed text and span pointing to trimmed content
    ///
    /// # Example
    /// ```ignore
    /// let (text, span) = cursor.extract_trimmed(0, 10);
    /// // For "  red 01%  " → ("red 01%", Span pointing to "red 01%")
    /// ```
    #[allow(dead_code)]
    pub fn extract_trimmed(&self, start: usize, end: usize) -> (&'a str, Span) {
        let text = &self.source[start..end];
        let trimmed = text.trim();

        // Calculate trim offsets
        // trim_start: number of leading whitespace characters
        // trim_end: number of trailing whitespace characters
        let trim_start = text.len() - text.trim_start().len();
        let trim_end = text.len() - text.trim_end().len();

        // Adjusted span points to the trimmed content
        // Start: original start + leading whitespace
        // End: original end - trailing whitespace
        let adjusted_span = Span {
            start: self.base_offset + (start + trim_start) as u32,
            end: self.base_offset + (end - trim_end) as u32,
        };

        (trimmed, adjusted_span)
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

    /// Get current position
    #[allow(dead_code)]
    pub fn position(&self) -> usize {
        self.pos
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
    use super::*;

    #[test]
    fn test_new_cursor() {
        let source = "red 01%, blue 02%";
        let span = Span {
            start: 100,
            end: 117,
        };
        let cursor = ValueCursor::new(source, span);

        assert_eq!(cursor.pos, 0);
        assert_eq!(cursor.base_offset, 100);
        assert_eq!(cursor.paren_depth, 0);
        assert!(!cursor.in_quote);
    }

    #[test]
    fn test_peek() {
        let source = "abc";
        let span = Span { start: 0, end: 3 };
        let cursor = ValueCursor::new(source, span);

        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_peek_eof() {
        let source = "";
        let span = Span { start: 0, end: 0 };
        let cursor = ValueCursor::new(source, span);

        assert_eq!(cursor.peek(), None);
    }

    #[test]
    fn test_is_eof() {
        let source = "a";
        let span = Span { start: 0, end: 1 };
        let mut cursor = ValueCursor::new(source, span);

        assert!(!cursor.is_eof());
        cursor.pos = 1;
        assert!(cursor.is_eof());
    }

    #[test]
    fn test_skip_whitespace() {
        let source = "   abc";
        let span = Span { start: 0, end: 6 };
        let mut cursor = ValueCursor::new(source, span);

        let pos = cursor.skip_whitespace();
        assert_eq!(pos, 3);
        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_skip_whitespace_no_whitespace() {
        let source = "abc";
        let span = Span { start: 0, end: 3 };
        let mut cursor = ValueCursor::new(source, span);

        let pos = cursor.skip_whitespace();
        assert_eq!(pos, 0);
        assert_eq!(cursor.peek(), Some('a'));
    }

    #[test]
    fn test_consume_until_simple() {
        let source = "red, blue";
        let span = Span { start: 0, end: 9 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert_eq!(&source[start..end], "red");
    }

    #[test]
    fn test_consume_until_respects_parens() {
        let source = "rgba(1, 2, 3), blue";
        let span = Span { start: 0, end: 19 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 13);
        assert_eq!(&source[start..end], "rgba(1, 2, 3)");
    }

    #[test]
    fn test_consume_until_respects_quotes() {
        let source = r#""foo, bar", baz"#;
        let span = Span { start: 0, end: 15 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 10);
        assert_eq!(&source[start..end], r#""foo, bar""#);
    }

    #[test]
    fn test_consume_until_eof() {
        let source = "red blue";
        let span = Span { start: 0, end: 8 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 8);
        assert_eq!(&source[start..end], "red blue");
    }

    #[test]
    fn test_extract() {
        let source = "red 01%";
        let span = Span {
            start: 100,
            end: 107,
        };
        let cursor = ValueCursor::new(source, span);

        let (text, result_span) = cursor.extract(0, 7);
        assert_eq!(text, "red 01%");
        assert_eq!(result_span.start, 100);
        assert_eq!(result_span.end, 107);
    }

    #[test]
    fn test_extract_trimmed() {
        let source = "  red 01%  ";
        let span = Span {
            start: 100,
            end: 111,
        };
        let cursor = ValueCursor::new(source, span);

        let (text, result_span) = cursor.extract_trimmed(0, 11);
        assert_eq!(text, "red 01%");
        assert_eq!(result_span.start, 102); // +2 for leading spaces
        assert_eq!(result_span.end, 109); // -2 for trailing spaces
    }

    #[test]
    fn test_extract_trimmed_no_whitespace() {
        let source = "red 01%";
        let span = Span {
            start: 100,
            end: 107,
        };
        let cursor = ValueCursor::new(source, span);

        let (text, result_span) = cursor.extract_trimmed(0, 7);
        assert_eq!(text, "red 01%");
        assert_eq!(result_span.start, 100);
        assert_eq!(result_span.end, 107);
    }

    #[test]
    fn test_advance() {
        let source = "abc";
        let span = Span { start: 0, end: 3 };
        let mut cursor = ValueCursor::new(source, span);

        cursor.advance('a');
        assert_eq!(cursor.pos, 1);
        assert_eq!(cursor.peek(), Some('b'));
    }

    #[test]
    fn test_utf8_multibyte_char() {
        // '€' is 3 bytes in UTF-8
        let source = "€100";
        let span = Span { start: 0, end: 6 };
        let mut cursor = ValueCursor::new(source, span);

        assert_eq!(cursor.peek(), Some('€'));
        cursor.advance('€');
        assert_eq!(cursor.pos, 3); // Advanced 3 bytes
        assert_eq!(cursor.peek(), Some('1'));
    }

    #[test]
    fn test_nested_parens() {
        let source = "calc(10px + calc(5px + 2px)), blue";
        let span = Span { start: 0, end: 34 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 28);
        assert_eq!(&source[start..end], "calc(10px + calc(5px + 2px))");
    }

    #[test]
    fn test_mixed_quotes() {
        let source = r#"'foo "bar" baz', "qux 'quux'""#;
        let span = Span { start: 0, end: 30 };
        let mut cursor = ValueCursor::new(source, span);

        let (start, end) = cursor.consume_until(|c| c == ',');
        assert_eq!(start, 0);
        assert_eq!(end, 15);
        assert_eq!(&source[start..end], r#"'foo "bar" baz'"#);
    }

    #[test]
    fn test_position_and_set_position() {
        let source = "abc";
        let span = Span { start: 0, end: 3 };
        let mut cursor = ValueCursor::new(source, span);

        assert_eq!(cursor.position(), 0);
        cursor.set_position(2);
        assert_eq!(cursor.position(), 2);
        assert_eq!(cursor.peek(), Some('c'));
    }

    #[test]
    fn test_empty_string() {
        let source = "";
        let span = Span { start: 0, end: 0 };
        let cursor = ValueCursor::new(source, span);

        assert!(cursor.is_eof());
        assert_eq!(cursor.peek(), None);
    }

    #[test]
    fn test_span_calculation() {
        let source = "test";
        let span = Span { start: 50, end: 54 };
        let cursor = ValueCursor::new(source, span);

        let result_span = cursor.span(1, 3);
        assert_eq!(result_span.start, 51);
        assert_eq!(result_span.end, 53);
    }

    #[test]
    fn test_update_state_parens() {
        let source = "((()))";
        let span = Span { start: 0, end: 6 };
        let mut cursor = ValueCursor::new(source, span);

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
        let span = Span { start: 0, end: 6 };
        let mut cursor = ValueCursor::new(source, span);

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
        let span = Span { start: 0, end: 5 };
        let mut cursor = ValueCursor::new(source, span);

        // consume_until whitespace should split at the space between "(" and b
        let (start, end) = cursor.consume_until(char::is_whitespace);
        assert_eq!(&source[start..end], r#""(""#);
        assert_eq!(cursor.paren_depth, 0);
    }

    #[test]
    fn test_update_state_parens_inside_quotes() {
        let source = r#""(""#;
        let span = Span { start: 0, end: 3 };
        let mut cursor = ValueCursor::new(source, span);

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
        let span = Span { start: 0, end: 1 };
        let mut cursor = ValueCursor::new(source, span);

        cursor.update_state(')');
        assert_eq!(cursor.paren_depth, 0); // Should stay at 0, not underflow
    }
}
