//! Fragment analysis and child printing helpers

use super::Printer;
use crate::ast::internal::{Fragment, FragmentNode, is_collapsible_ws, text_edge_ws};

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
    /// run speaks for the boundary). Collapsible whitespace only: an NBSP or form feed is content.
    /// The single boundary-authoring question — the element boundary probes, the block
    /// section paths, and `is_inline_fragment` all route through it.
    /// See conformance_prettier_svelte.md §Svelte: Blocks.
    ///
    /// Answered from the text's precomputed scalars wherever they settle it, which is most
    /// edges: `newline_count` counts the whole raw, so a text with no newline has none at its
    /// edge either, and a whitespace-only text is all edge, so any newline it has is there.
    /// Only a content text holding a newline walks its edge run ([`edge_run_holds_newline`]).
    #[inline]
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
        let holds = if text.newline_count == 0 || text.is_collapsible_ws_only {
            text.newline_count != 0
        } else {
            edge_run_holds_newline(&self.source.as_bytes()[text.raw_span.range()], is_leading)
        };
        debug_assert_eq!(
            holds,
            text_edge_ws(text.raw(self.source), is_leading).contains('\n')
        );
        holds
    }
}

/// Whether the leading (else trailing) [`is_collapsible_ws`] run of `raw` holds a `\n` —
/// [`text_edge_ws`]'s run, asked as it is walked: the walk stops at the first `\n` or at the
/// first content byte, whichever comes first.
///
/// Out of line: only a content text holding a newline reaches it
/// ([`Printer::nodes_boundary_newline`] settles every other edge from the text's scalars).
#[inline(never)]
fn edge_run_holds_newline(raw: &[u8], leading: bool) -> bool {
    let first_stop = |b: &u8| *b == b'\n' || !is_collapsible_ws(*b);
    let stop = if leading {
        raw.iter().find(|b| first_stop(b))
    } else {
        raw.iter().rev().find(|b| first_stop(b))
    };
    stop == Some(&b'\n')
}
