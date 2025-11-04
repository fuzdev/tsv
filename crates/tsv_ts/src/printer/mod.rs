// TypeScript printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and program printing orchestration
// - **statements.rs**: Statement printing (declarations, control flow, etc.)
// - **expressions.rs**: Expression printing (literals, identifiers, binary ops, etc.)
// - **types.rs**: Type annotation printing (TypeScript-specific type syntax)
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod expressions;
mod statements;
mod types;

use crate::ast::internal;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::OutputBuffer;

/// Print configuration
#[derive(Debug, Clone)]
pub struct PrintConfig {
    /// Indent string (default: tabs)
    #[allow(dead_code)]
    pub indent: &'static str,
    /// Maximum line width (default: 100)
    #[expect(dead_code, reason = "TODO: Use for line wrapping decisions")]
    pub print_width: usize,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            indent: "\t",
            print_width: 100,
        }
    }
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    #[allow(dead_code)]
    pub(crate) indent_level: usize,
    /// Print configuration
    #[allow(dead_code)]
    config: PrintConfig,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Original source code (for extracting raw values, preserving escape sequences, etc.)
    pub(crate) source: &'a str,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given interner, source, and default config
    pub fn new(interner: Rc<RefCell<DefaultStringInterner>>, source: &'a str) -> Self {
        Self::with_config(interner, source, PrintConfig::default())
    }

    /// Create a new printer with the given interner, source, and config
    pub fn with_config(
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            interner,
            source,
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
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
    #[allow(dead_code)]
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

    /// Print a TypeScript program
    pub fn print_program(&mut self, program: &internal::Program) {
        for (i, statement) in program.body.iter().enumerate() {
            if i > 0 {
                self.write("\n");
            }
            self.print_statement(statement);
        }
        // Add trailing newline (matches prettier)
        self.write("\n");
    }
}
