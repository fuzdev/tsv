// CSS printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - core Printer, top-level node printing, and
//   the shared block-body routine (`print_css_block_children`, used by rules + at-rules)
// - **selectors.rs**: Selector printing (reusable across rules and at-rules)
// - **rules.rs**: CSS rule printing (selector + block structure)
// - **declarations.rs**: Declaration printing + wrapping logic
// - **values.rs**: CSS value printing (all value types)
// - **atrules.rs**: At-rule printing (@media, @keyframes, etc., uses selectors and rules)
//
// ## Design Principles
//
// 1. **Match Prettier**: Output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change CSS rendering semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability
// 4. **Reusability**: Shared printing logic (selectors, block-body iteration) used by multiple modules
// 5. **Hierarchy-Following**: Module structure mirrors CSS spec (rules → declarations → values)

mod atrules;
mod declarations;
mod rules;
mod selectors;
pub mod value_normalization;
mod values;

use crate::ast::internal::{Comment, CssBlockChild, CssDeclaration, CssNode, CssStyleSheet};
use crate::lexer::{Lexer, TokenKind};
use tsv_lang::{
    CommentPosition, EmbedContext, INDENT, OutputBuffer, Span, TAB_WIDTH, classify_comment_fast,
    comments_in_range,
    doc::{
        self, TextResolver,
        arena::{DocArena, DocId},
    },
    is_format_ignore_directive, printing,
};

/// Render-time text resolver for the CSS printer.
///
/// CSS doc trees carry no interned identifiers — the printer emits source slices
/// directly (`self.write`, `text_pooled`), never `DocText::Symbol`. The only thing
/// to resolve at render is a [`DocText::SourceSpan`](tsv_lang::doc::DocText::SourceSpan):
/// a verbatim source slice emitted with no allocation (see
/// [`values`]'s `build_dimension_doc`). So `resolve` (symbol lookup) is
/// unreachable; only `resolve_source_span` does work. This is what makes the CSS
/// render path source-aware, the analogue of the `SourceTextResolver` the TS and
/// Svelte printers wrap around their interners.
struct CssSourceResolver<'a> {
    source: &'a str,
}

impl TextResolver for CssSourceResolver<'_> {
    fn resolve(&self, _id: u32) -> &str {
        // CSS never emits `DocText::Symbol`, so this is never called.
        #[allow(clippy::unreachable)]
        {
            unreachable!("CSS doc trees contain no Symbol nodes")
        }
    }

    fn resolve_source_span(&self, span: Span) -> &str {
        span.extract(self.source)
    }
}

/// Printer state for building output
pub(crate) struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (base indent offset, first-line offset, layout mode, etc.).
    pub(crate) embed: EmbedContext,
    /// Arena allocator for doc nodes (borrowed so a multi-file driver can reuse
    /// one arena across files; see [`DocArena::reset`]).
    pub(crate) arena: &'a DocArena,
    /// Original source (for blank line detection and raw value extraction)
    pub(crate) source: &'a str,
    /// All comments sorted by span.start
    pub(crate) comments: &'a [Comment],
    /// Precomputed line break positions for O(log n) line boundary lookups
    pub(crate) line_breaks: &'a [u32],
    /// True while printing the body of an `@keyframes` block, so the `from`/`to`
    /// keyframe-selector keywords can be lowercased (they're case-insensitive
    /// keywords there; outside keyframes a `from`/`to` type selector is preserved).
    pub(crate) in_keyframes: bool,
}

impl<'a> Printer<'a> {
    /// Create a new printer with source, comments, and line_breaks
    pub(crate) fn new(
        arena: &'a DocArena,
        source: &'a str,
        comments: &'a [Comment],
        line_breaks: &'a [u32],
    ) -> Self {
        Self::with_embed(
            arena,
            source,
            comments,
            line_breaks,
            EmbedContext::default(),
        )
    }

    /// Create a new printer with the given embedding context.
    pub(crate) fn with_embed(
        arena: &'a DocArena,
        source: &'a str,
        comments: &'a [Comment],
        line_breaks: &'a [u32],
        embed: EmbedContext,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            embed,
            arena,
            source,
            comments,
            line_breaks,
            in_keyframes: false,
        }
    }

    /// Get a reference to the doc arena (convenience for `self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        self.arena
    }

    /// Check if two positions are on the same line (O(log n) binary search)
    #[inline]
    pub(crate) fn is_same_line(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::is_same_line_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if there's a blank line (2+ newlines) between two positions (O(log n) binary search)
    #[inline]
    pub(crate) fn has_blank_line_between(&self, prev_end: u32, curr_start: u32) -> bool {
        printing::has_blank_line_between_fast(self.line_breaks, prev_end, curr_start)
    }

    /// Check if a declaration has value comments (comments inside the value, not property name)
    ///
    /// Value comments are comments that appear after the colon, e.g., `color: /* comment */ red;`
    /// Detected by scanning the source text directly (value comments are not stored in the Vec).
    pub(crate) fn has_value_comments_in_decl(&self, decl: &CssDeclaration<'_>) -> bool {
        // Free O(1) negative gate: `has_block_comment` (recorded at parse time from the
        // lexer's comment tokens) is false iff no `/* … */` appears anywhere in the
        // declaration — property→colon gap or value/`!important`/trailing region. No block
        // comment anywhere ⟹ no value comment, so skip the colon scan + `/*` substring check
        // + value re-lex entirely on the common comment-free path (this fn runs up to 3× per
        // declaration). When a comment is present the scan below runs unchanged, so the result
        // is byte-identical.
        if !decl.has_block_comment {
            return false;
        }
        let decl_source = decl.span.extract(self.source);
        // The parser recorded the `property : value` colon; rebase it to the
        // declaration slice instead of re-scanning (see `CssDeclaration::colon_offset`).
        let colon_pos = (decl.colon_offset - decl.span.start) as usize;
        let value_part = &decl_source[colon_pos + 1..];
        // Fast path: no `/*` at all → no block comment possible.
        if !value_part.contains("/*") {
            return false;
        }
        // A `/*` is present, but it's only a real value comment when it isn't opaque
        // token content — inside a string (`content: "a/*b"`) or an unquoted `url()`
        // (`url(a/*b)`) the `/*` is literal, not a comment start. Lex the value and look
        // for an actual `Comment` token (the lexer consumes String/Url tokens whole), so
        // the naive substring scan can't false-positive there.
        let mut lexer = Lexer::new(value_part);
        loop {
            match lexer.next_token() {
                Ok(t) if t.kind == TokenKind::Comment => return true,
                Ok(t) if t.kind == TokenKind::Eof => return false,
                Ok(_) => {}
                // A lex error (e.g. a genuinely unterminated `/* …`) — fall back to the
                // conservative substring answer rather than dropping a real comment.
                Err(_) => return true,
            }
        }
    }

    /// Write a string to the buffer
    pub(crate) fn write(&mut self, s: &str) {
        self.buffer.write(s);
    }

    /// Write indentation based on current indent level
    ///
    /// Used for printing nested structures like CSS rules.
    pub(crate) fn write_indent(&mut self) {
        tsv_lang::write_indent(&mut self.buffer, self.indent_level, INDENT);
    }

    /// Write indentation at the current level plus `extra` additional levels.
    ///
    /// For continuation lines in the imperative `@import` media-query list wrap, which
    /// indents one or two levels past the statement. The caller writes the preceding
    /// newline; this only emits the (deeper) indentation.
    pub(crate) fn write_indent_extra(&mut self, extra: usize) {
        self.indent_level += extra;
        self.write_indent();
        self.indent_level -= extra;
    }

    /// Remove trailing newline from buffer (for inline comment handling)
    pub(crate) fn buffer_remove_trailing_newline(&mut self) {
        self.buffer.pop_if_ends_with('\n');
    }

    /// Get the current column position (for doc-builder width calculations)
    ///
    /// Includes base_indent_offset to account for Svelte wrapper indentation
    /// that will be added to each line during final formatting.
    pub(crate) fn current_column(&self) -> usize {
        let col = self.buffer.current_column(TAB_WIDTH);
        // Add wrapper indent width so fill calculations account for final indentation
        col + (self.embed.base_indent_offset * TAB_WIDTH)
    }

    /// Get the effective indent level for width calculations
    ///
    /// Includes base_indent_offset to account for external context (e.g., Svelte wrapper)
    /// that adds indentation to the final output.
    pub(crate) fn effective_indent(&self) -> usize {
        self.indent_level + self.embed.base_indent_offset
    }

    /// Get the visual width of current indentation in characters
    ///
    /// Converts indent level to actual character width based on tab_width.
    pub(crate) fn indent_width(&self) -> usize {
        self.effective_indent() * TAB_WIDTH
    }

    /// Write a DocId to the buffer, accounting for current column and indent level
    ///
    /// This handles the common pattern of:
    /// 1. Get current column position (which already includes base_indent_offset after newlines)
    /// 2. Print doc with indent-aware width calculations
    /// 3. Write the result to the buffer
    ///
    /// Note: base_indent_offset is already accounted for in position tracking after newlines
    /// (see doc::render_single_doc line breaks). We should NOT add it again here.
    pub(crate) fn write_arena_doc(&mut self, d: DocId) {
        let current_col = self.current_column();
        // Render into the arena-parked scratch: one warm buffer across the
        // file's rules instead of an alloc/free per rule.
        let mut output = self.arena.take_render_scratch();
        doc::arena_print_doc_with_indent_resolved_into(
            self.arena,
            d,
            &self.embed,
            current_col,
            self.indent_level,
            &CssSourceResolver {
                source: self.source,
            },
            &mut output,
        );
        self.write(&output);
        self.arena.park_render_scratch(output);
    }

    /// Like `write_arena_doc`, but reserving `suffix_width` columns for the
    /// punctuation the caller appends after the doc (` {`/`;` after an at-rule
    /// prelude, `) {`/`,` after a selector). The reservation rides
    /// `EmbedContext::suffix_width` (read by every group's fit check); fill-based docs
    /// additionally carry it as the fill's `trailing_reserve` (fills don't read
    /// `suffix_width`). Shared by the selector and at-rule-prelude writers.
    pub(crate) fn write_arena_doc_with_suffix(&mut self, d: DocId, suffix_width: usize) {
        let current_col = self.current_column();
        let mut embed = self.embed;
        embed.suffix_width = suffix_width;
        let mut output = self.arena.take_render_scratch();
        doc::arena_print_doc_with_indent_resolved_into(
            self.arena,
            d,
            &embed,
            current_col,
            self.indent_level,
            &CssSourceResolver {
                source: self.source,
            },
            &mut output,
        );
        self.write(&output);
        self.arena.park_render_scratch(output);
    }

    /// Get the formatted output
    pub(crate) fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Print a list of CSS nodes (rules) with comments interspersed by position
    pub(crate) fn print_css_nodes(&mut self, nodes: &[CssNode<'_>]) {
        // Use comment index for efficient traversal (comments are sorted)
        let mut comment_idx = 0;
        let mut prev_end: u32 = 0;
        let mut printed_any = false;

        for node in nodes {
            let node_start = node.span().start;
            let node_end = node.span().end;

            // Print comments between prev_end and this node
            let idx_before = comment_idx;
            let comments_before =
                self.print_leading_comments(prev_end, node_start, &mut comment_idx);

            // Check printed comments for a format-ignore directive (O(k) where k = comments_before)
            let has_ignore = self.comments[idx_before..comment_idx]
                .iter()
                .any(|c| is_format_ignore_directive(c.content(self.source)));

            // Add separator before node
            if printed_any || comments_before > 0 {
                // Determine where to measure blank line from
                let gap_start = if comments_before > 0 {
                    self.comments
                        .get(comment_idx.saturating_sub(1))
                        .map_or(prev_end, |c| c.span.end)
                } else {
                    prev_end
                };

                if self.has_blank_line_between(gap_start, node_start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            // Leading indent for this top-level node (matches the nested
            // `print_css_block_children` pattern). No-op at `indent_level` 0
            // (standalone); the embedded stylesheet renders at the wrapper level.
            self.write_indent();

            // format-ignore: emit raw source instead of formatting
            if has_ignore {
                self.write(node.span().extract(self.source));
            } else {
                self.print_css_node(node);
            }

            // Check for inline comments on same line as node's closing brace
            let inline_count = self.print_inline_comments_after_node(node_end, &mut comment_idx);

            prev_end = if inline_count > 0 {
                self.comments
                    .get(comment_idx - 1)
                    .map_or(node_end, |c| c.span.end)
            } else {
                node_end
            };

            printed_any = true;
        }

        // Print trailing comments after all nodes
        let had_trailing = self.print_trailing_comments(prev_end, &mut comment_idx);
        if had_trailing {
            printed_any = true;
        }

        // Add trailing newline (only if there's content — empty files stay empty)
        if printed_any {
            self.write("\n");
        }
    }

    /// Print leading comments between prev_end and curr_start
    /// Returns the number of comments printed
    fn print_leading_comments(
        &mut self,
        prev_end: u32,
        curr_start: u32,
        comment_idx: &mut usize,
    ) -> usize {
        let mut printed = 0;
        let mut last_end = prev_end;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];
            if comment.span.start >= curr_start {
                break;
            }

            // Skip comments that are inside previous node's span (e.g., prelude comments in at-rules)
            // These are handled by the node's own printing logic via comments_in_range()
            if comment.span.end <= prev_end {
                *comment_idx += 1;
                continue;
            }

            let position = classify_comment_fast(comment, prev_end, curr_start, self.line_breaks);

            // Skip trailing comments (same line as prev node)
            if prev_end > 0 && matches!(position, CommentPosition::Trailing) {
                *comment_idx += 1;
                last_end = comment.span.end;
                continue;
            }

            // Print with proper spacing
            let mut starts_line = true;
            if printed > 0 {
                // Check if this comment is on the same line as the previous comment
                if self.is_same_line(last_end, comment.span.start) {
                    self.write(" ");
                    starts_line = false;
                } else if self.has_blank_line_between(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            } else if prev_end > 0 {
                // First comment after a node
                if self.has_blank_line_between(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            // Leading indent for a top-level comment that starts its own line
            // (skip a mid-line comment joined to the previous by a space; no-op
            // at indent_level 0 / standalone).
            if starts_line {
                self.write_indent();
            }
            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed += 1;
        }

        printed
    }

    /// Print inline comments on the same line after a node
    /// Returns the number of comments printed
    fn print_inline_comments_after_node(
        &mut self,
        node_end: u32,
        comment_idx: &mut usize,
    ) -> usize {
        let mut printed = 0;
        let mut last_end = node_end;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];

            // Skip comments inside the node's span (e.g. at-rule prelude comments,
            // block-interior comments). These are emitted by the node's own printing
            // logic via comments_in_range(); without this guard the loop would break on
            // an interior comment (its start precedes node_end, so is_same_line reverses
            // to false) and the genuine trailing comment after `;`/`}` would be dropped.
            if comment.span.end <= node_end {
                *comment_idx += 1;
                continue;
            }

            if !self.is_same_line(last_end, comment.span.start) {
                break;
            }

            self.write(" ");
            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed += 1;
        }

        printed
    }

    /// Print trailing comments after all nodes
    /// Returns true if any comments were printed
    fn print_trailing_comments(&mut self, prev_end: u32, comment_idx: &mut usize) -> bool {
        let mut last_end = prev_end;
        let mut printed = false;

        while *comment_idx < self.comments.len() {
            let comment = &self.comments[*comment_idx];

            // Skip comments that are inside previous node's span (e.g., prelude comments in at-rules)
            // These are handled by the node's own printing logic via comments_in_range()
            if comment.span.end <= prev_end {
                *comment_idx += 1;
                continue;
            }

            // Skip inline comments (same line as last item) - already handled
            // But only if there was a last item (prev_end > 0) - otherwise this is the first content
            if prev_end > 0 && self.is_same_line(prev_end, comment.span.start) {
                *comment_idx += 1;
                last_end = comment.span.end;
                continue;
            }

            // Print with proper spacing (but no leading newline for first content)
            if last_end > 0 {
                if self.has_blank_line_between(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            // Leading indent for this top-level trailing comment (no-op standalone).
            self.write_indent();
            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed = true;
        }

        printed
    }

    /// Print a single CSS node
    fn print_css_node(&mut self, node: &CssNode<'_>) {
        match node {
            CssNode::Rule(rule) => self.print_css_rule(rule),
            CssNode::Atrule(atrule) => self.print_css_atrule(atrule),
        }
    }

    /// Print a CSS comment
    pub(crate) fn print_css_comment(&mut self, comment: &Comment) {
        // Write comment with delimiters - content is preserved exactly as written
        self.write("/*");
        self.write(comment.content(self.source));
        self.write("*/");
    }

    /// Join the comments fully within `[start, end)` as space-separated `/*…*/`
    /// blocks (empty string when there are none). The single source-to-string form of
    /// a comment run, shared by the at-rule prelude and selector comment interleaving.
    /// Multi-line comment interiors stay verbatim under Svelte `<style>` embedding:
    /// the CSS renders at its final indent, so an interior line keeps its content at
    /// column 0 with no post-hoc re-indent.
    pub(crate) fn comment_blocks_in_range(&self, start: u32, end: u32) -> String {
        let mut out = String::new();
        for comment in comments_in_range(self.comments, start, end) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str("/*");
            out.push_str(comment.content(self.source));
            out.push_str("*/");
        }
        out
    }

    /// Split the comments in `[start, end)` around `split_pos` (a delimiter byte
    /// offset — a comma or an `and`/`or` keyword), returning the joined `/*…*/` text
    /// before and after it. With no split position the whole run goes to the first
    /// element. Comments never straddle a delimiter, so the range split is exact.
    pub(crate) fn split_comments_at(
        &self,
        start: u32,
        end: u32,
        split_pos: Option<u32>,
    ) -> (String, String) {
        match split_pos {
            Some(pos) => (
                self.comment_blocks_in_range(start, pos),
                self.comment_blocks_in_range(pos, end),
            ),
            None => (self.comment_blocks_in_range(start, end), String::new()),
        }
    }

    /// Try to print inline comments after the current item
    ///
    /// Checks if the next item is a comment on the same line as `prev_end`.
    /// If so, prints it inline and returns the number of comments consumed.
    ///
    /// This consolidates the repeated pattern across rules.rs and atrules.rs.
    pub(crate) fn try_print_inline_comments(
        &mut self,
        children: &[CssBlockChild<'_>],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        let mut consumed = 0;
        let mut last_end = prev_end;

        while let Some(CssBlockChild::Comment(next_comment)) =
            children.get(current_idx + 1 + consumed)
            && self.is_same_line(last_end, next_comment.span.start)
        {
            self.write(" ");
            self.print_css_comment(next_comment);
            last_end = next_comment.span.end;
            consumed += 1;
        }

        consumed
    }

    /// Try to print inline comments after a declaration
    ///
    /// Like `try_print_inline_comments`, but a declaration ends with its own `\n`:
    /// pull that newline back before trailing the first same-line comment, then
    /// re-add it after. Delegates the per-comment loop so the two stay in lockstep.
    pub(crate) fn try_print_inline_comments_after_decl(
        &mut self,
        children: &[CssBlockChild<'_>],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        // Only a same-line comment trails the declaration; if the next child isn't
        // one, leave the declaration's trailing newline in place.
        let has_inline = matches!(
            children.get(current_idx + 1),
            Some(CssBlockChild::Comment(next)) if self.is_same_line(prev_end, next.span.start)
        );
        if !has_inline {
            return 0;
        }

        self.buffer_remove_trailing_newline();
        let consumed = self.try_print_inline_comments(children, current_idx, prev_end);
        self.write("\n");
        consumed
    }

    /// Print a declaration — honoring a pending format-ignore directive — then any
    /// same-line trailing comments, returning the number of comments consumed (advance
    /// the loop index by it). Shared by the three CSS block-body loops (top-level rule
    /// body, nested-rule body, at-rule direct-declaration body) so their declaration
    /// handling stays in lockstep; drift between two of them was the at-rule
    /// trailing-comment bug.
    ///
    /// The pending-ignore flag is passed by `&mut` and **consumed here** (read, then
    /// reset to false) so the directive applies to exactly this one declaration. Folding
    /// the reset into the helper means a caller cannot honor the flag without also
    /// clearing it — the failure mode that left the at-rule body loop ignoring the
    /// directive (it hardcoded `false`, skipping both honor and reset).
    pub(crate) fn print_decl_with_inline_comments(
        &mut self,
        children: &[CssBlockChild<'_>],
        index: usize,
        decl: &CssDeclaration<'_>,
        format_ignore_next: &mut bool,
    ) -> usize {
        if std::mem::take(format_ignore_next) {
            self.write_format_ignore_declaration(decl);
        } else {
            self.print_css_declaration(decl);
        }
        self.try_print_inline_comments_after_decl(children, index, decl.span.end)
    }

    /// Print the children of a CSS block body — a rule's declaration list or an
    /// at-rule block's child list — applying the per-child policy that the three
    /// former block loops (top-level rule body, nested-rule body, at-rule direct
    /// body) each re-implemented and drifted on: source-blank-line preservation, a
    /// trailing-newline separator, inline trailing-comment capture after a nested
    /// rule/at-rule, and a pending format-ignore directive.
    ///
    /// `start_index` skips any pre-`{` comments the caller already consumed inline
    /// before the opening brace (0 for an at-rule block, whose prelude owns that
    /// region). Centralizing the loop is what makes that drift impossible: every
    /// child type is handled in exactly one place. The two earlier drift bugs —
    /// a trailing comment after a nested at-rule's `}` and a format-ignore inside
    /// an at-rule body — were each "one of the three loops handled case X, another
    /// didn't"; with a single routine there is no "another".
    pub(crate) fn print_css_block_children(
        &mut self,
        children: &[CssBlockChild<'_>],
        start_index: usize,
    ) {
        let mut i = start_index;
        let mut format_ignore_next = false;
        while i < children.len() {
            // Preserve a source blank line before any child (uniform across types).
            if i > start_index && self.has_blank_line_before_child(children, i) {
                self.write("\n");
            }
            match &children[i] {
                CssBlockChild::Declaration(decl) => {
                    i += self.print_decl_with_inline_comments(
                        children,
                        i,
                        decl,
                        &mut format_ignore_next,
                    );
                }
                CssBlockChild::Comment(comment) => {
                    // A format-ignore directive applies to the next child.
                    if is_format_ignore_directive(comment.content(self.source)) {
                        format_ignore_next = true;
                    }
                    self.write_indent();
                    self.print_css_comment(comment);
                    self.write("\n");
                }
                CssBlockChild::Rule(rule) => {
                    self.write_indent();
                    if std::mem::take(&mut format_ignore_next) {
                        self.write(rule.span.extract(self.source));
                    } else {
                        self.print_css_rule(rule);
                    }
                    // Keep a same-line comment after the closing `}` on its line.
                    i += self.try_print_inline_comments(children, i, rule.span.end);
                    self.write("\n");
                }
                CssBlockChild::Atrule(atrule) => {
                    self.write_indent();
                    if std::mem::take(&mut format_ignore_next) {
                        self.write(atrule.span.extract(self.source));
                    } else {
                        self.print_css_atrule(atrule);
                    }
                    i += self.try_print_inline_comments(children, i, atrule.span.end);
                    self.write("\n");
                }
            }
            i += 1;
        }
    }

    /// Find the position after the `;` following a declaration's span end.
    ///
    /// Declaration spans don't include the trailing `;`. In unformatted source,
    /// the `;` may be on a separate line, adding extra newlines to the gap.
    /// This scans forward past whitespace to find and skip the `;`.
    fn end_after_semicolon(&self, span_end: u32) -> u32 {
        let start = span_end as usize;
        for (i, &b) in self.source.as_bytes().iter().enumerate().skip(start) {
            match b {
                b';' => return (i + 1) as u32,
                b' ' | b'\t' | b'\n' | b'\r' => continue,
                _ => break,
            }
        }
        span_end
    }

    /// Check if there's a blank line before a block child, accounting for the
    /// previous sibling's trailing `;` (not included in declaration spans).
    pub(crate) fn has_blank_line_before_child(
        &self,
        children: &[CssBlockChild<'_>],
        index: usize,
    ) -> bool {
        let Some(prev) = children.get(index.wrapping_sub(1)) else {
            return false;
        };
        let curr_start = children[index].span().start;
        let prev_end = prev.span().end;
        let effective_end = if matches!(prev, CssBlockChild::Declaration(_)) {
            self.end_after_semicolon(prev_end)
        } else {
            prev_end
        };
        self.has_blank_line_between(effective_end, curr_start)
    }
}

/// Format CSS stylesheet to a string
/// Requires source for blank line preservation and raw value extraction
pub(crate) fn format_css(stylesheet: &CssStyleSheet<'_>, source: &str) -> String {
    let arena = DocArena::for_source(source);
    format_css_in(stylesheet, source, &arena)
}

/// Format a CSS stylesheet into a caller-provided doc arena (the reuse path).
pub(crate) fn format_css_in(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    arena: &DocArena,
) -> String {
    // Fill the arena-parked line-break table (one warm table across a
    // multi-file driver's files instead of a fresh Vec per file).
    let mut line_breaks = arena.take_line_breaks_scratch();
    printing::build_line_breaks_into(source, &mut line_breaks);
    let mut printer = Printer::new(arena, source, &stylesheet.comments, &line_breaks);
    printer.print_css_nodes(stylesheet.nodes);
    let output = printer.into_string();
    arena.park_line_breaks_scratch(line_breaks);
    output
}

/// Format a CSS stylesheet embedded in another language (e.g., Svelte), using
/// `embed.base_indent_offset` to account for the host's wrapper indentation.
pub(crate) fn format_css_embedded(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    line_breaks: &[u32],
    embed: EmbedContext,
) -> String {
    // Fresh-arena wrapper (top-level embedding tests / any caller without a host
    // arena to lend). The Svelte host reuses its own document arena via
    // `format_css_embedded_in`. `source` is the whole host document (spans are
    // absolute), so the caller's `line_breaks` is the host's whole-source table —
    // never island-local.
    let arena = DocArena::for_source(source);
    format_css_embedded_in(stylesheet, source, line_breaks, embed, &arena)
}

/// Embedded CSS formatting into a caller-provided doc arena — the arena-sharing
/// path the Svelte host uses so a `<style>` block reuses the host document's
/// `DocArena` instead of allocating a second whole-host-sized one per block.
///
/// The embedded CSS still renders through its own column-0 render pass to an
/// owned `String`; only the doc-node *storage* is shared. The arena is not
/// reset, so the host's in-flight nodes remain valid, and the CSS's transient
/// nodes are reclaimed at the host driver's next `DocArena::reset()` (between
/// files) like any other island's.
pub(crate) fn format_css_embedded_in(
    stylesheet: &CssStyleSheet<'_>,
    source: &str,
    line_breaks: &[u32],
    embed: EmbedContext,
    arena: &DocArena,
) -> String {
    // Render at the host's final indentation directly (fold `base_indent_offset`
    // into the starting `indent_level`, zeroing the offset), mirroring how the
    // `<script>` embed renders the TS doc at `start_indent_level=1`. Structural
    // lines get the wrapper indent from `write_indent` / hardlines; verbatim
    // content (raw at-rule preludes, comment interiors, `Invalid` selector text)
    // is written with no indent, so its embedded newlines stay at column 0 — the
    // same as standalone, matching prettier. This retires the Svelte host's
    // post-hoc line re-indenter, which compounded a preserved newline one indent
    // level per format pass (an F1 non-idempotency; see script_style.rs).
    let base = embed.base_indent_offset;
    let embed = EmbedContext {
        base_indent_offset: 0,
        ..embed
    };
    let mut printer = Printer::with_embed(arena, source, &stylesheet.comments, line_breaks, embed);
    printer.indent_level = base;
    printer.print_css_nodes(stylesheet.nodes);
    printer.into_string()
}
