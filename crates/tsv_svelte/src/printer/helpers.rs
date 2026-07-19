//! Fragment analysis and child printing helpers

use super::Printer;
use crate::ast::internal::{Fragment, FragmentNode};

impl<'a> Printer<'a> {
    /// Check if a fragment's content is inline (huggable at both ends).
    ///
    /// Returns true when neither boundary is newline-authored, allowing content to be
    /// hugged to control flow tags like `{#if cond}<Comp/>{/if}`. A space-only boundary
    /// run does not block hugging — it is render-free (trimmed at compile) and the
    /// formatter trims it, so a space-authored body must reach the same layout as the
    /// glued authoring. Content that is itself multiline (a component with wrapping
    /// attrs) still counts as inline — only the boundary run speaks for the boundary.
    pub(super) fn is_inline_fragment(&self, fragment: &Fragment<'_>) -> bool {
        !self.fragment_boundary_newline(fragment, true)
            && !self.fragment_boundary_newline(fragment, false)
    }

    /// Whether the fragment's boundary whitespace run is newline-authored — the node-slice
    /// predicate [`Printer::nodes_boundary_newline`] over the fragment's nodes.
    pub(super) fn fragment_boundary_newline(
        &self,
        fragment: &Fragment<'_>,
        is_leading: bool,
    ) -> bool {
        self.nodes_boundary_newline(fragment.nodes, is_leading)
    }

    /// Whether the boundary whitespace **run** at one edge of `nodes` (the leading run of
    /// the first text node / trailing run of the last) contains a newline — a
    /// **newline-authored** boundary, which keeps its layout meaning (the construct stays
    /// multiline).
    ///
    /// A space/tab-only run does NOT count: it is render-free (the compiler trims every
    /// fragment edge at compile — `clean_nodes`), so it neither survives inline nor
    /// selects the layout. Interior newlines don't count either — they are fill
    /// separators, not boundary authoring (`{#if c}x\ny{/if}` fills; only the boundary
    /// run speaks for the boundary). ASCII whitespace only: an NBSP is content.
    /// The single boundary-authoring question — the element boundary probes, the block
    /// section paths, and `is_inline_fragment` all route through it.
    /// See conformance_prettier.md §Svelte: Blocks.
    pub(super) fn nodes_boundary_newline(
        &self,
        nodes: &[FragmentNode<'_>],
        is_leading: bool,
    ) -> bool {
        let node = if is_leading {
            nodes.first()
        } else {
            nodes.last()
        };
        let Some(FragmentNode::Text(text)) = node else {
            return false;
        };
        let raw = text.raw(self.source);
        let run = if is_leading {
            &raw[..raw.len()
                - raw
                    .trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .len()]
        } else {
            &raw[raw
                .trim_end_matches(|c: char| c.is_ascii_whitespace())
                .len()..]
        };
        run.contains('\n')
    }
}
