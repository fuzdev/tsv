// Member-only chain handling
//
// Handles chains that contain only member accesses (no calls) using
// fill() for greedy packing of segments.

use super::super::printing::{ChainPrinter, print_node, print_node_inner};
use super::super::types::{ChainGroup, ChainNode, ChainNodeRefVec, DocBuf};
use super::helpers::push_gap_comments_and_break;
use crate::ast::internal::Expression;
use smallvec::smallvec;
use tsv_lang::doc::arena::DocId;

/// True if a member-only chain has a line comment in any inter-member gap.
///
/// Block-only comments stay on the fill path (they format inline without forcing a
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
        .any(|node| match node.comment_range() {
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
        match node.comment_range() {
            Some((obj_end, prop_start)) if printer.has_comments_between(obj_end, prop_start) => {
                push_gap_comments_and_break(&mut rest, printer, obj_end, prop_start, true);
                rest.push(print_node_inner(node, printer, false, true));
            }
            _ => {
                rest.push(d.hardline());
                rest.push(print_node(node, printer));
            }
        }
    }

    d.concat(&[first_doc, d.indent(d.concat(&rest))])
}

/// Build doc for member-only chains using fill for greedy packing
///
/// Break points are ONLY at member access (`.foo`), not at non-null (`!`).
/// This ensures `.foo!` stays together as a unit.
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
    // affect the formatting output. The fill-based approach below handles
    // line breaking based on width, which is the correct behavior.

    // For member-only chains, build first_doc from just the base identifier
    // and any immediately following non-null assertions (not the entire first group).
    // This ensures all member accesses become fill segments that can be wrapped.
    //
    // The grouping logic puts almost all members in the first group (for the
    // "short chain fits on one line" case), but for fill-based breaking we need
    // each member access to be a separate segment.
    let mut first_doc_end = 0;
    for (i, node) in all_nodes.iter().enumerate() {
        if node.is_member() {
            // Stop at first member - that starts the fill segments
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
    let first_doc = if first_doc_nodes.is_empty() {
        d.empty()
    } else {
        d.concat(&first_doc_nodes)
    };

    // If no remaining nodes after first_doc, just return it
    if first_doc_end >= all_nodes.len() {
        return first_doc;
    }

    // Build segments where each segment ENDS with a member access
    // Break points are BEFORE each member access (softlines in fill)
    //
    // Example: `a!.b!.c!` with nodes [Base(a), NonNull, Member(.b), NonNull, Member(.c), NonNull]
    //   first_doc = "a!"
    //   remaining nodes: [Member(.b), NonNull, Member(.c), NonNull]
    //   segments = [".b!", ".c!"]
    //   result = group(first + indent(fill([.b!, softline, .c!])))
    //
    // Fill packs segments greedily - as many as fit on each line.

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
        if node.is_member() && seen_member {
            segments.push(d.concat(&std::mem::take(&mut current_segment)));
            seen_member = false;
        }

        // print_node handles block comments for member nodes
        current_segment.push(print_node(node, printer));

        if node.is_member() {
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

    // Single segment: identifier bases get flat concat (no break point),
    // non-identifier bases (regex literals, etc.) get a breakable group so
    // the dot can break when the line exceeds print width.
    //
    // Identifier bases stay flat to avoid regressions in fill contexts —
    // a breakable `obj.prop` would split at the dot whenever remaining
    // width on the current fill line is small.
    if segments.len() == 1 {
        let segment = segments[0];

        let base_is_identifier = matches!(
            all_nodes.first(),
            Some(ChainNode::Base {
                expr: Expression::Identifier(_)
                    | Expression::ThisExpression(_)
                    | Expression::Super(_),
                ..
            })
        );

        if base_is_identifier {
            return d.concat(&[first_doc, segment]);
        }

        let on_line = d.concat(&[first_doc, segment]);
        let expanded = d.concat(&[first_doc, d.indent(d.concat(&[d.softline(), segment]))]);
        return d.conditional_group(&[on_line, expanded]);
    }

    // For 2+ segments: use conditional_group([oneLine, expanded]).
    // Short chains (≤2 segments): fill includes the base for greedy packing from
    // the base position, allowing the fill to break after the base when needed.
    // Long chains (3+ segments): fill starts after the base and packs greedily.

    // Build on_line: everything concatenated flat
    // Note: on_line does NOT need trailing_reserve because fits_with_lookahead
    // already sees trailing content (comma, etc.) in rest_commands with the
    // correct mode (Break → "," is counted).
    let mut on_line_parts: DocBuf = smallvec![first_doc];
    for &segment in &segments {
        on_line_parts.push(segment);
    }
    let on_line = d.concat(&on_line_parts);

    if segments.len() <= 2 {
        // Short trailing members (≤2 segments like `.right.start`): include the base
        // as the first item of the fill, with softlines between base and segments.
        // Fill packs greedily: keeps items on the same line if they fit, breaks to
        // the next indented line when they don't.
        //
        // This correctly handles both:
        // - Long base + comments consuming the line: fill breaks after base, packs
        //   short segments together: `...labeled\n\t.right.start`
        // - Short base + long last segment: fill packs first segment with base,
        //   breaks before long segment: `ssss.data\n\t.fallbackBBBB...`
        let mut fill_with_base_parts: DocBuf = smallvec![first_doc];
        for &segment in &segments {
            fill_with_base_parts.push(d.softline());
            fill_with_base_parts.push(segment);
        }
        let fill_with_base = d.fill(&fill_with_base_parts);
        let expanded = d.indent(fill_with_base);
        d.conditional_group(&[on_line, expanded])
    } else {
        // Long chain (3+ segments): fill-based greedy packing after base
        let mut fill_parts = DocBuf::new();
        for &segment in &segments {
            if !fill_parts.is_empty() {
                fill_parts.push(d.softline());
            }
            fill_parts.push(segment);
        }
        let fill_doc = d.fill(&fill_parts);
        let fill_expanded = d.concat(&[first_doc, d.indent(fill_doc)]);
        d.conditional_group(&[on_line, fill_expanded])
    }
}
