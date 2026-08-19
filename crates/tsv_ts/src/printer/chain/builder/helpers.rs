// Chain builder helper functions
//
// Shared utilities used across the builder submodules:
// - ChainPartsBuilder: Builder for constructing chain parts with comments

use super::super::printing::{
    group_comment_gap, print_group, print_group_expanded, print_group_expanded_skip_first_comments,
    print_group_skip_first_comments, push_gap_comments_and_break,
};
use super::super::types::ChainGroup;
use crate::printer::Printer;
use tsv_lang::ClassifiedComments;
use tsv_lang::doc::{DocBuf, arena::DocId};

/// Whether a gap's comments force a line break: a line comment (trailing or
/// leading) consumes the rest of its line, and a leading comment needs its own
/// line — only a same-line trailing **block** comment can stay inline with the
/// code around it.
pub(super) fn gap_has_break_forcing_comments(classified: &ClassifiedComments<'_>) -> bool {
    !classified.trailing_line.is_empty()
        || !classified.leading_block.is_empty()
        || !classified.leading_line.is_empty()
}

/// Builder for constructing chain parts with proper comment handling.
///
/// Encapsulates the logic for interleaving comments, line breaks, and groups
/// when building the rest of a chain (everything after the first group).
pub(crate) struct ChainPartsBuilder<'a, 'p, 'pr> {
    parts: &'p mut DocBuf,
    printer: &'a Printer<'pr>,
    use_expanded: bool,
}

impl<'a, 'p, 'pr> ChainPartsBuilder<'a, 'p, 'pr> {
    pub(crate) fn new(
        parts: &'p mut DocBuf,
        printer: &'a Printer<'pr>,
        use_expanded: bool,
        group_count: usize,
    ) -> Self {
        // Each group produces ~5 docs: trailing comments, line break, block comments,
        // leading comments, and the group doc itself. `parts` is the caller-owned
        // pooled buffer, filled in place (retaining a prior spill's capacity).
        parts.reserve(group_count * 5);
        Self {
            parts,
            printer,
            use_expanded,
        }
    }

    /// Add a group with its associated comments and line breaks
    pub(crate) fn add_group(&mut self, group: &ChainGroup<'_>) {
        self.add_comments_and_break(group);
        self.add_group_doc(group);
    }

    /// Add a group without a preceding line break, but with trailing comments
    /// Used for trailing member accesses that should stay on same line as `})`
    pub(crate) fn add_group_no_break(&mut self, group: &ChainGroup<'_>) {
        self.add_trailing_comments_only(group);
        self.add_group_doc(group);
    }

    /// Add only trailing **block** comments (no line break, no leading comments).
    /// Used when the next element should stay on the same line as the previous —
    /// `.map(x => x) /* comment */.length`.
    ///
    /// Only a block comment can be emitted here, and that is a property of the
    /// caller, not a limitation: [`build_rest_parts_with_comments`] routes a group
    /// to [`Self::add_group_no_break`] **only** when its gap holds no trailing line
    /// comment and no leading comment (its `last_has_break_forcing_comments`),
    /// precisely because both need a break this path refuses to emit. A line comment
    /// reaching an emitter that cannot break would swallow the member after it.
    ///
    /// Leading comments (on their own line before the member) are likewise not
    /// emitted here; Prettier moves them elsewhere (e.g. after `=`), which needs a
    /// structural transformation beyond this path.
    fn add_trailing_comments_only(&mut self, group: &ChainGroup<'_>) {
        if let Some((object_end, property_start)) = group_comment_gap(group, self.printer) {
            let classified = self.printer.classify_comments(object_end, property_start);
            debug_assert!(
                !gap_has_break_forcing_comments(&classified),
                "a break-forcing comment reached the no-break chain path — \
                 `last_has_break_forcing_comments` should have routed this group to `add_group`"
            );

            self.parts.push(
                self.printer
                    .build_trailing_block_doc(&classified.trailing_block),
            );
        }
    }

    /// Add trailing comments, line break, and leading comments before a group.
    /// Delegates to the shared [`push_gap_comments_and_break`] so this group path
    /// and the member-only breaking path render gap comments identically.
    fn add_comments_and_break(&mut self, group: &ChainGroup<'_>) {
        if let Some((object_end, property_start)) = group_comment_gap(group, self.printer) {
            push_gap_comments_and_break(
                self.parts,
                self.printer,
                object_end,
                property_start,
                group
                    .nodes
                    .first()
                    .and_then(super::super::ChainNode::paren_gap_skip),
            );
        } else {
            // No member range - just add line break
            let d = self.printer.arena();
            self.parts.push(d.hardline());
        }
    }

    /// Add the group's doc (either expanded or normal)
    ///
    /// Skips block comments for the first member since `add_comments_and_break`
    /// already handles them (emitting before the line break).
    fn add_group_doc(&mut self, group: &ChainGroup<'_>) {
        self.parts.push(if self.use_expanded {
            print_group_expanded_skip_first_comments(group, self.printer)
        } else {
            print_group_skip_first_comments(group, self.printer)
        });
    }
}

/// Build rest parts with comments and blank line preservation
/// Handles both trailing line comments (same line) and leading line comments (own line)
/// Emits: [trailing_comments?, line_break, leading_comments?, group] for each rest group
pub(crate) fn build_rest_parts_with_comments<'a>(
    parts: &mut DocBuf,
    rest_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
    use_expanded: bool,
) {
    // Check if last group is a simple member (no calls) - it should stay on same line as `})`
    // e.g., `.filter().map({...})).length` - `.length` stays on same line as `})`
    let last_is_simple_member = rest_groups.last().is_some_and(|g| {
        g.nodes.len() == 1 && g.nodes.iter().all(|n| n.is_member() && !n.is_call())
    });

    // Check if last group has comments that force a line break — those can't ride
    // the no-break path (`add_group_no_break` emits only trailing block comments).
    let last_has_break_forcing_comments = last_is_simple_member
        && rest_groups.last().is_some_and(|g| {
            group_comment_gap(g, printer).is_some_and(|(object_end, property_start)| {
                let classified = printer.classify_comments(object_end, property_start);
                gap_has_break_forcing_comments(&classified)
            })
        });

    let mut builder = ChainPartsBuilder::new(parts, printer, use_expanded, rest_groups.len());
    for (i, group) in rest_groups.iter().enumerate() {
        // Don't add hardline before last group if it's a simple member WITHOUT
        // comments that force a break
        let is_last = i == rest_groups.len() - 1;
        if is_last && last_is_simple_member && !last_has_break_forcing_comments {
            builder.add_group_no_break(group);
        } else {
            builder.add_group(group);
        }
    }
}

/// Build an expanded chain doc with first group(s) inline and rest indented
///
/// Common pattern for expanded chains: first group(s) + hardline + indent(rest)
pub(super) fn build_expanded_chain_doc<'a>(
    groups: &[ChainGroup<'a>],
    split_at: usize,
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    if groups.is_empty() {
        return d.empty();
    }

    let (first_groups, rest) = groups.split_at(split_at.min(groups.len()));

    // Print first group(s) inline
    let first_docs: DocBuf = first_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    let first_doc = d.concat(&first_docs);

    if rest.is_empty() {
        return first_doc;
    }

    // Print rest with hardlines and indent (including trailing comments and blank line preservation)
    let mut rest_parts = d.pooled_docbuf();
    build_rest_parts_with_comments(&mut rest_parts, rest, printer, false);

    d.concat(&[first_doc, d.indent(d.concat(&rest_parts))])
}

/// Build the expanded doc variant (first group(s) + indented rest)
pub(super) fn build_expanded_doc<'a>(
    groups: &[ChainGroup<'a>],
    should_merge: bool,
    printer: &Printer<'_>,
) -> DocId {
    let split_at = if should_merge { 2 } else { 1 };
    build_expanded_chain_doc(groups, split_at, printer)
}

/// Build first groups doc (merged when should_merge)
pub(super) fn build_first_groups_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let first_docs: DocBuf = first_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    d.concat(&first_docs)
}

/// Build first groups doc with expanded calls
pub(super) fn build_first_groups_expanded_doc<'a>(
    first_groups: &[ChainGroup<'a>],
    printer: &Printer<'_>,
) -> DocId {
    let d = printer.arena();
    let first_docs: DocBuf = first_groups
        .iter()
        .map(|g| print_group_expanded(g, printer))
        .collect();
    d.concat(&first_docs)
}
