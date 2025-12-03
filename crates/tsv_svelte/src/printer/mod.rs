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
use string_interner::DefaultStringInterner;
use tsv_lang::{Comment, OutputBuffer, PrintConfig, SymbolResolver, comments_in_range};

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
    /// TypeScript comments from scripts and template expressions
    ts_comments: &'a [Comment],
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source, interner, comments, and default config
    pub fn new(
        source: &'a str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        ts_comments: &'a [Comment],
    ) -> Self {
        Self::with_config(source, interner, ts_comments, PrintConfig::default())
    }

    /// Create a new printer with the given source, interner, comments, and config
    pub fn with_config(
        source: &'a str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        ts_comments: &'a [Comment],
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source,
            interner,
            ts_comments,
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

    /// Write indentation based on current indent level
    #[allow(dead_code)]
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, self.config.indent);
    }

    /// Get the formatted output
    pub fn into_string(self) -> String {
        self.buffer.into_string()
    }
}

/// Format a Svelte AST back to source code
pub fn format_svelte(root: &internal::Root, source: &str) -> String {
    let mut printer = Printer::new(source, Rc::clone(&root.interner), &root.ts_comments);
    printer.print_root(root);
    printer.into_string()
}

/// Format a Svelte AST back to source code
pub fn print_svelte(root: &internal::Root, source: &str) -> String {
    let mut printer = Printer::new(source, Rc::clone(&root.interner), &root.ts_comments);
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

        // Format svelte:options (if present) - always first
        if let Some(options) = &root.options {
            self.print_svelte_options(options);
            has_previous_section = true;
        }

        // Format module script (if present)
        if let Some(script) = &root.module {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
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

    /// Format `<svelte:options ... />` tag
    ///
    /// Always outputs self-closing form with attributes.
    fn print_svelte_options(&mut self, options: &internal::SvelteOptions) {
        self.write("<svelte:options");

        // Format attributes
        for attr in &options.attributes {
            self.write(" ");
            self.print_attribute_node(attr);
        }

        self.write(" />\n");
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
        let mut prev_was_text = false;
        let mut prev_was_comment = false;
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
                        // continue to next iteration for whitespace-only nodes
                    } else {
                        // Text with content - analyze whitespace using TextAnalysis trait
                        let text_has_leading_newline = text.raw.has_leading_newline();
                        let text_has_leading_space = text.raw.has_leading_space_only();
                        let text_has_trailing_newline = text.raw.has_trailing_newline();

                        // Upgrade pending whitespace if text has leading newline
                        if text_has_leading_newline {
                            pending_ws = match pending_ws {
                                PendingWhitespace::BlankLine => PendingWhitespace::BlankLine,
                                _ => PendingWhitespace::Newline,
                            };
                        } else if text_has_leading_space && pending_ws == PendingWhitespace::None {
                            // Text has leading space but no newline, upgrade None to Space
                            pending_ws = PendingWhitespace::Space;
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
                        let has_trailing_space = text.raw.has_trailing_space_only();

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
                        prev_was_text = true;
                        prev_was_comment = false;

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
                    let is_component = el.kind == internal::ElementKind::Component;
                    let is_inline_html = !is_block && !is_component;

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
                                // Space before block → newline
                                // Space after comment before component → newline (prettier behavior)
                                // Space after comment before inline HTML → preserve space
                                // Otherwise → preserve space
                                if is_block || (prev_was_comment && !is_inline_html) {
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
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::ExpressionTag(tag) => {
                    // Resolve pending whitespace before expression (treat as inline)
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::None => {}
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => {
                                // Space after comment before expression → newline (prettier behavior)
                                if prev_was_comment {
                                    self.write("\n");
                                } else {
                                    self.write(" ");
                                }
                            }
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                        }
                    }

                    self.print_expression_tag(tag);

                    // Update state
                    has_output_content = true;
                    prev_was_block = false; // Expressions are inline
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::Comment(comment) => {
                    // Resolve pending whitespace before comment (treat as inline)
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::None => {
                                // If previous was block, need newline before comment
                                if prev_was_block {
                                    self.write("\n");
                                }
                            }
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => {
                                // Space before comment after non-text element → newline (prettier behavior)
                                if !prev_was_text {
                                    self.write("\n");
                                } else {
                                    self.write(" ");
                                }
                            }
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                        }
                    }

                    self.print_comment(comment);

                    // Update state
                    has_output_content = true;
                    prev_was_block = false; // Comments are inline
                    prev_was_text = false;
                    prev_was_comment = true;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::IfBlock(block) => {
                    // Control blocks at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_if_block(block);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::EachBlock(block) => {
                    // Control blocks at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_each_block(block);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::AwaitBlock(block) => {
                    // Control blocks at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_await_block(block);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::KeyBlock(block) => {
                    // Control blocks at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_key_block(block);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::SnippetBlock(block) => {
                    // Snippet blocks at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_snippet_block(block);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::HtmlTag(tag) => {
                    // Template tags at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_html_tag(tag);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::ConstTag(tag) => {
                    // Template tags at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_const_tag(tag);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::DebugTag(tag) => {
                    // Template tags at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_debug_tag(tag);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::RenderTag(tag) => {
                    // Template tags at root level: preserve blank lines
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_render_tag(tag);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
                    pending_ws = PendingWhitespace::None;
                }
                FragmentNode::SpecialElement(elem) => {
                    // Special elements at root level: treat like block elements
                    if has_output_content {
                        match pending_ws {
                            PendingWhitespace::BlankLine => self.write("\n\n"),
                            PendingWhitespace::Newline => self.write("\n"),
                            PendingWhitespace::Space => self.write("\n"),
                            PendingWhitespace::None => self.write("\n"),
                            PendingWhitespace::AlreadyHandled => {}
                        }
                    }
                    self.print_special_element(elem);
                    has_output_content = true;
                    prev_was_block = true;
                    prev_was_text = false;
                    prev_was_comment = false;
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
            FragmentNode::Comment(comment) => self.print_comment(comment),
            FragmentNode::IfBlock(block) => self.print_if_block(block),
            FragmentNode::EachBlock(block) => self.print_each_block(block),
            FragmentNode::AwaitBlock(block) => self.print_await_block(block),
            FragmentNode::KeyBlock(block) => self.print_key_block(block),
            FragmentNode::SnippetBlock(block) => self.print_snippet_block(block),
            FragmentNode::HtmlTag(tag) => self.print_html_tag(tag),
            FragmentNode::ConstTag(tag) => self.print_const_tag(tag),
            FragmentNode::DebugTag(tag) => self.print_debug_tag(tag),
            FragmentNode::RenderTag(tag) => self.print_render_tag(tag),
            FragmentNode::SpecialElement(elem) => self.print_special_element(elem),
        }
    }

    /// Format an HTML comment: <!-- content -->
    fn print_comment(&mut self, comment: &internal::HtmlComment) {
        self.write("<!--");
        self.write(&comment.content);
        self.write("-->");
    }

    /// Check if a fragment's content is inline (no newlines in source)
    fn is_inline_fragment(&self, fragment: &internal::Fragment) -> bool {
        let (Some(first), Some(last)) = (fragment.nodes.first(), fragment.nodes.last()) else {
            return true;
        };
        let first_start = first.span().start as usize;
        let last_end = last.span().end as usize;
        let content = &self.source[first_start..last_end];
        !content.contains('\n')
    }

    /// Format an if block: {#if test}...{:else}...{/if}
    #[allow(clippy::literal_string_with_formatting_args)]
    fn print_if_block(&mut self, block: &internal::IfBlock) {
        self.write("{#if ");
        let formatted =
            tsv_ts::format_expression(&block.test, self.source, Rc::clone(&self.interner));
        self.write(&formatted);
        self.write("}");

        let is_inline = self.is_inline_fragment(&block.consequent);

        if is_inline {
            self.print_inline_children(&block.consequent);
        } else {
            self.print_block_children(&block.consequent);
        }

        if let Some(alt) = &block.alternate {
            // Check if alternate is an else-if or plain else
            if let Some(FragmentNode::IfBlock(else_if)) = alt.nodes.first() {
                if else_if.elseif {
                    if !is_inline {
                        self.write("\n");
                        self.write_indent();
                    }
                    self.write("{:else if ");
                    let formatted = tsv_ts::format_expression(
                        &else_if.test,
                        self.source,
                        Rc::clone(&self.interner),
                    );
                    self.write(&formatted);
                    self.write("}");

                    let else_if_inline = self.is_inline_fragment(&else_if.consequent);
                    if else_if_inline {
                        self.print_inline_children(&else_if.consequent);
                    } else {
                        self.print_block_children(&else_if.consequent);
                    }

                    if let Some(nested_alt) = &else_if.alternate {
                        self.print_if_alternate(nested_alt, else_if_inline);
                    }
                } else {
                    if !is_inline {
                        self.write("\n");
                        self.write_indent();
                    }
                    self.write("{:else}");
                    let alt_inline = self.is_inline_fragment(alt);
                    if alt_inline {
                        self.print_inline_children(alt);
                    } else {
                        self.print_block_children(alt);
                    }
                }
            } else {
                if !is_inline {
                    self.write("\n");
                    self.write_indent();
                }
                self.write("{:else}");
                let alt_inline = self.is_inline_fragment(alt);
                if alt_inline {
                    self.print_inline_children(alt);
                } else {
                    self.print_block_children(alt);
                }
            }
        }

        if is_inline {
            self.write("{/if}");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("{/if}");
        }
    }

    /// Print the alternate branch of an if block (recursive helper)
    #[allow(clippy::literal_string_with_formatting_args)]
    fn print_if_alternate(&mut self, alt: &internal::Fragment, parent_is_inline: bool) {
        if let Some(FragmentNode::IfBlock(else_if)) = alt.nodes.first() {
            if else_if.elseif {
                if !parent_is_inline {
                    self.write("\n");
                    self.write_indent();
                }
                self.write("{:else if ");
                let formatted = tsv_ts::format_expression(
                    &else_if.test,
                    self.source,
                    Rc::clone(&self.interner),
                );
                self.write(&formatted);
                self.write("}");

                let is_inline = self.is_inline_fragment(&else_if.consequent);
                if is_inline {
                    self.print_inline_children(&else_if.consequent);
                } else {
                    self.print_block_children(&else_if.consequent);
                }

                if let Some(nested_alt) = &else_if.alternate {
                    self.print_if_alternate(nested_alt, is_inline);
                }
            } else {
                if !parent_is_inline {
                    self.write("\n");
                    self.write_indent();
                }
                self.write("{:else}");
                let is_inline = self.is_inline_fragment(alt);
                if is_inline {
                    self.print_inline_children(alt);
                } else {
                    self.print_block_children(alt);
                }
            }
        } else {
            if !parent_is_inline {
                self.write("\n");
                self.write_indent();
            }
            self.write("{:else}");
            let is_inline = self.is_inline_fragment(alt);
            if is_inline {
                self.print_inline_children(alt);
            } else {
                self.print_block_children(alt);
            }
        }
    }

    /// Format an each block: {#each expr as item}...{/each}
    #[allow(clippy::literal_string_with_formatting_args)]
    fn print_each_block(&mut self, block: &internal::EachBlock) {
        self.write("{#each ");
        self.print_ts_expression(&block.expression);

        if let Some(context) = &block.context {
            // Has `as` clause: {#each expr as context, index (key)}
            self.write(" as ");
            self.print_ts_pattern(context);
            if let Some(idx) = &block.index {
                self.write(", ");
                self.write(idx);
            }
            if let Some(key) = &block.key {
                self.write(" (");
                self.print_ts_expression(key);
                self.write(")");
            }
        } else if let Some(idx) = &block.index {
            // No `as` clause but has index: {#each expr, index}
            self.write(", ");
            self.write(idx);
        }
        // else: just {#each expr}

        self.write("}");

        let is_inline = self.is_inline_fragment(&block.body);
        if is_inline {
            self.print_inline_children(&block.body);
        } else {
            self.print_block_children(&block.body);
        }

        if let Some(fallback) = &block.fallback {
            if !is_inline {
                self.write("\n");
                self.write_indent();
            }
            self.write("{:else}");
            let fallback_inline = self.is_inline_fragment(fallback);
            if fallback_inline {
                self.print_inline_children(fallback);
            } else {
                self.print_block_children(fallback);
            }
        }

        if is_inline {
            self.write("{/each}");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("{/each}");
        }
    }

    /// Format an await block: {#await expr}...{:then}...{:catch}...{/await}
    fn print_await_block(&mut self, block: &internal::AwaitBlock) {
        // Check for shorthand syntax: {#await promise then value}
        let is_shorthand = block.pending.is_none() && block.value.is_some();

        self.write("{#await ");
        self.write(&self.source[block.expression.span().range()]);

        // Determine if content is inline based on the main content fragment
        let main_fragment = if is_shorthand {
            block.then.as_ref()
        } else {
            block.pending.as_ref()
        };
        let is_inline = main_fragment.is_none_or(|f| self.is_inline_fragment(f));

        if is_shorthand {
            if let Some(value) = &block.value {
                self.write(" then ");
                self.write(&self.source[value.span().range()]);
            }
            self.write("}");
            if let Some(then_block) = &block.then {
                if is_inline {
                    self.print_inline_children(then_block);
                } else {
                    self.print_block_children(then_block);
                }
            }
        } else {
            self.write("}");
            if let Some(pending) = &block.pending {
                if is_inline {
                    self.print_inline_children(pending);
                } else {
                    self.print_block_children(pending);
                }
            }
            if let Some(then_block) = &block.then {
                if !is_inline {
                    self.write("\n");
                    self.write_indent();
                }
                self.write("{:then");
                if let Some(value) = &block.value {
                    self.write(" ");
                    self.write(&self.source[value.span().range()]);
                }
                self.write("}");
                let then_inline = self.is_inline_fragment(then_block);
                if then_inline {
                    self.print_inline_children(then_block);
                } else {
                    self.print_block_children(then_block);
                }
            }
            if let Some(catch_block) = &block.catch {
                if !is_inline {
                    self.write("\n");
                    self.write_indent();
                }
                self.write("{:catch");
                if let Some(error) = &block.error {
                    self.write(" ");
                    self.write(&self.source[error.span().range()]);
                }
                self.write("}");
                let catch_inline = self.is_inline_fragment(catch_block);
                if catch_inline {
                    self.print_inline_children(catch_block);
                } else {
                    self.print_block_children(catch_block);
                }
            }
        }

        if is_inline {
            self.write("{/await}");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("{/await}");
        }
    }

    /// Format a key block: {#key expr}...{/key}
    fn print_key_block(&mut self, block: &internal::KeyBlock) {
        self.write("{#key ");
        self.write(&self.source[block.expression.span().range()]);
        self.write("}");

        let is_inline = self.is_inline_fragment(&block.fragment);
        if is_inline {
            self.print_inline_children(&block.fragment);
        } else {
            self.print_block_children(&block.fragment);
        }

        if is_inline {
            self.write("{/key}");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("{/key}");
        }
    }

    /// Format a snippet block: {#snippet name(params)}...{/snippet}
    fn print_snippet_block(&mut self, block: &internal::SnippetBlock) {
        self.write("{#snippet ");
        self.write(&self.source[block.expression.span().range()]);
        // Emit type parameters if present (e.g., <T> for generics)
        if let Some(ref type_params) = block.type_parameters {
            self.write("<");
            self.write(type_params);
            self.write(">");
        }
        self.write("(");
        // Use raw_parameters if present (TypeScript with type annotations)
        // Otherwise format parsed parameters
        if let Some(ref raw_params) = block.raw_parameters {
            self.write(raw_params);
        } else {
            for (i, param) in block.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                // Format parameter expressions (handles defaults and destructuring)
                let formatted =
                    tsv_ts::format_expression(param, self.source, Rc::clone(&self.interner));
                self.write(&formatted);
            }
        }
        self.write(")}");

        let is_inline = self.is_inline_fragment(&block.body);
        if is_inline {
            self.print_inline_children(&block.body);
        } else {
            self.print_block_children(&block.body);
        }

        if is_inline {
            self.write("{/snippet}");
        } else {
            self.write("\n");
            self.write_indent();
            self.write("{/snippet}");
        }
    }

    /// Format an html tag: {@html expr}
    fn print_html_tag(&mut self, tag: &internal::HtmlTag) {
        self.write("{@html ");
        let formatted =
            tsv_ts::format_expression(&tag.expression, self.source, Rc::clone(&self.interner));
        self.write(&formatted);
        self.write("}");
    }

    /// Format a const tag: {@const name = expr}
    fn print_const_tag(&mut self, tag: &internal::ConstTag) {
        self.write("{@const ");
        // Format the id (pattern) with current indent level for multiline patterns
        let formatted_id = tsv_ts::format_expression_with_indent(
            &tag.id,
            self.source,
            Rc::clone(&self.interner),
            self.indent_level,
        );
        self.write(&formatted_id);
        self.write(" = ");
        // Format the init expression
        let formatted_init = tsv_ts::format_expression_with_indent(
            &tag.init,
            self.source,
            Rc::clone(&self.interner),
            self.indent_level,
        );
        self.write(&formatted_init);
        self.write("}");
    }

    /// Format a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Unlike Prettier (which strips comments), we preserve TS comments.
    /// Comments are looked up from Root.ts_comments by span position.
    fn print_debug_tag(&mut self, tag: &internal::DebugTag) {
        self.write("{@debug");

        // Get comments within the tag's content (after "{@debug" and before "}")
        // The tag span includes the full `{@debug ... }`, so we look inside
        let tag_comments: Vec<_> =
            comments_in_range(self.ts_comments, tag.span.start, tag.span.end).collect();

        if tag.identifiers.is_empty() && tag_comments.is_empty() {
            // Just {@debug} with no identifiers or comments
            self.write("}");
            return;
        }

        self.write(" ");

        // Track position as we emit content
        // Start after "{@debug " (7 characters from tag start)
        let mut last_end = tag.span.start + 7; // "{@debug" = 7 chars

        for (i, id) in tag.identifiers.iter().enumerate() {
            if i > 0 {
                self.write(", ");
                last_end += 2; // ", "
            }

            // Emit any comments that appear before this identifier
            for comment in &tag_comments {
                if comment.span.start >= last_end && comment.span.end <= id.span().start {
                    if comment.is_block {
                        self.write("/*");
                        self.write(&comment.content);
                        self.write("*/ ");
                    } else {
                        self.write("//");
                        self.write(&comment.content);
                        self.write("\n");
                    }
                    last_end = comment.span.end;
                }
            }

            let formatted = tsv_ts::format_expression(id, self.source, Rc::clone(&self.interner));
            self.write(&formatted);
            last_end = id.span().end;
        }

        // Emit any trailing comments (after last identifier)
        for comment in &tag_comments {
            if comment.span.start >= last_end {
                self.write(" ");
                if comment.is_block {
                    self.write("/*");
                    self.write(&comment.content);
                    self.write("*/");
                } else {
                    self.write("//");
                    self.write(&comment.content);
                }
            }
        }

        self.write("}");
    }

    /// Format a render tag: {@render fn()} or {@render fn?.()}
    fn print_render_tag(&mut self, tag: &internal::RenderTag) {
        self.write("{@render ");
        let formatted =
            tsv_ts::format_expression(&tag.expression, self.source, Rc::clone(&self.interner));
        self.write(&formatted);
        self.write("}");
    }

    /// Print children inline (no newlines added)
    fn print_inline_children(&mut self, fragment: &internal::Fragment) {
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    // For inline, preserve trimmed text
                    let trimmed = text.raw.trim();
                    if !trimmed.is_empty() {
                        self.write(trimmed);
                    }
                }
                _ => {
                    self.print_fragment_node(node, false, false);
                }
            }
        }
    }

    /// Helper to print children of a block with proper indentation
    fn print_block_children(&mut self, fragment: &internal::Fragment) {
        // For now, just print each child on a new line with indentation
        if fragment.nodes.is_empty() {
            return;
        }
        self.indent_level += 1;
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    if !text.raw.trim().is_empty() {
                        self.write("\n");
                        self.write_indent();
                        self.write(text.raw.trim());
                    }
                }
                _ => {
                    self.write("\n");
                    self.write_indent();
                    self.print_fragment_node(node, true, false);
                }
            }
        }
        self.indent_level -= 1;
    }
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
        &self.interner
    }
}
