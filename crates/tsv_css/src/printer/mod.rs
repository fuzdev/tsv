// CSS printer - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates printing of CSS nodes, core Printer
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
// 4. **Reusability**: Shared printing logic (selectors) used by multiple modules
// 5. **Hierarchy-Following**: Module structure mirrors CSS spec (rules → declarations → values)

mod atrules;
mod declarations;
mod rules;
mod selectors;
pub mod value_normalization;
mod values;

use crate::ast::internal::{Comment, CssBlockChild, CssNode, CssStyleSheet, CssValue};
use tsv_lang::{
    CommentPosition, EmbedContext, INDENT, OutputBuffer, TAB_WIDTH, classify_comment_fast,
    doc::{
        self,
        arena::{DocArena, DocId},
    },
    is_format_ignore_directive, printing,
};

/// Check if function args have wrappable content (break points)
///
/// Returns true if:
/// 1. Multiple comma-separated args (linear-gradient, rgb, etc.)
/// 2. Single arg that is a List with multiple space-separated items (drop-shadow)
pub(crate) fn has_wrappable_args(args: &[CssValue]) -> bool {
    args.len() >= 2
        || (args.len() == 1
            && matches!(&args[0], CssValue::List { values, .. } if values.len() >= 2))
}

/// Printer state for building output
pub(crate) struct Printer<'a> {
    /// Output buffer
    buffer: OutputBuffer,
    /// Current indentation level
    pub(crate) indent_level: usize,
    /// Embedding context (base indent offset, first-line offset, layout mode, etc.).
    pub(crate) embed: EmbedContext,
    /// Arena allocator for doc nodes
    pub(crate) arena: DocArena,
    /// Original source (for blank line detection and raw value extraction)
    pub(crate) source: &'a str,
    /// All comments sorted by span.start
    pub(crate) comments: &'a [Comment],
    /// Precomputed line break positions for O(log n) line boundary lookups
    pub(crate) line_breaks: &'a [u32],
}

impl<'a> Printer<'a> {
    /// Create a new printer with source, comments, and line_breaks
    pub(crate) fn new(source: &'a str, comments: &'a [Comment], line_breaks: &'a [u32]) -> Self {
        Self::with_embed(source, comments, line_breaks, EmbedContext::default())
    }

    /// Create a new printer with the given embedding context.
    pub(crate) fn with_embed(
        source: &'a str,
        comments: &'a [Comment],
        line_breaks: &'a [u32],
        embed: EmbedContext,
    ) -> Self {
        Self {
            buffer: OutputBuffer::with_capacity(source.len()),
            indent_level: 0,
            embed,
            arena: DocArena::for_source(source),
            source,
            comments,
            line_breaks,
        }
    }

    /// Get a reference to the doc arena (convenience for `&self.arena`).
    #[inline]
    pub(crate) fn d(&self) -> &DocArena {
        &self.arena
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
    pub(crate) fn has_value_comments_in_decl(
        &self,
        decl: &crate::ast::internal::CssDeclaration,
    ) -> bool {
        let decl_source = decl.span.extract(self.source);
        if let Some(colon_pos) = value_normalization::find_declaration_colon(decl_source) {
            let value_part = &decl_source[colon_pos + 1..];
            value_part.contains("/*")
        } else {
            false
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
    /// For continuation lines in wrapped at-rule preludes / media queries, which
    /// indent one or two levels past the statement. The caller writes the preceding
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

    /// Check if the output buffer ends with a newline
    pub(crate) fn output_ends_with_newline(&self) -> bool {
        self.buffer.ends_with('\n')
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
        let output = doc::arena_print_doc_with_indent(
            &self.arena,
            d,
            &self.embed,
            current_col,
            self.indent_level,
        );
        self.write(&output);
    }

    /// Get the formatted output
    pub(crate) fn into_string(self) -> String {
        self.buffer.into_string()
    }

    /// Print a list of CSS nodes (rules) with comments interspersed by position
    pub(crate) fn print_css_nodes(&mut self, nodes: &[CssNode]) {
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
                .any(|c| is_format_ignore_directive(&c.content));

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

                if self.has_blank_line_between_spans(gap_start, node_start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

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
            if printed > 0 {
                // Check if this comment is on the same line as the previous comment
                if self.is_same_line(last_end, comment.span.start) {
                    self.write(" ");
                } else if self.has_blank_line_between_spans(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            } else if prev_end > 0 {
                // First comment after a node
                if self.has_blank_line_between_spans(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
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
                if self.has_blank_line_between_spans(last_end, comment.span.start) {
                    self.write("\n\n");
                } else {
                    self.write("\n");
                }
            }

            self.print_css_comment(comment);
            last_end = comment.span.end;
            *comment_idx += 1;
            printed = true;
        }

        printed
    }

    /// Check if there's an opening brace between two spans
    ///
    /// Used to detect if a comment is inside a block (after `{`) vs after a selector (before `{`)
    pub(crate) fn has_opening_brace_between(&self, prev_end: u32, curr_start: u32) -> bool {
        let prev_end = prev_end as usize;
        let curr_start = curr_start as usize;

        if prev_end > curr_start || curr_start > self.source.len() {
            return false;
        }

        let between = &self.source[prev_end..curr_start];
        between.contains('{')
    }

    /// Normalize comment spacing in raw strings
    ///
    /// Print a single CSS node
    fn print_css_node(&mut self, node: &CssNode) {
        match node {
            CssNode::Rule(rule) => self.print_css_rule(rule),
            CssNode::Atrule(atrule) => self.print_css_atrule(atrule),
        }
    }

    /// Print a CSS comment
    pub(crate) fn print_css_comment(&mut self, comment: &Comment) {
        // Write comment with delimiters - content is preserved exactly as written
        self.write("/*");
        self.write(&comment.content);
        self.write("*/");
    }

    /// Try to print inline comments after the current item
    ///
    /// Checks if the next item is a comment on the same line as `prev_end`.
    /// If so, prints it inline and returns the number of comments consumed.
    ///
    /// This consolidates the repeated pattern across rules.rs and atrules.rs.
    pub(crate) fn try_print_inline_comments(
        &mut self,
        children: &[CssBlockChild],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        let mut consumed = 0;
        let mut last_end = prev_end;

        while let Some(CssBlockChild::Comment(next_comment)) =
            children.get(current_idx + 1 + consumed)
            && self.is_same_line(last_end, next_comment.span.start)
        {
            self.write(" /*");
            self.write(&next_comment.content);
            self.write("*/");
            last_end = next_comment.span.end;
            consumed += 1;
        }

        consumed
    }

    /// Try to print inline comments after a declaration
    ///
    /// Similar to `try_print_inline_comments` but handles the declaration-specific
    /// case where we need to remove the trailing newline before the first comment.
    pub(crate) fn try_print_inline_comments_after_decl(
        &mut self,
        children: &[CssBlockChild],
        current_idx: usize,
        prev_end: u32,
    ) -> usize {
        let mut consumed = 0;
        let mut last_end = prev_end;

        while let Some(CssBlockChild::Comment(next_comment)) =
            children.get(current_idx + 1 + consumed)
            && self.is_same_line(last_end, next_comment.span.start)
        {
            if consumed == 0 {
                // First inline comment - remove the trailing newline from declaration
                self.buffer_remove_trailing_newline();
            }
            self.write(" /*");
            self.write(&next_comment.content);
            self.write("*/");
            last_end = next_comment.span.end;
            consumed += 1;
        }

        if consumed > 0 {
            self.write("\n");
        }

        consumed
    }

    /// Check if there's a blank line between two spans in the source
    pub(crate) fn has_blank_line_between_spans(&self, prev_end: u32, curr_start: u32) -> bool {
        self.has_blank_line_between(prev_end, curr_start)
    }

    /// Check if previous sibling is a comment
    pub(crate) fn prev_is_comment(children: &[CssBlockChild], index: usize) -> bool {
        index > 0 && matches!(children.get(index - 1), Some(CssBlockChild::Comment(_)))
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
        children: &[CssBlockChild],
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
        self.has_blank_line_between_spans(effective_end, curr_start)
    }
}

/// Format CSS stylesheet to a string
/// Requires source for blank line preservation and raw value extraction
pub(crate) fn format_css(stylesheet: &CssStyleSheet, source: &str) -> String {
    let mut printer = Printer::new(source, &stylesheet.comments, &stylesheet.line_breaks);
    printer.print_css_nodes(&stylesheet.nodes);
    printer.into_string()
}

/// Format a CSS stylesheet embedded in another language (e.g., Svelte), using
/// `embed.base_indent_offset` to account for the host's wrapper indentation.
pub(crate) fn format_css_embedded(
    stylesheet: &CssStyleSheet,
    source: &str,
    embed: EmbedContext,
) -> String {
    let mut printer =
        Printer::with_embed(source, &stylesheet.comments, &stylesheet.line_breaks, embed);
    printer.print_css_nodes(&stylesheet.nodes);
    printer.into_string()
}
