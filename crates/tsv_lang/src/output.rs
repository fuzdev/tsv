// Shared output utilities for formatters
//
// Provides zero-cost abstractions for building formatted output across all language formatters.
// These types are designed to be inlined by the compiler for zero runtime overhead.

/// Output buffer for building formatted strings
///
/// A thin wrapper around String that provides a consistent API for all formatters.
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

    /// Consume the buffer and return the formatted string
    #[inline]
    pub fn into_string(self) -> String {
        self.buffer
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
