// Core Formatter struct and helpers for Svelte formatting

use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::OutputBuffer;

/// Format configuration
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// Indent string (default: tabs)
    #[allow(dead_code)]
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    pub print_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
        }
    }
}

/// Formatter state for building output
pub struct Formatter {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    #[allow(dead_code)]
    pub(crate) indent_level: usize,
    /// Format configuration
    #[allow(dead_code)]
    config: FormatConfig,
    /// Source code (needed for preserving whitespace semantics)
    source: String,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
}

impl Formatter {
    /// Create a new formatter with the given source, interner, and default config
    pub fn new(source: &str, interner: Rc<RefCell<DefaultStringInterner>>) -> Self {
        Self::with_config(source, interner, FormatConfig::default())
    }

    /// Create a new formatter with the given source, interner, and config
    pub fn with_config(
        source: &str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        config: FormatConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source: source.to_string(),
            interner,
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Get the source code
    #[allow(dead_code)]
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    /// Resolve a symbol from the interner to a string
    ///
    /// This centralizes symbol resolution and provides a single point
    /// for error handling and potential debugging/logging.
    ///
    /// Note: This allocates a String on every call. For hot paths where multiple
    /// operations are needed on the same symbol, use `with_resolved_symbol()` instead.
    pub(crate) fn resolve_symbol(&self, symbol: DefaultSymbol) -> String {
        self.interner
            .borrow()
            .resolve(symbol)
            .expect("Symbol not found in interner")
            .to_string()
    }

    /// Execute a callback with a borrowed string for a symbol (zero-allocation)
    ///
    /// This is more efficient than `resolve_symbol()` when you need to perform
    /// multiple operations on the resolved string without needing ownership.
    #[inline]
    pub(crate) fn with_resolved_symbol<F, R>(&self, symbol: DefaultSymbol, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        let interner = self.interner.borrow();
        let s = interner
            .resolve(symbol)
            .expect("Symbol not found in interner");
        f(s)
    }

    /// Write indentation based on current indent level
    #[allow(dead_code)]
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, self.config.indent);
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Check if two spans are on the same line in the source
    ///
    /// Used for inline run grouping to preserve authorial layout intent.
    /// Only checks the whitespace **between** the two spans, not the span content itself.
    /// This allows tags that span multiple lines (e.g., `<br \n>`) to still be grouped together.
    pub(crate) fn are_on_same_line(&self, span1: tsv_lang::Span, span2: tsv_lang::Span) -> bool {
        // Determine which span comes first
        let (first, second) = if span1.start <= span2.start {
            (span1, span2)
        } else {
            (span2, span1)
        };

        // If spans overlap or touch, they're on the same line
        if first.end >= second.start {
            return true;
        }

        // Check the whitespace between the spans
        let start = first.end as usize;
        let end = second.start as usize;

        if start >= self.source.len() || end > self.source.len() {
            return false;
        }

        let between = &self.source[start..end];
        !between.contains('\n')
    }
}
