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
use std::rc::Rc;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{
    Comment, EmbedContext, INDENT, OutputBuffer, SharedInterner, SymbolResolver, TAB_WIDTH,
};

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

/// Which section a fragment comment should travel with during canonical reordering.
/// Comments attach to the nearest section that follows them in source order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CommentSection {
    Options,
    ModuleScript,
    InstanceScript,
    Template,
    Style,
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
pub(crate) struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (layout mode, offsets)
    embed: EmbedContext,
    /// Arena allocator for doc nodes
    pub(crate) arena: DocArena,
    /// Source code (needed for preserving whitespace semantics)
    pub(crate) source: &'a str,
    /// Shared string interner for resolving symbols
    interner: SharedInterner,
    /// Comments from scripts and template expressions
    comments: &'a [Comment],
    /// Precomputed line break positions (byte offsets of '\n' in source)
    line_breaks: Vec<u32>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source, interner, and comments (standalone layout).
    pub(crate) fn new(source: &'a str, interner: SharedInterner, comments: &'a [Comment]) -> Self {
        Self::with_embed(source, interner, comments, EmbedContext::default())
    }

    /// Create a new printer with the given source, interner, comments, and embed context.
    pub(crate) fn with_embed(
        source: &'a str,
        interner: SharedInterner,
        comments: &'a [Comment],
        embed: EmbedContext,
    ) -> Self {
        let line_breaks = tsv_lang::printing::build_line_breaks(source);
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            embed,
            arena: DocArena::for_source(source),
            source,
            interner,
            comments,
            line_breaks,
        }
    }

    /// Get a reference to the doc arena (convenience for `&self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        &self.arena
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Get the source code
    pub(crate) fn source(&self) -> &str {
        self.source
    }

    /// Standard [`tsv_ts::PrinterInputs`] for embedding TypeScript: this
    /// document's source, interner, comments, and line breaks. Call sites
    /// needing empty comments override via
    /// `PrinterInputs { comments: &[], ..self.ts_inputs() }`.
    pub(crate) fn ts_inputs(&self) -> tsv_ts::PrinterInputs<'_> {
        tsv_ts::PrinterInputs {
            source: self.source,
            interner: Rc::clone(&self.interner),
            comments: self.comments,
            line_breaks: &self.line_breaks,
        }
    }

    /// Write indentation based on current indent level
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, INDENT);
    }

    /// Get the formatted output
    ///
    /// Simply extracts the buffer. Whitespace stripping is handled by the doc rendering layer:
    /// - Normal elements: rendered with `print_doc_with_indent_resolved()` which strips
    /// - Whitespace-sensitive elements: rendered with `print_doc_with_indent_resolved_preserve_whitespace()` which preserves
    pub(crate) fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Render a DocId immediately at current buffer position
    ///
    /// This is the foundation for doc-first formatting. Instead of using
    /// imperative printing, callers build a Doc and render it in one step.
    ///
    /// The doc is rendered starting at the current column position with
    /// the current indent level, so it seamlessly integrates with any
    /// preceding output.
    ///
    /// Always uses the preserve-whitespace variant because the doc tree may contain
    /// whitespace-sensitive elements (<pre>, <textarea>) whose trailing whitespace
    /// must be preserved. Normal elements have trailing whitespace stripped during
    /// doc building, not rendering.
    pub(crate) fn render_doc_immediate(&mut self, d: DocId) {
        let col = self.buffer.current_column(TAB_WIDTH);
        let output = {
            let interner = self.interner.borrow();
            tsv_lang::doc::arena_print_doc_with_indent_resolved_preserve_whitespace(
                &self.arena,
                d,
                &self.embed,
                col,
                self.indent_level,
                &*interner,
            )
        };
        self.write(&output);
    }
}

impl<'a> Printer<'a> {
    /// Build a DocId for a TS expression (with comments) in our arena.
    ///
    /// Uses the standard parameters: self.comments, self.embed, self.line_breaks.
    /// For calls that need a custom embed context or empty comments, use the
    /// tsv_ts functions directly.
    pub(crate) fn build_ts_expression_doc(&self, expr: &tsv_ts::Expression) -> DocId {
        tsv_ts::build_expression_doc_with_comments(self.d(), expr, &self.ts_inputs(), &self.embed)
    }

    /// Build a DocId for a TS expression without comments.
    ///
    /// Used for contexts like @const patterns or this={expr} where no comments
    /// are expected between the expression and its container.
    pub(crate) fn build_ts_expression_doc_no_comments(&self, expr: &tsv_ts::Expression) -> DocId {
        let inputs = tsv_ts::PrinterInputs {
            comments: &[],
            ..self.ts_inputs()
        };
        tsv_ts::build_expression_doc_with_comments(self.d(), expr, &inputs, &self.embed)
    }

    /// Format a TS expression to a string.
    ///
    /// Returns a simple formatted string with no indent context or comments.
    pub(crate) fn format_ts_expression(&self, expr: &tsv_ts::Expression) -> String {
        let inputs = tsv_ts::PrinterInputs {
            comments: &[],
            ..self.ts_inputs()
        };
        tsv_ts::format_expression(expr, &inputs, EmbedContext::default())
    }
}

/// Format a Svelte AST back to source code
pub(crate) fn format_svelte(root: &internal::Root, source: &str) -> String {
    let mut printer = Printer::new(source, Rc::clone(&root.interner), &root.comments);
    printer.print_root(root);
    printer.into_string()
}

impl<'a> Printer<'a> {
    /// Check if the last non-whitespace fragment node before `target_start` is
    /// a `<!-- prettier-ignore -->` comment.
    fn has_prettier_ignore_before(&self, fragment: &internal::Fragment, target_start: u32) -> bool {
        let mut last_comment = None;
        for node in &fragment.nodes {
            let node_end = node.span().end;
            if node_end > target_start {
                break;
            }
            match node {
                FragmentNode::Comment(comment) => {
                    last_comment = Some(comment);
                }
                FragmentNode::Text(text) if text.raw.is_whitespace_only() => {
                    // Skip whitespace text nodes
                }
                _ => {
                    // Non-comment, non-whitespace node resets
                    last_comment = None;
                }
            }
        }
        last_comment.is_some_and(|c| c.content.trim() == "prettier-ignore")
    }

    /// Classify which section a fragment comment should travel with during
    /// canonical reordering. Each comment attaches to the nearest section
    /// that follows it in source order.
    fn classify_fragment_comment(
        &self,
        comment: &internal::HtmlComment,
        comment_idx: usize,
        root: &internal::Root,
    ) -> CommentSection {
        // prettier-ignore-start/end mark ranges within the template —
        // they must stay in the fragment so the range preservation logic sees them
        let trimmed = comment.content.trim();
        if trimmed == "prettier-ignore-start" || trimmed == "prettier-ignore-end" {
            return CommentSection::Template;
        }

        let comment_end = comment.span.end;
        let mut nearest: Option<(u32, CommentSection)> = None;

        // Check next non-comment, non-whitespace fragment node
        for node in root.fragment.nodes.iter().skip(comment_idx + 1) {
            match node {
                FragmentNode::Text(t) if t.raw.is_whitespace_only() => continue,
                FragmentNode::Comment(_) => continue,
                other => {
                    let pos = other.span().start;
                    if pos >= comment_end {
                        nearest = Some((pos, CommentSection::Template));
                    }
                    break;
                }
            }
        }

        // Check options
        if let Some(options) = &root.options {
            let start = options.span.start;
            if start >= comment_end && nearest.as_ref().is_none_or(|(p, _)| start < *p) {
                nearest = Some((start, CommentSection::Options));
            }
        }

        // Check module script
        if let Some(module) = &root.module {
            let start = module.span.start;
            if start >= comment_end && nearest.as_ref().is_none_or(|(p, _)| start < *p) {
                nearest = Some((start, CommentSection::ModuleScript));
            }
        }

        // Check instance script
        if let Some(instance) = &root.instance {
            let start = instance.span.start;
            if start >= comment_end && nearest.as_ref().is_none_or(|(p, _)| start < *p) {
                nearest = Some((start, CommentSection::InstanceScript));
            }
        }

        // Check style
        if let Some(css) = &root.css {
            let start = css.span.start;
            if start >= comment_end && nearest.as_ref().is_none_or(|(p, _)| start < *p) {
                nearest = Some((start, CommentSection::Style));
            }
        }

        nearest.map_or(CommentSection::Template, |(_, section)| section)
    }

    /// Print section-attached comments and preserve authorial blank lines.
    /// Returns true if any comments were printed.
    fn print_section_comments(
        &mut self,
        comment_indices: &[usize],
        fragment: &internal::Fragment,
        section_start: u32,
    ) -> bool {
        if comment_indices.is_empty() {
            return false;
        }
        let mut prev_end: Option<u32> = None;
        for &i in comment_indices {
            if let FragmentNode::Comment(comment) = &fragment.nodes[i] {
                // Preserve authorial blank line between consecutive comments
                if let Some(end) = prev_end {
                    let between = &self.source[end as usize..comment.span.start as usize];
                    if between.has_blank_line() {
                        self.write("\n");
                    }
                }
                self.print_comment(comment);
                self.write("\n");
                prev_end = Some(comment.span.end);
            }
        }
        // Preserve authorial blank line between last comment and section
        if let Some(&last_idx) = comment_indices.last() {
            let last_end = fragment.nodes[last_idx].span().end;
            let between = &self.source[last_end as usize..section_start as usize];
            if between.has_blank_line() {
                self.write("\n");
            }
        }
        true
    }

    /// Format a Svelte Root node
    ///
    /// Orchestrates formatting of the four main sections of a .svelte file:
    /// 1. Module script: `<script context="module">`
    /// 2. Instance script: `<script>`
    /// 3. Template: The HTML/Svelte template
    /// 4. Style: `<style>`
    ///
    /// Sections are ordered canonically and separated by blank lines.
    /// Comments travel with the section they immediately precede in source order.
    pub(crate) fn print_root(&mut self, root: &internal::Root) {
        // Classify fragment comments by the section they should travel with.
        let mut options_comments: Vec<usize> = Vec::new();
        let mut module_comments: Vec<usize> = Vec::new();
        let mut instance_comments: Vec<usize> = Vec::new();
        let mut style_comments: Vec<usize> = Vec::new();

        for (i, node) in root.fragment.nodes.iter().enumerate() {
            if let FragmentNode::Comment(comment) = node {
                match self.classify_fragment_comment(comment, i, root) {
                    CommentSection::Options => options_comments.push(i),
                    CommentSection::ModuleScript => module_comments.push(i),
                    CommentSection::InstanceScript => instance_comments.push(i),
                    CommentSection::Style => style_comments.push(i),
                    CommentSection::Template => {}
                }
            }
        }

        // Non-template comments are skipped during fragment printing
        let mut printed_comment_indices: Vec<usize> = Vec::new();
        printed_comment_indices.extend(&options_comments);
        printed_comment_indices.extend(&module_comments);
        printed_comment_indices.extend(&instance_comments);
        printed_comment_indices.extend(&style_comments);

        let mut has_previous_section = false;

        // Format svelte:options (if present) - always first
        if let Some(options) = &root.options {
            self.print_section_comments(&options_comments, &root.fragment, options.span.start);
            self.print_svelte_options(options);
            has_previous_section = true;
        }

        // Format scripts (module then instance)
        for (script, comments) in [
            (root.module.as_ref(), &module_comments),
            (root.instance.as_ref(), &instance_comments),
        ] {
            if let Some(script) = script {
                if has_previous_section {
                    self.write("\n"); // Blank line between sections
                }
                self.print_section_comments(comments, &root.fragment, script.span.start);
                if self.has_prettier_ignore_before(&root.fragment, script.span.start) {
                    self.write(script.span.extract(self.source));
                    self.write("\n");
                } else {
                    self.print_script(script);
                }
                has_previous_section = true;
            }
        }

        // Format template fragment (if not empty)
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
            let ignore_style = self.has_prettier_ignore_before(&root.fragment, style.span.start);
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_section_comments(&style_comments, &root.fragment, style.span.start);
            if ignore_style {
                self.write(style.span.extract(self.source));
                self.write("\n");
            } else {
                self.print_style(style);
            }
        }
    }

    /// Format `<svelte:options ... />` tag
    ///
    /// Always outputs self-closing form with attributes.
    /// Uses doc-based attribute wrapping for width-aware line breaking.
    fn print_svelte_options(&mut self, options: &internal::SvelteOptions) {
        if options.attributes.is_empty() {
            self.write("<svelte:options />\n");
            return;
        }

        let d = self.d();
        let (attr_indent, has_multiline) = self.build_indented_attrs_doc(&options.attributes);
        let line = d.line();
        let inner = d.concat(&[d.text("<svelte:options"), attr_indent, line, d.text("/>")]);

        let group = if has_multiline {
            d.group_break(inner)
        } else {
            d.group(inner)
        };

        self.render_doc_immediate(group);
        self.write("\n");
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

        // Find first non-whitespace node index (excluding skipped indices).
        //
        // This is the root-fragment leading boundary: prettier trims ALL leading
        // whitespace here, non-breaking spaces included (the top-level output must
        // not start with stray whitespace), mirroring the trailing line-rtrim. So
        // a leading nbsp-only node is dropped here even though it counts as content
        // mid-template — hence the Unicode `trim()` rather than the ASCII
        // `is_whitespace_only` used for inter-element separators.
        let first_non_ws_idx = fragment.nodes.iter().enumerate().position(|(i, node)| {
            !skip_indices.contains(&i)
                && !matches!(node, FragmentNode::Text(text) if text.raw.trim().is_empty())
        });

        let mut i = 0;
        while i < fragment.nodes.len() {
            let node = &fragment.nodes[i];
            // Skip indices that were already printed (e.g., comments before script)
            if skip_indices.contains(&i) {
                i += 1;
                continue;
            }

            // Detect inline runs: inline node directly adjacent (span-touching) to a
            // control flow block. Route these through the doc-based printer which handles
            // `has_preceding_breakable` correctly, breaking fn() args before inserting newlines.
            if let Some(run_end) = self.detect_root_inline_run(&fragment.nodes, i) {
                // Resolve pending whitespace before the run (same as ExpressionTag handling)
                if state.has_output_content {
                    match state.pending_ws {
                        PendingWhitespace::None => {}
                        PendingWhitespace::AlreadyHandled => {}
                        PendingWhitespace::Space => {
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

                // Build and render the inline run through the doc-based path
                let doc = self.build_nodes_doc_with_context(&fragment.nodes[i..=run_end], false);
                self.render_doc_immediate(doc);

                state.has_output_content = true;
                state.prev_kind = PrevNodeKind::Block; // control flow blocks are block-like
                state.pending_ws = PendingWhitespace::None;
                i = run_end + 1;
                continue;
            }

            match node {
                FragmentNode::Text(text) => {
                    // Skip leading whitespace-only text nodes at root level
                    if Some(i) < first_non_ws_idx {
                        i += 1;
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
                        // Keep prev_kind as-is: block followed by inline always needs newline
                        // (Prettier behavior: <div>block</div><span> becomes two lines)
                        // continue to next iteration for whitespace-only nodes
                    } else {
                        // Text with content - analyze whitespace using TextAnalysis trait
                        let text_has_leading_newline = text.raw.has_leading_newline();
                        let text_has_leading_space = text.raw.has_leading_space_only();
                        let text_has_trailing_newline = text.raw.has_trailing_newline();

                        // Upgrade pending whitespace based on text's leading whitespace
                        if text.raw.leading_whitespace().has_blank_line() {
                            state.pending_ws.upgrade(PendingWhitespace::BlankLine);
                        } else if text_has_leading_newline {
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
                        } else if text.raw.has_trailing_blank_line() {
                            PendingWhitespace::BlankLine
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
                                // Space after block before inline → newline (prettier always separates)
                                // Space after comment before component → newline (prettier behavior)
                                // Otherwise → preserve space
                                if is_block
                                    || state.prev_kind == PrevNodeKind::Block
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

                    let trimmed = comment.content.trim();

                    // prettier-ignore-start/end: preserve all nodes between as raw source
                    // Only active at root level (nested ranges are treated as regular comments)
                    if trimmed == "prettier-ignore-start" {
                        // Find the matching prettier-ignore-end
                        let mut end_idx = None;
                        for j in (i + 1)..fragment.nodes.len() {
                            if let FragmentNode::Comment(end_comment) = &fragment.nodes[j]
                                && end_comment.content.trim() == "prettier-ignore-end"
                            {
                                end_idx = Some(j);
                                break;
                            }
                        }
                        if let Some(end_idx) = end_idx {
                            // Emit raw source from after start comment through end comment
                            let raw_start = comment.span.end as usize;
                            let end_comment = &fragment.nodes[end_idx];
                            let raw_end = end_comment.span().end as usize;
                            self.write(&self.source[raw_start..raw_end]);
                            state.has_output_content = true;
                            state.prev_kind = PrevNodeKind::Block;
                            state.pending_ws = PendingWhitespace::None;
                            i = end_idx + 1;
                            continue;
                        }
                    }

                    // prettier-ignore: preserve next non-whitespace node as raw source
                    if trimmed == "prettier-ignore" {
                        let mut next_idx = i + 1;
                        while next_idx < fragment.nodes.len() {
                            if let FragmentNode::Text(text) = &fragment.nodes[next_idx]
                                && text.raw.is_whitespace_only()
                            {
                                next_idx += 1;
                                continue;
                            }
                            break;
                        }
                        if next_idx < fragment.nodes.len() {
                            self.write("\n");
                            let raw = fragment.nodes[next_idx].span().extract(self.source);
                            self.write(raw);
                            state.has_output_content = true;
                            state.prev_kind = PrevNodeKind::Block;
                            state.pending_ws = PendingWhitespace::None;
                            i = next_idx + 1;
                            continue;
                        }
                    }

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
            i += 1;
        }
    }

    /// Detect an inline run starting at `start_idx` in the root fragment.
    ///
    /// An inline run is an inline node (ExpressionTag, HtmlTag, RenderTag, or inline Element)
    /// directly span-adjacent to a control flow block (no whitespace between `}` and `{#if`),
    /// optionally with text nodes between them. Multiple control flow blocks may be chained.
    /// These must be routed through the doc-based printer so that function call args
    /// break before the control flow block gets a forced newline.
    ///
    /// Returns `Some(end_idx)` (inclusive) if an inline run was detected, None otherwise.
    fn detect_root_inline_run(&self, nodes: &[FragmentNode], start_idx: usize) -> Option<usize> {
        // Must start with an inline node (not text, not control flow)
        if !self.is_inline_run_node(&nodes[start_idx]) {
            return None;
        }

        // Walk forward through span-adjacent nodes looking for a control flow block.
        // Track last_cf separately — the run ends at the last control flow block,
        // not at trailing text/whitespace that happens to be span-adjacent.
        let mut last_cf_idx = None;

        for j in (start_idx + 1)..nodes.len() {
            let prev_end = nodes[j - 1].span().end;
            let curr_start = nodes[j].span().start;

            // Must be directly adjacent (no whitespace gap in source)
            if prev_end != curr_start {
                break;
            }

            match &nodes[j] {
                FragmentNode::Text(text) => {
                    // Whitespace-only text nodes break inline runs — they signal
                    // intentional separation between nodes. Inline runs are for
                    // span-adjacent sequences like `{expr}{#if cond}` or
                    // `{expr} content text {#if cond}` (text with actual content).
                    if text.raw.is_whitespace_only() {
                        break;
                    }
                    // Text nodes with content can appear between start and control flow
                }
                FragmentNode::IfBlock(_)
                | FragmentNode::EachBlock(_)
                | FragmentNode::AwaitBlock(_)
                | FragmentNode::KeyBlock(_) => {
                    last_cf_idx = Some(j);
                    // Don't break - continue scanning for chained control flow blocks
                }
                node if self.is_inline_run_node(node) => {
                    // Inline nodes (expressions, html tags, render tags, inline elements)
                }
                _ => break,
            }
        }

        last_cf_idx
    }

    /// Check if a fragment node can participate in an inline run (as start or intermediate node).
    fn is_inline_run_node(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::ExpressionTag(_)
            | FragmentNode::HtmlTag(_)
            | FragmentNode::RenderTag(_) => true,
            FragmentNode::Element(el) => !self.is_block_element(el),
            FragmentNode::SpecialElement(el) => !el.kind.is_block(),
            _ => false,
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
    fn interner(&self) -> &SharedInterner {
        &self.interner
    }
}
