// Member-only chain handling
//
// Handles chains that contain only member accesses (no calls), giving each
// lookup its own group — prettier's `printMemberExpression` shape.

use super::super::printing::{
    ChainPrinter, node_comment_gap, print_node, print_node_inner, push_gap_comments_and_break,
};
use super::super::types::{ChainGroup, ChainNode, ChainNodeRefVec};

use crate::ast::internal::Expression;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// True if a member-only chain has a line comment in any inter-member gap.
///
/// Block-only comments stay on the width-driven path (they format inline without forcing a
/// break); a line comment must end its line, so it forces the chain to break to
/// preserve the comment where the author wrote it — see
/// [`build_member_only_chain_with_comments_doc`].
pub(super) fn member_only_has_interior_line_comments<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> bool {
    groups
        .iter()
        .flat_map(|g| g.nodes.iter())
        .any(|node| match node_comment_gap(node, printer) {
            Some((start, end)) => {
                let c = printer.classify_comments(start, end);
                !c.trailing_line.is_empty() || !c.leading_line.is_empty()
            }
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
pub(super) fn build_member_only_chain_with_comments_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
) -> DocId {
    let d = printer.arena();
    let all_nodes: ChainNodeRefVec<'_, 'a> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    // first_doc = base + any leading non-member nodes (e.g. a non-null on the base).
    let first_doc_end = all_nodes
        .iter()
        .take_while(|n| !n.is_member())
        .count()
        .max(1);
    let first_doc_nodes: DocBuf = all_nodes
        .iter()
        .take(first_doc_end)
        .map(|n| print_node(n, printer))
        .collect();
    let first_doc = d.concat(&first_doc_nodes);

    // Each remaining node breaks onto its own line. When its gap carries comments,
    // emit them in place (trailing on the previous line, leading on their own) and
    // print the node skipping its own comments; otherwise just break before it.
    let mut rest = DocBuf::new();
    for node in &all_nodes[first_doc_end..] {
        // `gap_end` is the property start for a plain member, but the `[` for a
        // computed one — the comments inside its brackets belong to the bracket
        // builder, not to this chain gap. See `node_comment_gap`.
        match node_comment_gap(node, printer) {
            Some((obj_end, gap_end)) if printer.has_comments_to_emit_between(obj_end, gap_end) => {
                push_gap_comments_and_break(&mut rest, printer, obj_end, gap_end, true);
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

/// Whether a node opens a new segment — i.e. whether a break point may precede it.
///
/// A `.prop` lookup may break onto its own line; a computed `[i]` / `?.[i]` lookup may
/// NOT. Prettier's `printMemberExpression` (member.js) inlines every computed lookup
/// (`shouldInline` includes `node.computed`), so a computed access stays glued to the
/// object and sheds width by breaking its own brackets instead (`computed_lookup_doc`).
/// Giving it a segment of its own would put a softline before the `[`, which prettier
/// never emits — and, because the brackets used to be unbreakable, was tsv's only way to
/// fit an overlong computed access.
fn starts_segment(node: &ChainNode<'_>) -> bool {
    node.is_member() && !node.is_computed()
}

/// Build doc for member-only chains: one group per lookup, mirroring prettier's
/// `printMemberExpression`.
///
/// Break points are ONLY at member access (`.foo`), not at non-null (`!`) and not at a
/// computed lookup (`[i]` — see `starts_segment`). This ensures `.foo!` stays together as
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
pub(super) fn build_member_only_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    printer: &P,
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

    // For member-only chains, build first_doc from just the base identifier
    // and any immediately following non-null assertions (not the entire first group).
    // This gives every member access its own segment, and so its own break point.
    //
    // The grouping logic puts almost all members in the first group (for the
    // "short chain fits on one line" case), so the segments are re-derived here.
    let mut first_doc_end = 0;
    for (i, node) in all_nodes.iter().enumerate() {
        if starts_segment(node) {
            // Stop at first member - that starts the segments
            break;
        }
        first_doc_end = i + 1;
    }

    // Build first_doc from base + any trailing non-null assertions
    let first_doc_nodes: DocBuf = all_nodes
        .iter()
        .take(first_doc_end)
        .map(|n| print_node(n, printer))
        .collect();
    // `concat` short-circuits the empty case to `empty()`.
    let first_doc = d.concat(&first_doc_nodes);

    // If no remaining nodes after first_doc, just return it
    if first_doc_end >= all_nodes.len() {
        return first_doc;
    }

    // Build segments where each segment STARTS with a member access and carries the
    // nodes glued to it (a trailing `!`, a computed `[i]`). Each gets its own break
    // point, placed BEFORE the member access.
    //
    // Example: `a!.b!.c!` with nodes [Base(a), NonNull, Member(.b), NonNull, Member(.c), NonNull]
    //   first_doc = "a!"
    //   remaining nodes: [Member(.b), NonNull, Member(.c), NonNull]
    //   segments = [".b!", ".c!"]
    //   result = a! + group(indent(softline + .b!)) + group(indent(softline + .c!))

    // Build segments by collecting nodes until we see the NEXT member
    // Each segment includes everything up to and including a member (+ trailing non-null)
    // Note: Block comments are handled by print_node for member nodes
    let remaining_nodes = &all_nodes[first_doc_end..];
    let mut segments: DocBuf = DocBuf::new();
    let mut current_segment: DocBuf = DocBuf::new();
    let mut seen_member = false;

    for (i, node) in remaining_nodes.iter().enumerate() {
        // Check if this is a member and we already have content that includes a member
        // If so, flush before adding this member
        if starts_segment(node) && seen_member {
            segments.push(d.concat(&std::mem::take(&mut current_segment)));
            seen_member = false;
        }

        // print_node handles block comments for member nodes
        current_segment.push(print_node(node, printer));

        if starts_segment(node) {
            seen_member = true;
        }

        // If this is the last node, flush
        if i == remaining_nodes.len() - 1 && !current_segment.is_empty() {
            segments.push(d.concat(&std::mem::take(&mut current_segment)));
        }
    }

    // If no segments, just return the first doc
    if segments.is_empty() {
        return first_doc;
    }

    // A lone `a.prop` off an identifier base gets no break point at all — prettier's
    // `shouldInline` inlines the lookup when object and property are both identifiers
    // and the whole thing isn't itself inside a member (member.js). Everything else
    // falls through to the per-lookup groups below.
    if segments.len() == 1
        && matches!(
            all_nodes.first(),
            Some(ChainNode::Base {
                expr: Expression::Identifier(_)
                    | Expression::ThisExpression(_)
                    | Expression::Super(_),
                ..
            })
        )
    {
        return d.concat(&[first_doc, segments[0]]);
    }

    // Mirror prettier's `printMemberExpression`, which gives EACH lookup
    // its own `group(indent([softline, lookup]))` and leaves the object doc beside it
    // (member.js). Because a nested member's doc is `[objectDoc, lookupGroup]`, the
    // groups appear innermost-first in the stream — so this is a left fold over the
    // segments, each wrapping only its own break point.
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
    let mut doc = first_doc;
    for &segment in &segments {
        doc = d.concat(&[doc, d.group(d.indent(d.concat(&[d.softline(), segment])))]);
    }
    doc
}
