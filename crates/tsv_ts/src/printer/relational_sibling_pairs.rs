//! Comma-list owners preserve the authored pair that separates a comparison from a
//! later sibling's possible type-argument closer.

use super::Printer;
use crate::ast::internal::{
    ArrowFunctionBody, BinaryExpression, BinaryOperator, Decorator, Expression, ExpressionKind,
    ObjectPatternProperty, ObjectProperty,
};
use tsv_lang::doc::arena::DocId;

/// Whether an expression or one of its expression children holds a binary satisfying
/// `predicate`. Function, class and block bodies are the walk's boundary: their statements
/// and members are not expression children. Parameters, decorators, a superclass and an
/// arrow's expression body are children and are visited; types contain no value expressions.
///
/// Shared by the opener and closer questions so their notion of an expression child cannot
/// drift. Every wrapper is explicit; a new expression kind must settle its own children.
fn expression_holds_binary<'e>(
    expr: &'e Expression<'e>,
    predicate: impl Copy + Fn(&BinaryExpression<'_>) -> bool,
) -> bool {
    let holds = |child| expression_holds_binary(child, predicate);
    let decorators_hold = |decorators: Option<&'e [Decorator<'e>]>| {
        decorators.is_some_and(|items| items.iter().any(|item| holds(&item.expression)))
    };
    match &expr.kind {
        ExpressionKind::BinaryExpression(binary) => {
            predicate(binary) || holds(binary.left) || holds(binary.right)
        }
        ExpressionKind::AssignmentExpression(assign) => holds(assign.left) || holds(assign.right),
        ExpressionKind::AssignmentPattern(assign) => {
            holds(assign.left) || holds(assign.right) || decorators_hold(assign.decorators)
        }
        ExpressionKind::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectProperty::Property(property) => holds(property.key) || holds(property.value),
                ObjectProperty::SpreadElement(spread) => holds(spread.argument),
            })
        }
        ExpressionKind::ObjectPattern(object) => {
            decorators_hold(object.decorators)
                || object.properties.iter().any(|property| match property {
                    ObjectPatternProperty::Property(property) => {
                        holds(property.key) || holds(property.value)
                    }
                    ObjectPatternProperty::RestElement(rest) => holds(rest.argument),
                })
        }
        ExpressionKind::ArrayExpression(array) => {
            array.elements.iter().flatten().any(|child| holds(child))
        }
        ExpressionKind::ArrayPattern(array) => {
            decorators_hold(array.decorators) || array.elements.iter().flatten().any(holds)
        }
        ExpressionKind::CallExpression(call) => {
            holds(call.callee) || call.arguments.iter().any(|child| holds(child))
        }
        ExpressionKind::NewExpression(new) => {
            holds(new.callee) || new.arguments.iter().any(|child| holds(child))
        }
        ExpressionKind::MemberExpression(member) => holds(member.object) || holds(member.property),
        ExpressionKind::ConditionalExpression(cond) => {
            holds(cond.test) || holds(cond.consequent) || holds(cond.alternate)
        }
        ExpressionKind::ArrowFunctionExpression(arrow) => {
            arrow.params.iter().any(holds)
                || match &arrow.body {
                    ArrowFunctionBody::Expression(body) => holds(body),
                    ArrowFunctionBody::BlockStatement(_) => false,
                }
        }
        ExpressionKind::FunctionExpression(function) => function.params.iter().any(holds),
        ExpressionKind::ClassExpression(class) => {
            decorators_hold(class.decorators) || class.super_class.is_some_and(holds)
        }
        ExpressionKind::UnaryExpression(unary) => holds(unary.argument),
        ExpressionKind::UpdateExpression(update) => holds(update.argument),
        ExpressionKind::SpreadElement(spread) => holds(spread.argument),
        ExpressionKind::RestElement(rest) => holds(rest.argument),
        ExpressionKind::AwaitExpression(await_expr) => holds(await_expr.argument),
        ExpressionKind::YieldExpression(yield_expr) => yield_expr.argument.is_some_and(holds),
        ExpressionKind::SequenceExpression(sequence) => {
            sequence.expressions.iter().any(|child| holds(child))
        }
        ExpressionKind::TemplateLiteral(template) => {
            template.expressions.iter().any(|child| holds(child))
        }
        ExpressionKind::TaggedTemplateExpression(tagged) => {
            holds(tagged.tag) || tagged.quasi.expressions.iter().any(|child| holds(child))
        }
        ExpressionKind::TSTypeAssertion(assertion) => holds(assertion.expression),
        ExpressionKind::TSAsExpression(assertion) => holds(assertion.expression),
        ExpressionKind::TSSatisfiesExpression(assertion) => holds(assertion.expression),
        ExpressionKind::TSInstantiationExpression(instantiation) => holds(instantiation.expression),
        ExpressionKind::TSNonNullExpression(non_null) => holds(non_null.expression),
        ExpressionKind::TSParameterProperty(property) => holds(property.parameter),
        ExpressionKind::ImportExpression(import) => {
            holds(import.source) || import.options.is_some_and(holds)
        }
        ExpressionKind::JsdocCast(cast) => holds(cast.inner),
        ExpressionKind::ParenthesizedExpression(paren) => holds(paren.expression),
        ExpressionKind::Literal(_)
        | ExpressionKind::Identifier(_)
        | ExpressionKind::PrivateIdentifier(_)
        | ExpressionKind::RegexLiteral(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Super(_)
        | ExpressionKind::MetaProperty(_) => false,
    }
}

/// Whether `expr` prints in a pair of its own at a comma-list element position — an
/// assignment, a sequence or a JSDoc cast — so a retained authored pair would wrap it twice.
fn supplies_its_own_pair(expr: &Expression<'_>) -> bool {
    matches!(
        expr.kind,
        ExpressionKind::AssignmentExpression(_)
            | ExpressionKind::SequenceExpression(_)
            | ExpressionKind::JsdocCast(_)
    )
}

impl Printer<'_> {
    /// Mark authored pairs around non-last comma-list elements holding a binary `<` / `<<`
    /// when a later sibling holds a binary `may_close_type_arguments` closer. Called by
    /// each list owner before any element doc is built, including alternate layouts, so a
    /// shared argument doc cannot predate its mark.
    ///
    /// Non-last is the INDEX, not the following comma byte: a trailing comma disappears
    /// on print, while a later sibling remains. Only a non-last element can read the next
    /// significant `)` as its own shell rather than the list's closer. An assignment,
    /// sequence or JSDoc cast supplies its own pair and is not wrapped again. The closer
    /// is the parser's flag, never a guess from its operator or follower; grouping pairs
    /// re-anchor the region, so keeping one cannot change that flag on the next pass.
    /// No pair is ever synthesized where the source has none.
    ///
    /// Array holes remain list positions. The source-shell preflight avoids walking
    /// expression trees on lists with no authored pair; the suffix closer is computed
    /// once from the right, never separately for every candidate.
    pub(in crate::printer) fn mark_relational_sibling_authored_pairs<'e>(
        &self,
        elements: impl Clone
        + DoubleEndedIterator<Item = Option<&'e Expression<'e>>>
        + ExactSizeIterator,
    ) {
        let len = elements.len();
        if len < 2
            || !elements.clone().take(len - 1).flatten().any(|expr| {
                self.paren_shell_close_after(expr.span().end).is_some()
                    && !supplies_its_own_pair(expr)
            })
        {
            return;
        }
        let mut later_closer = false;
        for (i, element) in elements.enumerate().rev() {
            let Some(expr) = element else { continue };
            if i + 1 < len
                && later_closer
                && !supplies_its_own_pair(expr)
                && self.paren_shell_close_after(expr.span().end).is_some()
                && expression_holds_binary(expr, |binary| {
                    matches!(
                        binary.operator,
                        BinaryOperator::LessThan | BinaryOperator::LeftShift
                    )
                })
            {
                self.relational_sibling_pair_targets.mark(expr.span());
                self.has_relational_sibling_pairs.set(true);
            }
            if !later_closer {
                later_closer =
                    expression_holds_binary(expr, |binary| binary.may_close_type_arguments);
            }
        }
    }

    /// `doc` inside the authored pair the list owner kept around `expr`
    /// ([`Self::relational_sibling_pair_is_kept`]), for a carrier that prints its elements
    /// itself rather than through `needs_parens(Argument)`. `already_parenthesized` is the
    /// position's own pair around the same element (a `for` header's `[~In]` pair), which
    /// is the kept pair when present: wrapping again would print it twice.
    pub(in crate::printer) fn wrap_relational_sibling_pair(
        &self,
        expr: &Expression<'_>,
        already_parenthesized: bool,
        doc: DocId,
    ) -> DocId {
        if !already_parenthesized && self.relational_sibling_pair_is_kept(expr) {
            self.d().parens(doc)
        } else {
            doc
        }
    }

    /// The pair recorded by the list owner, with a document-level fast gate so an ordinary
    /// expression never probes the set. Marks are never consumed: every layout candidate
    /// asks the same question of the same span.
    #[inline]
    pub(in crate::printer) fn relational_sibling_pair_is_kept(
        &self,
        expr: &Expression<'_>,
    ) -> bool {
        self.has_relational_sibling_pairs.get()
            && self.relational_sibling_pair_targets.is_marked(expr.span())
    }
}
