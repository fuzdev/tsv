// Adapter binding the main Printer to the chain renderer
//
// Implements the chain module's ChainPrinter and SymbolLookup traits
// to enable the chain printer to delegate back to the main Printer
// for expression building and comment handling.

use super::{ChainPrinter, SymbolLookup};
use crate::ast::internal;
use crate::printer::{CommentSpacing, Printer, comments_in_range};
use string_interner::DefaultSymbol;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{ClassifiedComments, Comment};

impl<'a> SymbolLookup for Printer<'a> {
    fn with_name<R>(&self, symbol: DefaultSymbol, f: impl FnOnce(&str) -> R) -> Option<R> {
        self.interner.borrow().resolve(symbol).map(f)
    }
}

impl<'a> ChainPrinter for Printer<'a> {
    fn arena(&self) -> &DocArena {
        self.arena
    }

    fn print_expression(&self, expr: &internal::Expression) -> DocId {
        self.build_expression_doc(expr)
    }

    fn build_parenthesized_base_inner_logical(&self, binary: &internal::BinaryExpression) -> DocId {
        let d = self.d();
        let inner = self.build_binary_chain_parts_indented(binary);
        d.group(inner)
    }

    fn build_parenthesized_base_inner_binary(&self, binary: &internal::BinaryExpression) -> DocId {
        self.build_binary_chain_for_parens(binary)
    }

    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> DocId {
        self.build_call_args_doc_for_chain(call, optional)
    }

    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> DocId {
        self.build_call_args_doc_for_chain_expanded(call, optional)
    }

    fn print_call_args_standard_expanded(
        &self,
        call: &internal::CallExpression,
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

    fn get_property_span(&self, expr: &internal::Expression) -> tsv_lang::Span {
        expr.span()
    }

    fn is_expression_statement(&self) -> bool {
        self.is_expression_statement.get()
    }

    fn clear_expression_statement(&self) {
        self.is_expression_statement.set(false);
    }

    fn get_line_breaks(&self) -> &[u32] {
        self.line_breaks
    }

    fn has_comments_between(&self, start: u32, end: u32) -> bool {
        comments_in_range(self.comments, start, end)
            .next()
            .is_some()
    }

    fn classify_comments(&self, start: u32, end: u32) -> ClassifiedComments<'_> {
        ClassifiedComments::from_range(self.comments, start, end, self.line_breaks)
    }

    fn build_trailing_block_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = Vec::with_capacity(comments.len() * 2);
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
        let mut parts = Vec::with_capacity(comments.len() + 1);
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        // Add boundary to flush the line_suffix before any following softline
        parts.push(d.line_suffix_boundary());
        d.concat(&parts)
    }

    fn build_leading_block_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        // Emit block comments on their own lines (with hardline after each)
        let mut parts = Vec::with_capacity(comments.len() * 2);
        for comment in comments {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.hardline());
        }
        d.concat(&parts)
    }

    fn build_leading_line_doc(&self, comments: &[&Comment]) -> DocId {
        let d = self.d();
        if comments.is_empty() {
            return d.empty();
        }

        // Emit line comments on their own lines (with hardline after each)
        let mut parts = Vec::with_capacity(comments.len() * 2);
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
        let mut parts = Vec::with_capacity(comments.len());
        for comment in comments {
            parts.push(self.build_trailing_line_comment_doc(comment));
        }
        d.concat(&parts)
    }

    fn get_source(&self) -> &str {
        self.source
    }

    fn should_force_expand(&self) -> bool {
        self.force_chain_expand.get()
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

        let mut parts = Vec::new();
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
