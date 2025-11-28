// Shared output utilities for printers
//
// Provides zero-cost abstractions for building formatted output across all language printers.
// These types are designed to be inlined by the compiler for zero runtime overhead.

/// Output buffer for building formatted strings
///
/// A thin wrapper around String that provides a consistent API for all printers.
/// The compiler will inline these methods, making this zero-cost.
pub struct OutputBuffer {
    buffer: String,
}

impl OutputBuffer {
    /// Create a new empty output buffer
    #[inline]
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Create a new output buffer with preallocated capacity
    #[inline]
    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
        }
    }

    /// Write a string slice to the buffer
    #[inline]
    pub fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    /// Remove the last character if it matches the given character
    #[inline]
    pub fn pop_if_ends_with(&mut self, ch: char) {
        if self.buffer.ends_with(ch) {
            self.buffer.pop();
        }
    }

    /// Get the current length of the buffer
    #[inline]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the last n characters of the buffer (for debugging)
    #[inline]
    #[allow(dead_code)]
    pub fn tail(&self, n: usize) -> &str {
        let start = self.buffer.len().saturating_sub(n);
        &self.buffer[start..]
    }

    /// Consume the buffer and return the formatted string
    #[inline]
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// Get the current column position (chars since last newline)
    ///
    /// Used for width calculations when embedding doc-builder output into
    /// imperative printing. Tabs are counted as `tab_width` characters.
    #[inline]
    pub fn current_column(&self, tab_width: usize) -> usize {
        // Find the last newline and count chars after it
        let last_newline = self.buffer.rfind('\n');
        let line_start = last_newline.map(|pos| pos + 1).unwrap_or(0);
        let line = &self.buffer[line_start..];

        // Count column width, treating tabs as tab_width
        line.chars()
            .map(|ch| if ch == '\t' { tab_width } else { 1 })
            .sum()
    }
}

impl Default for OutputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Write indentation to an output buffer
///
/// Writes `level` repetitions of the `indent` string to the buffer.
/// This is a standalone function to avoid coupling OutputBuffer with indentation logic.
///
/// # Example
///
/// ```ignore
/// let mut buf = OutputBuffer::new();
/// write_indent(&mut buf, 2, "\t");  // Writes two tabs
/// ```
#[inline]
pub fn write_indent(buf: &mut OutputBuffer, level: usize, indent: &str) {
    for _ in 0..level {
        buf.write(indent);
    }
}
