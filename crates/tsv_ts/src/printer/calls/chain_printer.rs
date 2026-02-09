// ChainPrinter trait implementation for Printer
//
// Implements the chain module's ChainPrinter and SymbolLookup traits
// to enable the chain printer to delegate back to the main Printer
// for expression building and comment handling.

use super::super::chain::{ChainPrinter, SymbolLookup};
use super::super::{CommentSpacing, Printer, comments_in_range};
use crate::ast::internal;
use string_interner::DefaultSymbol;
use tsv_lang::doc::arena::{DocArena, DocId};
use tsv_lang::{ClassifiedComments, Comment};

impl<'a> SymbolLookup for Printer<'a> {
    fn lookup(&self, symbol: DefaultSymbol) -> Option<String> {
        self.interner
            .borrow()
            .resolve(symbol)
            .map(ToString::to_string)
    }
}

impl<'a> ChainPrinter for Printer<'a> {
    fn arena(&self) -> &DocArena {
        self.arena
    }

    fn print_expression(&self, expr: &internal::Expression) -> DocId {
        self.build_expression_doc(expr)
    }

    fn print_parenthesized_base(&self, expr: &internal::Expression) -> DocId {
        let d = self.d();
        // Build the inner expression with proper grouping for parenthesized chains
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Different structures for different operator types:
                // - Arithmetic: parens_break for indent-on-break structure
                // - Logical (&&, ||): Keep original structure with continuation indent
                if binary.operator.is_logical() {
                    let inner = self.build_binary_chain_parts_indented(binary);
                    d.group(d.parens(inner))
                } else {
                    let inner = self.build_binary_chain_for_parens(binary);
                    d.parens_break(inner)
                }
            }
            // Await in chain base: (await fn(...)).method()
            // Use parens_break so chain prefers breaking at parens over inside call args
            // Note: YieldExpression uses simple parens - Prettier treats yield differently
            internal::Expression::AwaitExpression(_) => {
                let inner = self.build_expression_doc(expr);
                d.parens_break(inner)
            }
            _ => d.parens(self.build_expression_doc(expr)),
        }
    }

    fn print_parenthesized_base_expanded(&self, expr: &internal::Expression) -> DocId {
        let d = self.d();
        // Expanded version with hardlines so fits() can measure actual line widths.
        // Used for args_break state in conditional_group.
        let inner = self.build_expression_doc(expr);
        d.concat(&[
            d.text("("),
            d.indent(d.concat(&[d.hardline(), inner])),
            d.hardline(),
            d.text(")"),
        ])
    }

    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> DocId {
        self.build_call_args_doc_for_chain(call, optional)
    }

    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> DocId {
        self.build_call_args_doc_for_chain_expanded(call, optional)
    }

    fn build_block_comments_doc(&self, start: u32, end: u32, spacing: CommentSpacing) -> DocId {
        let d = self.d();
        // Filter block comments based on position:
        // - Leading spacing: typically used for trailing position (same line as start)
        // - Trailing spacing: typically used for inside brackets (space after comment)
        // The position filter is tied to the semantics of where we're inserting:
        // - Leading (space before): we're adding inline with previous element, so same line
        // - Trailing (space after): we're adding inside brackets, also same line
        let same_line = !matches!(spacing, CommentSpacing::None);
        let block_comments = self.filter_block_comments(start, end, same_line);

        if block_comments.is_empty() {
            return d.empty();
        }

        let mut parts = Vec::new();
        for comment in block_comments {
            match spacing {
                CommentSpacing::Leading => {
                    // Space before comment (for inline trailing comments: `method() /* c */`)
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    // Space after comment (for inside brackets: `[/* c */ key]`)
                    parts.push(self.build_comment_doc(comment));
                    parts.push(d.text(" "));
                }
                CommentSpacing::None => {
                    // Different line comments - filter gave us nothing since same_line=false
                    parts.push(self.build_comment_doc(comment));
                }
            }
        }
        d.concat(&parts)
    }

    fn get_property_span(&self, expr: &internal::Expression) -> tsv_lang::Span {
        expr.span()
    }

    fn is_expression_statement(&self) -> bool {
        self.is_expression_statement.get()
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

        let mut parts = Vec::with_capacity(comments.len());
        for comment in comments {
            // Different line block comments - no surrounding spaces
            parts.push(self.build_comment_doc(comment));
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

    fn get_tab_width(&self) -> usize {
        self.config.tab_width
    }

    fn get_print_width(&self) -> usize {
        self.config.print_width
    }

    fn should_force_expand(&self) -> bool {
        self.force_chain_expand.get()
    }

    fn fits_chain_tail(&self, doc: DocId, available: usize) -> bool {
        use tsv_lang::doc::{Mode, arena_fits};
        arena_fits(
            self.arena,
            doc,
            available,
            Mode::Flat,
            &self.config,
            Some(&*self.interner.borrow()),
        )
    }
}
