// Decorator printing for TypeScript
//
// Class-level decorators (always own-line), class-member decorators, and
// parameter decorators (both inline vs own-line preserved from source),
// including comment placement between decorators and the decorated member.

use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::{
    CommentPosition, classify_comment, comments_in_range, doc::arena::DocId, has_comments_in_range,
    has_line_comments_in_range,
};

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
        decorator: &internal::Decorator<'_>,
    ) -> DocId {
        let d = self.d();
        let expr_doc = self.build_expression_doc(&decorator.expression);
        if can_decorator_expression_unparenthesized(&decorator.expression) {
            expr_doc
        } else {
            d.parens(expr_doc)
        }
    }

    /// Push a decorator's "head" — `@`, any comment authored between `@` and the
    /// decorator expression (`@/* c */ dec`, including inside stripped parens
    /// `@(/* c */ dec)`; dropping it is content loss, and it hugs the expression
    /// inline or drops to its own line per the author), and the expression itself —
    /// into `parts`. Shared by the class-level (`build_decorators_doc`),
    /// class-member (`build_class_member_decorators_doc`), and parameter
    /// (`build_param_decorators_doc`) printers, which each append their own
    /// trailing-comment and separator handling.
    fn push_decorator_head(&self, parts: &mut DocBuf, decorator: &internal::Decorator<'_>) {
        let d = self.d();
        parts.push(d.text("@"));
        if let Some(c) =
            self.build_rhs_comments_opt(decorator.span.start + 1, decorator.expression.span().start)
        {
            parts.push(c);
        }
        parts.push(self.build_decorator_expression_doc(decorator));
    }

    /// prettier's `hasNewlineBetweenOrAfterDecorators`: true when any decorator is
    /// followed (skipping spaces/tabs) by a newline before the next token, so the
    /// decorators break onto their own lines. Comments between the decorator and
    /// the newline do NOT count — prettier only skips spaces/tabs, so `@fn /* c */\nb`
    /// yields false (the first non-space char is `/`). Drives both class-member
    /// decorators and parameter decorators.
    pub(in crate::printer) fn has_newline_after_any_decorator(
        &self,
        decorators: &[internal::Decorator<'_>],
    ) -> bool {
        decorators.iter().any(|dec| {
            let end = dec.span.end as usize;
            self.source[end..]
                .bytes()
                .find(|&b| b != b' ' && b != b'\t')
                .is_some_and(|b| b == b'\n' || b == b'\r')
        })
    }

    /// True when `expr` is a single parameter whose decorators are written on
    /// their own line — such a parameter can't hug (the forced break would split
    /// the pattern mid-hug), so the parameter list expands instead, matching
    /// prettier.
    pub(in crate::printer) fn param_has_own_line_decorators(
        &self,
        expr: &internal::Expression<'_>,
    ) -> bool {
        param_decorators(expr).is_some_and(|decs| self.has_newline_after_any_decorator(decs))
    }

    /// The source position where a parameter's rendered form begins: its first
    /// decorator when it carries parameter decorators, else the binding itself.
    /// Decorators precede the binding but are stored *on* it, so the binding span
    /// alone skips them — measuring a blank line to it would miscount a decorator
    /// line as an author blank line.
    pub(in crate::printer) fn param_start_with_decorators(
        &self,
        expr: &internal::Expression<'_>,
    ) -> u32 {
        param_decorators(expr)
            .and_then(|decs| decs.first())
            .map_or_else(|| expr.span().start, |first| first.span.start)
    }

    /// Prefix a parameter binding's doc with its parameter decorators, preserving
    /// any comments the author interleaved with them — matching prettier's generic
    /// `printDecorators`: a decorator written on its own line in the source (a
    /// newline after any decorator) keeps each decorator on its own line and expands
    /// the parameter list (the `hardline` carries the break to the enclosing
    /// parameter group); an inline decorator stays inline, separated by a single
    /// space. A no-op when there are no decorators. Used for destructuring and
    /// default parameters (`@dec { a }: T`, `@dec a = 1`) — whose decorators acorn
    /// stores on the pattern / `AssignmentPattern` node — plus the parameter-property
    /// path. `inner_start` is the source position where the rendered binding begins
    /// (its first modifier for a parameter property, else the binding node); it
    /// bounds the scan for comments after the last decorator.
    pub(in crate::printer) fn with_param_decorators(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        inner: DocId,
        inner_start: u32,
    ) -> DocId {
        let Some(decorators) = decorators.filter(|d| !d.is_empty()) else {
            return inner;
        };
        // Common case — no comment interleaved with the decorators: emit the bare
        // `@expr <sep> … <sep> binding` flat, skipping the comment scans and the
        // per-decorator segment wrapping the comment-aware path needs.
        if !has_comments_in_range(self.comments, decorators[0].span.start, inner_start) {
            let d = self.d();
            let sep = if self.has_newline_after_any_decorator(decorators) {
                d.hardline()
            } else {
                d.text(" ")
            };
            let mut parts = DocBuf::new();
            for decorator in decorators {
                parts.push(d.text("@"));
                parts.push(self.build_decorator_expression_doc(decorator));
                parts.push(sep);
            }
            parts.push(inner);
            return d.concat(&parts);
        }
        self.build_param_decorators_doc(decorators, inner, inner_start)
    }

    /// The comment-aware core of `with_param_decorators` (decorators non-empty and
    /// at least one comment interleaved among them — the bare case takes the flat
    /// fast path). Emits each decorator plus any comment written between `@` and the
    /// decorator expression (`@/* c */ dec`), between two decorators
    /// (`@dec1 /* c */ @dec2`), or between the last decorator and the binding
    /// (`@dec /* c */ x`), at the author's position — the parameter analog of
    /// `build_class_member_decorators_doc`. Each decorator (with its inline trailing
    /// comments) and each own-line comment is one segment, the binding is the final
    /// segment, and the segments join with the layout separator (a `hardline` in the
    /// own-line / line-comment layout, a space inline). This is why a comment
    /// interleaved with parameter decorators is NOT hoisted into the leading-comment
    /// run: `build_leading_param_comments` stops collecting at the first decorator.
    fn build_param_decorators_doc(
        &self,
        decorators: &[internal::Decorator<'_>],
        inner: DocId,
        inner_start: u32,
    ) -> DocId {
        let d = self.d();

        // Own-line layout when a newline follows any decorator (prettier's
        // `hasNewlineBetweenOrAfterDecorators`) or a line comment sits among the
        // decorators — a `//` runs to end-of-line, so the binding can't share its
        // line and the `hardline` keeps it from swallowing the next token.
        let own_line = self.has_newline_after_any_decorator(decorators)
            || has_line_comments_in_range(self.comments, decorators[0].span.start, inner_start);
        let sep = if own_line { d.hardline() } else { d.text(" ") };

        let mut segments: DocBuf = DocBuf::new();
        // LeadingInline comments (authored on the *next decorator's* line) prefix
        // that decorator's segment; carried across the iteration boundary.
        let mut pending: DocBuf = DocBuf::new();
        for (i, decorator) in decorators.iter().enumerate() {
            let is_last = i == decorators.len() - 1;
            let boundary = decorators
                .get(i + 1)
                .map_or(inner_start, |next| next.span.start);

            let mut seg: DocBuf = DocBuf::new();
            for leading in std::mem::take(&mut pending) {
                seg.push(leading);
                seg.push(d.text(" "));
            }
            self.push_decorator_head(&mut seg, decorator);

            // Comments between this decorator and the next boundary: a same-line
            // trailing comment hugs the decorator inline; an own-line comment (and a
            // LeadingInline one before the *binding*, which prettier drops to its own
            // line) becomes its own segment; a LeadingInline one before the *next
            // decorator* prefixes that decorator.
            let mut own_line_segments: DocBuf = DocBuf::new();
            for comment in comments_in_range(self.comments, decorator.span.end, boundary) {
                match classify_comment(comment, decorator.span.end, boundary, self.source) {
                    CommentPosition::Trailing => {
                        seg.push(d.text(" "));
                        seg.push(self.build_comment_doc(comment));
                    }
                    CommentPosition::LeadingInline if !is_last => {
                        pending.push(self.build_comment_doc(comment));
                    }
                    CommentPosition::LeadingOwnLine | CommentPosition::LeadingInline => {
                        own_line_segments.push(self.build_comment_doc(comment));
                    }
                }
            }
            segments.push(d.concat(&seg));
            segments.extend(own_line_segments);
        }
        segments.push(inner);
        d.join_doc(segments, sep)
    }

    /// Build a Doc for a list of decorators, each on its own line
    ///
    /// Returns None if there are no decorators.
    /// Each decorator is formatted as `@expression` followed by hardline.
    /// Used for class-level decorators which always go on their own line.
    pub(in crate::printer) fn build_decorators_doc(
        &self,
        decorators: Option<&[internal::Decorator<'_>]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let d = self.d();
        let mut parts = DocBuf::new();
        for (i, decorator) in decorators.iter().enumerate() {
            self.push_decorator_head(&mut parts, decorator);
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
        decorators: Option<&[internal::Decorator<'_>]>,
        next_token_start: u32,
    ) -> Option<DocId> {
        let decorators = decorators?;
        if decorators.is_empty() {
            return None;
        }
        let d = self.d();

        // Own-line if any decorator has a newline between it and the next token
        // (prettier's hasNewlineBetweenOrAfterDecorators).
        let has_newline_after = self.has_newline_after_any_decorator(decorators);

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
            self.push_decorator_head(&mut dec_parts, decorator);

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
        let mut group_parts: DocBuf = smallvec![joined, trailing];
        if needs_break {
            group_parts.push(d.break_parent());
        }
        Some(d.group(d.concat(&group_parts)))
    }
}

/// The parameter decorators attached to a binding form (identifier / object or
/// array pattern / assignment-pattern default), reaching inside a
/// `TSParameterProperty` onto its inner binding — matching where acorn stores a
/// parameter's decorators. Returns `None` for any non-parameter or undecorated
/// form.
fn param_decorators<'arena>(
    expr: &internal::Expression<'arena>,
) -> Option<&'arena [internal::Decorator<'arena>]> {
    match expr {
        internal::Expression::Identifier(id) => id.decorators(),
        internal::Expression::ObjectPattern(obj) => obj.decorators,
        internal::Expression::ArrayPattern(arr) => arr.decorators,
        internal::Expression::AssignmentPattern(ap) => ap.decorators,
        internal::Expression::TSParameterProperty(pp) => param_decorators(pp.parameter),
        _ => None,
    }
}

/// Whether `expr` is a bare-decorator member chain: an identifier, or a
/// non-computed, non-optional member chain of identifiers down to one.
fn is_decorator_member_expression(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::Identifier(_) => true,
        internal::Expression::MemberExpression(member) => {
            !member.computed
                && !member.optional
                && matches!(member.property, internal::Expression::Identifier(_))
                && is_decorator_member_expression(member.object)
        }
        _ => false,
    }
}

/// Whether a decorator expression is valid without parens (see
/// `Printer::build_decorator_expression_doc`).
fn can_decorator_expression_unparenthesized(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::CallExpression(call) => {
            !call.optional && is_decorator_member_expression(call.callee)
        }
        _ => is_decorator_member_expression(expr),
    }
}
