// Svelte printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Core Printer struct and root-level printing orchestration
// - **nodes/**: Node-specific printing (elements, expressions, control flow, etc.)
// - **text.rs**: Text-analysis predicates (leading/trailing whitespace, blank lines)
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
mod helpers;
mod nodes;
mod script_style;
mod text;

use self::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{
    Comment, EmbedContext, INDENT, OutputBuffer, Span, TAB_WIDTH, is_format_ignore_directive,
    is_format_ignore_range_end, is_format_ignore_range_start,
};
use tsv_ts::Expression;

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

/// Printer state for building output
pub(crate) struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (layout mode, offsets)
    embed: EmbedContext,
    /// Arena allocator for doc nodes (borrowed so a multi-file driver can reuse
    /// one arena across files; see [`DocArena::reset`]).
    pub(crate) arena: &'a DocArena,
    /// Source code (needed for preserving whitespace semantics)
    pub(crate) source: &'a str,
    /// Comments from scripts and template expressions
    comments: &'a [Comment],
    /// Whether any of `comments` is owned by a node (`owned_by_node`). Computed once
    /// per document at construction and handed to `tsv_ts` via `ts_inputs()`, so the
    /// embedded owned-comment path short-circuits per `{expr}` without an O(comments)
    /// rescan there. `owned_by_node` is set during the eager parse of embedded TS, so
    /// it is already final before printing.
    has_owned_comments: bool,
    /// Whether any of `comments` is a `format-ignore` directive. Computed once per document
    /// at construction and handed to `tsv_ts` via `ts_inputs()`, so the embedded
    /// `has_format_ignore_in_range` short-circuits per `{expr}` without an O(comments)
    /// rescan there — the same per-`{expr}` trap `has_owned_comments` documents.
    has_format_ignore: bool,
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
    /// Span starts of control-flow blocks the root fragment marked as part of a **single-line
    /// inline run** (`{x}{#if c}…{/if}` with no newline). The unified
    /// [`Printer::build_nodes_doc_multiline`] builds these in inline context (long body
    /// inner-breaks) rather than multiline context (body drops to its own line), reproducing
    /// the root's pre-unification `build_nodes_doc` layout (the load-bearing
    /// single-line-run discriminator). Span-keyed, so it scopes to **root-level** runs only —
    /// element-nested blocks (different spans) keep the multiline body-drop divergence (e.g.
    /// `blocks/await/preceding_sibling_body_long`). Populated once by
    /// [`Printer::mark_root_inline_run_blocks`] before the root content is built.
    root_inline_run_block_starts: RefCell<HashSet<u32>>,
}

impl<'a> Printer<'a> {
    /// Create a new printer with the given source and comments (standalone layout).
    pub(crate) fn new(arena: &'a DocArena, source: &'a str, comments: &'a [Comment]) -> Self {
        Self::with_embed(arena, source, comments, EmbedContext::default())
    }

    /// Create a new printer with the given source, comments, and embed context.
    pub(crate) fn with_embed(
        arena: &'a DocArena,
        source: &'a str,
        comments: &'a [Comment],
        embed: EmbedContext,
    ) -> Self {
        // The document's one whole-source line-break table: every embedded
        // island borrows it (`build_program_doc` for `<script>`/`{expr}` TS,
        // `tsv_css::format_embedded_in` for `<style>` CSS) — never rebuild it
        // per island. Filled into the arena-parked scratch (one warm table
        // across a multi-file driver's files); `into_string` parks it back.
        let mut line_breaks = arena.take_line_breaks_scratch();
        tsv_lang::printing::build_line_breaks_into(source, &mut line_breaks);
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            embed,
            arena,
            source,
            comments,
            has_owned_comments: comments.iter().any(|c| c.owned_by_node),
            has_format_ignore: comments
                .iter()
                .any(|c| is_format_ignore_directive(c.content(source))),
            line_breaks,
            block_dangle_allowed: Cell::new(true),
            root_inline_run_block_starts: RefCell::new(HashSet::new()),
        }
    }

    /// Whether `node` is a control-flow block the root marked as part of a single-line inline
    /// run — see [`Printer::root_inline_run_block_starts`]. Read by `build_nodes_doc_multiline`
    /// to build the block in inline (inner-break) rather than multiline (body-drop) context.
    pub(crate) fn is_root_inline_run_block(&self, node: &FragmentNode<'_>) -> bool {
        self.root_inline_run_block_starts
            .borrow()
            .contains(&node.span().start)
    }

    /// Get a reference to the doc arena (convenience for `self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        self.arena
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

    /// Write `span` of the source **verbatim** to the buffer.
    ///
    /// The format-ignore seam for a whole `<script>` / `<style>` section: the island's
    /// comments (which `Root.comments` holds) ride out inside the raw slice and never
    /// reach an emitter, so the ledger is told the range is covered. The doc-side
    /// verbatim seams use [`Self::verbatim_source_doc`].
    pub(crate) fn write_verbatim_span(&mut self, span: Span) {
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        self.write(span.extract(self.source));
    }

    /// A doc emitting `span` of the source **verbatim** — the doc-side twin of
    /// [`Self::write_verbatim_span`] (a format-ignored template node, a format-ignore
    /// range).
    pub(crate) fn verbatim_source_doc(&self, span: Span) -> DocId {
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_verbatim_range(self.source, span.start, span.end);

        self.d().source_span(span, self.source)
    }

    /// Get the source code
    pub(crate) fn source(&self) -> &str {
        self.source
    }

    /// Standard [`tsv_ts::PrinterInputs`] for embedding TypeScript: this
    /// document's source, comments, and line breaks. Call sites
    /// needing empty comments override via
    /// `PrinterInputs { comments: &[], ..self.ts_inputs() }`.
    pub(crate) fn ts_inputs(&self) -> tsv_ts::PrinterInputs<'_> {
        tsv_ts::PrinterInputs {
            source: self.source,
            comments: self.comments,
            line_breaks: &self.line_breaks,
            // The document-level owned-comment flag, computed once at construction
            // (never here — this is called per `{expr}`; see the field's doc).
            has_owned_comments: self.has_owned_comments,
            // Likewise the document-level format-ignore flag (computed once at construction).
            has_format_ignore: self.has_format_ignore,
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
        // Park the line-break table back on the arena for the next format
        // (capacity retained; see `with_embed`).
        self.arena.park_line_breaks_scratch(self.line_breaks);
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
        // Render into the arena-parked scratch: one warm buffer across the
        // document's root nodes instead of an alloc/free per node.
        let mut output = self.arena.take_render_scratch();
        {
            // Source-aware resolver: the doc tree's verbatim leaves — this
            // printer's own markup text / comment slices plus any embedded
            // `tsv_ts` docs — are `DocText::SourceSpan` (host-absolute spans).
            let resolver = tsv_lang::doc::SourceTextResolver {
                source: self.source,
            };
            tsv_lang::doc::arena_print_doc_with_indent_resolved_preserve_whitespace_into(
                self.arena,
                d,
                &self.embed,
                col,
                self.indent_level,
                &resolver,
                &mut output,
            );
        }
        self.write(&output);
        self.arena.park_render_scratch(output);
    }
}

impl<'a> Printer<'a> {
    /// Build a DocId for a TS expression (with comments) in our arena.
    ///
    /// Uses the standard parameters: self.comments, self.embed, self.line_breaks.
    /// For calls that need a custom embed context or empty comments, use the
    /// tsv_ts functions directly.
    pub(crate) fn build_ts_expression_doc(&self, expr: &Expression<'_>) -> DocId {
        tsv_ts::build_expression_doc_with_comments(self.d(), expr, &self.ts_inputs(), &self.embed)
    }

    /// Build a DocId for a TS expression without comments.
    ///
    /// Only for an expression whose span cannot *contain* a comment — a non-computed
    /// object-pattern key (a bare identifier or literal), whose surrounding gaps are the
    /// caller's to emit. Anything with an interior takes [`Self::build_ts_expression_doc`]:
    /// passing an empty comment list means every comment inside the expression is silently
    /// dropped, with no gate anywhere downstream to notice. That is not hypothetical — it is
    /// exactly how `<svelte:element this={…}>` dropped every comment in its expression.
    ///
    /// TODO: retire this, or explain why the one remaining caller needs it. The case for
    /// retiring: it is a footgun whose whole behavior is "drop the comments", and its own
    /// sibling six lines away in `build_pattern_key_doc` (the *computed* key) deliberately
    /// uses the comment-aware builder, with a comment explaining that the `[`→key gap
    /// emitter skips owned comments so the key's doc must claim them. The non-computed
    /// branch is the same shape and does the opposite. The case for caution: nobody has
    /// explained what prints `{ /* c */ k: v }`'s comment today. It *is* printed exactly
    /// once (the ledger agrees), yet the comment is glued to `k` — hence `owned_by_node` —
    /// so `build_pattern_leading_comments` should skip it and this builder cannot emit it.
    /// One of those three facts is wrong; find out which before touching it.
    pub(crate) fn build_ts_expression_doc_no_comments(&self, expr: &Expression<'_>) -> DocId {
        let inputs = tsv_ts::PrinterInputs {
            comments: &[],
            ..self.ts_inputs()
        };
        tsv_ts::build_expression_doc_with_comments(self.d(), expr, &inputs, &self.embed)
    }
}

/// Format a Svelte AST back to source code
pub(crate) fn format_svelte(root: &internal::Root<'_>, source: &str) -> String {
    let arena = DocArena::for_source(source);
    format_svelte_in(root, source, &arena)
}

/// Format a Svelte AST into a caller-provided doc arena (the reuse path).
pub(crate) fn format_svelte_in(
    root: &internal::Root<'_>,
    source: &str,
    arena: &DocArena,
) -> String {
    // The print-once comment ledger's expectation for this document (diagnostic; see
    // `tsv_lang::comment_ledger`). `Root.comments` is the `<script>` + template-expression
    // JS comments; the `<style>` island registers its own through `tsv_css`. The template's
    // `<!-- -->` (`FragmentNode::Comment`) comments are AST nodes rather than detached, so
    // they register by span through a recursive fragment walk — hoisted section comments
    // included, since they still live in `Root.fragment.nodes` (see `print_root`).
    #[cfg(feature = "comment_check")]
    {
        tsv_lang::comment_ledger::register_parsed(source, &root.comments);
        let mut html_comment_spans = Vec::new();
        collect_html_comment_spans(&root.fragment, &mut html_comment_spans);
        tsv_lang::comment_ledger::register_parsed_spans(source, html_comment_spans);
    }

    let mut printer = Printer::new(arena, source, &root.comments);
    printer.print_root(root);
    printer.into_string()
}

/// Collect the spans of every `<!-- -->` (`FragmentNode::Comment`) in a fragment, recursing
/// into every nested fragment (elements, special elements, and the `{#if}` / `{#each}` /
/// `{#await}` / `{#key}` / `{#snippet}` block bodies). The print-once comment ledger reads
/// only the span, so no `HtmlComment` need be manufactured into a `Comment`.
#[cfg(feature = "comment_check")]
fn collect_html_comment_spans(fragment: &internal::Fragment<'_>, out: &mut Vec<Span>) {
    for node in fragment.nodes {
        match node {
            FragmentNode::Comment(comment) => out.push(comment.span),
            FragmentNode::Element(el) => collect_html_comment_spans(&el.fragment, out),
            FragmentNode::SpecialElement(el) => collect_html_comment_spans(&el.fragment, out),
            FragmentNode::IfBlock(block) => {
                collect_html_comment_spans(&block.consequent, out);
                if let Some(alternate) = &block.alternate {
                    collect_html_comment_spans(alternate, out);
                }
            }
            FragmentNode::EachBlock(block) => {
                collect_html_comment_spans(&block.body, out);
                if let Some(fallback) = &block.fallback {
                    collect_html_comment_spans(fallback, out);
                }
            }
            FragmentNode::AwaitBlock(block) => {
                if let Some(pending) = &block.pending {
                    collect_html_comment_spans(pending, out);
                }
                if let Some(then) = &block.then {
                    collect_html_comment_spans(then, out);
                }
                if let Some(catch) = &block.catch {
                    collect_html_comment_spans(catch, out);
                }
            }
            FragmentNode::KeyBlock(block) => collect_html_comment_spans(&block.fragment, out),
            FragmentNode::SnippetBlock(block) => collect_html_comment_spans(&block.body, out),
            _ => {}
        }
    }
}

impl<'a> Printer<'a> {
    /// Check if the last non-whitespace fragment node before `target_start` is
    /// a `<!-- format-ignore -->` (or `prettier-ignore`) comment.
    fn has_format_ignore_before(
        &self,
        fragment: &internal::Fragment<'_>,
        target_start: u32,
    ) -> bool {
        let mut last_comment = None;
        for node in fragment.nodes {
            let node_end = node.span().end;
            if node_end > target_start {
                break;
            }
            match node {
                FragmentNode::Comment(comment) => {
                    last_comment = Some(comment);
                }
                FragmentNode::Text(text) if text.is_ascii_ws_only => {
                    // Skip whitespace text nodes
                }
                _ => {
                    // Non-comment, non-whitespace node resets
                    last_comment = None;
                }
            }
        }
        last_comment.is_some_and(|c| is_format_ignore_directive(c.content(self.source)))
    }

    /// Classify which section a fragment comment should travel with during
    /// canonical reordering. Each comment attaches to the nearest section
    /// that follows it in source order.
    fn classify_fragment_comment(
        &self,
        comment: &internal::HtmlComment,
        comment_idx: usize,
        root: &internal::Root<'_>,
    ) -> CommentSection {
        // format-ignore-start/end mark ranges within the template —
        // they must stay in the fragment so the range preservation logic sees them
        if is_format_ignore_range_start(comment.content(self.source))
            || is_format_ignore_range_end(comment.content(self.source))
        {
            return CommentSection::Template;
        }

        let comment_end = comment.span.end;
        let mut nearest: Option<(u32, CommentSection)> = None;

        // Check next non-comment, non-whitespace fragment node
        for node in root.fragment.nodes.iter().skip(comment_idx + 1) {
            match node {
                FragmentNode::Text(t) if t.is_ascii_ws_only => continue,
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
        fragment: &internal::Fragment<'_>,
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
    pub(crate) fn print_root(&mut self, root: &internal::Root<'_>) {
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
                    self.write_verbatim_span(script.span);
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
            !matches!(node, FragmentNode::Text(text) if text.is_ascii_ws_only)
        });

        if has_content {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.print_root_fragment(&root.fragment, &printed_comment_indices);
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
                self.write_verbatim_span(style.span);
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
    fn print_svelte_options(&mut self, options: &internal::SvelteOptions<'_>) {
        if options.attributes.is_empty() {
            self.write("<svelte:options />\n");
            return;
        }

        let d = self.d();
        let (attr_indent, has_multiline) = self.build_indented_attrs_doc(options.attributes);
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

    /// Render the whole template fragment through the same doc-based content path the
    /// elements use — the root is **not special**.
    ///
    /// Prettier prints the root markup with the same `printChildren` as element children,
    /// just inside a force-broken `group([…, hardline])`. [`Printer::build_nodes_doc_multiline`]
    /// *is* that force-broken layout, so the root's sibling separation, blank-line handling,
    /// block hugging, and the inline-element `>`-fold all come from the shared builder. Three
    /// root-only concerns are handled around that call:
    ///
    /// - **Boundary trim (B1):** the template content is the span of `fragment.nodes` from the
    ///   first to the last node that isn't a section comment printed with its section
    ///   (`skip_indices`) and isn't leading/trailing Unicode-whitespace-only text. Prettier trims
    ///   the fragment boundary with a Unicode `trim()`, so a leading nbsp-only node (content
    ///   mid-template) is dropped here. Section comments are *usually* boundary-contiguous, but a
    ///   section sitting mid-template leaves one interior; those are dropped by the segmentation
    ///   loop below (they are already printed with their hoisted section).
    /// - **Single-line inline runs (B4):** a `{x}{#if c}…{/if}` run with no newline must
    ///   *inner-break* a long body, not drop it — the load-bearing discriminator the element
    ///   path does not apply. [`Self::mark_root_inline_run_blocks`] marks those blocks so the
    ///   shared builder builds them in inline context (see `root_inline_run_block_starts`).
    /// - **`format-ignore` ranges (B6):** `<!-- format-ignore-start -->` … `-end` is a
    ///   *root-only* directive (it does not activate inside an element), so the content is
    ///   split at top-level ranges: each range emits its source verbatim, the surrounding
    ///   segments go through the shared builder. (Single-node `format-ignore` is handled by
    ///   the shared builder itself.)
    fn print_root_fragment(&mut self, fragment: &internal::Fragment<'_>, skip_indices: &[usize]) {
        // Effective template range: drop section comments (`skip_indices`) and Unicode-ws-only
        // boundary text. Both kinds only occur at the boundaries, so the kept content is a
        // contiguous slice.
        let source = self.source;
        let skippable = |i: usize, n: &FragmentNode<'_>| {
            skip_indices.contains(&i)
                || matches!(n, FragmentNode::Text(t) if t.raw(source).trim().is_empty())
        };
        let Some(start) = fragment
            .nodes
            .iter()
            .enumerate()
            .position(|(i, n)| !skippable(i, n))
        else {
            return;
        };
        // `rposition` finds at least `start`, so the fallback never triggers (panic-free).
        let end = fragment
            .nodes
            .iter()
            .enumerate()
            .rposition(|(i, n)| !skippable(i, n))
            .unwrap_or(start);
        let nodes = &fragment.nodes[start..=end];

        // Mark single-line-run control-flow blocks so the shared builder inner-breaks them (B4).
        self.mark_root_inline_run_blocks(nodes);

        // Split at `format-ignore` ranges and at interior section comments (both rare); most
        // templates are one segment.
        let mut out: DocBuf = DocBuf::new();
        let mut seg_start = 0;
        let mut i = 0;
        while i < nodes.len() {
            // A comment printed with its (hoisted) section — `skip_indices` indexes the full
            // `fragment.nodes`, so offset by `start`. These are *usually* boundary-contiguous
            // (trimmed away by `start`/`end`), but a `<script>`/`<style>`/`<svelte:options>`
            // sitting **mid-template** (template content on both sides) leaves its comment
            // interior to the slice. Drop it here so the shared builder doesn't re-emit it as
            // template content (it is already printed with its section). The gap is re-bridged
            // with the same boundary-aware separator the `format-ignore` range path uses.
            if skip_indices.contains(&(start + i)) {
                if nodes[seg_start..i]
                    .iter()
                    .any(|n| !n.is_whitespace_only_text())
                {
                    out.push(self.build_nodes_doc_multiline(&nodes[seg_start..i]));
                    if let Some(sep) = self.range_trailing_separator(nodes, i) {
                        out.push(sep);
                    }
                }
                seg_start = i + 1;
                i += 1;
                continue;
            }
            let is_range_start = matches!(
                &nodes[i],
                FragmentNode::Comment(c) if is_format_ignore_range_start(c.content(source))
            );
            if is_range_start
                && let Some(range_end) = (i + 1..nodes.len()).find(|&j| {
                    matches!(&nodes[j],
                        FragmentNode::Comment(c) if is_format_ignore_range_end(c.content(source)))
                })
            {
                // Segment up to and including the start comment (it prints normally).
                out.push(self.build_nodes_doc_multiline(&nodes[seg_start..=i]));
                // Verbatim source from just after the start comment through the end
                // comment — emit the slice as a span, no allocation.
                let raw_start = nodes[i].span().end;
                let raw_end = nodes[range_end].span().end;
                out.push(self.verbatim_source_doc(Span::new(raw_start, raw_end)));
                // The whitespace after the end comment is trimmed by the next segment's
                // boundary, so re-emit it as the separator before that segment.
                if let Some(sep) = self.range_trailing_separator(nodes, range_end) {
                    out.push(sep);
                }
                seg_start = range_end + 1;
                i = range_end + 1;
                continue;
            }
            i += 1;
        }
        if seg_start < nodes.len() {
            out.push(self.build_nodes_doc_multiline(&nodes[seg_start..]));
        }

        if !out.is_empty() {
            let doc = self.d().concat(&out);
            self.render_doc_immediate(doc);
        }
    }

    /// Mark control-flow blocks in **single-line** root inline runs (`{x}{#if c}…{/if}` with no
    /// newline) so [`Printer::build_nodes_doc_multiline`] builds them in inline context — see
    /// [`Printer::root_inline_run_block_starts`]. A *multi-line* run (its content text spans
    /// source lines) keeps the multiline layout, so its blocks are left unmarked. Span-keyed,
    /// so only root-level run blocks are affected.
    fn mark_root_inline_run_blocks(&self, nodes: &[FragmentNode<'_>]) {
        let mut marks = self.root_inline_run_block_starts.borrow_mut();
        marks.clear();
        let mut i = 0;
        while i < nodes.len() {
            if let Some(run_end) = self.detect_root_inline_run(nodes, i) {
                let run = &nodes[i..=run_end];
                let multiline = run
                    .iter()
                    .any(|n| matches!(n, FragmentNode::Text(t) if t.has_newline()));
                if !multiline {
                    for n in run {
                        if nodes::is_control_flow_block(n) {
                            marks.insert(n.span().start);
                        }
                    }
                }
                i = run_end + 1;
            } else {
                i += 1;
            }
        }
    }

    /// The separator to emit after a `format-ignore` range, before the next segment: the
    /// whitespace immediately following the end comment (which the next segment's boundary
    /// trim would otherwise drop). A blank line → `literalline` (the un-indented blank) +
    /// `hardline`; a single newline / adjacency → `hardline`. `None` when nothing follows.
    fn range_trailing_separator(
        &self,
        nodes: &[FragmentNode<'_>],
        range_end: usize,
    ) -> Option<DocId> {
        if range_end + 1 >= nodes.len() {
            return None;
        }
        let d = self.d();
        let blank = matches!(
            &nodes[range_end + 1],
            FragmentNode::Text(t) if t.has_blank_line()
        );
        Some(if blank {
            d.concat(&[d.literalline(), d.hardline()])
        } else {
            d.hardline()
        })
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
    /// element, comment, `{@const}`/`{@debug}`, `{const}`/`{let}`). A content-text node
    /// bridges across the boundary (its content hugs, its own internal/edge newlines
    /// become line breaks in the multiline layout); the run's leading/trailing edge
    /// whitespace is carried as
    /// `pending_ws` by the caller. Lone blocks and pure-inline (no control-flow)
    /// sequences return `None`, keeping the per-node path's behavior for them.
    ///
    /// Returns `Some(end_idx)` (inclusive) for a qualifying run, else `None`.
    fn detect_root_inline_run(
        &self,
        nodes: &[FragmentNode<'_>],
        start_idx: usize,
    ) -> Option<usize> {
        let is_content_text =
            |n: &FragmentNode<'_>| matches!(n, FragmentNode::Text(t) if !t.is_ascii_ws_only);

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
                if text.is_ascii_ws_only {
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
    fn is_inline_run_node(&self, node: &FragmentNode<'_>) -> bool {
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
        // The hoisted-section (direct-write) emit path for a `<!-- -->` comment, recorded at
        // the write like `tsv_css`'s `print_css_comment`; the template path is the doc-tagged
        // `build_html_comment_doc`. Registered by span in `format_svelte_in`. See
        // `tsv_lang::comment_ledger`.
        #[cfg(feature = "comment_check")]
        tsv_lang::comment_ledger::record_emitted(self.source, comment.span);

        self.write("<!--");
        self.write(comment.content(self.source));
        self.write("-->");
    }
}
