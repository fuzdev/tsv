// Element-specific formatting for Svelte templates
//
// Hug-mode helpers shared by the element/special-element doc builders.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;

impl<'a> Printer<'a> {
    //
    // Hug mode helpers (used by element_doc.rs and special_doc.rs)
    //

    /// Check if element should hug the start
    ///
    /// Matches Prettier's shouldHugStart: returns false (don't hug) if:
    /// - Element is a block element
    /// - First child is text starting with collapsible (ASCII) whitespace
    ///
    /// Non-breaking spaces (U+00A0 / U+202F) are content, not collapsible
    /// whitespace, so a leading nbsp still hugs (matching prettier-plugin-svelte's
    /// `STARTS_WITH_HTML_COLLAPSE_WHITESPACE_RE = /^[\t\n\f\r ]/`).
    pub(crate) fn should_hug_start(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block {
            return false;
        }
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match &element.fragment.nodes[0] {
            FragmentNode::Text(text) => !text
                .raw(self.source)
                .starts_with(|c: char| c.is_ascii_whitespace()),
            _ => true,
        }
    }

    /// Check if element should hug the end
    ///
    /// Matches Prettier's shouldHugEnd: returns false (don't hug) if:
    /// - Element is a block element
    /// - Last child is text ending with collapsible (ASCII) whitespace
    ///
    /// Non-breaking spaces are content, so a trailing nbsp still hugs (matching
    /// `ENDS_WITH_HTML_COLLAPSE_WHITESPACE_RE = /[\t\n\f\r ]$/`).
    pub(crate) fn should_hug_end(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block {
            return false;
        }
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match element.fragment.nodes.last() {
            Some(FragmentNode::Text(text)) => !text
                .raw(self.source)
                .ends_with(|c: char| c.is_ascii_whitespace()),
            _ => true,
        }
    }
}
