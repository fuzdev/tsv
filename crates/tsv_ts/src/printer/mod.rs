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
    /// Comments from the program (for printing leading/trailing comments)
    pub(crate) comments: &'a Vec<internal::Comment>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given interner, source, comments, and default config
    pub fn new(
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        comments: &'a Vec<internal::Comment>,
    ) -> Self {
        Self::with_config(interner, source, comments, PrintConfig::default())
    }

    /// Create a new printer with the given interner, source, comments, and config
    pub fn with_config(
        interner: Rc<RefCell<DefaultStringInterner>>,
        source: &'a str,
        comments: &'a Vec<internal::Comment>,
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            interner,
            source,
            comments,
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

    /// Print a TypeScript comment
    pub(crate) fn print_comment(&mut self, comment: &internal::Comment) {
        if comment.is_block {
            // Block comment: /* content */
            self.write("/*");

            // Check if multi-line - if so, strip and re-apply indentation
            if comment.content.contains('\n') {
                let stripped = self.strip_comment_indentation(&comment.content, comment.span.start);
                let lines: Vec<&str> = stripped.split('\n').collect();
                for (i, line) in lines.iter().enumerate() {
                    if i > 0 {
                        // Add newline and indentation for subsequent lines
                        self.write("\n");
                        self.write_indent();
                    }
                    self.write(line);
                }
            } else {
                // Single-line block comment
                self.write(&comment.content);
            }

            self.write("*/");
        } else {
            // Line comment: // content (no closing delimiter)
            self.write("//");
            self.write(&comment.content);
        }
    }

    /// Strip leading indentation from multi-line comment content
    ///
    /// Matches Svelte's acorn.js:115-124 behavior
    fn strip_comment_indentation(&self, content: &str, comment_start: u32) -> String {
        let comment_start = comment_start as usize;

        // Find start of line where comment begins
        let mut line_start = comment_start;
        while line_start > 0 && self.source.as_bytes()[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Find the indentation characters (spaces/tabs before the comment)
        let mut indentation_end = line_start;
        while indentation_end < self.source.len() {
            let ch = self.source.as_bytes()[indentation_end];
            if ch == b' ' || ch == b'\t' {
                indentation_end += 1;
            } else {
                break;
            }
        }

        let indentation = &self.source[line_start..indentation_end];

        // Strip this indentation from the start of each line in the comment
        if indentation.is_empty() {
            return content.to_string();
        }

        // Process line by line, stripping indentation from the start of each line
        let mut result = String::with_capacity(content.len());
        let line_iter = content.split_inclusive('\n');

        for line in line_iter {
            if let Some(stripped) = line.strip_prefix(indentation) {
                result.push_str(stripped);
            } else {
                result.push_str(line);
            }
        }

        result
    }

    /// Check if two positions are on the same line
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        let prev_end = prev_end as usize;
        let curr_start = curr_start as usize;

        // Adjacent tokens are on the same line
        if prev_end == curr_start {
            return true;
        }

        if prev_end > curr_start || curr_start > self.source.len() {
            return false;
        }

        let between = &self.source[prev_end..curr_start];
        !between.contains('\n')
    }

    /// Check if there's a blank line between two spans
    ///
    /// Returns true if there are 2+ newlines between the spans
    pub(crate) fn has_blank_line_between_spans(&self, prev_end: u32, curr_start: u32) -> bool {
        let prev_end = prev_end as usize;
        let curr_start = curr_start as usize;

        if prev_end > curr_start || curr_start > self.source.len() {
            return false;
        }

        let between = &self.source[prev_end..curr_start];
        // Blank line = 2+ newlines in the whitespace
        between.matches('\n').count() >= 2
    }

    /// Print a TypeScript program
    pub fn print_program(&mut self, program: &internal::Program) {
        let mut prev_end = 0u32;  // Start of file

        for (i, statement) in program.body.iter().enumerate() {
            // Always add newline between statements (separator)
            if i > 0 {
                self.write("\n");
            }

            // Print leading comments before this statement
            // (blank line preservation handled inside print_leading_comments)
            self.print_leading_comments(prev_end, statement.span().start);

            self.print_statement(statement);

            prev_end = statement.span().end;
        }

        // Add trailing newline (matches prettier)
        self.write("\n");
    }

    /// Print leading comments (comments between prev_end and curr_start)
    /// Returns true if any comments were printed
    fn print_leading_comments(&mut self, prev_end: u32, curr_start: u32) -> bool {
        // TODO: Optimize comment lookup with binary search or range queries
        // Current: O(n) linear scan through all comments for each statement
        // Better: Sort comments by position once, use binary search for range [prev_end, curr_start]
        // Or: Build interval tree / segment tree for O(log n) range queries
        // Impact: Significant for files with 100+ comments (e.g., heavily documented APIs)
        // Note: Comments already collected during parsing, just need efficient retrieval
        let mut last_comment_end = prev_end;
        let mut printed_any = false;

        for comment in self.comments.iter() {
            // Check if comment is between prev and current position
            if comment.span.start >= prev_end && comment.span.end <= curr_start {
                // Skip comments that are on the same line as prev_end
                // (those are trailing inline comments, already printed)
                if self.is_same_line(prev_end, comment.span.start) {
                    continue;
                }

                // Check if comment is on the same line as curr_start
                // (those are same-line leading comments, print inline without newline)
                if self.is_same_line(comment.span.end, curr_start) {
                    self.write_indent();
                    self.print_comment(comment);
                    self.write(" ");
                    printed_any = true;
                    last_comment_end = comment.span.end;
                    continue;
                }

                // Check if we need a blank line before this comment
                if comment.span.start > last_comment_end
                    && self.has_blank_line_between_spans(last_comment_end, comment.span.start)
                {
                    self.write("\n");
                }

                self.write_indent();
                self.print_comment(comment);
                self.write("\n");

                // Update last_comment_end for next iteration
                last_comment_end = comment.span.end;
                printed_any = true;
            }
        }

        // Check if there's a blank line after the last comment and before curr_start
        if printed_any && last_comment_end < curr_start
            && self.has_blank_line_between_spans(last_comment_end, curr_start)
        {
            self.write("\n");
        }

        printed_any
    }

    /// Print inline comments in statement (handles comments before and after semicolon)
    ///
    /// Matches prettier's behavior: comments before semicolon are moved to after it.
    /// Example: `const x = 1 /* comment */;` → `const x = 1; /* comment */`
    pub(crate) fn print_inline_comments_in_statement(&mut self, expr_end: u32, stmt_end: u32) {
        // Collect all inline comments (before and after semicolon)
        let mut has_comments = false;
        for comment in self.comments.iter() {
            // Comments between expression end and statement end (before semicolon)
            // OR comments after statement end on same line (after semicolon)
            if (comment.span.start >= expr_end && comment.span.end <= stmt_end)
                || (comment.span.start >= stmt_end && self.is_same_line(stmt_end, comment.span.start))
            {
                if !has_comments {
                    self.write(" ");
                    has_comments = true;
                }
                self.print_comment(comment);
            }
        }
    }

    /// Find the position of `=` character in the source between two positions
    pub(crate) fn find_equals_position(&self, start: u32, end: u32) -> u32 {
        let start = start as usize;
        let end = end as usize;
        let slice = &self.source[start..end];

        if let Some(offset) = slice.find('=') {
            (start + offset) as u32
        } else {
            // Fallback: return midpoint if `=` not found
            ((start + end) / 2) as u32
        }
    }

    /// Check if there are comments between two positions (read-only check)
    pub(crate) fn has_comments_between(&self, start: u32, end: u32) -> bool {
        self.comments.iter().any(|c| c.span.start >= start && c.span.end <= end)
    }

    /// Print inline comments between two positions (same-line comments only)
    /// Returns true if any comments were printed
    /// Note: Adds space before each comment, but NOT after (caller handles trailing space)
    pub(crate) fn print_inline_comments_between(&mut self, start: u32, end: u32) -> bool {
        let mut printed_any = false;
        for comment in self.comments.iter() {
            if comment.span.start >= start && comment.span.end <= end {
                self.write(" ");
                self.print_comment(comment);
                printed_any = true;
            }
        }
        printed_any
    }
}
