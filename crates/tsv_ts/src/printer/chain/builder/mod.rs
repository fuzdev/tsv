// Chain doc building for TypeScript member chain formatting
//
// This module handles the main doc-building logic for chain formatting:
// - build_chain_doc: main entry point
// - Submodules handle specific chain patterns
//
// ## Architecture
//
// - **member_only.rs**: Member-only chains using fill()
// - **expansion.rs**: Chain expansion analysis helpers
// - **helpers.rs**: Shared utilities and ChainPartsBuilder

mod expansion;
mod helpers;
mod member_only;

use expansion::{
    call_callback_status, call_has_complex_args, ends_with_member, has_blank_lines_between_methods,
    has_comments_forcing_expansion,
};
use helpers::{
    build_expanded_doc, build_first_groups_doc, build_first_groups_expanded_doc,
    build_rest_parts_with_comments,
};
use member_only::{
    build_member_only_chain_doc, build_member_only_chain_with_comments_doc,
    member_only_has_interior_line_comments,
};

use super::analysis::should_merge_first_groups;
use super::printing::{
    has_inside_bracket_comments, print_group, print_group_expanded, print_group_standard_expanded,
    print_node,
};
use super::types::{ChainGroup, ChainNode, ChainNodeRefVec};
use crate::ast::internal::Expression;
use crate::printer::Printer;
use crate::printer::calls::arg_predicates::contains_call_expression;
use smallvec::smallvec;
use tsv_lang::Span;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// Cutoff for short chains when groups should NOT be merged
const SHORT_CHAIN_CUTOFF: usize = 2;
/// Cutoff for short chains when groups SHOULD be merged (factory pattern)
const SHORT_CHAIN_CUTOFF_MERGED: usize = 3;

//
// Helper functions for common patterns
//

/// Build expanded docs for rest groups (each call uses hardlines)
fn build_rest_expanded_docs<'a>(rest_groups: &[ChainGroup<'a>], printer: &Printer<'_>) -> DocBuf {
    rest_groups
        .iter()
        .map(|g| print_group_expanded(g, printer))
        .collect()
}

/// Build flat docs for groups
fn build_groups_flat_docs<'a>(groups: &[ChainGroup<'a>], printer: &Printer<'_>) -> DocBuf {
    groups.iter().map(|g| print_group(g, printer)).collect()
}

/// Check if a single-arg call has an object/array that will break
fn call_has_breaking_single_arg(
    call: &crate::ast::internal::CallExpression<'_>,
    printer: &Printer<'_>,
) -> bool {
    if call.arguments.len() != 1 {
        return false;
    }
    let d = printer.arena();
    match &call.arguments[0] {
        Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
            let arg_doc = printer.build_expression_doc(&call.arguments[0]);
            d.will_break(arg_doc)
        }
        // Object/array-body arrows (typed or not) are expandable per prettier's
        // couldExpandArg — they hug the call's open paren rather than forcing the
        // chain to expand — so they are NOT treated as a breaking single arg here.
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
pub fn build_chain_doc<'a>(
    groups: &[ChainGroup<'a>],
    chain_span: Span,
    printer: &Printer<'_>,
) -> DocId {
    // Activate arg-doc sharing for the outermost chain only (nested chains observe it
    // already active and reuse the map), so the flat and expanded group builds across
    // every `conditional_group` candidate share one recursive arg build instead of
    // rebuilding — the member-chain rebuild fix.
    let was_active = printer.enter_chain_arg_share();
    // Compute the chain-level comment presence ONCE and stash it (save/restore, so a
    // nested chain in a call arg / base restores the parent's value on exit). The print
    // path reads this to skip per-member comment classification on comment-free chains,
    // and `build_chain_doc_impl` reads it below instead of recomputing the search.
    let prev_has_comments = printer.set_chain_has_comments(
        printer.has_comments_on_page_between(chain_span.start, chain_span.end),
    );
    let result = build_chain_doc_impl(groups, printer);
    printer.restore_chain_has_comments(prev_has_comments);
    printer.exit_chain_arg_share(was_active);
    result
}

fn build_chain_doc_impl<'a>(groups: &[ChainGroup<'a>], printer: &Printer<'_>) -> DocId {
    let d = printer.arena();
    if groups.is_empty() {
        return d.empty();
    }

    // Single group: just print it
    if groups.len() == 1 {
        // Clear before printing — call args in this group may contain nested
        // chains that should not inherit is_expression_statement.
        printer.clear_expression_statement();
        return print_group(&groups[0], printer);
    }

    // Check force expansion early — iterate lazily, the common short-chain path
    // must not materialize a call-node Vec
    let has_calls = groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .any(ChainNode::is_call);

    // Zero-comment fast gate: one binary search over the whole chain window
    // short-circuits every per-node comment scan below — the expansion-forcing
    // check, the member-only line-comment check, and the inside-bracket comment
    // check, each of which otherwise runs a comment lookup per chain node. Sound
    // because every per-node comment sub-range lies within the chain's span, so no
    // comment anywhere in the window means every sub-query is empty. Chains are
    // comment-sparse between segments, so the gate nearly always fires. `build_chain_doc`
    // already computed this and stashed it (also feeding the print path), so read it back
    // rather than repeating the search.
    let chain_has_comments = printer.chain_has_comments();

    // Prettier's logic (member-chain.js:351-359):
    // If groups.length <= cutoff && !nodeHasComment:
    //   return group(oneLine)  // Simple group, NO fill()
    // Else:
    //   return conditionalGroup([oneLine, expanded with hardline breaks])
    //
    // We match this: short member-only chains use simple group(), not fill()
    let should_merge = should_merge_first_groups(groups, printer);
    // Reset after capturing — sub-expressions (call args, assignment RHS, etc.)
    // must not inherit this flag. Prettier checks parent per-chain.
    printer.clear_expression_statement();
    let cutoff = if should_merge {
        SHORT_CHAIN_CUTOFF_MERGED
    } else {
        SHORT_CHAIN_CUTOFF
    };

    // When first group has a parenthesized base with indent-on-break softlines,
    // use conditionalGroup so the chain can break at group boundaries
    // rather than inside the parenthesized expression.
    // Binary expressions are excluded — they have natural break points (at operators)
    // and should break there rather than at the chain.
    let first_has_parens = groups
        .first()
        .and_then(|g| g.nodes.first())
        .is_some_and(|n| match n {
            ChainNode::Base {
                needs_parens: true,
                expr,
                ..
            } => !matches!(expr, Expression::BinaryExpression(_)),
            _ => false,
        });

    // Member-only chains with inside-bracket comments in computed members need the
    // conditional_group path (not fill), so the bracket content can break when the
    // chain expands. Fill can't break inside a computed member's brackets.
    let has_bracket_comments = chain_has_comments
        && groups
            .iter()
            .flat_map(|g| g.nodes.iter())
            .any(|n| has_inside_bracket_comments(n, printer));

    if !has_calls && !first_has_parens && !has_bracket_comments {
        // Member-only chain with interior line comments: break the chain and emit
        // each comment in place (shared comment-aware path), instead of the fill
        // path's line_suffix — which defers mid-chain line comments to end of line,
        // merging/reversing multiple. Prettier hoists these; tsv preserves position.
        if chain_has_comments && member_only_has_interior_line_comments(groups, printer) {
            return build_member_only_chain_with_comments_doc(groups, printer);
        }
        // Member-only chain: use fill for greedy packing
        return build_member_only_chain_doc(groups, printer);
    }

    // Split groups into first (merged) and rest based on should_merge
    let split_at = if should_merge { 2 } else { 1 }.min(groups.len());
    let (first_groups, rest_groups) = groups.split_at(split_at);

    // Build doc for first group(s) - merged when should_merge
    let first_doc = build_first_groups_doc(first_groups, printer);

    // Short chains: use group-based breaking
    // Prettier (member-chain.js:351-359) only checks nodeHasComment for short chains.
    // Force expand conditions like "2+ callbacks with breaking body" and "3+ calls
    // with complex args" only apply to long chains (member-chain.js:400-407).
    // Comments between chain segments DO block the short chain path (matching
    // Prettier's nodeHasComment check).
    if groups.len() <= cutoff
        && !(has_calls && chain_has_comments && has_comments_forcing_expansion(groups, printer))
    {
        return build_short_chain_doc(
            first_groups,
            rest_groups,
            first_doc,
            first_has_parens,
            has_calls,
            printer,
        );
    }

    // Long chains: force expand conditions (Prettier member-chain.js:400-407)
    let force_expand = has_calls && should_force_chain_expand(groups, chain_has_comments, printer);
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
fn should_force_chain_expand<'a>(
    groups: &[ChainGroup<'a>],
    chain_has_comments: bool,
    printer: &Printer<'_>,
) -> bool {
    // Iterate call nodes in place — no materialized Vec
    let call_nodes = || {
        groups
            .iter()
            .flat_map(|g| g.nodes.iter())
            .filter(|n| n.is_call())
    };

    // Prettier's chain expansion rules (member-chain.js:400-408):
    // 1. Blank lines BETWEEN methods (not just before first) force expansion
    // 2. 3+ calls with complex args force expansion
    // 3. 2+ calls with callbacks, where any callback has a multiline body, force expansion
    let has_blank_lines_between = has_blank_lines_between_methods(groups, printer);

    // Single pass: count calls and callbacks, and check if any callback breaks
    let line_breaks = printer.get_line_breaks();
    let (call_count, calls_with_callbacks, any_callback_breaks) = call_nodes().fold(
        (0usize, 0usize, false),
        |(calls, count, any_breaks), node| {
            let status = call_callback_status(node, line_breaks);
            (
                calls + 1,
                count + usize::from(status.has_callback),
                any_breaks || status.will_break,
            )
        },
    );

    // Comments between chain segments force expansion, EXCEPT for comments before
    // trailing members (which are handled specially by add_group_no_break)
    let has_forcing_comments =
        chain_has_comments && has_comments_forcing_expansion(groups, printer);

    has_blank_lines_between
        || has_forcing_comments
        || (call_count > 2 && call_nodes().any(call_has_complex_args))
        || (calls_with_callbacks >= 2 && any_callback_breaks)
}

/// Build doc for short chains (groups.len() <= cutoff)
fn build_short_chain_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    first_doc: DocId,
    first_has_parens: bool,
    has_calls: bool,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    if rest_groups.is_empty() {
        return d.group(first_doc);
    }

    // `base_call(args).a.b...` — a plain base call followed by ONLY trailing member
    // accesses. Prettier prints this via member.js (printMemberExpression), NOT the
    // member chain: the call's args group and each trailing member are independent
    // sibling groups, so the args break only when the call itself overflows and
    // otherwise the trailing members break individually. The chain conditionalGroup
    // would instead expand the call args even when the call fits inline.
    if is_base_call_then_only_members(first_groups, rest_groups, printer) {
        return build_base_call_then_members_doc(first_groups, rest_groups, printer);
    }

    // Check if first groups contain calls with multiple args that might need expansion
    let first_has_multiarg_calls = first_groups.iter().flat_map(|g| g.nodes.iter()).any(|n| {
        matches!(
            n,
            ChainNode::Call { call, .. }
            if call.arguments.len() > 1
        )
    });

    // For short chains, prettier just concatenates groups directly WITHOUT softlines.
    // This ensures hardlines inside groups don't cause breaks between groups.
    let rest_docs: DocBuf = rest_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    let mut on_line_parts: DocBuf = smallvec![first_doc];
    on_line_parts.extend(rest_docs.iter().copied());
    let on_line = d.concat(&on_line_parts);

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
        let mut state_first_expanded_parts: DocBuf = smallvec![first_expanded_doc];
        state_first_expanded_parts.extend(rest_docs.iter().copied());
        let state_first_expanded = d.concat(&state_first_expanded_parts);
        return d.conditional_group(&[on_line, state_first_expanded]);
    }

    // Prettier's short chain behavior (member-chain.js lines 351-360):
    // For chains with groups.length <= cutoff, just return group(oneLine).
    if !first_has_calls {
        // When first group has a parenthesized base with indent-on-break softlines
        // and no calls anywhere in the chain, use conditionalGroup to break at
        // group boundaries rather than inside the parenthesized expression.
        // When there ARE calls, the inner group breaks naturally via group(oneLine).
        if first_has_parens && !has_calls {
            // `rest_groups` is the single trailing lookup group: `group_chain_nodes` only
            // opens a new group at a memberish AFTER a call, so a call-free chain never
            // splits past the first group. (`concat` is total — it can't drop a doc should
            // that ever stop holding.)
            let lookup = d.concat(&rest_docs);

            // A computed lookup takes no break point before it, ever — member.js's
            // `shouldInline` includes `node.computed` — so `(x as T)![i]` and
            // `(x as T)[i][j]` stay glued to the base and shed width by breaking their own
            // brackets instead (`computed_lookup_doc`). Same rule as `starts_segment` in
            // the member-only path.
            if rest_groups
                .first()
                .and_then(|g| g.nodes.first())
                .is_some_and(ChainNode::is_computed)
            {
                return d.concat(&[first_doc, lookup]);
            }

            // A `.prop` lookup hugs the base's closing `)` when it fits after the base's
            // last line, and drops to its own indented line otherwise — prettier's
            // `printMemberExpression` (`[objectDoc, group(indent([softline, lookup]))]`).
            // The base breaks on its own (parens hang-break or inner call args), so we must
            // not force the lookup onto its own line just because the base is multi-line;
            // the softline lets it hug the `)`.
            let member = d.group(d.indent(d.concat(&[d.softline(), lookup])));
            return d.concat(&[first_doc, member]);
        }
        return d.group(on_line);
    }

    // Check for nested calls in first call's args
    let first_call_arg_contains_call = first_groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter_map(ChainNode::as_call_expression)
        .any(|call| call.arguments.iter().any(contains_call_expression));

    if !first_call_arg_contains_call {
        // Prettier: group(printedGroups.flat()) for short chains (member-chain.js:351-359).
        // group() lets hardlines in the first call (e.g., multiline array) render
        // naturally while the second call's inner group handles its own arg layout.
        return d.group(on_line);
    }

    // When first call's arg contains calls, try both expansion directions
    let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
    let mut state_last_expanded_parts: DocBuf = smallvec![first_doc];
    state_last_expanded_parts.extend(rest_expanded);
    let state_last_expanded = d.concat(&state_last_expanded_parts);

    let first_expanded_doc = build_first_groups_expanded_doc(first_groups, printer);
    let mut state_first_expanded_parts: DocBuf = smallvec![first_expanded_doc];
    state_first_expanded_parts.extend(rest_docs.iter().copied());
    let state_first_expanded = d.concat(&state_first_expanded_parts);

    d.conditional_group(&[on_line, state_last_expanded, state_first_expanded])
}

/// Whether the chain is `base_call(args).a.b...` — a bare base call followed by ONLY
/// plain `.prop` member accesses (no further calls, no computed/private/non-null
/// nodes, no inter-element comments). This is prettier's `printMemberExpression`
/// (member.js) territory, not the member chain; other shapes fall back to the chain
/// conditionalGroup.
fn is_base_call_then_only_members<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> bool {
    // Runs on every short chain with rest groups — kept off the heap via the
    // stack-friendly `ChainNodeRefVec` (the common short chain stays inline).
    let all_nodes: ChainNodeRefVec<'_, 'a> = first_groups
        .iter()
        .chain(rest_groups.iter())
        .flat_map(|g| g.nodes.iter())
        .collect();
    // Need base + one call + at least one trailing member.
    if all_nodes.len() < 3 {
        return false;
    }
    // Base must be a bare (non-parenthesized) expression directly followed by the call.
    if !matches!(
        all_nodes[0],
        ChainNode::Base {
            needs_parens: false,
            ..
        }
    ) || !all_nodes[1].is_call()
    {
        return false;
    }
    // Exactly one call in the whole chain (the base call).
    if all_nodes.iter().filter(|n| n.is_call()).count() != 1 {
        return false;
    }
    // Everything after the base call must be a plain `.prop` member — computed,
    // private, and non-null nodes have their own break structure.
    if !all_nodes[2..]
        .iter()
        .all(|n| matches!(n, ChainNode::Member { .. }))
    {
        return false;
    }
    // No inter-element comments (those need the comment-aware chain path).
    all_nodes[1..].iter().all(|n| {
        n.comment_range()
            .is_none_or(|(start, end)| !printer.has_comments_to_emit_between(start, end))
    })
}

/// Build the member.js sibling-group doc for `base_call(args).a.b...`: the base call
/// prints inline (its args group breaks only if the call itself overflows) and each
/// trailing member is `group(indent([softline, .prop]))`, so the overflowing member
/// drops to its own indented line while earlier members hug the call's `)`.
fn build_base_call_then_members_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let all_nodes: ChainNodeRefVec<'_, 'a> = first_groups
        .iter()
        .chain(rest_groups.iter())
        .flat_map(|g| g.nodes.iter())
        .collect();
    // The leading non-member nodes are the base call; the rest are trailing members.
    let first_member_idx = all_nodes.iter().take_while(|n| !n.is_member()).count();
    let prefix_docs: DocBuf = all_nodes[..first_member_idx]
        .iter()
        .map(|n| print_node(n, printer))
        .collect();
    let mut parts: DocBuf = smallvec![d.concat(&prefix_docs)];
    for node in &all_nodes[first_member_idx..] {
        let member = print_node(node, printer);
        parts.push(d.group(d.indent(d.concat(&[d.softline(), member]))));
    }
    d.concat(&parts)
}

/// Build doc for short chains with multi-arg calls in first groups
fn build_multiarg_short_chain_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    first_doc: DocId,
    on_line: DocId,
    rest_docs: &[DocId],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    // State: First args inline, rest groups with arrow-hugging expanded call args
    // `(sig =>\n  body,\n)` — more compact (fewer lines) but longer first line
    let rest_expanded = build_rest_expanded_docs(rest_groups, printer);
    let mut state_last_hugged_parts: DocBuf = smallvec![first_doc];
    state_last_hugged_parts.extend(rest_expanded);
    let state_last_hugged = d.concat(&state_last_hugged_parts);

    // State: First args inline, rest groups with standard expanded call args
    // `(\n  args,\n)` — shorter first line, used when arrow-hugging doesn't fit
    let rest_standard_expanded: DocBuf = rest_groups
        .iter()
        .map(|g| print_group_standard_expanded(g, printer))
        .collect();
    let mut state_last_standard_parts: DocBuf = smallvec![first_doc];
    state_last_standard_parts.extend(rest_standard_expanded);
    let state_last_standard = d.concat(&state_last_standard_parts);

    // State: First call's args expanded, rest groups flexible.
    // Wrap the expanded first group in group_break so it renders in Break mode when this
    // state is selected: the conditional_group renders a chosen non-last state in Flat mode
    // (arena_render.rs), and without the wrapper the expanded call args' hardlines make
    // newlines while the mode stays Flat, so a nested arrow signature's fits() measures its
    // body line() as a space and wrongly breaks the param list — the head/prefix analog of
    // the arrow-sig protection the sibling expanded states already apply
    // (build_member_ending_chain_doc / build_breaking_object_chain_doc). Selection is
    // unchanged: fits() early-returns at the first hardline either way (same remaining<0
    // gate), so only state_first_expanded's render mode flips Flat→Break. state_all_expanded
    // is the Break-mode last fallback, so it keeps the raw doc.
    let first_expanded_doc = build_first_groups_expanded_doc(first_groups, printer);
    let mut state_first_expanded_parts: DocBuf = smallvec![d.group_break(first_expanded_doc)];
    state_first_expanded_parts.extend(rest_docs.iter().copied());
    let state_first_expanded = d.concat(&state_first_expanded_parts);

    // State: Everything expanded (first args broken, chain broken)
    let mut rest_parts_hard = d.pooled_docbuf();
    build_rest_parts_with_comments(&mut rest_parts_hard, rest_groups, printer, true);
    let state_all_expanded = d.concat(&[first_expanded_doc, d.indent(d.concat(&rest_parts_hard))]);

    d.conditional_group(&[
        on_line,
        state_last_hugged,
        state_last_standard,
        state_first_expanded,
        state_all_expanded,
    ])
}

/// Build doc for long chains (groups.len() > cutoff)
fn build_long_chain_doc<'a>(
    groups: &[ChainGroup<'a>],
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    should_merge: bool,
    force_expand: bool,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    // Print every group's flat doc once. Both the "any non-last group breaks" scan
    // just below and the oneLine variant (`on_line_doc`) consume these flat group
    // docs, so one build feeds both. A member chain builds the same group flat across
    // `conditional_group` candidates, and the arg-doc share (see `build_chain_doc`)
    // makes each flat rebuild byte-identical to the first — so reusing this single
    // build is byte-identical to the prior discard-then-rebuild.
    let on_line: DocBuf = groups.iter().map(|g| print_group(g, printer)).collect();

    // Check if any group except the last will break.
    let any_non_last_breaks = on_line[..on_line.len() - 1]
        .iter()
        .any(|&doc| d.will_break(doc));

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

    // oneLine variant (reuses the flat group docs built above)
    let on_line_doc = d.concat(&on_line);

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
        return d.conditional_group(&[on_line_doc, args_expanded_doc, expanded]);
    }

    // Default: two-state conditional group
    d.conditional_group(&[on_line_doc, expanded])
}

/// Build doc for chains ending with member access (e.g., `.length`)
fn build_member_ending_chain_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    on_line_doc: DocId,
    expanded: DocId,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
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
    let mut args_expanded_parts = first_docs;
    args_expanded_parts.extend(rest_expanded);
    let args_expanded_inner = d.concat(&args_expanded_parts);
    // Wrap in group_break: when the conditional_group selects this state in Flat
    // mode, the group forces Break mode during rendering. Without this, hardlines
    // in the expanded call args create newlines but the mode stays Flat, causing
    // nested groups (arrow sig groups) to evaluate fits() with Flat-mode rest
    // commands — body line() = space, not newline — breaking the signature.
    let args_expanded_doc = d.group_break(args_expanded_inner);

    // When the arg will break internally, directly use args_expanded_doc
    if rest_has_breaking_arg {
        return args_expanded_doc;
    }

    // Try: 1. Everything inline, 2. Args expanded chain inline, 3. Chain expanded
    d.conditional_group(&[on_line_doc, args_expanded_doc, expanded])
}

/// Build doc for chains where the last call has a single breaking argument that
/// prettier keeps flat-chained (oneLine) rather than expanding the chain.
fn build_breaking_object_chain_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> Option<DocId> {
    let d = printer.arena();
    // The last call's single argument breaks and is one prettier keeps on the flat
    // chain: a direct object/array literal, OR a `new`/call expression wrapping one
    // (e.g. `new Response(body, {…})`). Callbacks (function/arrow args) are excluded —
    // for those, prettier expands the chain when other calls also take function args
    // (member-chain.js `lastGroupWillBreakAndOtherCallsHaveFunctionArguments`).
    let last_group_will_break_object = rest_groups.last().is_some_and(|g| {
        g.nodes
            .iter()
            .rev()
            .find_map(ChainNode::as_call_expression)
            .is_some_and(|call| {
                call.arguments.len() == 1
                    && matches!(
                        &call.arguments[0],
                        Expression::ObjectExpression(_)
                            | Expression::ArrayExpression(_)
                            | Expression::NewExpression(_)
                            | Expression::CallExpression(_)
                    )
                    && {
                        let arg_doc = printer.build_expression_doc(&call.arguments[0]);
                        d.will_break(arg_doc)
                    }
            })
    });

    if !last_group_will_break_object {
        return None;
    }

    // First groups and all but the last rest group stay flat. Keeping the chain
    // prefix flat-measurable is load-bearing: arena_fits must see the prefix's true
    // width so the conditional_group falls through to the fully-expanded chain when
    // the prefix itself overflows. Wrapping the WHOLE chain in group_break instead
    // makes fits() inherit Break mode into the prefix's inner call-arg groups and
    // early-return at their softlines, wrongly selecting this state (and breaking an
    // earlier call's args) even when the prefix doesn't fit.
    let rest_len = rest_groups.len();
    let mut all_parts = build_groups_flat_docs(first_groups, printer);
    for (i, g) in rest_groups.iter().enumerate() {
        if i == rest_len - 1 {
            // Only the last group is force-broken: when this state is selected in
            // Flat mode, its expanded call args still render in Break mode (so nested
            // groups, e.g. arrow sigs, evaluate fits() against Break-mode rest commands).
            all_parts.push(d.group_break(print_group_expanded(g, printer)));
        } else {
            all_parts.push(print_group(g, printer));
        }
    }
    Some(d.concat(&all_parts))
}
