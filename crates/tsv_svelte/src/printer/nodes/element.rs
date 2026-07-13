// Element-specific formatting for Svelte templates
//
// The content-boundary whitespace probes behind `ElementContext::hug_both`.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Whether the content touches the opening tag — no collapsible whitespace at the start
    /// of the fragment.
    ///
    /// Matches Prettier's `shouldHugStart`. Non-breaking spaces (U+00A0 / U+202F) are content,
    /// not collapsible whitespace, so a leading nbsp still hugs (matching prettier-plugin-svelte's
    /// `STARTS_WITH_HTML_COLLAPSE_WHITESPACE_RE = /^[\t\n\f\r ]/`).
    ///
    /// This is a *whitespace probe*, not a layout decision: it says what the author wrote, and
    /// only [`Printer::compute_element_layout`] decides what that means. Hugging is all-or-nothing
    /// there — see [`ElementContext::hug_both`](super::element_doc::ElementContext).
    pub(crate) fn should_hug_start(&self, nodes: &[FragmentNode<'_>], is_block: bool) -> bool {
        if is_block {
            return false;
        }
        match nodes.first() {
            Some(FragmentNode::Text(text)) => !text
                .raw(self.source)
                .starts_with(|c: char| c.is_ascii_whitespace()),
            _ => true,
        }
    }

    /// Whether the content touches the closing tag — no collapsible whitespace at the end of
    /// the fragment. Mirror of [`Self::should_hug_start`] (Prettier's `shouldHugEnd`,
    /// `ENDS_WITH_HTML_COLLAPSE_WHITESPACE_RE = /[\t\n\f\r ]$/`).
    pub(crate) fn should_hug_end(&self, nodes: &[FragmentNode<'_>], is_block: bool) -> bool {
        if is_block {
            return false;
        }
        match nodes.last() {
            Some(FragmentNode::Text(text)) => !text
                .raw(self.source)
                .ends_with(|c: char| c.is_ascii_whitespace()),
            _ => true,
        }
    }
}
