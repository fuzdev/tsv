// Chain builder helper functions
//
// Shared utilities used across the builder submodules:
// - ChainPartsBuilder: Builder for constructing chain parts with comments

use super::super::printing::{
    ChainPrinter, build_chain_line_break, print_group, print_group_expanded,
    print_group_expanded_skip_first_comments, print_group_skip_first_comments,
};
use super::super::types::{ChainGroup, DocBuf};
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing::has_blank_line_between_strict;

/// Emit a chain gap's comments and the line break into `parts`, for the gap
/// between `object_end` and `property_start` (i.e. before a `.member`).
///
/// Order: trailing block comments (`prev /* c */`), trailing line comments (same
/// line, via `line_suffix`), the line break (blank-line aware), then leading block
/// and line comments on their own lines — with blank-line preservation around the
/// leading run. Uses single-pass classification (one binary search).
///
/// This is the single definition of "how a forced chain break renders the comments
/// in its gap", shared by the call-chain group path ([`ChainPartsBuilder`]) and the
/// member-only breaking path, so the two cannot drift (the historical member-only
/// `line_suffix`-everything approach was exactly such a drift — it merged/reversed
/// consecutive mid-chain line comments).
// The same-line/later-line classification (`classify_comments` →
// `tsv_lang::ClassifiedComments`) is shared with `conditional.rs`
// split_pre_operator_comments and `calls/arg_comments.rs` PartitionedComments, so the
// "same-line trails, later-line breaks, never merge" rule lives in one place. Only the
// emission differs per shape — dot (here) / operator / comma — which is intentional
// (this dot path also owns blank-line preservation around the leading run).
pub(crate) fn push_gap_comments_and_break<P: ChainPrinter>(
    parts: &mut DocBuf,
    printer: &P,
    object_end: u32,
    property_start: u32,
    use_hardline: bool,
) {
    // Classify all comments in one pass (single binary search)
    let classified = printer.classify_comments(object_end, property_start);

    // Trailing block comments (same line as previous element): `method() /* c */`
    parts.push(printer.build_trailing_block_doc(&classified.trailing_block));
    // Trailing line comments (same line as previous element), via line_suffix
    parts.push(printer.build_trailing_line_doc(&classified.trailing_line));
    // Line break with blank line preservation
    parts.push(build_chain_line_break(
        printer,
        object_end,
        property_start,
        use_hardline,
    ));

    // When comments exist, build_chain_line_break skips blank line detection.
    // Check for blank lines before the first comment and after the last comment.
    let has_leading_comments =
        !classified.leading_block.is_empty() || !classified.leading_line.is_empty();

    // Blank line before first leading comment
    if use_hardline && has_leading_comments {
        let first_start = classified
            .leading_block
            .first()
            .map(|c| c.span.start)
            .into_iter()
            .chain(classified.leading_line.first().map(|c| c.span.start))
            .min();
        if let Some(start) = first_start
            && has_blank_line_between_strict(printer.get_source(), object_end, start)
        {
            parts.push(printer.arena().hardline());
        }
    }

    // Leading block comments (on their own line)
    parts.push(printer.build_leading_comments_doc(&classified.leading_block));
    // Leading line comments (on their own line)
    parts.push(printer.build_leading_comments_doc(&classified.leading_line));

    // Blank line after last leading comment (before property)
    if use_hardline && has_leading_comments {
        let last_end = classified
            .leading_line
            .last()
            .or_else(|| classified.leading_block.last())
            .map(|c| c.span.end);
        if let Some(end) = last_end
            && has_blank_line_between_strict(printer.get_source(), end, property_start)
        {
            parts.push(printer.arena().hardline());
        }
    }
}

/// Builder for constructing chain parts with proper comment handling.
///
/// Encapsulates the logic for interleaving comments, line breaks, and groups
/// when building the rest of a chain (everything after the first group).
pub(crate) struct ChainPartsBuilder<'a, P: ChainPrinter> {
    parts: DocBuf,
    printer: &'a P,
    use_hardline: bool,
    use_expanded: bool,
}

impl<'a, P: ChainPrinter> ChainPartsBuilder<'a, P> {
    pub(crate) fn new(
        printer: &'a P,
        use_hardline: bool,
        use_expanded: bool,
        group_count: usize,
    ) -> Self {
        Self {
            // Each group produces ~5 docs: trailing comments, line break, block comments,
            // leading comments, and the group doc itself
            parts: DocBuf::with_capacity(group_count * 5),
            printer,
            use_hardline,
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

    /// Add only trailing comments (no line break, no leading comments)
    /// Used when the next element should stay on the same line as the previous.
    ///
    /// Emits:
    /// 1. Trailing block comments (same line as previous element)
    /// 2. Trailing line comments (same line, via line_suffix)
    ///
    /// Skips leading comments and line breaks since we want the member to stay
    /// on the same line. Leading comments that appear on their own line before
    /// a trailing member are a complex case - Prettier moves them elsewhere
    /// (e.g., after `=`), which requires structural transformation beyond what
    /// this function handles.
    fn add_trailing_comments_only(&mut self, group: &ChainGroup<'_>) {
        if let Some((object_end, property_start)) = group.first_member_range() {
            let classified = self.printer.classify_comments(object_end, property_start);

            // Trailing block comments (same line as previous element)
            // e.g., `.map(x => x) /* comment */.length`
            self.parts.push(
                self.printer
                    .build_trailing_block_doc(&classified.trailing_block),
            );

            // Trailing line comments (same line as previous element)
            // e.g., `.map(x => x) // comment` - goes to end of line via line_suffix
            self.parts.push(
                self.printer
                    .build_trailing_line_doc(&classified.trailing_line),
            );

            // Note: Leading comments (on their own line before the member) are
            // intentionally not emitted here. They would need a line break, but
            // we're explicitly avoiding breaks to keep the member on the same line.
        }
    }

    /// Add trailing comments, line break, and leading comments before a group.
    /// Delegates to the shared [`push_gap_comments_and_break`] so this group path
    /// and the member-only breaking path render gap comments identically.
    fn add_comments_and_break(&mut self, group: &ChainGroup<'_>) {
        if let Some((object_end, property_start)) = group.first_member_range() {
            push_gap_comments_and_break(
                &mut self.parts,
                self.printer,
                object_end,
                property_start,
                self.use_hardline,
            );
        } else {
            // No member range - just add line break
            let d = self.printer.arena();
            self.parts.push(if self.use_hardline {
                d.hardline()
            } else {
                d.softline()
            });
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

    pub(crate) fn build(self) -> DocBuf {
        self.parts
    }
}

/// Build rest parts with comments and blank line preservation
/// Handles both trailing line comments (same line) and leading line comments (own line)
/// Emits: [trailing_comments?, line_break, leading_comments?, group] for each rest group
pub(crate) fn build_rest_parts_with_comments<'a, P: ChainPrinter>(
    rest_groups: &[ChainGroup<'a>],
    printer: &P,
    use_hardline: bool,
    use_expanded: bool,
) -> DocBuf {
    // Check if last group is a simple member (no calls) - it should stay on same line as `})`
    // e.g., `.filter().map({...})).length` - `.length` stays on same line as `})`
    let last_is_simple_member = rest_groups.last().is_some_and(|g| {
        g.nodes.len() == 1 && g.nodes.iter().all(|n| n.is_member() && !n.is_call())
    });

    // Check if last group has comments that force a line break.
    // Line comments (`// ...`) consume the rest of the line, so we can't emit them
    // and then print more code on the same line. Leading comments also need their
    // own line. Only trailing block comments can stay inline.
    let last_has_break_forcing_comments = last_is_simple_member
        && rest_groups.last().is_some_and(|g| {
            if let Some((object_end, property_start)) = g.first_member_range() {
                let classified = printer.classify_comments(object_end, property_start);
                // Any line comments or leading comments force a break
                !classified.trailing_line.is_empty()
                    || !classified.leading_block.is_empty()
                    || !classified.leading_line.is_empty()
            } else {
                false
            }
        });

    let mut builder =
        ChainPartsBuilder::new(printer, use_hardline, use_expanded, rest_groups.len());
    for (i, group) in rest_groups.iter().enumerate() {
        // Don't add hardline before last group if it's a simple member WITHOUT
        // comments that force a break
        let is_last = i == rest_groups.len() - 1;
        if is_last && last_is_simple_member && use_hardline && !last_has_break_forcing_comments {
            builder.add_group_no_break(group);
        } else {
            builder.add_group(group);
        }
    }
    builder.build()
}

/// Build an expanded chain doc with first group(s) inline and rest indented
///
/// Common pattern for expanded chains: first group(s) + hardline + indent(rest)
pub(super) fn build_expanded_chain_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    split_at: usize,
    printer: &P,
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
    let rest_parts = build_rest_parts_with_comments(rest, printer, true, false);

    d.concat(&[first_doc, d.indent(d.concat(&rest_parts))])
}

/// Build the expanded doc variant (first group(s) + indented rest)
pub(super) fn build_expanded_doc<'a, P: ChainPrinter>(
    groups: &[ChainGroup<'a>],
    should_merge: bool,
    printer: &P,
) -> DocId {
    let split_at = if should_merge { 2 } else { 1 };
    build_expanded_chain_doc(groups, split_at, printer)
}

/// Build first groups doc (merged when should_merge)
pub(super) fn build_first_groups_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    printer: &P,
) -> DocId {
    let d = printer.arena();
    let first_docs: DocBuf = first_groups
        .iter()
        .map(|g| print_group(g, printer))
        .collect();
    d.concat(&first_docs)
}

/// Build first groups doc with expanded calls
pub(super) fn build_first_groups_expanded_doc<'a, P: ChainPrinter>(
    first_groups: &[ChainGroup<'a>],
    printer: &P,
) -> DocId {
    let d = printer.arena();
    let first_docs: DocBuf = first_groups
        .iter()
        .map(|g| print_group_expanded(g, printer))
        .collect();
    d.concat(&first_docs)
}
