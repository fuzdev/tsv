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
use std::cell::Cell;
use std::rc::Rc;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{
    Comment, EmbedContext, INDENT, OutputBuffer, SharedInterner, SymbolResolver, TAB_WIDTH,
    is_format_ignore_directive, is_format_ignore_range_end, is_format_ignore_range_start,
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

    /// Classify a root text node's **leading** whitespace into its pending-whitespace level
    /// (`None` when it has none). Shared by the inline-run boundary and the content-text
    /// node handler, which both `upgrade()` with the result.
    fn leading_of(raw: &str) -> Self {
        if raw.leading_whitespace().has_blank_line() {
            Self::BlankLine
        } else if raw.has_leading_newline() {
            Self::Newline
        } else if raw.has_leading_space_only() {
            Self::Space
        } else {
            Self::None
        }
    }

    /// Classify a root text node's **trailing** whitespace into its pending-whitespace level
    /// (`None` when it has none). A trailing space-only run maps to `Space`; the content-text
    /// handler, which writes that space inline, remaps it to `AlreadyHandled`, while the
    /// inline-run path (whose doc trimmed the boundary) keeps `Space` to re-emit it.
    fn trailing_of(raw: &str) -> Self {
        if raw.has_trailing_blank_line() {
            Self::BlankLine
        } else if raw.has_trailing_newline() {
            Self::Newline
        } else if raw.has_trailing_space_only() {
            Self::Space
        } else {
            Self::None
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
    /// Whether a wrapped block-tag head may dangle its `}` (and, later, expand its
    /// body) in the current context. True almost everywhere — including inside
    /// inline elements / components, where the body-expand is render-safe because a
    /// block's *body-boundary* whitespace is non-significant (the sibling boundary,
    /// e.g. `</span>{#if …}`, stays hugged regardless, since the expand never injects
    /// whitespace there). Set false only while building the content of a
    /// whitespace-significant element (`<pre>` / `<textarea>`), where every injected
    /// whitespace would render. Save/restore discipline:
    /// `build_whitespace_sensitive_content_doc` sets it false for its children and
    /// restores the previous value on the way out (so nested contexts reset
    /// correctly).
    block_dangle_allowed: Cell<bool>,
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
            block_dangle_allowed: Cell::new(true),
        }
    }

    /// Get a reference to the doc arena (convenience for `&self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        &self.arena
    }

    /// Whether a wrapped block-tag head may dangle its `}` in the current context.
    /// See [`Printer::block_dangle_allowed`] for the save/restore discipline.
    #[inline]
    pub(crate) fn block_dangle_allowed(&self) -> bool {
        self.block_dangle_allowed.get()
    }

    /// Set [`Printer::block_dangle_allowed`] to `allowed`, returning the previous
    /// value for the caller to restore. Used by the whitespace-sensitive element
    /// builder to gate the dangle off while building `<pre>` / `<textarea>` content.
    #[inline]
    pub(crate) fn set_block_dangle_allowed(&self, allowed: bool) -> bool {
        self.block_dangle_allowed.replace(allowed)
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
    /// a `<!-- format-ignore -->` (or `prettier-ignore`) comment.
    fn has_format_ignore_before(&self, fragment: &internal::Fragment, target_start: u32) -> bool {
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
        last_comment.is_some_and(|c| is_format_ignore_directive(&c.content))
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
        // format-ignore-start/end mark ranges within the template —
        // they must stay in the fragment so the range preservation logic sees them
        if is_format_ignore_range_start(&comment.content)
            || is_format_ignore_range_end(&comment.content)
        {
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
                if self.has_format_ignore_before(&root.fragment, script.span.start) {
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
            let ignore_style = self.has_format_ignore_before(&root.fragment, style.span.start);
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

            // Detect inline runs: a maximal span-adjacent (no whitespace gap) sequence
            // mixing content text / inline nodes with a control-flow block. Route these
            // through the doc-based element-content path so the block hugs its
            // directly-adjacent neighbors (`text{#if}…{/if}text`) instead of getting a
            // forced newline, while inter-line newlines, spaces, and `has_preceding_breakable`
            // (fn-arg wrapping before block expansion) all resolve exactly as they do for
            // element children.
            if let Some(run_end) = self.detect_root_inline_run(&fragment.nodes, i) {
                let run = &fragment.nodes[i..=run_end];
                // A run that spans multiple source lines — a root-level text node carries
                // a newline (`text{#if}…{/if}text⏎{#each}…`) — renders through the
                // multiline element-content layout so the inter-line structure is kept and
                // boundary whitespace is trimmed (carried as pending_ws). A single-line run
                // (`{fn(…)}{#if …}…{/if}`, `a{#if}…{/if}c`) uses the inline path, where a
                // long block keeps its body inline and breaks its inner content via
                // `has_preceding_breakable` (matching prettier) rather than expanding.
                let multiline = run
                    .iter()
                    .any(|n| matches!(n, FragmentNode::Text(t) if t.raw.contains('\n')));

                // Leading boundary: the multiline path trims a content-text run-start's
                // leading whitespace, so fold it into pending_ws here (the separator from
                // any preceding root content). The inline path renders boundary whitespace
                // verbatim, so it needs no upgrade.
                if multiline && let FragmentNode::Text(text) = &fragment.nodes[i] {
                    state
                        .pending_ws
                        .upgrade(PendingWhitespace::leading_of(&text.raw));
                }

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

                // Build and render the run through the doc-based content path.
                let doc = if multiline {
                    self.build_nodes_doc_multiline(run)
                } else {
                    self.build_nodes_doc_with_context(run, false)
                };
                self.render_doc_immediate(doc);

                state.has_output_content = true;
                state.prev_kind = PrevNodeKind::Block; // control flow blocks are block-like
                // Trailing boundary: the multiline path trimmed a content-text run-end's
                // trailing whitespace — carry it as pending_ws so an inter-line newline /
                // blank line / space is preserved. The inline path rendered it verbatim.
                state.pending_ws = match &fragment.nodes[run_end] {
                    FragmentNode::Text(text) if multiline && !text.raw.is_whitespace_only() => {
                        PendingWhitespace::trailing_of(&text.raw)
                    }
                    _ => PendingWhitespace::None,
                };
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
                        // Text with content - upgrade pending whitespace from its leading ws.
                        state
                            .pending_ws
                            .upgrade(PendingWhitespace::leading_of(&text.raw));

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

                        // Set pending whitespace for next node based on trailing whitespace.
                        // A trailing space was already written inline above (AlreadyHandled);
                        // otherwise classify the trailing ws like the inline-run boundary.
                        let trailing_ws = if has_trailing_space {
                            PendingWhitespace::AlreadyHandled
                        } else {
                            PendingWhitespace::trailing_of(&text.raw)
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

                    // format-ignore-start/end: preserve all nodes between as raw source
                    // Only active at root level (nested ranges are treated as regular comments)
                    if is_format_ignore_range_start(&comment.content) {
                        // Find the matching range-end marker
                        let mut end_idx = None;
                        for j in (i + 1)..fragment.nodes.len() {
                            if let FragmentNode::Comment(end_comment) = &fragment.nodes[j]
                                && is_format_ignore_range_end(&end_comment.content)
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

                    // format-ignore: preserve next non-whitespace node as raw source
                    if is_format_ignore_directive(&comment.content) {
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

    /// Detect a "root inline run" starting at `start_idx` in the root fragment.
    ///
    /// A run is a maximal span-adjacent (no whitespace gap in source) sequence that
    /// contains at least one control-flow block **and** at least one content-text or
    /// inline node — e.g. `text{#if}…{/if}text`, `{#each}…{/each}text`, `{x}{#if}…`,
    /// or several chained together. The run renders through the element-content path
    /// (`build_nodes_doc_multiline`) so a directly-adjacent block hugs its text/inline
    /// neighbors (matching prettier and the inside-an-element layout) instead of the
    /// per-node root printer forcing a newline around the block (which would inject
    /// render-significant whitespace — a no-whitespace boundary must never gain a space).
    ///
    /// The walk starts at content text, an inline node, or a control-flow block, and
    /// breaks on whitespace-only text, a non-adjacent node, or any other node (block
    /// element, comment, `{@const}`/`{@debug}`). A content-text node bridges across the
    /// boundary (its content hugs, its own internal/edge newlines become line breaks in
    /// the multiline layout); the run's leading/trailing edge whitespace is carried as
    /// `pending_ws` by the caller. Lone blocks and pure-inline (no control-flow)
    /// sequences return `None`, keeping the per-node path's behavior for them.
    ///
    /// Returns `Some(end_idx)` (inclusive) for a qualifying run, else `None`.
    fn detect_root_inline_run(&self, nodes: &[FragmentNode], start_idx: usize) -> Option<usize> {
        let is_content_text =
            |n: &FragmentNode| matches!(n, FragmentNode::Text(t) if !t.raw.is_whitespace_only());

        // The start must be a node that can participate in a run.
        let start = &nodes[start_idx];
        if !(is_content_text(start)
            || self.is_inline_run_node(start)
            || nodes::is_control_flow_block(start))
        {
            return None;
        }

        let mut last_idx = start_idx;
        let mut has_control_flow = nodes::is_control_flow_block(start);
        let mut has_content_or_inline = is_content_text(start) || self.is_inline_run_node(start);

        for j in (start_idx + 1)..nodes.len() {
            // Must be directly adjacent (no whitespace gap in source).
            if nodes[j - 1].span().end != nodes[j].span().start {
                break;
            }

            let node = &nodes[j];
            if let FragmentNode::Text(text) = node {
                // Whitespace-only text separates runs (intentional separation).
                if text.raw.is_whitespace_only() {
                    break;
                }
                last_idx = j;
                has_content_or_inline = true;
            } else if nodes::is_control_flow_block(node) {
                last_idx = j;
                has_control_flow = true;
            } else if self.is_inline_run_node(node) {
                last_idx = j;
                has_content_or_inline = true;
            } else {
                // Block element, comment, `{@const}`/`{@debug}`, etc. end the run.
                break;
            }
        }

        // Only route runs that genuinely need hugging: a control-flow block plus an
        // adjacent content-text/inline node. A lone block (`last_idx == start_idx`) or a
        // pure-inline / pure-block sequence keeps the per-node path's existing behavior.
        (has_control_flow && has_content_or_inline && last_idx > start_idx).then_some(last_idx)
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
