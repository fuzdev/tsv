// Decorator printing for TypeScript
//
// Class-level decorators (always own-line) and class-member decorators
// (inline vs own-line preserved from source), including comment placement
// between decorators and the decorated member.

use crate::ast::internal;
use tsv_lang::doc::DocBuf;
use tsv_lang::{CommentPosition, classify_comment, comments_in_range, doc::arena::DocId};

use super::Printer;

impl<'a> Printer<'a> {
    /// Build a Doc for a decorator's expression, parenthesizing when the bare
    /// decorator grammar doesn't cover it.
    ///
    /// Bare form: an identifier / non-computed non-optional member chain, or one
    /// non-optional call on such a chain — anything else (`@(fn().fn1())`,
    /// `@(a?.b)`, `@(a[b])`) keeps parens. Prettier ref:
    /// `canDecoratorExpressionUnparenthesized` in parentheses/parent-needs-parentheses.js.
    pub(in crate::printer) fn build_decorator_expression_doc(
        &self,
        decorator: &internal::Decorator,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decorator.expression);
        if can_decorator_expression_unparenthesized(&decorator.expression) {
            expr_doc
        } else {
            d.parens(expr_doc)
        }
    }

    /// Build a Doc for a list of decorators, each on its own line
    ///
    /// Returns None if there are no decorators.
    /// Each decorator is formatted as `@expression` followed by hardline.
    /// Used for class-level decorators which always go on their own line.
    pub(in crate::printer) fn build_decorators_doc(
        &self,
        decorators: Option<&[internal::Decorator]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, decorator) in decorators.iter().enumerate() {
            parts.push(d.text("@"));
            parts.push(self.build_decorator_expression_doc(decorator));
            // Check for trailing comments after decorator: `@expr /* c */`
            // Boundary is next decorator's start, or next_token_start for the last one
            let boundary = decorators
                .get(i + 1)
                .map_or(next_token_start, |next| next.span.start);
            if self.has_comments_between(decorator.span.end, boundary) {
                let comment_doc =
                    self.build_inline_comments_between_doc(decorator.span.end, boundary);
                parts.push(comment_doc);
            }
            parts.push(d.hardline());
        }
        Some(d.concat(&parts))
    }

    /// Build a Doc for class member decorators (properties and methods)
    ///
    /// Returns None if there are no decorators.
    /// Prettier preserves the original formatting: if any decorator has a newline
    /// after it in the source, all decorators go on their own lines. Otherwise,
    /// decorators stay inline (separated by spaces).
    ///
    /// Comments between decorators and between the last decorator and the member
    /// are handled here. Classification determines placement:
    /// - Trailing (same line as decorator): emitted inline after decorator
    /// - LeadingInline (same line as next token): emitted inline before next decorator
    /// - LeadingOwnLine: emitted as a separate line item
    ///
    /// Prettier ref: `printClassMemberDecorators` in print/decorators.js
    /// uses `hasNewlineBetweenOrAfterDecorators` to decide `hardline` vs `line`.
    pub(in crate::printer) fn build_class_member_decorators_doc(
        &self,
        decorators: Option<&[internal::Decorator]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let d = self.d();

        // Check if any decorator has a newline between it and the next token.
        // Mirrors prettier's hasNewlineBetweenOrAfterDecorators: skip spaces/tabs
        // from locEnd(decorator), check if the next non-space char is a newline.
        // Comments between decorator and newline do NOT count — prettier only skips
        // spaces/tabs, so a comment like `@fn /* c */\nb` makes the first non-space
        // char '/' (not '\n'), resulting in false.
        let has_newline_after = decorators.iter().any(|dec| {
            let end = dec.span.end as usize;
            self.source[end..]
                .bytes()
                .find(|&b| b != b' ' && b != b'\t')
                .is_some_and(|b| b == b'\n' || b == b'\r')
        });

        // Track whether any line comment exists — line comments force the group
        // to break regardless of has_newline_after (the line comment takes up the
        // rest of the line, making flat mode impossible).
        let mut has_line_comment = false;

        // Build items for join(line, items).
        // Each item is a decorator doc (possibly with trailing/leading inline comments).
        // Own-line comments become separate items.
        let mut items: DocBuf = DocBuf::new();
        let mut pending_leading: DocBuf = DocBuf::new();

        for (i, decorator) in decorators.iter().enumerate() {
            let boundary = decorators
                .get(i + 1)
                .map_or(next_token_start, |next| next.span.start);

            // Build decorator doc with any pending leading inline comments
            let mut dec_parts: DocBuf = DocBuf::new();
            for leading in std::mem::take(&mut pending_leading) {
                dec_parts.push(leading);
                dec_parts.push(d.text(" "));
            }
            dec_parts.push(d.text("@"));
            dec_parts.push(self.build_decorator_expression_doc(decorator));

            // Handle comments between this decorator and the next boundary.
            // Comments between two decorators: ALL treated as leading on the next
            // decorator (matches prettier's comment attachment which visits `key`
            // before `decorators` in the tree walk).
            // Comments between the last decorator and the member: use classify_comment
            // to determine position (trailing stays inline, own-line gets own line).
            let is_last = i == decorators.len() - 1;
            let mut own_line_comments: DocBuf = DocBuf::new();
            for comment in comments_in_range(self.comments, decorator.span.end, boundary) {
                if !comment.is_block {
                    has_line_comment = true;
                }
                if !is_last {
                    // Between decorators: line comments stay trailing on the
                    // current decorator (they take the rest of the line).
                    // Block comments: if on the same line as the next decorator,
                    // they're leading inline; otherwise, on their own line.
                    if !comment.is_block {
                        // Line comment: trailing on current decorator
                        dec_parts.push(d.text(" "));
                        dec_parts.push(self.build_comment_doc(comment));
                    } else if self.is_same_line(comment.span.end, boundary) {
                        pending_leading.push(self.build_comment_doc(comment));
                    } else {
                        own_line_comments.push(self.build_comment_doc(comment));
                    }
                } else {
                    // Last decorator: classify comment position
                    let position =
                        classify_comment(comment, decorator.span.end, boundary, self.source);
                    match position {
                        CommentPosition::Trailing => {
                            dec_parts.push(d.text(" "));
                            dec_parts.push(self.build_comment_doc(comment));
                        }
                        CommentPosition::LeadingInline => {
                            pending_leading.push(self.build_comment_doc(comment));
                        }
                        CommentPosition::LeadingOwnLine => {
                            own_line_comments.push(self.build_comment_doc(comment));
                        }
                    }
                }
            }

            items.push(d.concat(&dec_parts));
            items.extend(own_line_comments);
        }

        // Handle remaining leading inline comments (leading on the member)
        if !pending_leading.is_empty() {
            let mut parts: DocBuf = DocBuf::new();
            for (j, leading) in pending_leading.into_iter().enumerate() {
                if j > 0 {
                    parts.push(d.text(" "));
                }
                parts.push(leading);
            }
            items.push(d.concat(&parts));
        }

        // group([join(line, items), hardline_or_line])
        // Between items: `line` (space in flat, newline in break)
        // After last item: `hardline` if source has newlines (forces group
        // to break), `line` otherwise (stays flat if group fits)
        let trailing = if has_newline_after {
            d.hardline()
        } else {
            d.line()
        };
        // Line comments and own-line block comments force the group to break.
        // Line comments take the rest of the line, making flat mode impossible.
        // Own-line block comments produce extra items in the join list.
        let needs_break = has_line_comment || items.len() > decorators.len();
        let joined = d.join_doc(items, d.line());
        let mut group_parts = vec![joined, trailing];
        if needs_break {
            group_parts.push(d.break_parent());
        }
        Some(d.group(d.concat(&group_parts)))
    }
}

/// Whether `expr` is a bare-decorator member chain: an identifier, or a
/// non-computed, non-optional member chain of identifiers down to one.
fn is_decorator_member_expression(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::Identifier(_) => true,
        internal::Expression::MemberExpression(member) => {
            !member.computed
                && !member.optional
                && matches!(&*member.property, internal::Expression::Identifier(_))
                && is_decorator_member_expression(&member.object)
        }
        _ => false,
    }
}

/// Whether a decorator expression is valid without parens (see
/// `Printer::build_decorator_expression_doc`).
fn can_decorator_expression_unparenthesized(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::CallExpression(call) => {
            !call.optional && is_decorator_member_expression(&call.callee)
        }
        _ => is_decorator_member_expression(expr),
    }
}
