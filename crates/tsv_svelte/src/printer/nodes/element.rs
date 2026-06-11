// Element-specific formatting for Svelte templates
//
// Public entry points that delegate to doc builders in element_doc.rs and
// special_doc.rs.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Format a Svelte element with context-aware formatting
    ///
    /// Doc-based path for all elements:
    /// - style/script: handled by build_raw_content_element_doc()
    /// - pre/textarea: handled by build_whitespace_sensitive_element_doc()
    /// - all others: handled by build_element_doc()
    pub(crate) fn print_element(&mut self, element: &internal::Element) {
        let doc = self.build_element_doc(element);
        self.render_doc_immediate(doc);
    }

    /// Format a Svelte special element
    ///
    /// Doc-based path for all special elements (svelte:*, slot, title).
    pub(crate) fn print_special_element(&mut self, element: &internal::SpecialElement) {
        let doc = self.build_special_element_doc(element);
        self.render_doc_immediate(doc);
    }

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
            FragmentNode::Text(text) => !text.raw.starts_with(|c: char| c.is_ascii_whitespace()),
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
            Some(FragmentNode::Text(text)) => {
                !text.raw.ends_with(|c: char| c.is_ascii_whitespace())
            }
            _ => true,
        }
    }
}
