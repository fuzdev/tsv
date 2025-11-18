// Svelte printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and root-level printing orchestration
// - **nodes/**: Node-specific printing (elements, expressions, control flow, etc.)
// - **text.rs**: Text content and whitespace normalization
// - **script_style.rs**: Script and style section printing
// - **attributes.rs**: HTML attribute and directive printing
// - **classification/**: HTML element classification adapters
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier-plugin-svelte for compatibility
// 2. **Preserve Semantics**: Never change HTML whitespace rendering semantics
// 3. **Source Layout**: Preserve authorial intent via inline run grouping
// 4. **Modularity**: Each module has single responsibility for future maintainability

mod attributes;
mod classification;
mod nodes;
mod script_style;
mod text;

use self::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_lang::OutputBuffer;

/// Pending whitespace state - buffers whitespace decisions until next node is known
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingWhitespace {
    /// No pending whitespace
    None,
    /// Whitespace already handled by previous node (e.g., text with trailing space)
    /// Don't add any additional spacing
    AlreadyHandled,
    /// Space(s) detected in source (no newlines)
    /// Will output as space before inline elements, newline before blocks
    Space,
    /// Single newline detected in source
    /// Will output as newline before any element
    Newline,
    /// Blank line (2+ newlines) detected in source
    /// Will output as double newline before any element
    BlankLine,
}

/// Print configuration
#[derive(Debug, Clone)]
pub struct PrintConfig {
    /// Indent string (default: tabs)
    #[allow(dead_code)]
    pub indent: &'static str,
    /// Maximum line width (default: 100)
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
    /// Source code (needed for preserving whitespace semantics)
    pub(crate) source: &'a str,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source, interner, and default config
    pub fn new(source: &'a str, interner: Rc<RefCell<DefaultStringInterner>>) -> Self {
        Self::with_config(source, interner, PrintConfig::default())
    }

    /// Create a new printer with the given source, interner, and config
    pub fn with_config(
        source: &'a str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source,
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
        self.source
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

/// Format a Svelte AST back to source code
pub fn format_svelte(root: &internal::Root, source: &str) -> String {
    let mut printer = Printer::new(source, root.interner.clone());
    printer.print_root(root);
    printer.into_string()
}

/// Format a Svelte AST back to source code
pub fn print_svelte(root: &internal::Root, source: &str) -> String {
    let mut printer = Printer::new(source, root.interner.clone());
    printer.print_root(root);
    printer.into_string()
}

impl<'a> Printer<'a> {
    /// Format a Svelte Root node
    ///
    /// Orchestrates formatting of the four main sections of a .svelte file:
    /// 1. Module script: `<script context="module">`
    /// 2. Instance script: `<script>`
    /// 3. Template: The HTML/Svelte template
    /// 4. Style: `<style>`
    ///
    /// Sections are ordered canonically and separated by blank lines.
    pub fn print_root(&mut self, root: &internal::Root) {
        let mut has_previous_section = false;

        // Format module script (if present)
        if let Some(script) = &root.module {
            self.print_script(script);
            has_previous_section = true;
        }

        // Format instance script (if present)
        if let Some(script) = &root.instance {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_script(script);
            has_previous_section = true;
        }

        // Format template fragment (if not empty)
        // Check if there are any non-whitespace nodes
        let has_content =
            root.fragment.nodes.iter().any(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            );

        if has_content {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_root_fragment(&root.fragment);
            self.write("\n"); // Template needs explicit newline
            has_previous_section = true;
        }

        // Format style (if present)
        if let Some(style) = &root.css {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_style(style);
        }
    }

    /// Format a Fragment with blank lines between root-level block elements
    ///
    /// Root-level formatting has special rules:
    /// - Blank lines preserved from source (authorial intent for logical grouping)
    /// - Multiple blank lines collapse to single blank line
    /// - Whitespace between inline elements is preserved (INCLUDING newlines)
    /// - Format preservation: inline stays inline, multiline stays multiline
    /// - Leading/trailing whitespace-only nodes are removed
    fn print_root_fragment(&mut self, fragment: &internal::Fragment) {
        let mut prev_was_block = false;
        let mut has_output_content = false;
        let mut pending_ws = PendingWhitespace::None;

        // Find first non-whitespace node index
        let first_non_ws_idx = fragment.nodes.iter().position(
            |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
        );

        for (i, node) in fragment.nodes.iter().enumerate() {
            match node {
                FragmentNode::Text(text) => {
                    // Skip leading whitespace-only text nodes at root level
                    if Some(i) < first_non_ws_idx {
                        continue;
                    }

                    // Check if this is a whitespace-only node
                    if text.raw.is_whitespace_only() {
                        // Buffer whitespace type - don't output yet, let next node decide
                        // Use upgrade semantics: don't overwrite blank lines with lesser whitespace
                        if text.raw.has_blank_line() {
                            pending_ws = PendingWhitespace::BlankLine;
                        } else if text.raw.contains('\n') {
                            // Only upgrade to Newline if not already BlankLine
                            if pending_ws != PendingWhitespace::BlankLine {
                                pending_ws = PendingWhitespace::Newline;
                            }
                        } else {
                            // Space only upgrades from None
                            if pending_ws == PendingWhitespace::None {
                                pending_ws = PendingWhitespace::Space;
                            }
                        }

                        prev_was_block = false;
                        continue;
                    } else {
                        // Text with content
                        // Check if text itself has leading whitespace with newlines
                        let text_has_leading_newline = {
                            let trimmed = text.raw.trim_start();
                            if trimmed.len() < text.raw.len() {
                                let leading = &text.raw[..text.raw.len() - trimmed.len()];
                                leading.contains('\n')
                            } else {
                                false
                            }
                        };

                        // Check if text has trailing newline (determines if next element should be on new line)
                        let text_has_trailing_newline = {
                            let trimmed = text.raw.trim_end();
                            if trimmed.len() < text.raw.len() {
                                let trailing = &text.raw[trimmed.len()..];
                                trailing.contains('\n')
                            } else {
                                false
                            }
                        };

                        // Upgrade pending whitespace if text has leading newline
                        if text_has_leading_newline {
                            pending_ws = match pending_ws {
                                PendingWhitespace::BlankLine => PendingWhitespace::BlankLine,
                                _ => PendingWhitespace::Newline,
                            };
                        }

                        // Resolve pending whitespace before outputting text
                        if has_output_content {
                            match pending_ws {
                                PendingWhitespace::None => {
                                    // No explicit whitespace, but add newline if previous was block
                                    if prev_was_block {
                                        self.write("\n");
                                    }
                                }
                                PendingWhitespace::AlreadyHandled => {}
                                PendingWhitespace::Space => self.write(" "),
                                PendingWhitespace::Newline => self.write("\n"),
                                PendingWhitespace::BlankLine => self.write("\n\n"),
                            }
                        }

                        // Check if text has trailing space (not newline) - those are semantic
                        let has_trailing_space = {
                            let trimmed = text.raw.trim_end();
                            if trimmed.len() < text.raw.len() {
                                let trailing = &text.raw[trimmed.len()..];
                                trailing.chars().any(|c| c == ' ' || c == '\t')
                                    && !trailing.contains('\n')
                            } else {
                                false
                            }
                        };

                        // Root-level text normalization: always trim leading, preserve internal spaces,
                        // preserve trailing space only if it's not a newline
                        let normalized = {
                            let mut result = self.normalize_whitespace(&text.raw, true); // Trim completely first
                            if has_trailing_space {
                                result.push(' '); // Add back trailing space (semantic)
                            }
                            result
                        };
                        self.write(&normalized);

                        // Update state
                        has_output_content = true;
                        prev_was_block = false;

                        // Set pending whitespace for next node based on trailing whitespace
                        // If text has trailing space, space is already output - don't add newline before blocks
                        // If text has trailing newline, next element should be on new line
                        pending_ws = if has_trailing_space {
                            PendingWhitespace::AlreadyHandled // Space already output, block should stay on same line
                        } else if text_has_trailing_newline {
                            PendingWhitespace::Newline
                        } else {
                            PendingWhitespace::None
                        };
                    }
                }
                FragmentNode::Element(el) => {
                    let is_block = self.is_block_element(el);

                    // Resolve pending whitespace before element
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::None => {
                                // No explicit whitespace
                                // Blocks always need newlines
                                // Inline elements need newlines if previous was block
                                if is_block || prev_was_block {
                                    self.write("\n");
                                }
                            }
                            PendingWhitespace::AlreadyHandled => {
                                // Previous node already handled spacing (e.g., text with trailing space)
                                // Don't add any additional spacing
                            }
                            PendingWhitespace::Space => {
                                // Space before block → newline, space before inline → preserve
                                if is_block {
                                    self.write("\n");
                                } else {
                                    self.write(" ");
                                }
                            }
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                        }
                    }

                    self.print_element(el);

                    // Update state
                    has_output_content = true;
                    prev_was_block = is_block;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::ExpressionTag(tag) => {
                    // Resolve pending whitespace before expression (treat as inline)
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::None => {}
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => self.write(" "), // Expressions are inline
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                        }
                    }

                    self.print_expression_tag(tag);

                    // Update state
                    has_output_content = true;
                    prev_was_block = false; // Expressions are inline
                    pending_ws = PendingWhitespace::None;
                }
            }
        }
    }

    /// Format a single fragment node (dispatch to specific type)
    ///
    /// # Parameters
    /// - `parent_is_block`: Whether the parent element is a block element (affects text trimming)
    /// - `parent_preserves_ws`: Whether the parent element preserves whitespace (like `<pre>`)
    pub(crate) fn print_fragment_node(
        &mut self,
        node: &FragmentNode,
        parent_is_block: bool,
        parent_preserves_ws: bool,
    ) {
        match node {
            FragmentNode::Element(element) => self.print_element(element),
            FragmentNode::Text(text) => self.print_text(text, parent_is_block, parent_preserves_ws),
            FragmentNode::ExpressionTag(tag) => self.print_expression_tag(tag),
        }
    }
}
