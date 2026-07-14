// Owned leading comments — the comment/paren binding seam.
//
// A comment glued to the token after it is **bound to that token**, and the node the
// token begins prints it (`Comment::owned_by_node`, set by the parser). Every gap
// emitter and range lookup skips an owned comment (`comments_in_range`), so the comment
// travels inside its node's doc — and a paren the printer synthesizes around *any*
// enclosing expression therefore lands outside the pair instead of between the two.
//
// Without this, both emission paths put the comment in front of the parens: the gap
// emitters print it before the wrapped doc, and `prepend_removed_paren_comments` hoists
// a left-edge comment out to the front of the outermost expression that starts at the
// stripped `(`. Either way the comment ends up leading a paren it was never written
// against — inert for an annotation the next token was carrying (`/* @__PURE__ */`).

use crate::ast::internal::{self, Expression};
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;

/// The child on `expr`'s **left spine** — the one whose first token is also `expr`'s
/// first token, when there is one.
///
/// Callers must still check that the child actually *starts* where `expr` does: a
/// `NewExpression` (`new F()`) and a `ParenthesizedExpression` have children that start
/// later, so they are their own left edge.
///
/// Kept separate from `needs_parens`'s `starts_with_jsdoc_cast`, which walks the same
/// spine for a different question ("does a cast sit at the left edge"). The two lists
/// differ in exactly two arms, and merging them would be a behavior change, not a cleanup:
/// `SequenceExpression` is a no-op there (a sequence already takes parens as a unary
/// argument), but `TSInstantiationExpression` is not — adding it would newly wrap
/// `!(/** @type {A} */ (x)<T>)`, which that rule arguably *should* do but is a
/// fixtures-first change of its own.
fn left_spine_child<'x>(expr: &'x Expression<'x>) -> Option<&'x Expression<'x>> {
    Some(match expr {
        Expression::MemberExpression(m) => m.object,
        Expression::CallExpression(c) => c.callee,
        Expression::BinaryExpression(b) => b.left,
        Expression::ConditionalExpression(c) => c.test,
        Expression::AssignmentExpression(a) => a.left,
        Expression::TaggedTemplateExpression(t) => t.tag,
        Expression::SequenceExpression(s) => s.expressions.first()?,
        Expression::TSNonNullExpression(n) => n.expression,
        Expression::TSAsExpression(a) => a.expression,
        Expression::TSSatisfiesExpression(s) => s.expression,
        Expression::TSInstantiationExpression(i) => i.expression,
        Expression::UpdateExpression(u) if !u.prefix => u.argument,
        _ => return None,
    })
}

impl<'a> Printer<'a> {
    /// Prepend the comment `expr` owns, glued to its own first token.
    ///
    /// The single seam, called from `build_expression_doc` — so the comment is part of
    /// the node's doc at every one of the ~29 sites where a *parent* decides to wrap that
    /// doc in parens, present or future. Nothing else prints an owned comment.
    pub(crate) fn prepend_owned_leading_comment(&self, expr: &Expression<'_>, doc: DocId) -> DocId {
        // A JSDoc cast holds its own copy of its comment and prints it against its own
        // `(` — see `build_jsdoc_cast_doc`. Claiming it here would print it twice.
        if matches!(expr, Expression::JsdocCast(_)) {
            return doc;
        }
        let start = expr.span().start;
        // A node whose left-spine child starts here is not the innermost — that child is
        // (or something below it). Let the recursion reach it.
        if left_spine_child(expr).is_some_and(|c| c.span().start == start) {
            return doc;
        }
        self.prepend_owned_leading_comment_at(start, doc)
    }

    /// [`Self::prepend_owned_leading_comment`] keyed on a span start rather than a node,
    /// for a caller that already knows its node is the left edge.
    ///
    /// The call-argument states and the curried-arrow chain **reassemble** an arrow from
    /// its signature and body instead of routing it through `build_expression_doc`, so the
    /// seam above never runs for them and the comment would be *dropped*. An arrow is
    /// always its own left edge, so there is no innermost-node check to make.
    pub(crate) fn prepend_owned_leading_comment_at(&self, start: u32, doc: DocId) -> DocId {
        let Some(comment) = self.owned_leading_comment_at(start) else {
            return doc;
        };
        let d = self.d();
        d.concat(&[self.build_comment_doc(comment), d.text(" "), doc])
    }

    /// The owned comment ending immediately before `start`, glued to the token there.
    fn owned_leading_comment_at(&self, start: u32) -> Option<&'a internal::Comment> {
        // Cheap reject before the search: a glued block comment ends in `*/` just before
        // the token, past nothing but spaces/tabs. Almost every expression bails here.
        // A `*/` inside a string literal can reach the search, which then finds no
        // comment ending there — the spans, not the bytes, are authoritative.
        let bytes = self.source.as_bytes();
        let mut i = start as usize;
        while i > 0 && matches!(bytes[i - 1], b' ' | b'\t') {
            i -= 1;
        }
        if i < 4 || bytes.get(i - 2..i)? != b"*/" {
            return None;
        }

        let idx = self
            .comments
            .partition_point(|c| c.span.end <= start)
            .checked_sub(1)?;
        let comment = self.comments.get(idx)?;
        (comment.owned_by_node && comment.span.end as usize == i).then_some(comment)
    }
}
