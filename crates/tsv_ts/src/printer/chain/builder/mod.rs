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
    build_rest_parts_with_comments, gap_has_break_forcing_comments,
};
use member_only::{
    build_member_only_chain_doc, build_member_only_chain_with_comments_doc,
    member_only_has_interior_line_comments,
};

use super::analysis::should_merge_first_groups;
use super::printing::{
    chain_gap_any, has_inside_bracket_comments, member_lookup_group, node_comment_gap, print_group,
    print_group_expanded, print_group_standard_expanded, print_node_inner,
};
use super::types::{ChainGroup, ChainNode};
use super::{InlineLookups, resolve_inline_lookups};
use crate::ast::internal::{ArrowFunctionBody, CallExpression, Expression};
use crate::printer::Printer;
use smallvec::SmallVec;
use smallvec::smallvec;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::{ClassifiedComments, Span};

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
fn call_has_breaking_single_arg(call: &CallExpression<'_>, printer: &Printer<'_>) -> bool {
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
    // Read the parent's `shouldInline` marks HERE, at the chain root and before any node
    // doc is built: a nested assignment inside the chain (a computed index, `a[x = 1].b`)
    // marks its own operands and would clear these first. See
    // [`super::inline_lookups`].
    let inline = resolve_inline_lookups(chain_span, groups, printer);
    let result = build_chain_doc_impl(groups, chain_span.end, inline, printer);
    printer.restore_chain_has_comments(prev_has_comments);
    printer.exit_chain_arg_share(was_active);
    result
}

/// `chain_end` is the chain's source end — the anchor for the trailing member's
/// trailer read ([`has_comments_forcing_expansion`]).
///
/// `inline` carries the `shouldInline` answers this chain's PARENT supplied
/// ([`super::inline_lookups`]). Its `every` arm reaches every site that decides a
/// lookup's break point — the member-only fold, the short chain's paren-base lookup, and
/// the peeled member tail — because they answer one question in three shapes, and a
/// subset would leave the chain breakable through whichever spelling was missed; its
/// `call_tail` arm reaches the peel alone, the only site holding a lookup whose parent is
/// the marked position. Both apply only to the WHOLE chain: the recursive calls below pass
/// [`InlineLookups::NONE`], since a peeled sub-chain is not the span either mark named.
fn build_chain_doc_impl<'a>(
    groups: &[ChainGroup<'a>],
    chain_end: u32,
    inline: InlineLookups,
    printer: &Printer<'_>,
) -> DocId {
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

    // A trailing member tail (`….bb(cb).prop`) prints OUTSIDE the member chain:
    // prettier roots the chain at the outermost CALL, so a `.prop` above it is
    // `printMemberExpression`'s to lay out — the chain's own expand decision
    // (`any_non_last_breaks`, the call-count rules) never sees it, which is what
    // keeps `obj.aa(x).bb(cb)` flat with the arrow hugged and `.prop` trailing.
    // Folding the tail into the chain made `.bb` non-last and force-expanded it.
    if has_calls && let Some(peeled) = peel_trailing_member_tail(groups, chain_end, inline, printer)
    {
        return build_peeled_tail_doc(groups, chain_end, &peeled, inline, printer);
    }

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

    // Member-only chain with interior line comments: break the chain and emit each
    // comment in place (shared comment-aware path), instead of the fill path's
    // line_suffix — which defers mid-chain line comments to end of line, merging/
    // reversing multiple. Prettier hoists these; tsv preserves position.
    //
    // Routed ahead of `first_has_parens` because that gate is about LAYOUT — letting a
    // parenthesized base break at group boundaries rather than inside itself — and this
    // chain breaks at every member regardless, so there is no width decision left for it
    // to make. Behind it, a parenthesized head (a cast, an `await`, an arrow) fell
    // through to the call-chain path, whose boundary-less `line_suffix` defers the
    // comment to end of line; it then merged with whatever else flushed there
    // (`.g; // c1 // c2`), the second `//` becoming text of the first. `has_bracket_comments`
    // still routes first: fill can't break inside a computed member's brackets, which is
    // a shape question this builder doesn't answer.
    if !has_calls
        && !has_bracket_comments
        && chain_has_comments
        && member_only_has_interior_line_comments(groups, printer)
    {
        return build_member_only_chain_with_comments_doc(groups, printer);
    }

    if !has_calls && !first_has_parens && !has_bracket_comments {
        // Member-only chain: use fill for greedy packing
        return build_member_only_chain_doc(groups, inline.every, printer);
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
        && !(has_calls
            && chain_has_comments
            && has_comments_forcing_expansion(groups, chain_end, printer))
    {
        return build_short_chain_doc(
            first_groups,
            rest_groups,
            first_doc,
            first_has_parens,
            has_calls,
            inline.every,
            printer,
        );
    }

    // Long chains: force expand conditions (Prettier member-chain.js:400-407)
    let force_expand =
        has_calls && should_force_chain_expand(groups, chain_end, chain_has_comments, printer);
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
    chain_end: u32,
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
    let (call_count, calls_with_callbacks, any_callback_breaks) = call_nodes().fold(
        (0usize, 0usize, false),
        |(calls, count, any_breaks), node| {
            let status = call_callback_status(node, printer);
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
        chain_has_comments && has_comments_forcing_expansion(groups, chain_end, printer);

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
    inline_every_lookup: bool,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    if rest_groups.is_empty() {
        return d.group(first_doc);
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

            // No break point before the lookup — the same `shouldInline` clauses
            // (member.js) the member-only builder's flat arm answers:
            //
            // - the parent-supplied `every` clauses — an assignment **target**, a `new`
            //   **callee** — so the base's own parens become the only break point left and
            //   hang instead (`(⏎\taaa as Tttt⏎).bbb… = v`, `new (⏎\taaa as Tttt⏎).bbb…()`).
            //   See [`super::inline_lookups`].
            // - `node.computed`: a computed lookup takes no break point before it, ever, so
            //   `(x as T)![i]` and `(x as T)[i][j]` stay glued to the base and shed width
            //   by breaking their own brackets (`computed_lookup_doc`). Same rule as
            //   `starts_segment` in the member-only path.
            if inline_every_lookup
                || rest_groups
                    .first()
                    .and_then(|g| g.nodes.first())
                    .is_some_and(ChainNode::is_computed)
            {
                return d.concat(&[first_doc, lookup]);
            }

            // A `.prop` lookup hugs the base's closing `)` when it fits after the base's
            // last line, and drops to its own indented line otherwise. The base breaks on
            // its own (parens hang-break or inner call args), so we must not force the
            // lookup onto its own line just because the base is multi-line; the softline
            // lets it hug the `)`.
            return d.concat(&[first_doc, member_lookup_group(d, lookup)]);
        }
        return d.group(on_line);
    }

    // Prettier: group(printedGroups.flat()) for short chains (member-chain.js:351-359).
    // group() lets hardlines in the first call (e.g., multiline array) render
    // naturally while each call's inner args group handles its own layout — including
    // the last-argument hug of a first call whose argument breaks (`X.map((x) => ({`).
    // A chain-level conditional_group here would measure the whole line flat and
    // pre-empt that inner hug, force-expanding the first call's argument list instead.
    d.group(on_line)
}

/// The trailing member tail as the two contiguous runs it spans: what the last
/// call's own group holds after that call, then the whole groups following it.
///
/// `group_chain_nodes` tiles the linearized chain, so the two runs are adjacent in
/// the one `ChainNodeVec` the linearizer built — but a [`ChainGroup`] is a bare
/// sub-slice carrying no absolute offset into that buffer, and joining two adjacent
/// slices needs `slice::from_raw_parts` (`unsafe_code` is `forbid`ed). So the PAIR is
/// the tail, rather than a `ChainNodeVec` copy of it: collecting one put a 464-byte
/// buffer in [`build_chain_doc_impl`]'s frame — which is on the expression recursion
/// cycle — to carry a mean of 0.02 nodes.
#[derive(Debug, Clone, Copy)]
struct TailRuns<'a> {
    /// The run left in the last call's own group, after that call.
    head: &'a [ChainNode<'a>],
    /// The whole groups after that one, in order.
    rest: &'a [ChainGroup<'a>],
}

impl<'a> TailRuns<'a> {
    /// The tail's nodes in source order.
    fn iter(self) -> impl Iterator<Item = &'a ChainNode<'a>> {
        self.head
            .iter()
            .chain(self.rest.iter().flat_map(|g| g.nodes.iter()))
    }

    /// The tail's node count. Walked, not stored — see [`PeeledTail::tail_len`].
    fn len(self) -> usize {
        self.head.len() + self.rest.iter().map(|g| g.nodes.len()).sum::<usize>()
    }
}

/// A chain's trailing member tail, split off by [`peel_trailing_member_tail`]
/// together with the facts [`build_peeled_tail_doc`] and [`append_member_tail`]
/// consume — answered once at the peel so the consumers neither re-scan the
/// groups nor re-classify the gap.
struct PeeledTail<'a, 'p> {
    /// Index of the group holding the chain's last call.
    last_call_group: usize,
    /// Index of that call within its group's nodes.
    last_call_idx: usize,
    /// Every node after the last call — a VIEW of the linearized chain, not a copy.
    tail: TailRuns<'a>,
    /// `tail`'s node count, walked once at the peel: [`append_member_tail`] asks it
    /// per node to find the last one, and a run pair cannot answer it in O(1).
    tail_len: usize,
    /// The prefix→tail gap's comments — only same-line trailing blocks by
    /// construction (the peel refuses break-forcing ones). `None` when the chain
    /// window holds no comments or the tail's first node has no gap.
    gap_comments: Option<ClassifiedComments<'p>>,
    /// The tail's LAST member takes no break point — prettier's call-object clause
    /// (member.js `shouldInline`), the only one of its clauses that can reach a peeled
    /// tail from the parent. Every other member keeps member.js's per-member break
    /// point; see the peel.
    inlined_last: bool,
}

// The peel is INLINED into `build_chain_doc_impl`, which sits on the expression
// recursion cycle, so this type's width is a per-level stack cost on the deepest
// shape tsv formats (`docs/cli.md` §Recursion Depth). Pinned because that is the
// whole point of [`TailRuns`]: a field that collected the tail again — or any other
// buffer sized for a worst case — would silently restore ~424 bytes to every level
// of a nested member chain, with output byte-identical and no test able to see it.
// 64-bit only (the count is pointer-width-relative).
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<PeeledTail<'static, 'static>>() == 192);

/// Split off the trailing member tail — every node AFTER the chain's last call,
/// which SPANS group boundaries (the grouping may keep the first post-call member
/// in the call's own group: `read(...).a.b` groups as `[[base, call, .a], [.b]]`),
/// so it is named as the run pair [`TailRuns`] rather than copied. Returns `Some`
/// only when the tail is ALL plain
/// `.prop` members and its gaps are quiet: a computed / private / non-null node
/// keeps its own break structure on the existing paths, and a break-forcing comment
/// (trailing line, or any leading) needs the comment-aware chain paths. A trailing
/// same-line block comment is fine — the append emits it inline, as
/// `add_group_no_break` does.
///
/// ⚠️ A **blank line** in the prefix→tail gap is NOT a refusal, though it reads like the
/// blank-between-methods signal `should_force_chain_expand` acts on. That signal is about
/// blanks the member CHAIN contains, and this one is outside it: prettier roots the chain
/// at the outermost call, so a blank before a trailing `.prop` sits between the chain and
/// a `printMemberExpression` above it, which has no blank handling at all — prettier
/// collapses it and lays the lookup out on width alone. Refusing here read it as chain
/// signal instead, and cost three things at once: a chain that FITS expanded
/// (`aa.b().c().d()⏎⏎.e` → three lines), the blank itself survived where prettier collapses
/// it, and the refused path has no break point for a trailing member at all — so an
/// over-width lookup printed flat on pass 1 and broke on pass 2, a non-idempotency the
/// blank-injection audit reaches. See `expressions/member/call_base_tail_blank`.
fn peel_trailing_member_tail<'a, 'p>(
    groups: &'a [ChainGroup<'a>],
    chain_end: u32,
    inline: InlineLookups,
    printer: &'p Printer<'_>,
) -> Option<PeeledTail<'a, 'p>> {
    // The tail is every node AFTER the chain's last call, so the tail's own last node
    // IS the chain's last node — and every tail node has to be a plain member. Asking
    // that of the last node first costs two loads and refuses the ~98% of chains that
    // end in a call (24,099 peels over fuz_app/src, 23,742 of them tail-less), each of
    // which otherwise pays both reverse scans and the tail walk to reach the same no.
    if !matches!(groups.last()?.nodes.last()?, ChainNode::Member { .. }) {
        return None;
    }

    let last_call_group = groups
        .iter()
        .rposition(|g| g.nodes.iter().any(ChainNode::is_call))?;
    let last_call_idx = groups[last_call_group]
        .nodes
        .iter()
        .rposition(ChainNode::is_call)?;

    let tail = TailRuns {
        head: &groups[last_call_group].nodes[last_call_idx + 1..],
        rest: &groups[last_call_group + 1..],
    };
    let tail_len = tail.len();
    if tail_len == 0 || !tail.iter().all(|n| matches!(n, ChainNode::Member { .. })) {
        return None;
    }
    let first = tail.iter().next()?;
    // The prefix→tail gap: refuse a break-forcing comment. A blank here is not one —
    // see the ⚠️ above.
    let mut gap_comments = None;
    if printer.chain_has_comments()
        && let Some((object_end, property_start)) = node_comment_gap(first, printer)
    {
        let classified =
            printer.classify_chain_gap(object_end, property_start, first.paren_gap_skip());
        if gap_has_break_forcing_comments(&classified) {
            return None;
        }
        gap_comments = Some(classified);
    }
    // The tail's interior gaps must be comment-free — an interior comment needs the
    // comment-aware chain paths.
    if printer.chain_has_comments()
        && tail.iter().skip(1).any(|n| {
            n.comment_range().is_some_and(|gap| {
                chain_gap_any(gap, n.paren_gap_skip(), |start, end| {
                    printer.has_comments_to_emit_between(start, end)
                })
            })
        })
    {
        return None;
    }

    // Prettier's call-object clause (member.js `shouldInline`) takes the break point back
    // in ONE position — a chain sitting directly under an assignment or a declarator — and
    // it can only ever reach the tail's LAST member: `findAncestor` skips member ancestors,
    // so every member below the last one has a MEMBER parent, which the clause does not
    // name. Both halves are read here, the POSITION from the parent's mark and the OBJECT
    // from the chain. See [`super::inline_lookups`].
    //
    // Its two disjuncts are the two things that object can be:
    //
    // - a call WITH ARGUMENTS (`isCallExpressionWithArguments`), which the last member's
    //   object is only when the tail is a LONE lookup — with two, the last one's object is
    //   the member below it (`const x = fn(a).b⏎\t.c`, prettier's own layout);
    // - a doc prettier LABELLED `memberChain`, which every chain carries except the
    //   `groups.length <= cutoff` shortcut. `printMemberExpression` propagates that label
    //   up through the tail, so a labelled prefix reaches the last member across any number
    //   of lookups.
    //
    // Both disjuncts sit behind the POSITION, which is one flag: a chain in any other
    // position never pays for either question.
    let lone_tail_off_call_with_args = tail_len == 1
        && matches!(
            groups[last_call_group].nodes[last_call_idx],
            ChainNode::Call { call, .. } if !call.arguments.is_empty()
        );
    let inlined_last = inline.call_tail
        && (lone_tail_off_call_with_args
            || prefix_prints_as_member_chain(groups, last_call_group, chain_end, printer));
    Some(PeeledTail {
        last_call_group,
        last_call_idx,
        tail,
        tail_len,
        gap_comments,
        inlined_last,
    })
}

/// Whether the peeled PREFIX prints as a doc prettier would label `memberChain` — the
/// second disjunct of the call-object clause (see [`peel_trailing_member_tail`]).
///
/// Prettier labels every `printMemberChain` result except the `groups.length <= cutoff`
/// shortcut, so this asks exactly the question `build_chain_doc_impl` is about to answer
/// for the same groups: the prefix's own group count against its own merge-adjusted
/// cutoff, plus the comment check that sends a short chain down the long path anyway.
///
/// Asked from the peel because `build_chain_doc_impl` CLEARS the expression-statement flag
/// `should_merge_first_groups` reads. Truncating the last group cannot change the answer —
/// the merge reads only the first two groups' leading nodes — so the untruncated slice
/// stands in for the prefix the build assembles.
fn prefix_prints_as_member_chain(
    groups: &[ChainGroup<'_>],
    last_call_group: usize,
    chain_end: u32,
    printer: &Printer<'_>,
) -> bool {
    let prefix = &groups[..=last_call_group];
    let cutoff = if should_merge_first_groups(prefix, printer) {
        SHORT_CHAIN_CUTOFF_MERGED
    } else {
        SHORT_CHAIN_CUTOFF
    };
    prefix.len() > cutoff
        || (printer.chain_has_comments()
            && has_comments_forcing_expansion(prefix, chain_end, printer))
}

/// Build the doc for a peeled chain: the prefix (everything through the last
/// call) prints as its own chain, then the member tail is appended outside it.
/// The prefix is a plain reborrow of `groups` whenever the call closes its group
/// — only a tail node sharing the call's group forces rebuilding the prefix with
/// that group truncated.
fn build_peeled_tail_doc<'a>(
    groups: &[ChainGroup<'a>],
    chain_end: u32,
    peeled: &PeeledTail<'a, '_>,
    inline: InlineLookups,
    printer: &Printer<'_>,
) -> DocId {
    let call_group = &groups[peeled.last_call_group];
    // The prefix ends with the call, so its build never reaches the trailing-MEMBER
    // trailer read that `chain_end` anchors; it is passed through unchanged. The parent's
    // marks are NOT: a peeled prefix is a sub-chain, not the span either mark named, so it
    // takes [`InlineLookups::NONE`] (and, ending in a call, never reaches the member-only
    // builder that would read `every` anyway).
    let chain_doc = if peeled.last_call_idx + 1 == call_group.nodes.len() {
        build_chain_doc_impl(
            &groups[..=peeled.last_call_group],
            chain_end,
            InlineLookups::NONE,
            printer,
        )
    } else {
        let mut prefix: SmallVec<[ChainGroup<'a>; 4]> =
            groups[..peeled.last_call_group].iter().copied().collect();
        prefix.push(ChainGroup::new(&call_group.nodes[..=peeled.last_call_idx]));
        build_chain_doc_impl(&prefix, chain_end, InlineLookups::NONE, printer)
    };
    append_member_tail(chain_doc, peeled, inline.every, printer)
}

/// Append a peeled member tail to the chain's doc. The gap's same-line block
/// comments stay inline; the members keep member.js's per-member break points —
/// each `.prop` rides [`member_lookup_group`], so the overflowing member drops to
/// its own line while everything before it stays where the width left it
/// (`expressions/member/call_base_trailing_members_long`,
/// `expressions/member/chain_base_tail_long`). A member that FITS still hugs: the
/// group's fit look-ahead ends at the next lookup's own softline, so only the
/// member that overflows takes its break.
///
/// The two ways a lookup gives that break point up are both prettier's
/// `shouldInline` (member.js), and each enters here already answered:
///
/// - `inline_every_lookup` glues EVERY member — a chain the parent marked, an
///   assignment TARGET or a `new` CALLEE, carries no break point at any lookup, so
///   the width falls to the call's arguments or to the operator
///   ([`super::inline_lookups`]).
/// - [`PeeledTail::inlined_last`] glues the LAST one — the call-object clause,
///   resolved at the peel where the object's shape is visible
///   (`expressions/member/chain_base_tail_inlined_long`).
fn append_member_tail(
    chain_doc: DocId,
    peeled: &PeeledTail<'_, '_>,
    inline_every_lookup: bool,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let mut parts: DocBuf = smallvec![chain_doc];
    if let Some(classified) = &peeled.gap_comments {
        parts.push(printer.build_trailing_block_doc(&classified.trailing_block));
    }
    for (i, node) in peeled.tail.iter().enumerate() {
        // The first member's gap comments were just emitted above — skip them in the
        // node print so they can't double-print (the add_group_no_break seam).
        let member = print_node_inner(node, printer, false, i == 0);
        let inlined = inline_every_lookup || (peeled.inlined_last && i + 1 == peeled.tail_len);
        parts.push(if inlined {
            member
        } else {
            member_lookup_group(d, member)
        });
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
    // EXCEPTION: When chain ends with member AND has exactly one call in rest.
    // Only the tails the peel refuses still reach this — a computed / private /
    // non-null node in the tail, or a commented / blank-preceded gap
    // (`member_ending_computed`, `member_ending_nonnull`); a plain `.prop` tail
    // was peeled off before the chain's expand decision ever ran.
    let force_expand_from_breaking =
        any_non_last_breaks && !(chain_ends_with_member && rest_call_count == 1);

    // Prettier's fourth force-expand condition
    // (`lastGroupWillBreakAndOtherCallsHaveFunctionArguments`, member-chain.js): the
    // LAST group is a call that breaks, and some EARLIER call takes a function/arrow
    // argument. Prettier then prints `group(expanded)` — no `conditionalGroup` at all —
    // so the expanded chain is its settled form and stays that way across passes.
    //
    // It has to be here, beside its three siblings, and not inside one of the
    // hug-state builders below: the hug those shapes would otherwise take comes from
    // the `on_line` state, whose fits truncates at the argument's forced break and so
    // never consults them. Wired into a builder instead, the refusal is inert — the
    // flat authoring settles on the expanded chain while the broken authoring settles
    // on the hug, two tsv fixed points for one document where prettier has one.
    let last_group_breaking_call = groups
        .last()
        .and_then(|g| g.nodes.last())
        .is_some_and(ChainNode::is_call)
        && on_line.last().is_some_and(|&doc| d.will_break(doc));
    let force_expand_from_refusal =
        last_group_breaking_call && other_calls_have_function_arguments(first_groups, rest_groups);

    // Build expanded variant
    let expanded = build_expanded_doc(groups, should_merge, printer);

    if force_expand || force_expand_from_breaking || force_expand_from_refusal {
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

    // The optional MIDDLE state — chain prefix flat, last group broken — between
    // `on_line` and the fully expanded fallback. Two builders answer for it, in
    // precedence order, and they partition the same shape by how the last call's
    // argument was AUTHORED:
    //
    // - it already BREAKS (multiline as written): the argument re-reads as
    //   authored-expanded and this state is the hug prettier settles on;
    // - it is authored FLAT: the hug is still prettier's settled form, but only
    //   from its SECOND pass, so the state is admitted through a never-fits gate
    //   that withdraws it wherever the broken chain is the shared fixed point.
    //
    // With neither, the chain is the plain two-state group.
    let middle_state =
        build_breaking_object_chain_doc(first_groups, rest_groups, printer).or_else(|| {
            build_flat_object_hug_state(first_groups, rest_groups, *on_line.last()?, printer)
        });

    match middle_state {
        Some(state) => d.conditional_group(&[on_line_doc, state, expanded]),
        None => d.conditional_group(&[on_line_doc, expanded]),
    }
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
    // chain: a direct object/array literal, a `new`/call expression wrapping one
    // (e.g. `new Response(body, {…})`), or an arrow whose grammar-parenthesized
    // expression body is an object literal. Prettier's own refusal for these shapes
    // (`lastGroupWillBreakAndOtherCallsHaveFunctionArguments`) is not spelled here —
    // it force-expands the whole chain upstream in `build_long_chain_doc`, before any
    // state is built, which is the only place it can act: the hug these shapes would
    // otherwise take comes from the `on_line` state, not from this one.
    //
    // The arrow must be in this kind set and not only in the flat-authored gate: a
    // forced break DEEP in the argument (an authored blank between properties, a
    // forced method body) is one prettier's propagateBreaks lifts to the object's own
    // group, so its one-line measurement truncates at the `{` — while tsv's flat walk
    // would accumulate the properties' width and overflow before ever reaching the
    // break. This state's `group_break`-wrapped last group restores exactly that
    // truncation. Other callbacks (block bodies, non-object bodies) stay excluded.
    let last_group_will_break_object = last_group_single_argument(rest_groups).is_some_and(|arg| {
        (matches!(
            arg,
            Expression::ObjectExpression(_)
                | Expression::ArrayExpression(_)
                | Expression::NewExpression(_)
                | Expression::CallExpression(_)
        ) || is_arrow_with_paren_object_body(arg))
            && d.will_break(printer.build_expression_doc(arg))
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
    Some(build_prefix_flat_last_expanded_doc(
        first_groups,
        rest_groups,
        printer,
    ))
}

/// The shared hug-state construction: chain prefix flat, last group force-broken.
///
/// Only the last group is force-broken: when this state is selected in Flat
/// mode, its expanded call args still render in Break mode (so nested groups,
/// e.g. arrow sigs, evaluate fits() against Break-mode rest commands) — and the
/// forced break is what makes the conditional group's measurement truncate at
/// it, seeing only the chain head.
fn build_prefix_flat_last_expanded_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let mut all_parts = build_groups_flat_docs(first_groups, printer);
    if let Some((last, prefix)) = rest_groups.split_last() {
        all_parts.extend(prefix.iter().map(|g| print_group(g, printer)));
        all_parts.push(d.group_break(print_group_expanded(last, printer)));
    }
    d.concat(&all_parts)
}

/// Build the never-fits-gated hug state for a FLAT-authored, object-rooted last
/// argument — the member-chain hug convergence window (see the caller's comment
/// and `docs/conformance_prettier_ts.md` §Member-chain wide-last-argument hug
/// convergence).
///
/// Prettier reaches its fixed point here in two passes: pass 1's flat argument
/// carries no forced break, so `fits(oneLine)` reads the whole flat content,
/// overflows, and the chain expands — the argument, unfittable on the expanded
/// continuation line too, breaks inside it. Pass 2 re-reads that object as
/// authored-expanded (printObject's newline-after-`{` rule), truncates its fit
/// measurement at the forced break, and collapses back to the flat chain with
/// the argument hugging. This state IS that settled form, admitted in pass 1 via
/// `DocArena::gated_state`: eligible only while the last group's flat form
/// (the probe) cannot fit on the expanded chain's continuation line — otherwise
/// the broken chain keeping the argument flat is the shared fixed point and the
/// expanded fallback must win.
///
/// The window is deliberately object-rooted — a bare object literal, or an
/// arrow whose grammar-parenthesized expression body is one:
/// - an ARRAY carries no authored-multiline re-read rule, so a flat-authored
///   array is stable at the broken chain in prettier (probed) — admitting a hug
///   would diverge;
/// - a `new`/call wrapper reaches the settled form in two passes only when an
///   object INSIDE it breaks, a deeper discriminator this gate cannot express —
///   a known residual, left to the expanded fallback, which is prettier's own
///   pass-1 output there (so tsv and prettier agree at every pass, and share the
///   two-pass convergence). The settled form both keep is pinned by the
///   `calls/chained/last_arg_wrapped_object` fixture.
///
/// A `will_break` last group is outside the window — the same question prettier's
/// `lastGroupWillBreakAndOtherCallsHaveFunctionArguments` asks
/// (`willBreak(printedGroups.at(-1))`), asked of the same doc. Its forced break
/// re-reads as authored-expanded and the `on_line` state's truncated measurement
/// already lands the hug (the pass-2 path, live today); the chain-level refusal in
/// `build_long_chain_doc` is what holds it back when an earlier call takes a
/// function argument. The refusal is repeated here because that one requires the
/// break and this window is defined by its absence: the flat authoring prettier
/// starts from carries no break at all, so nothing upstream has fired yet.
fn build_flat_object_hug_state<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
    probe: DocId,
    printer: &Printer<'_>,
) -> Option<DocId> {
    let d = printer.arena();
    // The probe IS the last group's flat doc, so its `will_break` answers the
    // authored-flat question without rebuilding the argument subtree that
    // `build_breaking_object_chain_doc` already built and discarded.
    if d.will_break(probe) {
        return None;
    }
    let arg = last_group_single_argument(rest_groups)?;
    // TODO: widen to a `new`/call wrapper whose own last argument is an object —
    // the settled form there is the args-expanded shape, not this hug state, so it
    // needs a second state rather than a wider kind test (`last_arg_wrapped_object`).
    let object_rooted =
        matches!(arg, Expression::ObjectExpression(_)) || is_arrow_with_paren_object_body(arg);
    if !object_rooted {
        return None;
    }
    if other_calls_have_function_arguments(first_groups, rest_groups) {
        return None;
    }
    let contents = build_prefix_flat_last_expanded_doc(first_groups, rest_groups, printer);
    Some(d.gated_state(probe, contents))
}

/// The SOLE argument of the chain's last call — the one whose kind decides which
/// hug state (if any) the chain admits.
///
/// The call is found by scanning the last group's nodes in reverse, since a group
/// is `[member, …, call]` and may carry a trailing non-call node (a `!`). Stated
/// once because the two hug-state builders ask it of the same group and must not
/// drift: one reading the last call and the other the last node would let a shape
/// into one state that the other's gate had already refused.
fn last_group_single_argument<'a>(rest_groups: &[ChainGroup<'a>]) -> Option<&'a Expression<'a>> {
    let call = rest_groups
        .last()?
        .nodes
        .iter()
        .rev()
        .find_map(ChainNode::as_call_expression)?;
    match call.arguments {
        [arg] => Some(arg),
        _ => None,
    }
}

/// An arrow whose grammar-parenthesized expression body is an object literal —
/// the `.map((item) => ({ … }))` shape, the arrow spelling of the object-rooted
/// kind set.
fn is_arrow_with_paren_object_body(arg: &Expression<'_>) -> bool {
    matches!(
        arg,
        Expression::ArrowFunctionExpression(arrow) if matches!(
            &arrow.body,
            ArrowFunctionBody::Expression(body)
                if matches!(&**body, Expression::ObjectExpression(_))
        )
    )
}

/// Prettier's `lastGroupWillBreakAndOtherCallsHaveFunctionArguments` operand:
/// does any call BEFORE the chain's last one take a function/arrow argument?
///
/// The last call is dropped by holding each one back a step rather than by
/// collecting and popping: the answer needs no storage, and this walk sits on the
/// chain's hot path — the same reason `should_force_chain_expand` iterates its call
/// nodes in place.
fn other_calls_have_function_arguments<'a>(
    first_groups: &[ChainGroup<'a>],
    rest_groups: &[ChainGroup<'a>],
) -> bool {
    let takes_function_argument = |call: &CallExpression<'_>| {
        call.arguments.iter().any(|a| {
            matches!(
                a,
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            )
        })
    };
    let mut pending: Option<&CallExpression<'_>> = None;
    for call in first_groups
        .iter()
        .chain(rest_groups.iter())
        .flat_map(|g| g.nodes.iter())
        .filter_map(ChainNode::as_call_expression)
    {
        if let Some(prev) = pending.replace(call)
            && takes_function_argument(prev)
        {
            return true;
        }
    }
    false
}
