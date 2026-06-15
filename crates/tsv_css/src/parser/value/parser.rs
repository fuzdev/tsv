// ValueParser - Same-source recursive parser for CSS values
//
// Maintains ONE source string throughout entire parse tree to avoid position drift.
// Tracks (start, end) ranges within that source instead of creating substrings.

use crate::ast::internal::CssValue;
use crate::parser::value::cursor::ValueCursor;
use tsv_lang::Span;

/// Position-tracking parser for CSS values
///
/// Maintains ONE source string throughout entire parse tree.
/// Tracks (start, end) ranges within that source.
/// All spans calculated relative to SAME source → no position drift.
///
/// # Example
/// ```ignore
/// // Internal API - not publicly accessible
/// use tsv_css::parser::value::parser::ValueParser;
/// use tsv_lang::Span;
///
/// let source = "red 01%, blue 02%";
/// let span = Span { start: 100, end: 117 };
/// let parser = ValueParser::new(source, span);
/// let value = parser.parse();
/// // All nested values have accurate spans pointing to correct bytes in source
/// ```
#[derive(Debug)]
pub(crate) struct ValueParser<'a> {
    /// Original value string (NEVER changes through recursion)
    source: &'a str,

    /// Parse range start in source (current level's slice start)
    start: usize,

    /// Parse range end in source (current level's slice end)
    end: usize,

    /// Base offset in full CSS document (for absolute span calculation)
    base_offset: u32,
}

impl<'a> ValueParser<'a> {
    /// Create parser from value string and its span in full CSS
    ///
    /// # Arguments
    /// * `source` - The value string to parse
    /// * `base_span` - The span of this value in the full CSS document
    ///
    /// # Example
    /// ```ignore
    /// let parser = ValueParser::new("red, blue", Span { start: 50, end: 59 });
    /// ```
    pub fn new(source: &'a str, base_span: Span) -> Self {
        Self {
            source,
            start: 0,
            end: source.len(),
            base_offset: base_span.start,
        }
    }

    /// Get current parse text (zero-cost slice)
    ///
    /// Returns a view into the source string for the current parse range.
    /// No allocation - just returns a slice reference.
    fn text(&self) -> &'a str {
        &self.source[self.start..self.end]
    }

    /// Get absolute span for current range
    ///
    /// Calculates the span in the full CSS document by adding base_offset
    /// to the current range positions.
    fn absolute_span(&self) -> Span {
        Span {
            start: self.base_offset + self.start as u32,
            end: self.base_offset + self.end as u32,
        }
    }

    /// Create sub-parser for a range (same source, different start/end)
    ///
    /// This is the key to avoiding position drift: we create a new parser
    /// that tracks a different range in the SAME source string, rather than
    /// creating a substring.
    ///
    /// # Arguments
    /// * `range_start` - Start offset relative to current start
    /// * `range_end` - End offset relative to current start
    ///
    /// # Returns
    /// New parser with same source but adjusted range
    fn sub_parser(&self, range_start: usize, range_end: usize) -> ValueParser<'a> {
        ValueParser {
            source: self.source,             // ✅ SAME source!
            start: self.start + range_start, // Offset into same source
            end: self.start + range_end,
            base_offset: self.base_offset, // Same base offset
        }
    }

    /// Calculate trimmed end position for a text range
    ///
    /// Removes trailing whitespace from a range and returns the adjusted end position.
    ///
    /// # Arguments
    /// * `text` - The full text being parsed
    /// * `start` - Start position in text
    /// * `end` - End position in text (may include trailing whitespace)
    ///
    /// # Returns
    /// End position after removing trailing whitespace
    fn trimmed_end(&self, text: &str, start: usize, end: usize) -> usize {
        let slice = &text[start..end];
        let trimmed_len = slice.trim_end().len();
        start + trimmed_len
    }

    /// Main parse entry point
    ///
    /// Determines the type of value and delegates to appropriate parser.
    pub fn parse(&self) -> CssValue {
        let text = self.text();
        let trimmed = text.trim();

        // Empty value
        if trimmed.is_empty() {
            return CssValue::Identifier {
                name: String::new(),
                span: self.absolute_span(),
            };
        }

        // Check what kind of value we have (use trimmed for detection)
        if super::lists::contains_comma(trimmed) {
            self.parse_comma_separated()
        } else if super::lists::contains_space_separator(trimmed) {
            self.parse_space_separated()
        } else {
            self.parse_single()
        }
    }

    /// Parse comma-separated values: "a, b, c"
    ///
    /// Uses same-source recursion - all parsed values point to ranges
    /// in the SAME source string, avoiding position drift.
    fn parse_comma_separated(&self) -> CssValue {
        let text = self.text();
        let mut cursor = ValueCursor::new(text);
        let mut values = Vec::new();

        loop {
            cursor.skip_whitespace();
            if cursor.is_eof() {
                break;
            }

            let (value_start, value_end_raw) = cursor.consume_until(|c| c == ',');
            let value_end = self.trimmed_end(text, value_start, value_end_raw);

            if value_end > value_start {
                // Non-empty value
                let sub_parser = self.sub_parser(value_start, value_end);
                values.push(sub_parser.parse()); // Recursive, but same source!
            }

            cursor.set_position(value_end_raw);
            if cursor.peek() == Some(',') {
                cursor.advance(',');
            }
        }

        CssValue::CommaSeparated {
            values,
            span: self.absolute_span(),
        }
    }

    /// Parse space-separated values: "a b c"
    ///
    /// Uses same-source recursion - all parsed values point to ranges
    /// in the SAME source string, avoiding position drift.
    fn parse_space_separated(&self) -> CssValue {
        let text = self.text();
        let mut cursor = ValueCursor::new(text);
        let mut values = Vec::new();

        loop {
            cursor.skip_whitespace();
            if cursor.is_eof() {
                break;
            }

            let (value_start, value_end_raw) = cursor.consume_until(char::is_whitespace);
            let value_end = self.trimmed_end(text, value_start, value_end_raw);

            if value_end > value_start {
                // Non-empty value
                let sub_parser = self.sub_parser(value_start, value_end);
                values.push(sub_parser.parse()); // Recursive, but same source!
            }

            cursor.set_position(value_end_raw);
            // Skip past whitespace delimiter (already handled by next loop's skip_whitespace)
        }

        CssValue::List {
            values,
            span: self.absolute_span(),
        }
    }

    /// Parse single value (leaf node)
    ///
    /// Delegates to existing single-value parsers.
    /// Trims whitespace before parsing.
    fn parse_single(&self) -> CssValue {
        let text = self.text().trim();
        let span = self.absolute_span();

        // Delegate to existing single-value parsers
        super::parse_single_value(text, span).unwrap_or_else(|| CssValue::Identifier {
            name: text.to_string(),
            span,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_parser() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.source, "red, blue");
        assert_eq!(parser.start, 0);
        assert_eq!(parser.end, 9);
        assert_eq!(parser.base_offset, 100);
    }

    #[test]
    fn test_text() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.text(), "red, blue");
    }

    #[test]
    fn test_text_with_range() {
        let source = "red, blue";
        let parser = ValueParser {
            source,
            start: 5,
            end: 9,
            base_offset: 100,
        };

        assert_eq!(parser.text(), "blue");
    }

    #[test]
    fn test_absolute_span() {
        let source = "red, blue";
        let span = Span {
            start: 100,
            end: 109,
        };
        let parser = ValueParser::new(source, span);

        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 100);
        assert_eq!(abs_span.end, 109);
    }

    #[test]
    fn test_absolute_span_with_range() {
        let source = "red, blue";
        let parser = ValueParser {
            source,
            start: 5, // "blue" starts at byte 5
            end: 9,   // "blue" ends at byte 9
            base_offset: 100,
        };

        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 105); // 100 + 5
        assert_eq!(abs_span.end, 109); // 100 + 9
    }

    #[test]
    fn test_sub_parser() {
        let source = "red, blue, green";
        let span = Span {
            start: 100,
            end: 116,
        };
        let parser = ValueParser::new(source, span);

        // Create sub-parser for "blue" (bytes 5-9 in source)
        let sub = parser.sub_parser(5, 9);

        assert_eq!(sub.source, "red, blue, green"); // Same source
        assert_eq!(sub.start, 5);
        assert_eq!(sub.end, 9);
        assert_eq!(sub.base_offset, 100); // Same base
        assert_eq!(sub.text(), "blue");

        let sub_span = sub.absolute_span();
        assert_eq!(sub_span.start, 105); // 100 + 5
        assert_eq!(sub_span.end, 109); // 100 + 9
    }

    #[test]
    fn test_nested_sub_parsers() {
        let source = "a, b, c";
        let span = Span { start: 0, end: 7 };
        let parser = ValueParser::new(source, span);

        // First level: "b, c" (bytes 3-7)
        let sub1 = parser.sub_parser(3, 7);
        assert_eq!(sub1.text(), "b, c");
        assert_eq!(sub1.absolute_span().start, 3);
        assert_eq!(sub1.absolute_span().end, 7);

        // Second level: "c" (bytes 6-7, but relative to sub1's start)
        // In sub1's coordinates: "c" is at position 3 (because sub1.start = 3)
        // So we want bytes 6-7 in original, which is 6-3=3 to 7-3=4 relative to sub1
        let sub2 = sub1.sub_parser(3, 4);
        assert_eq!(sub2.source, "a, b, c"); // Still same source!
        assert_eq!(sub2.text(), "c");
        assert_eq!(sub2.absolute_span().start, 6); // 0 + 3 + 3
        assert_eq!(sub2.absolute_span().end, 7); // 0 + 3 + 4
    }

    #[test]
    fn test_multiline_source() {
        let source = "val1,\n    val2";
        let span = Span {
            start: 100,
            end: 114,
        };
        let parser = ValueParser::new(source, span);

        assert_eq!(parser.text(), "val1,\n    val2");
        assert_eq!(parser.source, "val1,\n    val2"); // Preserves whitespace
    }

    #[test]
    fn test_empty_range() {
        let source = "test";
        let parser = ValueParser {
            source,
            start: 2,
            end: 2, // Empty range
            base_offset: 0,
        };

        assert_eq!(parser.text(), "");
        let abs_span = parser.absolute_span();
        assert_eq!(abs_span.start, 2);
        assert_eq!(abs_span.end, 2);
    }

    // Phase 2 Tests: Parsing Methods

    #[test]
    fn test_parse_single_identifier() {
        let source = "auto";
        let span = Span { start: 0, end: 4 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::Identifier { .. }));
        if let CssValue::Identifier { name, .. } = value {
            assert_eq!(name, "auto");
        }
    }

    #[test]
    fn test_parse_comma_separated_simple() {
        let source = "red, blue, green";
        let span = Span {
            start: 100,
            end: 116,
        };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 3);
        }
    }

    #[test]
    fn test_parse_space_separated_simple() {
        let source = "10px 20px 30px";
        let span = Span { start: 0, end: 14 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::List { .. }));
        if let CssValue::List { values, .. } = value {
            assert_eq!(values.len(), 3);
        }
    }

    #[test]
    fn test_parse_multiline_comma_separated() {
        // Simulate box-shadow with multiline formatting
        let source = "0 1px rgba(0, 0, 0, 0.1),\n    0 2px rgba(0, 0, 0, 0.2)";
        let span = Span {
            start: 100,
            end: 154,
        };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);
            // Verify both values parsed correctly
            assert!(matches!(values[0], CssValue::List { .. }));
            assert!(matches!(values[1], CssValue::List { .. }));
        }
    }

    #[test]
    fn test_parse_nested_function_in_list() {
        // box-shadow value with rgba function
        let source = "0 2px 4px rgba(0, 0, 0, 0.1)";
        let span = Span { start: 0, end: 28 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::List { .. }));
        if let CssValue::List { values, .. } = value {
            assert_eq!(values.len(), 4); // "0", "2px", "4px", "rgba(...)"
            // Last value should be a color (rgba is recognized as color function)
            assert!(matches!(values[3], CssValue::Color { .. }));
        }
    }

    #[test]
    fn test_parse_preserves_leading_zeros_in_function() {
        // This is the key test - leading zeros should be preserved
        let source = "linear-gradient(90deg, red 01%, blue 02%)";
        let span = Span { start: 0, end: 42 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::Function { .. }));
        if let CssValue::Function { name, args, .. } = value {
            assert_eq!(name, "linear-gradient");
            // Arguments should have accurate spans pointing to original source
            assert_eq!(args.len(), 3); // "90deg", "red 01%", "blue 02%"
        }
    }

    #[test]
    fn test_parse_handles_empty_values() {
        // Edge case: double comma (empty value between)
        let source = "a,,c";
        let span = Span { start: 0, end: 4 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            // Empty values should be skipped
            assert_eq!(values.len(), 2);
        }
    }

    #[test]
    fn test_parse_handles_trailing_whitespace() {
        let source = "red  ,  blue  ";
        let span = Span { start: 0, end: 14 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));
        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);
            // Spans should exclude trailing whitespace
        }
    }

    #[test]
    fn test_parse_single_value_with_whitespace() {
        let source = "  auto  ";
        let span = Span { start: 0, end: 8 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::Identifier { .. }));
        if let CssValue::Identifier { name, .. } = value {
            assert_eq!(name, "auto");
        }
    }

    #[test]
    fn test_trimmed_end_helper() {
        let source = "red  ,  blue";
        let span = Span { start: 0, end: 12 };
        let parser = ValueParser::new(source, span);

        // Test trimming "red  " (positions 0-5, trimmed should be 0-3)
        let trimmed = parser.trimmed_end(source, 0, 5);
        assert_eq!(trimmed, 3);

        // Test trimming "  blue" (no trailing space)
        let trimmed = parser.trimmed_end(source, 8, 12);
        assert_eq!(trimmed, 12);
    }

    #[test]
    fn test_nested_sub_parsers_with_parse() {
        // Test same-source recursion through actual parsing
        let source = "a, b, c";
        let span = Span { start: 0, end: 7 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));

        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 3);

            // Each value should have correct span
            for (i, val) in values.iter().enumerate() {
                let val_span = val.span();
                // Verify spans are in correct order and non-overlapping
                if i > 0 {
                    let prev_span = values[i - 1].span();
                    assert!(val_span.start > prev_span.end);
                }
            }
        }
    }

    #[test]
    fn test_complex_multiline_box_shadow() {
        // Real-world test case from fixture
        let source = "0 2px 4px rgba(0, 0, 0, 0.1),\n    0 4px 8px rgba(0, 0, 0, 0.2)";
        let span = Span { start: 0, end: 63 };
        let parser = ValueParser::new(source, span);

        let value = parser.parse();
        assert!(matches!(value, CssValue::CommaSeparated { .. }));

        if let CssValue::CommaSeparated { values, .. } = value {
            assert_eq!(values.len(), 2);

            // First value: "0 2px 4px rgba(0, 0, 0, 0.1)"
            assert!(matches!(values[0], CssValue::List { .. }));
            if let CssValue::List { values: parts, .. } = &values[0] {
                assert_eq!(parts.len(), 4);
                // Last part should be a color (rgba recognized as color)
                assert!(matches!(parts[3], CssValue::Color { .. }));
            }

            // Second value: "0 4px 8px rgba(0, 0, 0, 0.2)"
            assert!(matches!(values[1], CssValue::List { .. }));
        }
    }
}
