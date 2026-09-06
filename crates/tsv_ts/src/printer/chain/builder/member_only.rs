// Member-only chain handling
//
// Handles chains that contain only member accesses (no calls), giving each
// lookup its own group — prettier's `printMemberExpression` shape.

use super::super::printing::{
    chain_gap_any, member_lookup_group, node_comment_gap, print_node, print_node_inner,
    push_gap_comments_and_break,
};
use super::super::types::{ChainGroup, ChainNode, ChainNodeRefVec};
use crate::printer::Printer;

use crate::ast::internal::Expression;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// True if a member-only chain has a line comment in any inter-member gap.
///
/// Block-only comments stay on the width-driven path (they format inline without forcing a
/// break); a line comment must end its line, so it forces the chain to break to
/// preserve the comment where the author wrote it — see
/// [`build_member_only_chain_with_comments_doc`].
///
/// The second spelling of "does a node's gap hold a `//`" — the grouping asks the same
/// question as `analysis::gap_has_line_comment`, over the RAW `ChainNode::comment_range`
/// rather than this printer-narrowed [`node_comment_gap`]. The two differ only for a
/// computed member (the narrow one cuts at the `[`), and deliberately; read that
/// function's warning before unifying them.
///
/// Routed through [`chain_gap_any`] like every other question about a chain gap. A
/// member-ONLY chain never widens — `apply_paren_gaps` requires a call — so the hole is
/// always `None` here and the seam is inert; it is used anyway so the reader does not have
/// to re-derive that, and so a widening this file cannot see cannot make it wrong.
pub(super) fn member_only_has_interior_line_comments<'a>(
    groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> bool {
    groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .any(|node| match node_comment_gap(node, printer) {
            Some(gap) => chain_gap_any(gap, node.paren_gap_skip(), |start, end| {
                printer.classify_comments(start, end).has_line_comments()
            }),
            None => false,
        })
}

/// Build a member-only chain that has interior line comments.
///
/// Reverses the historical "emit every mid-chain comment via `line_suffix`"
/// approach, which deferred line comments to end of line — merging and reversing
/// consecutive ones (`a.b // c1⏎// c2⏎.c` → `a.b.c; // c2 // c1`) and dropping
/// nothing only by luck. Instead the chain breaks at every member (the same shape a
/// call in the chain already forces) and each gap's comments are emitted in place
/// via the shared [`push_gap_comments_and_break`] — the exact primitive the
/// call-chain breaking path uses. Comments stay where the author wrote them.
///
/// Prettier hoists the own-line comment before the whole expression and trails the
/// rest; tsv preserves position. Documented divergence
/// (`chained/member_only_interior_line_comment`).
pub(super) fn build_member_only_chain_with_comments_doc<'a>(
    groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let all_nodes: ChainNodeRefVec<'_, 'a> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    // first_doc = base + any leading non-member nodes (e.g. a non-null on the base).
    let first_doc_end = all_nodes
        .iter()
        .take_while(|n| !n.is_member())
        .count()
        .max(1);
    let first_doc = d.concat_iter(
        all_nodes
            .iter()
            .take(first_doc_end)
            .map(|n| print_node(n, printer)),
    );

    // Each remaining node breaks onto its own line. When its gap carries comments,
    // emit them in place (trailing on the previous line, leading on their own) and
    // print the node skipping its own comments; otherwise just break before it.
    let mut rest = DocBuf::new();
    for node in &all_nodes[first_doc_end..] {
        // `gap_end` is the property start for a plain member, but the `[` for a
        // computed one — the comments inside its brackets belong to the bracket
        // builder, not to this chain gap. See `node_comment_gap`.
        match node_comment_gap(node, printer) {
            // The gate reads the same region the emitter below claims — [`chain_gap_any`]
            // and `push_gap_comments_and_break` take the identical hole.
            Some((obj_end, gap_end))
                if chain_gap_any((obj_end, gap_end), node.paren_gap_skip(), |start, end| {
                    printer.has_comments_to_emit_between(start, end)
                }) =>
            {
                push_gap_comments_and_break(
                    &mut rest,
                    printer,
                    obj_end,
                    gap_end,
                    node.paren_gap_skip(),
                );
                rest.push(print_node_inner(node, printer, false, true));
            }
            _ => {
                // A trailing non-null `!` glues to the preceding member — a break
                // before it is a syntax error (`[no LineTerminator here]`), so it must
                // never land on its own line. Every other gapless node keeps its own
                // line: a computed `[i]` lookup deliberately drops to its own line here
                // (the chain breaks at every bracket — see
                // `computed_pre_bracket_line_comment`).
                if !node.is_non_null() {
                    rest.push(d.hardline());
                }
                rest.push(print_node(node, printer));
            }
        }
    }

    d.concat(&[first_doc, d.indent(d.concat(&rest))])
}

/// Prettier's `shouldInline` clause for a lone `a.prop` (member.js): the lookup's OBJECT
/// is an `Identifier`, its PROPERTY is an `Identifier`, and its first non-chain-element-
/// wrapper parent is not itself a member. Every one of the three is literal, and after
/// linearization each names a position in the node list:
///
/// - **object** — the lookup sits IMMEDIATELY after the base, and that base is a bare
///   identifier. `this` and `super` are `ThisExpression` / `Super` nodes, and a leading
///   `!` or `[i]` makes the object a `TSNonNullExpression` / `MemberExpression`, so
///   `this.p`, `super.p`, `a!.p` and `a[i].p` are all outside the clause.
/// - **property** — the lookup is a [`ChainNode::Member`], not a
///   [`ChainNode::PrivateMember`] (`a.#p`).
/// - **parent** — no further LOOKUP may follow, of either kind: a `Member` /
///   `PrivateMember` parent is a `MemberExpression`, and so is a `ComputedMember` one
///   (`a.p[i]` makes `a.p`'s parent a member exactly as `a.p.q` does — `node.computed` is
///   the parent's OWN inline clause, not a wrapper the ancestor walk steps over). A
///   trailing `!` and the `?.` spelling ARE stepped over: `TSNonNullExpression` and
///   `ChainExpression` are prettier's two chain-element wrappers, so `a.p!` and `a?.p`
///   stay in.
///
/// Together those are `[Base(Identifier), Member, NonNull*]`. Widening any of the three
/// welds a lookup that has to drop to its own line past the print width — for most of
/// these shapes the only break point they have (`member/lone_lookup_base_long`,
/// `member/lone_lookup_adjacency_long`, `member/lone_lookup_numeric_index_long`). Keep
/// this separable from `inline_every_lookup` above rather than folding the two: they
/// answer to different prettier clauses and only agree on the output.
fn lone_lookup_off_bare_base(nodes: &[&ChainNode<'_>]) -> bool {
    matches!(
        nodes,
        [
            ChainNode::Base {
                expr: Expression::Identifier(_),
                ..
            },
            ChainNode::Member { .. },
            tail @ ..
        ] if tail.iter().all(|n| n.is_non_null())
    )
}

/// Build doc for member-only chains: one group per lookup, mirroring prettier's
/// `printMemberExpression`.
///
/// Break points are ONLY at member access (`.foo`), not at non-null (`!`) and not at a
/// computed lookup (`[i]` — see [`ChainNode::is_dot_lookup`]). This ensures `.foo!` stays together as
/// a unit.
///
/// Example: `a!.b!.c!` breaks as:
/// ```text
/// a!.b!
///     .c!
/// ```
/// NOT as:
/// ```text
/// a!
///     .b!
///     .c!
/// ```
///
/// `inline_every_lookup` suppresses every one of those break points: a chain the parent
/// marked — an assignment TARGET, a `new` CALLEE — prints as one unbreakable unit (see
/// [`super::super::inline_lookups::resolve_inline_lookups`]).
pub(super) fn build_member_only_chain_doc<'a>(
    groups: &[ChainGroup<'a>],
    inline_every_lookup: bool,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    // NOTE: We intentionally do NOT add break_parent for line comments here.
    // The break_parent approach causes issues with line_suffix flushing order -
    // the suffix gets flushed at the wrong line break. Instead, line comments
    // in member-only chains are handled via line_suffix, and the assignment
    // layout naturally handles them (suffix appears at end of line).

    // Flatten all nodes into individual segments
    let all_nodes: ChainNodeRefVec<'_, 'a> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    if all_nodes.is_empty() {
        return d.empty();
    }

    // Note: We intentionally do NOT check for blank lines here.
    // Blank lines in member-only chains are normalized (removed) - they don't
    // affect the formatting output. The per-lookup groups below handle line
    // breaking based on width, which is the correct behavior.

    // The chain takes NO break point at all — one flat concat of every node. Three of
    // prettier's `shouldInline` clauses (member.js) make a member-only chain flat, so they
    // meet here as one decision rather than as adjacent spellings of the same answer:
    //
    // - the assignment **target** clause and the `new` **callee** clause, which the
    //   parent marks before it builds the operand (`inline_every_lookup`) — the chain
    //   prints as one unbreakable unit, so the assignment sheds width after the operator
    //   instead of splitting the thing being assigned to and the `new` sheds into its
    //   argument list, each holding its line outright when nothing else can break;
    // - a lone `a.prop` off a bare base ([`lone_lookup_off_bare_base`]).
    //
    // A fourth clause reaches this builder without deciding anything here —
    // `node.computed` is answered by [`ChainNode::is_dot_lookup`], which opens no segment for a
    // computed lookup, so its own bracket group survives untouched on either path —
    // which is what keeps `chooseLayout`'s `canBreakLeftDoc` true for `params['key'] = …`.
    //
    if inline_every_lookup || lone_lookup_off_bare_base(&all_nodes) {
        return d.concat_iter(all_nodes.iter().map(|n| print_node(n, printer)));
    }

    // Mirror prettier's `printMemberExpression`, which gives EACH lookup
    // its own `group(indent([softline, lookup]))` and leaves the object doc beside it
    // (member.js). Because a nested member's doc is `[objectDoc, lookupGroup]`, the
    // groups appear innermost-first in the stream — so this is a left fold over the
    // segments, each wrapping only its own break point.
    //
    // ⚠️ The group wraps the SEGMENT-OPENING member ALONE; the nodes glued to it — a
    // trailing `!`, a computed `[i]` — are concatenated **after** it, outside the group.
    // That is prettier's own nesting, one node per `printMemberExpression` frame:
    // `a.p[i]` is `[[a, group(indent([softline, ".p"]))], "[i]"]`, never
    // `[a, group(indent([softline, ".p[i]"]))]`. Both halves of the difference are load
    // bearing. Inside the group, the lookup's `fits` walk would measure the glued run
    // FLAT and break `.p` for width the brackets were going to shed anyway
    // (`objx.aaa…[⏎\tiii⏎]` becomes `objx⏎\t.aaa…[⏎\t\tiii⏎\t]`); outside it, the walk
    // reaches the bracket group's softline in `Break` mode and stops, so the lookup stays
    // flat. And a glued run that DOES break renders at the chain's own indent rather than
    // one level in. See `member/lone_lookup_numeric_index_long`.
    //
    // The one-group-per-lookup shape is what makes a base that breaks INTERNALLY hug
    // its lookup: each group is measured from the column it starts at, and `fits` stops
    // at the next `Line` reached in `Break` mode, so a lookup after a broken `]`/`}`/`)`
    // is measured against a nearly-empty line and stays flat. A single conditional_group
    // over the whole chain cannot express that — one verdict has to cover the base and
    // every lookup at once, so the base's break spilled onto the first lookup
    // (`chained/member_after_breaking_base`).
    //
    // Long chains fall out of the same rule rather than needing greedy packing: every
    // lookup but the last is measured only as far as the next lookup's softline, so it
    // fits and stays flat, and the last one — measured against the real tail — is the
    // one that breaks (`alpha.bravo…papa⏎.quebec`).
    //
    // The base run — the base and whatever is glued to it ahead of the first lookup —
    // sits outside every group. The grouping logic puts almost all members in the first
    // group (for the "short chain fits on one line" case), so the segments are re-derived
    // here: each runs from one segment-starter to the next, so it opens with a member
    // access and carries the nodes glued to it (a trailing `!`, a computed `[i]`). Same
    // `take_while` idiom as the comment-aware twin above, over a different predicate: that
    // one counts `is_member`, so a computed head opens a segment there and is glued here.
    //
    // Example: `a!.b!.c!` with nodes [Base(a), NonNull, Member(.b), NonNull, Member(.c), NonNull]
    //   base run: a!
    //   segments: [.b !] [.c !]
    //   result = a! + group(indent(softline + .b)) + ! + group(indent(softline + .c)) + !
    //
    // Nodes print in source order, and `print_node` handles each member node's own block
    // comments.
    let base_run_end = all_nodes.iter().take_while(|n| !n.is_dot_lookup()).count();
    let mut parts = DocBuf::new();
    parts.extend(
        all_nodes[..base_run_end]
            .iter()
            .map(|n| print_node(n, printer)),
    );
    for segment in all_nodes[base_run_end..].chunk_by(|_, next| !next.is_dot_lookup()) {
        // A run opens with a segment-starter — the one node its break point belongs to —
        // and everything after it is glued: concatenated AFTER the group, never inside it.
        for (i, node) in segment.iter().enumerate() {
            let doc = print_node(node, printer);
            parts.push(if i == 0 {
                member_lookup_group(d, doc)
            } else {
                doc
            });
        }
    }
    d.concat(&parts)
}
