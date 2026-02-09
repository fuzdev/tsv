// Parenthesized non-null chain handling
//
// Handles chains starting with "(complex)!" pattern that need special
// break handling to prefer breaking at the chain point rather than
// inside the parenthesized expression.

use crate::ast::internal::Expression;

use super::super::printing::{ChainPrinter, print_group, print_node};
use super::super::types::{ChainGroup, ChainNode};
use super::helpers::build_chain_break_doc;
use tsv_lang::doc::arena::DocId;

/// Check if chain starts with "(complex)!" pattern that needs chain-preferring breaks
///
/// Returns true if:
/// - First group is [Base(needs_parens=true), NonNull]
/// - The base expression is one that we want to keep flat (await, ternary, type assertion)
///
/// For binary expressions, we return false because they have natural internal break
/// points (at operators) and should break there rather than at the chain.
pub(super) fn is_parenthesized_non_null_head(groups: &[ChainGroup]) -> bool {
    if groups.len() < 2 {
        return false;
    }
    let first = &groups[0];
    // Need at least 2 nodes: Base and NonNull
    // Additional nodes (like consecutive members) are allowed
    if first.nodes.len() < 2 {
        return false;
    }

    // First node should be Base with needs_parens, second should be NonNull
    let ChainNode::Base {
        expr,
        needs_parens: true,
    } = &first.nodes[0]
    else {
        return false;
    };
    if !first.nodes[1].is_non_null() {
        return false;
    }

    // Check if the base expression is one we want to keep flat
    // Binary expressions have natural break points (at operators), so exclude them
    matches!(
        expr,
        Expression::AwaitExpression(_)
            | Expression::ConditionalExpression(_)
            | Expression::TSAsExpression(_)
            | Expression::TSSatisfiesExpression(_)
            | Expression::TSTypeAssertion(_)
    )
}

/// Build doc for "(complex)!.method()" chains
///
/// For await/yield expressions with non-null, Prettier prefers breaking inside
/// the call args rather than at the parens. So we use simple parens (not
/// indent-on-break) for these cases.
///
/// For other expressions (ternary, type assertions), use conditional_group to
/// prefer breaking at the chain point rather than inside the parenthesized expression.
pub(super) fn build_parenthesized_non_null_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> DocId {
    let d = printer.arena();
    // Check if base is await/yield - these use simple parens and no chain breaking
    let is_await_yield = matches!(
        groups[0].nodes.first(),
        Some(ChainNode::Base { expr, .. })
        if matches!(expr, Expression::AwaitExpression(_) | Expression::YieldExpression(_))
    );

    // First group: (complex)! [possibly followed by consecutive members]
    let first_group_doc = if is_await_yield {
        // For await/yield, use simple parens so inner call args can expand
        let ChainNode::Base { expr, .. } = &groups[0].nodes[0] else {
            unreachable!()
        };
        let inner_doc = printer.print_expression(expr);
        let parens_doc = d.parens(inner_doc);

        // Build the full first group: (expr)! + any trailing nodes (members, etc.)
        let mut parts = vec![parens_doc];
        for node in groups[0].nodes.iter().skip(1) {
            parts.push(print_node(node, printer));
        }
        d.concat(&parts)
    } else {
        print_group(&groups[0], printer)
    };

    if groups.len() == 1 {
        return first_group_doc;
    }

    // Rest: .method().prop etc
    let rest_docs: Vec<DocId> = groups[1..]
        .iter()
        .map(|g| print_group(g, printer))
        .collect();

    // oneLine: everything concatenated flat
    let mut on_line_parts = vec![first_group_doc];
    on_line_parts.extend(rest_docs.iter().copied());
    let on_line = d.concat(&on_line_parts);

    // If the first group will break internally (e.g., binary || expression),
    // just use group(oneLine) and let the inner expression break naturally.
    // For expressions that don't break internally, use conditionalGroup to
    // prefer chain breaks over inner group breaks.
    if d.will_break(first_group_doc) {
        return d.group(on_line);
    }

    // expanded: chain breaks after !, keeping inner expression flat
    let expanded = build_chain_break_doc(first_group_doc, &rest_docs, printer);

    d.conditional_group(&[on_line, expanded])
}
