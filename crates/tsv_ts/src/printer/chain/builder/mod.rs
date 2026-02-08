// Chain doc building for TypeScript member chain formatting
//
// This module handles the main doc-building logic for chain formatting:
// - build_chain_doc: main entry point
// - Submodules handle specific chain patterns
//
// ## Architecture
//
// - **await_chains.rs**: Parenthesized await chain handling
// - **non_null_chains.rs**: Parenthesized non-null chain handling
// - **member_only.rs**: Member-only chains using fill()
// - **expansion.rs**: Chain expansion analysis helpers
// - **helpers.rs**: Shared utilities and ChainPartsBuilder

mod await_chains;
mod expansion;
mod helpers;
mod member_only;
mod non_null_chains;

use await_chains::{build_parenthesized_await_chain_doc, is_parenthesized_await_head};
use expansion::{
    call_callback_status, call_has_complex_args, ends_with_member, has_blank_lines_between_methods,
    has_comments_forcing_expansion, has_param_type_annotation,
};
use helpers::{
    build_expanded_doc, build_first_groups_doc, build_first_groups_expanded_doc,
    build_rest_parts_with_comments,
};
use member_only::build_member_only_chain_doc;
use non_null_chains::{build_parenthesized_non_null_chain_doc, is_parenthesized_non_null_head};

use super::analysis::should_merge_first_groups;
use super::printing::{ChainPrinter, print_group, print_group_expanded};
use super::types::{ChainGroup, ChainNode};
use crate::ast::internal::{ArrowFunctionBody, Expression};
use crate::printer::utils::contains_call_expression;
use tsv_lang::doc::{self, Doc};

/// Cutoff for short chains when groups should NOT be merged
const SHORT_CHAIN_CUTOFF: usize = 2;
/// Cutoff for short chains when groups SHOULD be merged (factory pattern)
const SHORT_CHAIN_CUTOFF_MERGED: usize = 3;

//
// Helper functions for common patterns
//

/// Build expanded docs for rest groups (each call uses hardlines)
fn build_rest_expanded_docs<'a, P: ChainPrinter>(
    rest_groups: &[ChainGroup<'a>],
    printer: &P,
) -> Vec<Doc> {
    rest_groups
        .iter()
        .map(|g| print_group_expanded(g, printer))
        .collect()
}

/// Build flat docs for groups
fn build_groups_flat_docs<'a, P: ChainPrinter>(groups: &[ChainGroup<'a>], printer: &P) -> Vec<Doc> {
    groups.iter().map(|g| print_group(g, printer)).collect()
}

/// Check if a single-arg call has an object/array that will break
fn call_has_breaking_single_arg<P: ChainPrinter>(
    call: &crate::ast::internal::CallExpression,
    printer: &P,
) -> bool {
    if call.arguments.len() != 1 {
        return false;
    }
    match &call.arguments[0] {
        Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
            let arg_doc = printer.print_expression(&call.arguments[0]);
            doc::will_break(&arg_doc)
        }
        // Arrow with type annotations and breaking object/array body
        Expression::ArrowFunctionExpression(arrow)
            if arrow.return_type.is_some()
                || arrow.type_parameters.is_some()
                || arrow.params.iter().any(has_param_type_annotation) =>
        {
            if let ArrowFunctionBody::Expression(body) = &arrow.body
                && matches!(
                    &**body,
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
                )
            {
                let body_doc = printer.print_expression(body);
                doc::will_break(&body_doc)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Build a doc for a chain from grouped nodes
///
/// Implements prettier's chain doc building logic:
/// - Member-only chains: use fill() for greedy packing
/// - Chains with calls: use group-based breaking
/// - Short chains (≤cutoff groups): simple group with softlines
/// - Longer chains: conditionalGroup([oneLine, expanded])
/// - 3+ calls with complex args: force expanded (no width-based decision)
pub fn build_chain_doc<'a, P: ChainPrinter>(groups: &[ChainGroup<'a>], printer: &P) -> Doc {
    if groups.is_empty() {
        return doc::empty();
    }

    // Single group: just print it
    if groups.len() == 1 {
        return print_group(&groups[0], printer);
    }

    // Check if first group is "(await ...)" pattern (without NonNull)
    // This takes priority because the parenthesized await needs special break handling.
    if is_parenthesized_await_head(groups) {
        return build_parenthesized_await_chain_doc(groups, printer);
    }

    // Check if first group ends with "(complex)!" pattern
    // This takes priority over member-only vs call chain distinction because
    // the parenthesized head needs special break handling regardless of chain type.
    if is_parenthesized_non_null_head(groups) {
        return build_parenthesized_non_null_chain_doc(groups, printer);
    }

    // Collect all call nodes in the chain for the 3+ calls rule
    let call_nodes: Vec<&ChainNode<'a>> = groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter(|n| n.is_call())
        .collect();

    // Check if this is a member-only chain (no calls)
    let has_calls = !call_nodes.is_empty();

    // Prettier's logic (member-chain.js:351-359):
    // If groups.length <= cutoff && !nodeHasComment:
    //   return group(oneLine)  // Simple group, NO fill()
    // Else:
    //   return conditionalGroup([oneLine, expanded with hardline breaks])
    //
    // We match this: short member-only chains use simple group(), not fill()
    let should_merge = should_merge_first_groups(groups, printer);
    let cutoff = if should_merge {
        SHORT_CHAIN_CUTOFF_MERGED
    } else {
        SHORT_CHAIN_CUTOFF
    };

    if !has_calls {
        // Member-only chain: use fill for greedy packing
        return build_member_only_chain_doc(groups, printer);
    }

    // Chains with calls - determine if we should force expansion
    let force_expand = should_force_chain_expand(groups, &call_nodes, printer);

    // Split groups into first (merged) and rest based on should_merge
    let split_at = if should_merge { 2 } else { 1 }.min(groups.len());
    let (first_groups, rest_groups) = groups.split_at(split_at);

    // Build doc for first group(s) - merged when should_merge
    let first_doc = build_first_groups_doc(first_groups, printer);

    // Short chains: use group-based breaking
    if groups.len() <= cutoff && !force_expand {
        return build_short_chain_doc(first_groups, rest_groups, first_doc, should_merge, printer);
    }

    // Long chains: check for additional break conditions
    build_long_chain_doc(
        groups,
        first_groups,
        rest_groups,
        should_merge,
        force_expand,
        printer,
    )
}

/// Check if chain expansion should be forced
fn should_force_chain_expand<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    call_nodes: &[&ChainNode<'a>],
    printer: &P,
) -> bool {
    // Prettier's chain expansion rules (member-chain.js:400-408):
    // 1. Blank lines BETWEEN methods (not just before first) force expansion
    // 2. 3+ calls with complex args force expansion
    // 3. 2+ calls with callbacks, where any callback has a multiline body, force expansion
    // 4. Inside template expressions with original breaks force expansion
    let has_blank_lines_between = has_blank_lines_between_methods(groups, printer);

    // Single pass: count callbacks and check if any breaks
    let line_breaks = printer.get_line_breaks();
    let (calls_with_callbacks, any_callback_breaks) =
        call_nodes
            .iter()
            .fold((0usize, false), |(count, any_breaks), node| {
                let status = call_callback_status(node, line_breaks);
                (
                    count + usize::from(status.has_callback),
                    any_breaks || status.will_break,
                )
            });

    // Comments between chain segments force expansion, EXCEPT for comments before
    // trailing members (which are handled specially by add_group_no_break)
    let has_forcing_comments = has_comments_forcing_expansion(groups, printer);

    has_blank_lines_between
        || has_forcing_comments
        || (call_nodes.len() > 2 && call_nodes.iter().any(|n| call_has_complex_args(n)))
        || (calls_with_callbacks >= 2 && any_callback_breaks)
        || printer.should_force_expand()
}

/// Build doc for short chains (groups.len() <= cutoff)
fn build_short_chain_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    first_doc: Doc,
    should_merge: bool,
    printer: &P,
) -> Doc {
    if rest_groups.is_empty() {
        return doc::group(first_doc);
    }

    // Check if first groups contain calls with multiple args that might need expansion
    let first_has_multiarg_calls = first_groups.iter().flat_map(|g| g.nodes.iter()).any(|n| {
        matches!(
            n,
            ChainNode::Call { expr: Expression::CallExpression(call), .. }
            if call.arguments.len() > 1
        )
    });

    // For short chains, prettier just concatenates groups directly WITHOUT softlines.
    // This ensures hardlines inside groups don't cause breaks between groups.
    let rest_docs: Vec<Doc> = rest_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    let on_line = doc::concat(
        std::iter::once(first_doc.clone())
            .chain(rest_docs.clone())
            .collect(),
    );

    // Check if first groups contain any calls (regardless of arg count)
    let first_has_calls = first_groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .any(ChainNode::is_call);

    // If first groups have multi-arg calls, use 4-state conditionalGroup.
    if first_has_multiarg_calls {
        return build_multiarg_short_chain_doc(
            first_groups,
            rest_groups,
            first_doc,
            on_line,
            &rest_docs,
            printer,
        );
    }

    // Check if chain ends with member (for callback arg breaking preference)
    let chain_ends_with_member = ends_with_member(rest_groups, first_groups);

    // When chain ends with member and first groups have calls, prefer expanding
    // first groups' call args over breaking the chain.
    if first_has_calls && chain_ends_with_member {
        let first_expanded_doc = build_first_groups_expanded_doc(first_groups, printer);
        let state_first_expanded = doc::concat(
            std::iter::once(first_expanded_doc)
                .chain(rest_docs)
                .collect(),
        );
        return doc::conditional_group(vec![on_line, state_first_expanded]);
    }

    // Prettier's short chain behavior (member-chain.js lines 351-360):
    // For chains with groups.length <= cutoff, just return group(oneLine).
    if !first_has_calls {
        return doc::group(on_line);
    }

    // Check for nested calls in first call's args
    let first_call_arg_contains_call = first_groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter_map(ChainNode::as_call_expression)
        .any(|call| call.arguments.iter().any(contains_call_expression));

    if !first_call_arg_contains_call {
        // For factory patterns (shouldMerge), use simple group
        if should_merge {
            return doc::group(on_line);
        }

        let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
        let state_last_expanded =
            doc::concat(std::iter::once(first_doc).chain(rest_expanded).collect());
        return doc::conditional_group(vec![on_line, state_last_expanded]);
    }

    // When first call's arg contains calls, try both expansion directions
    let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
    let state_last_expanded =
        doc::concat(std::iter::once(first_doc).chain(rest_expanded).collect());

    let first_expanded_doc = build_first_groups_expanded_doc(first_groups, printer);
    let state_first_expanded = doc::concat(
        std::iter::once(first_expanded_doc)
            .chain(rest_docs)
            .collect(),
    );

    doc::conditional_group(vec![on_line, state_last_expanded, state_first_expanded])
}

/// Build doc for short chains with multi-arg calls in first groups
fn build_multiarg_short_chain_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    first_doc: Doc,
    on_line: Doc,
    rest_docs: &[Doc],
    printer: &P,
) -> Doc {
    // State 1: First args inline, rest groups with expanded call args
    let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
    let state_last_expanded =
        doc::concat(std::iter::once(first_doc).chain(rest_expanded).collect());

    // State 2: First call's args expanded, rest groups flexible
    let first_expanded_doc = build_first_groups_expanded_doc(first_groups, printer);
    let state_first_expanded = doc::concat(
        std::iter::once(first_expanded_doc.clone())
            .chain(rest_docs.iter().cloned())
            .collect(),
    );

    // State 3: Everything expanded (first args broken, chain broken)
    let rest_parts_hard = build_rest_parts_with_comments(rest_groups, printer, true, true);
    let state_all_expanded = doc::concat(vec![
        first_expanded_doc,
        doc::indent(doc::concat(rest_parts_hard)),
    ]);

    doc::conditional_group(vec![
        on_line,
        state_last_expanded,
        state_first_expanded,
        state_all_expanded,
    ])
}

/// Build doc for long chains (groups.len() > cutoff)
fn build_long_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    should_merge: bool,
    force_expand: bool,
    printer: &P,
) -> Doc {
    // Check if any group except the last will break
    let any_non_last_breaks = groups[..groups.len() - 1].iter().any(|g| {
        let doc = print_group(g, printer);
        doc::will_break(&doc)
    });

    // Check if this chain ends with member access (not a call)
    let chain_ends_with_member = ends_with_member(rest_groups, first_groups);

    // Count calls in rest_groups (for chain_ends_with_member special case)
    let rest_call_count = rest_groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter(|n| n.is_call())
        .count();

    // For longer chains (>cutoff), force expanded if any non-last group breaks
    // EXCEPTION: When chain ends with member AND has exactly one call in rest
    let force_expand_from_breaking =
        any_non_last_breaks && !(chain_ends_with_member && rest_call_count == 1);

    // Build expanded variant
    let expanded = build_expanded_doc(groups, should_merge, printer);

    if force_expand || force_expand_from_breaking {
        return expanded;
    }

    // Print all groups inline (for oneLine variant)
    let on_line: Vec<Doc> = groups.iter().map(|g| print_group(g, printer)).collect();
    let on_line_doc = doc::concat(on_line);

    // Handle chains ending with member access with exactly one call in rest
    if chain_ends_with_member && rest_call_count == 1 {
        return build_member_ending_chain_doc(
            first_groups,
            rest_groups,
            on_line_doc,
            expanded,
            printer,
        );
    }

    // Handle chains with breaking object in last call
    if let Some(args_expanded_doc) =
        build_breaking_object_chain_doc(first_groups, rest_groups, printer)
    {
        return doc::conditional_group(vec![on_line_doc, args_expanded_doc, expanded]);
    }

    // Default: two-state conditional group
    doc::conditional_group(vec![on_line_doc, expanded])
}

/// Build doc for chains ending with member access (e.g., `.length`)
fn build_member_ending_chain_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    on_line_doc: Doc,
    expanded: Doc,
    printer: &P,
) -> Doc {
    // Check if the call's single arg needs expansion
    let rest_has_breaking_arg = rest_groups.iter().any(|g| {
        g.nodes
            .iter()
            .filter_map(ChainNode::as_call_expression)
            .any(|call| call_has_breaking_single_arg(call, printer))
    });

    // First groups stay flat, rest groups have calls expanded
    let first_docs = build_groups_flat_docs(first_groups, printer);
    let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
    let args_expanded_doc = doc::concat(first_docs.into_iter().chain(rest_expanded).collect());

    // When the arg will break internally, directly use args_expanded_doc
    if rest_has_breaking_arg {
        return args_expanded_doc;
    }

    // Try: 1. Everything inline, 2. Args expanded chain inline, 3. Chain expanded
    doc::conditional_group(vec![on_line_doc, args_expanded_doc, expanded])
}

/// Build doc for chains where last call has a breaking object/array argument
fn build_breaking_object_chain_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    printer: &P,
) -> Option<Doc> {
    // Check if the last call has a single object/array argument that will break
    // Note: We use a simpler check here (direct object/array only, no arrow functions)
    // because this is specifically for the last call's object literal expansion
    let last_group_will_break_object = rest_groups.last().is_some_and(|g| {
        g.nodes
            .iter()
            .rev()
            .find_map(ChainNode::as_call_expression)
            .is_some_and(|call| {
                call.arguments.len() == 1
                    && matches!(
                        &call.arguments[0],
                        Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
                    )
                    && {
                        let arg_doc = printer.print_expression(&call.arguments[0]);
                        doc::will_break(&arg_doc)
                    }
            })
    });

    if !last_group_will_break_object {
        return None;
    }

    // First groups stay flat
    let first_docs = build_groups_flat_docs(first_groups, printer);
    // Rest groups: all but last stay flat, last is expanded
    let rest_len = rest_groups.len();
    let rest_docs: Vec<Doc> = rest_groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            if i == rest_len - 1 {
                print_group_expanded(g, printer)
            } else {
                print_group(g, printer)
            }
        })
        .collect();

    Some(doc::concat(
        first_docs.into_iter().chain(rest_docs).collect(),
    ))
}
