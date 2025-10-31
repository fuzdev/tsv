// Fragment content classification for Svelte formatting
//
// Provides utilities to query content properties of fragments and analyze
// composition of template nodes.
//
// Note: Unlike element/whitespace classification, these methods operate on
// AST structures (Fragment, FragmentNode) and depend on formatter-specific
// traits (TextAnalysis). They remain formatter-specific for now and would
// be extracted to a shared module if/when other tools need similar AST queries

use super::super::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use crate::formatter::Formatter;
use crate::language::html;

impl Formatter {
    /// Check if a fragment node is inline content
    ///
    /// Returns true for:
    /// - Inline/void elements (span, br, etc.)
    /// - Expression tags
    /// - Text nodes (any text content)
    pub(crate) fn is_inline_node(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::Element(el) => {
                // Optimize: resolve symbol once for both checks
                self.with_resolved_symbol(el.name, |tag| {
                    html::is_inline_element(tag) || html::is_void_element(tag)
                })
            }
            FragmentNode::ExpressionTag(_) => true,
            FragmentNode::Text(_) => true,
        }
    }

    /// Check if fragment contains any non-whitespace text
    pub(crate) fn has_text_content(&self, fragment: &internal::Fragment) -> bool {
        fragment
            .nodes
            .iter()
            .any(|node| matches!(node, FragmentNode::Text(text) if text.raw.has_content()))
    }

    /// Check if fragment contains any block elements
    pub(crate) fn has_block_elements(&self, fragment: &internal::Fragment) -> bool {
        fragment
            .nodes
            .iter()
            .any(|node| matches!(node, FragmentNode::Element(el) if self.is_block_element(el)))
    }
}
