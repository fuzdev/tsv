// Adapter binding the main Printer to the chain renderer
//
// Implements the chain module's ChainPrinter and SymbolLookup traits
// to enable the chain printer to delegate back to the main Printer
// for expression building and comment handling.

use super::{ChainPrinter, SymbolLookup};
use crate::ast::internal;
use crate::printer::{CommentSpacing, Printer, comments_in_range};
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{ClassifiedComments, Comment, Span};

impl<'a> SymbolLookup for Printer<'a> {
    fn with_name<R>(
        &self,
        name: internal::IdentName,
        name_start: u32,
        f: impl FnOnce(&str) -> R,
    ) -> Option<R> {
        match name.escaped {
            Some(sym) => self.interner.borrow().resolve(sym).map(f),
            None => Some(f(
                &self.source[name_start as usize..name_start as usize + name.raw_len as usize]
            )),
        }
    }
}

impl<'a> ChainPrinter for Printer<'a> {
    fn arena(&self) -> &DocArena {
        self.arena
    }

    fn ident_doc(&self, name: internal::IdentName, name_start: u32) -> DocId {
        self.ident_name_doc(name, name_start)
    }

    fn print_expression(&self, expr: &internal::Expression<'_>) -> DocId {
        self.build_expression_doc(expr)
    }

    fn in_for_init(&self) -> bool {
        self.in_for_init.get()
    }

    fn build_parenthesized_base_inner_logical(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        let d = self.d();
        let inner = self.build_binary_chain_parts_indented(binary);
        d.group(inner)
    }

    fn build_parenthesized_base_inner_binary(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId {
        self.build_binary_chain_for_parens(binary)
    }

    fn print_call_args(&self, call: &internal::CallExpression<'_>, optional: bool) -> DocId {
        self.build_call_args_doc_for_chain(call, optional)
    }

    fn print_call_args_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        self.build_call_args_doc_for_chain_expanded(call, optional)
    }

    fn print_call_args_standard_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        self.build_call_args_doc_for_chain_standard_expanded(call, optional)
    }

    fn build_block_comments_doc(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        same_line_only: bool,
    ) -> DocId {
        let block_comments = if same_line_only {
            self.filter_block_comments(start, end, true)
        } else {
            comments_in_range(self.comments, start, end)
                .filter(|c| c.is_block)
                .collect()
        };
        self.format_block_comments(&block_comments, spacing)
    }

    fn build_computed_member_line_comment_bracket(
        &self,
        open: &'static str,
        inside_start: u32,
        prop_start: u32,
        prop_end: u32,
        bracket_end: u32,
        inner: DocId,
    ) -> Option<DocId> {
        // Only the break path — a line comment before the index or after it (before
        // `]`). A block-only or comment-free bracket falls through to the caller.
        if !self.has_line_comments_between(inside_start, prop_start)
            && !self.has_line_comments_between(prop_end, bracket_end)
        {
            return None;
        }
        let d = self.d();
        // Build the body (index + any index→`]` trailing comments), then hand it to the
        // shared bracket-break helper (it owns the `[`→index line-comment prefix and the
        // break shell, mirroring the computed-key bracket). `[`→index: a `[`-line comment
        // is pulled onto the `[` line, an own-line one keeps its line (blank-preserving).
        // index→`]`: a same-line comment trails the index, an own-line one keeps its line.
        let mut body_parts = DocBuf::new();
        body_parts.push(inner);
        let mut prev = prop_end;
        for comment in comments_in_range(self.comments, prop_end, bracket_end) {
            if self.is_same_line(prev, comment.span.start) {
                body_parts.push(d.text(" "));
            } else {
                body_parts.push(d.hardline());
            }
            body_parts.push(self.build_comment_doc(comment));
            prev = comment.span.end;
        }
        // The `[` is the char just before the index region (past `?.` for `?.[`).
        Some(self.build_bracket_line_comment_break(
            open,
            inside_start - 1,
            prop_start,
            d.concat(&body_parts),
        ))
    }

    fn get_property_span(&self, expr: &internal::Expression<'_>) -> Span {
        expr.span()
    }

    fn is_expression_statement(&self) -> bool {
        self.is_expression_statement.get()
    }

    fn clear_expression_statement(&self) {
        self.is_expression_statement.set(false);
    }

    fn enter_chain_arg_share(&self) -> bool {
        Printer::enter_chain_arg_share(self)
    }

    fn exit_chain_arg_share(&self, was_active: bool) {
        Printer::exit_chain_arg_share(self, was_active);
    }

    fn get_line_breaks(&self) -> &[u32] {
        self.line_breaks
    }

    fn has_comments_between(&self, start: u32, end: u32) -> bool {
        comments_in_range(self.comments, start, end)
            .next()
            .is_some()
    }

    fn chain_has_comments(&self) -> bool {
        self.chain_has_comments.get()
    }

    fn set_chain_has_comments(&self, has_comments: bool) -> bool {
        let prev = self.chain_has_comments.get();
        self.chain_has_comments.set(has_comments);
        prev
    }

    fn restore_chain_has_comments(&self, prev: bool) {
        self.chain_has_comments.set(prev);
    }

    fn classify_comments(&self, start: u32, end: u32) -> ClassifiedComments<'_> {
        ClassifiedComments::from_range(self.comments, start, end, self.line_breaks)
    }

    fn build_trailing_block_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = DocBuf::with_capacity(comments.len() * 2);
        for comment in comments {
            // Space before comment (for inline trailing comments: `method() /* c */`)
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        d.concat(&parts)
    }

    fn build_trailing_line_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        // Line comments in chains need special handling:
        // Use line_suffix_boundary + line_suffix to keep comment with preceding call
        // The boundary ensures the comment is flushed before the next softline
        let mut parts = DocBuf::with_capacity(comments.len() + 1);
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        // Add boundary to flush the line_suffix before any following softline
        parts.push(d.line_suffix_boundary());
        d.concat(&parts)
    }

    fn build_leading_comments_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        // Emit each comment on its own line (with hardline after each)
        let mut parts = DocBuf::with_capacity(comments.len() * 2);
        for comment in comments {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.hardline());
        }
        d.concat(&parts)
    }

    fn build_line_comments_no_boundary(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        // Build line_suffix docs WITHOUT a trailing boundary.
        // The comments will stay deferred until the actual end of line.
        let mut parts = DocBuf::with_capacity(comments.len());
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        d.concat(&parts)
    }

    fn get_source(&self) -> &str {
        self.source
    }
}

impl<'a> Printer<'a> {
    /// Format a slice of block comments with the given spacing style.
    ///
    /// Shared formatting for block comments with the given spacing style.
    fn format_block_comments(&self, block_comments: &[&Comment], spacing: CommentSpacing) -> DocId {
        let d = self.d();
        if block_comments.is_empty() {
            return d.empty();
        }

        let mut parts = DocBuf::new();
        for comment in block_comments {
            match spacing {
                CommentSpacing::Leading => {
                    // Space before comment: `method() /* c */`
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    // Space after comment: `/* c */ key`
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
                CommentSpacing::None => {
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        d.concat(&parts)
    }
}
