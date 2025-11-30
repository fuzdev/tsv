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

#[derive(Debug)]
pub struct LocationTracker {
    line_starts: Vec<usize>,
}

impl LocationTracker {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
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
        let start = self.offset_to_position(span.start as usize);
        let end = self.offset_to_position(span.end as usize);
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
}
