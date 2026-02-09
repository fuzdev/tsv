// Parenthesized await chain handling
//
// Handles chains starting with "(await ...)" pattern that need special
// break handling to prefer breaking at the chain point rather than
// inside the parenthesized await expression.

use crate::ast::internal::Expression;

use super::super::printing::{ChainPrinter, print_group, print_node};
use super::super::types::{ChainGroup, ChainNode};
use super::helpers::build_chain_break_doc;
use tsv_lang::doc::arena::DocId;

/// Check if chain starts with "(await ...)" pattern that needs chain-preferring breaks
///
/// Returns true if:
/// - First group's first node is Base(needs_parens=true) with an AwaitExpression
/// - First group does NOT start with [Base, NonNull] (that's handled by is_parenthesized_non_null_head)
///
/// The first group may contain additional nodes (consecutive members like `.prop`)
/// which are grouped together by Prettier's algorithm. These trailing members will
/// break together with the rest of the chain when needed.
///
/// Example: `(await fn({...})).prop?.method_long()` groups as:
/// - Group 0: [(Base await), (Member .prop)]
/// - Group 1: [(Member ?.method_long), (Call)]
///
/// When the chain exceeds print_width, we break at the group boundary (after `.prop`).
pub(super) fn is_parenthesized_await_head(groups: &[ChainGroup]) -> bool {
    if groups.len() < 2 {
        return false;
    }
    let first = &groups[0];
    if first.nodes.is_empty() {
        return false;
    }

    // First node should be Base with needs_parens=true and an AwaitExpression
    let ChainNode::Base {
        expr,
        needs_parens: true,
    } = &first.nodes[0]
    else {
        return false;
    };

    // Check it's an await expression
    if !matches!(expr, Expression::AwaitExpression(_)) {
        return false;
    }

    // Reject if second node is NonNull - that's the (await)!.method() pattern
    // handled by is_parenthesized_non_null_head instead
    if first.nodes.len() >= 2 && first.nodes[1].is_non_null() {
        return false;
    }

    true
}

/// Build doc for "(await ...).method()" chains
///
/// Uses conditional_group to prefer breaking at the chain point rather than
/// inside the parenthesized await expression. This matches prettier's behavior:
/// - First tries to fit everything on one line
/// - If that doesn't fit, breaks at the chain point (after `)`), keeping await expression flat
/// - If the await expression itself exceeds print width, it expands internally
///
/// Example:
/// ```text
/// // Fits on one line (100 chars):
/// const a = (await fn({...o, a: bbb})).prop?.method();
///
/// // Doesn't fit (101 chars) - breaks at chain, await stays flat:
/// const b = (
///     await fn({...o, a: bbbb})
/// ).prop?.method();
///
/// // Inner await exceeds 100 - breaks at chain AND expands await:
/// const d = (
///     await fn({
///         ...o,
///         a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
///     })
/// ).prop?.method();
/// ```
pub(super) fn build_parenthesized_await_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> DocId {
    let d = printer.arena();
    // First group: (await ...)
    // print_group calls print_parenthesized_base which returns the indent-on-break structure
    let first_group_doc = print_group(&groups[0], printer);

    if groups.len() == 1 {
        return first_group_doc;
    }

    // Build first group with hardline-expanded parens for args_break state.
    let first_group_expanded = print_first_group_with_expanded_parens(&groups[0], printer);

    // Rest: .method().prop etc
    let rest_docs: Vec<DocId> = groups[1..]
        .iter()
        .map(|g| print_group(g, printer))
        .collect();

    // Check if first group will break (e.g., objects with group_break have should_break=true).
    // When will_break is true, conditional_group won't work correctly because fits() measures
    // flat content but the actual render will be expanded. Use direct fits() check instead.
    let first_will_break = d.will_break(first_group_doc);

    if first_will_break {
        // Count calls to determine threshold: multi-call breaks at >= print_width, single at >
        let call_count = groups[0]
            .nodes
            .iter()
            .skip(1)
            .chain(groups[1..].iter().flat_map(|g| g.nodes.iter()))
            .filter(|n| matches!(n, ChainNode::Call { .. }))
            .count();

        // Build chain tail doc: ")" + trailing nodes from first group + rest groups
        // This contains no should_break groups, so fits() measures it accurately.
        let first_group_trailing: Vec<DocId> = groups[0]
            .nodes
            .iter()
            .skip(1)
            .map(|n| print_node(n, printer))
            .collect();
        let mut chain_tail_parts = vec![d.text(")")];
        chain_tail_parts.extend(first_group_trailing);
        chain_tail_parts.extend(rest_docs.iter().copied());
        let chain_tail_doc = d.concat(&chain_tail_parts);

        // Available width = print_width - base_indent - threshold_adjustment
        // base_indent: 2 tabs (function body + assignment) = 4 visual chars
        // threshold_adjustment: -2 for multi-call (>= behavior), -1 for single-call (> behavior)
        let base_indent = printer.get_tab_width() * 2;
        let print_width = printer.get_print_width();
        let threshold_adj = if call_count > 1 { 2 } else { 1 };
        let available = print_width.saturating_sub(base_indent + threshold_adj);

        // Choose between args_break (chain inline) and chain_break (chain on new lines)
        let mut args_break_parts = vec![first_group_expanded];
        args_break_parts.extend(rest_docs.iter().copied());
        let args_break = d.concat(&args_break_parts);
        let chain_break = build_chain_break_doc(first_group_expanded, &rest_docs, printer);

        return if printer.fits_chain_tail(chain_tail_doc, available) {
            args_break
        } else {
            chain_break
        };
    }

    // First group doesn't break - use conditional_group with hardline-expanded args_break
    // so fits() can correctly measure the actual line widths.
    //
    // Build three states:
    // 1. on_line: everything flat (parens and chain inline)
    // 2. args_break: parens expanded with hardlines, chain inline
    // 3. chain_break: parens expanded, chain also breaks

    // State 1: Everything on one line
    let mut on_line_parts = vec![first_group_doc];
    on_line_parts.extend(rest_docs.iter().copied());
    let on_line = d.concat(&on_line_parts);

    // State 2: Parens expanded (hardlines), chain stays together
    let mut args_break_parts = vec![first_group_expanded];
    args_break_parts.extend(rest_docs.iter().copied());
    let args_break = d.concat(&args_break_parts);

    // State 3: Chain breaks after first group
    let chain_break = build_chain_break_doc(first_group_expanded, &rest_docs, printer);

    // Let conditional_group decide via fits()
    d.conditional_group(&[on_line, args_break, chain_break])
}

/// Print the first group with hardline-expanded parens for the base expression.
///
/// Used for `args_break` state so fits() can measure actual broken line widths.
fn print_first_group_with_expanded_parens<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> DocId {
    let d = printer.arena();
    let mut docs = Vec::with_capacity(group.nodes.len());
    for (i, node) in group.nodes.iter().enumerate() {
        if i == 0 {
            // First node is the parenthesized base - use expanded version
            if let ChainNode::Base {
                expr,
                needs_parens: true,
            } = node
            {
                docs.push(printer.print_parenthesized_base_expanded(expr));
            } else {
                docs.push(print_node(node, printer));
            }
        } else {
            docs.push(print_node(node, printer));
        }
    }
    d.concat(&docs)
}
