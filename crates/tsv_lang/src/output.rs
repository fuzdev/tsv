// Shared output utilities for printers
//
// Provides zero-cost abstractions for building formatted output across all language printers.
// These types are designed to be inlined by the compiler for zero runtime overhead.

use crate::printing::visual_width;

/// Output buffer for building formatted strings
///
/// A thin wrapper around String that provides a consistent API for all printers.
/// The compiler will inline these methods, making this zero-cost.
#[derive(Debug)]
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

    /// Check if the buffer ends with a specific character
    #[inline]
    pub fn ends_with(&self, ch: char) -> bool {
        self.buffer.ends_with(ch)
    }

    /// Get the current length of the buffer
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Consume the buffer and return the formatted string
    ///
    /// This simply extracts the buffer contents. Whitespace stripping is handled by the
    /// doc rendering layer (`doc::print_doc*()` functions), not here. This keeps the
    /// buffer as a simple string builder without formatting responsibilities.
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// Get the current column position (chars since last newline)
    ///
    /// Used for width calculations when embedding doc-builder output into
    /// imperative printing. Tabs are counted as `tab_width` characters.
    ///
    /// One backward byte pass answers all three questions the column needs — where the
    /// line begins, whether it is ASCII, and how many tabs it holds. Asking them
    /// separately (`rposition`, then `visual_width`'s `is_ascii` and tab count) means
    /// three scans, each paying a vectorized-loop setup that does not shrink with the
    /// haystack — and the haystack here is the printer's *current line*, a partly-written
    /// one at that: a property name and its colon, ~9 bytes. The setup was the whole cost.
    ///
    /// A `\n` is never a UTF-8 continuation byte, so scanning back to it is exact whatever
    /// the line holds, and a non-ASCII byte hands the **whole** line to `visual_width` —
    /// never the remainder scanned so far, since a grapheme cluster can begin on the ASCII
    /// byte before it.
    #[inline]
    pub fn current_column(&self, tab_width: usize) -> usize {
        let bytes = self.buffer.as_bytes();
        let mut line_start = bytes.len();
        let mut tabs = 0usize;
        let mut ascii = true;
        while line_start > 0 {
            match bytes[line_start - 1] {
                b'\n' => break,
                b'\t' => tabs += 1,
                b if !b.is_ascii() => ascii = false,
                _ => {}
            }
            line_start -= 1;
        }

        if ascii {
            // `visual_width`'s ASCII arm, with the scans it would repeat already done:
            // one column per byte, and a tab is worth `tab_width` of them.
            return (bytes.len() - line_start) + tabs * (tab_width - 1);
        }
        visual_width(&self.buffer[line_start..], tab_width)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TAB_WIDTH;

    /// The column, spelled out independently of [`OutputBuffer::current_column`]: find the
    /// last newline, then measure what follows it. This is the oracle — the fused single
    /// backward pass must agree with it on every buffer.
    ///
    /// It has to be graded here because **no corpus can grade it**. The column only changes
    /// the output once a fits verdict crosses the print width, so an arithmetic slip on a
    /// rare byte (a tab, a control char, a wide grapheme) leaves every formatted file
    /// byte-identical and sails through the fixtures and any size of format or wire diff.
    fn reference(buffer: &str, tab_width: usize) -> usize {
        let line_start = buffer
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |pos| pos + 1);
        visual_width(&buffer[line_start..], tab_width)
    }

    fn assert_agrees(s: &str) {
        let mut buf = OutputBuffer::new();
        buf.write(s);
        assert_eq!(
            buf.current_column(TAB_WIDTH),
            reference(s, TAB_WIDTH),
            "current_column disagrees with the reference on {s:?}"
        );
    }

    #[test]
    fn agrees_on_exhaustive_short_buffers() {
        // Every string of length 0-3 over an alphabet spanning each arm of the scan: plain
        // ASCII, the two bytes the loop singles out (`\n`, `\t`), a control char, DEL, and
        // multi-byte UTF-8 (2-, 3- and 4-byte, plus a combining mark, a ZWJ and a variation
        // selector — the clusters that can cross the boundary onto a preceding ASCII byte).
        let alphabet = [
            "a", "Z", "0", "-", " ", "\t", "\n", "\r", "\x00", "\x1b", "\x7f", "é", "中", "🎉",
            "\u{0301}", "\u{200d}", "\u{fe0f}", "\u{00a0}",
        ];
        assert_agrees("");
        for a in alphabet {
            assert_agrees(a);
            for b in alphabet {
                assert_agrees(&format!("{a}{b}"));
                for c in alphabet {
                    assert_agrees(&format!("{a}{b}{c}"));
                }
            }
        }
    }

    #[test]
    fn agrees_on_realistic_printer_lines() {
        // What the printers actually hand it: an indented, partly-written line — and the
        // multi-line buffer that makes the backward scan's stop condition load-bearing.
        for s in [
            "\tcolor: ",
            "\t\t\tgrid-template-columns: ",
            ".selector {\n\tbackground-color: ",
            "a {\n\tb: c;\n}\n",
            "\t/* é 中 🎉 */ margin: ",
        ] {
            assert_agrees(s);
        }
    }
}
