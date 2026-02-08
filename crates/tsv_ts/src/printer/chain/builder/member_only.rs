// Member-only chain handling
//
// Handles chains that contain only member accesses (no calls) using
// fill() for greedy packing of segments.

use super::super::printing::{ChainPrinter, print_node};
use super::super::types::{ChainGroup, ChainNode};
use tsv_lang::doc::{self, Doc};

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
) -> Doc {
    // NOTE: We intentionally do NOT add break_parent for line comments here.
    // The break_parent approach causes issues with line_suffix flushing order -
    // the suffix gets flushed at the wrong line break. Instead, line comments
    // in member-only chains are handled via line_suffix, and the assignment
    // layout naturally handles them (suffix appears at end of line).

    // Flatten all nodes into individual segments
    let all_nodes: Vec<&ChainNode<'a>> = groups.iter().flat_map(|g| g.nodes.iter()).collect();

    if all_nodes.is_empty() {
        return doc::empty();
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
    let first_doc_nodes: Vec<Doc> = all_nodes
        .iter()
        .take(first_doc_end)
        .map(|n| print_node(n, printer))
        .collect();
    let first_doc = if first_doc_nodes.is_empty() {
        doc::empty()
    } else {
        doc::concat(first_doc_nodes)
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
    let mut segments: Vec<Doc> = Vec::new();
    let mut current_segment: Vec<Doc> = Vec::new();
    let mut seen_member = false;

    for (i, node) in remaining_nodes.iter().enumerate() {
        // Check if this is a member and we already have content that includes a member
        // If so, flush before adding this member
        if node.is_member() && seen_member {
            segments.push(doc::concat(std::mem::take(&mut current_segment)));
            seen_member = false;
        }

        // print_node handles block comments for member nodes
        current_segment.push(print_node(node, printer));

        if node.is_member() {
            seen_member = true;
        }

        // If this is the last node, flush
        if i == remaining_nodes.len() - 1 && !current_segment.is_empty() {
            segments.push(doc::concat(std::mem::take(&mut current_segment)));
        }
    }

    // If no segments, just return the first doc
    if segments.is_empty() {
        return first_doc;
    }

    // If only one segment, attach it directly (e.g., `a!` or `a!.b!`)
    if segments.len() == 1 {
        let Some(segment) = segments.pop() else {
            return first_doc;
        };
        return doc::concat(vec![first_doc, segment]);
    }

    // For 2+ segments: use conditional_group for proper break decisions
    // fill() handles greedy packing - it prints as many items as fit on
    // the current line, then breaks and continues.
    //
    // conditional_group([oneLine, expanded]) where:
    // - oneLine: all segments concatenated flat (no breaks)
    // - expanded: first_doc + indent(fill(segments with softlines))
    //
    // The fill starts at the position after first_doc and greedily packs.
    // Since fill is inside indent, overflow lines get the extra indent.

    // Build on_line: everything concatenated flat
    // Note: on_line does NOT need trailing_reserve because fits_with_lookahead
    // already sees trailing content (comma, etc.) in rest_commands with the
    // correct mode (Break → "," is counted).
    let mut on_line_parts = vec![first_doc.clone()];
    for segment in &segments {
        on_line_parts.push(segment.clone());
    }
    let on_line = doc::concat(on_line_parts);

    // Build fill_parts with softlines between segments
    let mut fill_parts = Vec::new();
    for segment in &segments {
        if !fill_parts.is_empty() {
            fill_parts.push(doc::softline());
        }
        fill_parts.push(segment.clone());
    }

    // Build fill with segments - this packs greedily at the current position.
    // Note: The fill now uses rest_commands for width calculation, so we don't
    // need to set a trailing_reserve here. The fill will see the actual trailing
    // content in the document tree.
    let fill_doc = doc::fill(fill_parts);

    // Use conditional_group with on_line (flat) and expanded (fill-based) variants.
    // The fill packs greedily, respecting print width including trailing content.
    let expanded = doc::concat(vec![first_doc, doc::indent(fill_doc)]);

    doc::conditional_group(vec![on_line, expanded])
}
