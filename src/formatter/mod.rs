// Formatter for TypeScript/Svelte/CSS
// Converts internal AST back to formatted source code

// Submodules organized by language
mod css; // CSS formatting (rules, declarations)
mod svelte; // Svelte formatting (templates, components)
mod typescript; // TypeScript formatting (expressions, statements, types)

use crate::ast::internal;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};

/// Format configuration
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// Indent string (default: tabs)
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    #[expect(dead_code, reason = "TODO: Use for line wrapping decisions")]
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
    buffer: String,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Format configuration
    config: FormatConfig,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Location tracker for determining source line positions
    location_tracker: Option<crate::location::LocationTracker>,
}

impl Formatter {
    /// Create a new formatter with the given interner and default config
    pub fn new(interner: Rc<RefCell<DefaultStringInterner>>) -> Self {
        Self::with_config(interner, FormatConfig::default())
    }

    /// Create a new formatter with the given interner and config
    pub fn with_config(interner: Rc<RefCell<DefaultStringInterner>>, config: FormatConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
            interner,
            location_tracker: None,
        }
    }

    /// Create a new formatter with source location tracking
    pub fn with_source(interner: Rc<RefCell<DefaultStringInterner>>, source: &str) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config: FormatConfig::default(),
            interner,
            location_tracker: Some(crate::location::LocationTracker::new(source)),
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
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
    ///
    /// Example:
    /// ```ignore
    /// self.with_resolved_symbol(tag_name, |tag| {
    ///     let is_inline = is_inline_element(tag);
    ///     let is_void = is_void_element(tag);
    ///     (is_inline, is_void)
    /// })
    /// ```
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
    ///
    /// Used for formatting nested structures like CSS rules,
    /// script tag content, and nested HTML elements.
    pub(crate) fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.write(self.config.indent);
        }
    }

    /// Check if two spans are on the same source line
    ///
    /// Returns true if both spans start on the same line in the original source.
    /// Used for inline run grouping: preserve source layout to maintain authorial intent.
    pub(crate) fn are_on_same_line(
        &self,
        span1: crate::span::Span,
        span2: crate::span::Span,
    ) -> bool {
        if let Some(tracker) = &self.location_tracker {
            let (line1, _) = tracker.get_line_column(span1.start as usize);
            let (line2, _) = tracker.get_line_column(span2.start as usize);
            line1 == line2
        } else {
            // Without location tracker, assume multiline (safer default)
            false
        }
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer
    }
}

/// Format a TypeScript program to a string
pub fn format_typescript(program: &internal::Program) -> String {
    let mut formatter = Formatter::new(program.interner.clone());
    formatter.format_program(program);
    formatter.into_string()
}

/// Format a Svelte root to a string
pub fn format_svelte(root: &internal::Root, source: &str) -> String {
    let mut formatter = Formatter::with_source(root.interner.clone(), source);
    formatter.format_root(root);
    formatter.into_string()
}

/// Format CSS nodes to a string
///
/// CSS doesn't use an interner, so we create a temporary one for the formatter.
pub fn format_css(nodes: &[internal::CssNode]) -> String {
    let interner = Rc::new(RefCell::new(string_interner::DefaultStringInterner::new()));
    let mut formatter = Formatter::new(interner);
    formatter.format_css_nodes(nodes);
    formatter.into_string()
}
