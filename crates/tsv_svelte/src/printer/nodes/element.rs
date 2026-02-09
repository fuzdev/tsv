// Element-specific formatting for Svelte templates
//
// Public entry points that delegate to doc builders in fragment_doc.rs.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Format a Svelte element with context-aware formatting
    ///
    /// Doc-based path for all elements:
    /// - style/script: handled by build_raw_content_element_doc()
    /// - pre/textarea: handled by build_whitespace_sensitive_element_doc()
    /// - all others: handled by build_element_doc()
    pub fn print_element(&mut self, element: &internal::Element) {
        let doc = self.build_element_doc(element);
        self.render_doc_immediate(doc);
    }

    /// Format a Svelte special element
    ///
    /// Doc-based path for all special elements (svelte:*, slot, title).
    pub fn print_special_element(&mut self, element: &internal::SpecialElement) {
        let doc = self.build_special_element_doc(element);
        self.render_doc_immediate(doc);
    }

    //
    // Hug mode helpers (used by fragment_doc.rs)
    //

    /// Check if element should hug the start
    ///
    /// Matches Prettier's shouldHugStart: returns false (don't hug) if:
    /// - Element is a block element
    /// - First child is text starting with ANY whitespace (space, tab, newline)
    pub(crate) fn should_hug_start(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block {
            return false;
        }
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match &element.fragment.nodes[0] {
            FragmentNode::Text(text) => {
                // Match Prettier: don't hug if text starts with ANY whitespace
                !text.raw.starts_with(char::is_whitespace)
            }
            _ => true,
        }
    }

    /// Check if element should hug the end
    ///
    /// Matches Prettier's shouldHugEnd: returns false (don't hug) if:
    /// - Element is a block element
    /// - Last child is text ending with ANY whitespace (space, tab, newline)
    pub(crate) fn should_hug_end(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block {
            return false;
        }
        if element.fragment.nodes.is_empty() {
            return true;
        }
        match element.fragment.nodes.last() {
            Some(FragmentNode::Text(text)) => {
                // Match Prettier: don't hug if text ends with ANY whitespace
                !text.raw.ends_with(char::is_whitespace)
            }
            _ => true,
        }
    }
}
