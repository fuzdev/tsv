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
mod blocks;
mod classification;
mod helpers;
mod nodes;
mod script_style;
mod tags;
mod text;

use self::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::DefaultStringInterner;
use tsv_lang::{Comment, OutputBuffer, PrintConfig, SymbolResolver};

/// Pending whitespace state - buffers whitespace decisions until next node is known
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PendingWhitespace {
    #[default]
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

impl PendingWhitespace {
    /// Upgrade whitespace level (never downgrades)
    fn upgrade(&mut self, other: PendingWhitespace) {
        use PendingWhitespace::{BlankLine, Newline, Space};
        *self = match (*self, other) {
            (BlankLine, _) => BlankLine,
            (_, BlankLine) => BlankLine,
            (Newline, _) => Newline,
            (_, Newline) => Newline,
            (Space, _) => Space,
            (_, Space) => Space,
            _ => *self,
        };
    }

    /// Resolve pending whitespace for block-like nodes (always newline or better)
    fn resolve_for_block(self) -> &'static str {
        match self {
            PendingWhitespace::BlankLine => "\n\n",
            PendingWhitespace::AlreadyHandled => "",
            _ => "\n", // Newline, Space, None all become newline for blocks
        }
    }
}

/// What kind of node was previously printed at root level
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum PrevNodeKind {
    #[default]
    None,
    Block,
    Inline,
    Text,
    Comment,
}

/// State tracker for root-level fragment printing
#[derive(Default)]
struct RootPrintState {
    prev_kind: PrevNodeKind,
    has_output_content: bool,
    pending_ws: PendingWhitespace,
}

impl RootPrintState {
    /// Mark that a block-like node was just printed
    fn after_block(&mut self) {
        self.has_output_content = true;
        self.prev_kind = PrevNodeKind::Block;
        self.pending_ws = PendingWhitespace::None;
    }

    /// Mark that an inline node was just printed
    fn after_inline(&mut self) {
        self.has_output_content = true;
        self.prev_kind = PrevNodeKind::Inline;
        self.pending_ws = PendingWhitespace::None;
    }

    /// Mark that a text node was just printed
    fn after_text(&mut self, trailing_ws: PendingWhitespace) {
        self.has_output_content = true;
        self.prev_kind = PrevNodeKind::Text;
        self.pending_ws = trailing_ws;
    }

    /// Mark that a comment was just printed
    fn after_comment(&mut self) {
        self.has_output_content = true;
        self.prev_kind = PrevNodeKind::Comment;
        self.pending_ws = PendingWhitespace::None;
    }
}

/// Printer state for building output
pub struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Print configuration
    config: PrintConfig,
    /// Source code (needed for preserving whitespace semantics)
    pub(crate) source: &'a str,
    /// Shared string interner for resolving symbols
    interner: Rc<RefCell<DefaultStringInterner>>,
    /// Comments from scripts and template expressions
    comments: &'a [Comment],
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source, interner, comments, and default config
    pub fn new(
        source: &'a str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        comments: &'a [Comment],
    ) -> Self {
        Self::with_config(source, interner, comments, PrintConfig::default())
    }

    /// Create a new printer with the given source, interner, comments, and config
    pub fn with_config(
        source: &'a str,
        interner: Rc<RefCell<DefaultStringInterner>>,
        comments: &'a [Comment],
        config: PrintConfig,
    ) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            indent_level: 0,
            config,
            source,
            interner,
            comments,
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Get the source code
    pub(crate) fn source(&self) -> &str {
        self.source
    }

    /// Write indentation based on current indent level
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
    let mut printer = Printer::new(source, Rc::clone(&root.interner), &root.comments);
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

        // Find the earliest script position to determine which comments come before scripts
        let first_script_start = [root.module.as_ref(), root.instance.as_ref()]
            .into_iter()
            .flatten()
            .map(|s| s.span.start)
            .min();

        // Track which comments we've printed before scripts
        let mut printed_comment_indices: Vec<usize> = Vec::new();

        // Format svelte:options (if present) - always first
        if let Some(options) = &root.options {
            self.print_svelte_options(options);
            has_previous_section = true;
        }

        // Print comments that come before the first script
        // These are attached to the script, so no blank line between them
        if let Some(script_start) = first_script_start {
            for (i, node) in root.fragment.nodes.iter().enumerate() {
                if let FragmentNode::Comment(comment) = node
                    && comment.span.end <= script_start
                {
                    if has_previous_section {
                        self.write("\n");
                    }
                    self.print_comment(comment);
                    self.write("\n");
                    // Don't set has_previous_section - comment is attached to script
                    printed_comment_indices.push(i);
                }
            }
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
        // Check if there are any non-whitespace nodes (excluding already-printed comments)
        let has_content = root.fragment.nodes.iter().enumerate().any(|(i, node)| {
            if printed_comment_indices.contains(&i) {
                return false;
            }
            !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())
        });

        if has_content {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_root_fragment_filtered(&root.fragment, &printed_comment_indices);
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
    ///
    /// The `skip_indices` parameter allows skipping nodes that were already printed
    /// (e.g., comments that appear before scripts are printed with the script section)
    fn print_root_fragment_filtered(
        &mut self,
        fragment: &internal::Fragment,
        skip_indices: &[usize],
    ) {
        let mut state = RootPrintState::default();

        // Find first non-whitespace node index (excluding skipped indices)
        let first_non_ws_idx = fragment.nodes.iter().enumerate().position(|(i, node)| {
            !skip_indices.contains(&i)
                && !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())
        });

        for (i, node) in fragment.nodes.iter().enumerate() {
            // Skip indices that were already printed (e.g., comments before script)
            if skip_indices.contains(&i) {
                continue;
            }
            match node {
                FragmentNode::Text(text) => {
                    // Skip leading whitespace-only text nodes at root level
                    if Some(i) < first_non_ws_idx {
                        continue;
                    }

                    // Check if this is a whitespace-only node
                    if text.raw.is_whitespace_only() {
                        // Buffer whitespace type - use upgrade semantics (never downgrade)
                        let ws_type = if text.raw.has_blank_line() {
                            PendingWhitespace::BlankLine
                        } else if text.raw.contains('\n') {
                            PendingWhitespace::Newline
                        } else {
                            PendingWhitespace::Space
                        };
                        state.pending_ws.upgrade(ws_type);
                        // Whitespace-only text clears block status so subsequent inline doesn't get newline
                        if state.prev_kind == PrevNodeKind::Block {
                            state.prev_kind = PrevNodeKind::Inline;
                        }
                        // continue to next iteration for whitespace-only nodes
                    } else {
                        // Text with content - analyze whitespace using TextAnalysis trait
                        let text_has_leading_newline = text.raw.has_leading_newline();
                        let text_has_leading_space = text.raw.has_leading_space_only();
                        let text_has_trailing_newline = text.raw.has_trailing_newline();

                        // Upgrade pending whitespace based on text's leading whitespace
                        if text_has_leading_newline {
                            state.pending_ws.upgrade(PendingWhitespace::Newline);
                        } else if text_has_leading_space {
                            state.pending_ws.upgrade(PendingWhitespace::Space);
                        }

                        // Resolve pending whitespace before outputting text
                        if state.has_output_content {
                            match state.pending_ws {
                                PendingWhitespace::None => {
                                    // No explicit whitespace, but add newline if previous was block
                                    if state.prev_kind == PrevNodeKind::Block {
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

                        // Set pending whitespace for next node based on trailing whitespace
                        let trailing_ws = if has_trailing_space {
                            PendingWhitespace::AlreadyHandled
                        } else if text_has_trailing_newline {
                            PendingWhitespace::Newline
                        } else {
                            PendingWhitespace::None
                        };
                        state.after_text(trailing_ws);
                    }
                }
                FragmentNode::Element(el) => {
                    let is_block = self.is_block_element(el);
                    let is_component = el.kind == internal::ElementKind::Component;
                    let is_inline_html = !is_block && !is_component;

                    // Resolve pending whitespace before element
                    if state.has_output_content {
                        match state.pending_ws {
                            PendingWhitespace::None => {
                                // No explicit whitespace
                                // Blocks always need newlines
                                // Inline elements need newlines if previous was block
                                if is_block || state.prev_kind == PrevNodeKind::Block {
                                    self.write("\n");
                                }
                            }
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => {
                                // Space before block → newline
                                // Space after comment before component → newline (prettier behavior)
                                // Otherwise → preserve space
                                if is_block
                                    || (state.prev_kind == PrevNodeKind::Comment && !is_inline_html)
                                {
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

                    // Update state - elements need special handling for is_block
                    state.has_output_content = true;
                    state.prev_kind = if is_block {
                        PrevNodeKind::Block
                    } else {
                        PrevNodeKind::Inline
                    };
                    state.pending_ws = PendingWhitespace::None;
                }
                FragmentNode::ExpressionTag(tag) => {
                    // Resolve pending whitespace before expression (treat as inline)
                    if state.has_output_content {
                        match state.pending_ws {
                            PendingWhitespace::None => {}
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => {
                                // Space after comment before expression → newline (prettier behavior)
                                if state.prev_kind == PrevNodeKind::Comment {
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
                    state.after_inline();
                }
                FragmentNode::Comment(comment) => {
                    // Resolve pending whitespace before comment (treat as inline)
                    if state.has_output_content {
                        match state.pending_ws {
                            PendingWhitespace::None => {
                                // If previous was block, need newline before comment
                                if state.prev_kind == PrevNodeKind::Block {
                                    self.write("\n");
                                }
                            }
                            PendingWhitespace::AlreadyHandled => {}
                            PendingWhitespace::Space => {
                                // Space before comment after non-text element → newline (prettier behavior)
                                if state.prev_kind != PrevNodeKind::Text {
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
                    state.after_comment();
                }
                // Block-like nodes: control blocks, tags, special elements
                // All use the same pattern: newline/blank line before, treated as block after
                FragmentNode::IfBlock(block) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_if_block(block);
                    state.after_block();
                }
                FragmentNode::EachBlock(block) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_each_block(block);
                    state.after_block();
                }
                FragmentNode::AwaitBlock(block) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_await_block(block);
                    state.after_block();
                }
                FragmentNode::KeyBlock(block) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_key_block(block);
                    state.after_block();
                }
                FragmentNode::SnippetBlock(block) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_snippet_block(block);
                    state.after_block();
                }
                FragmentNode::HtmlTag(tag) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_html_tag(tag);
                    state.after_block();
                }
                FragmentNode::ConstTag(tag) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_const_tag(tag);
                    state.after_block();
                }
                FragmentNode::DebugTag(tag) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_debug_tag(tag);
                    state.after_block();
                }
                FragmentNode::RenderTag(tag) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_render_tag(tag);
                    state.after_block();
                }
                FragmentNode::SpecialElement(elem) => {
                    if state.has_output_content {
                        self.write(state.pending_ws.resolve_for_block());
                    }
                    self.print_special_element(elem);
                    state.after_block();
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
}

// Implement SymbolResolver trait for shared symbol resolution utilities
impl<'a> SymbolResolver for Printer<'a> {
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
        &self.interner
    }
}
