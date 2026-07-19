// Element-specific formatting for Svelte templates
//
// The content-boundary whitespace probes behind `ElementContext::hug_both`.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Whether the content renders glued to the opening tag — no **newline-authored**
    /// boundary at the start of the fragment.
    ///
    /// A space/tab-only boundary run still hugs: it is render-free (the compiler trims
    /// every fragment edge at compile) and the formatter trims it, so the spaced
    /// authoring must reach the same layout as the glued one — including the sibling-`>`
    /// dangle, which keys on the element's hug-both layout. Only a newline in the run is
    /// boundary authoring (see `Printer::nodes_boundary_newline`, the single
    /// boundary-authoring question). Non-breaking spaces are content, not a boundary run
    /// (matching prettier-plugin-svelte's `STARTS_WITH_HTML_COLLAPSE_WHITESPACE_RE`).
    ///
    /// This is a *whitespace probe*, not a layout decision: it says what the author wrote,
    /// and only [`Printer::compute_element_layout`] decides what that means. Hugging is
    /// all-or-nothing there — see
    /// [`ElementContext::hug_both`](super::element_doc::ElementContext).
    pub(crate) fn should_hug_start(&self, nodes: &[FragmentNode<'_>], is_block: bool) -> bool {
        if is_block {
            return false;
        }
        !self.nodes_boundary_newline(nodes, true)
    }

    /// Whether the content renders glued to the closing tag — no newline-authored
    /// boundary at the end of the fragment. Mirror of [`Self::should_hug_start`].
    pub(crate) fn should_hug_end(&self, nodes: &[FragmentNode<'_>], is_block: bool) -> bool {
        if is_block {
            return false;
        }
        !self.nodes_boundary_newline(nodes, false)
    }
}
