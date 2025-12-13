// Fragment content classification for Svelte formatting
//
// Provides utilities to query content properties of fragments and analyze
// composition of template nodes.
//
// Note: Unlike element/whitespace classification, these methods operate on
// AST structures (Fragment, FragmentNode) and depend on printer-specific
// traits (TextAnalysis). They remain printer-specific for now and would
// be extracted to a shared module if/when other tools need similar AST queries

use super::super::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use tsv_html as html;
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Check if a fragment node is inline content
    ///
    /// Returns true for:
    /// - Inline elements (span, a, strong, etc.)
    /// - Inline void elements (br, img, input, etc.)
    /// - Components (all components treated as inline to preserve whitespace)
    /// - Expression tags
    /// - Text nodes (any text content)
    ///
    /// Returns false for:
    /// - Block elements (div, p, section, etc.)
    /// - Block void elements (hr)
    pub(crate) fn is_inline_node(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::Element(el) => {
                // Components are always treated as inline to preserve surrounding whitespace
                // This matches prettier's behavior where components in block parents
                // (e.g., `<p>text <Comp /> more</p>`) maintain their spacing
                use crate::ast::internal::ElementKind;
                if el.kind == ElementKind::Component {
                    return true;
                }

                // HTML elements: resolve symbol once for all checks
                self.with_resolved_symbol(el.name, |tag| {
                    // Element is inline if:
                    // 1. It's classified as inline (phrasing content), OR
                    // 2. It's void AND not block (inline void like <br>, but not <hr>)
                    html::is_inline_element(tag)
                        || (html::is_void_element(tag) && !html::is_block_element(tag))
                })
            }
            FragmentNode::ExpressionTag(_) => true,
            FragmentNode::Text(_) => true,
            // Comments are treated as inline (similar to text)
            FragmentNode::Comment(_) => true,
            // Template tags are inline (like expression tags)
            FragmentNode::HtmlTag(_)
            | FragmentNode::ConstTag(_)
            | FragmentNode::DebugTag(_)
            | FragmentNode::RenderTag(_) => true,
            // Control flow blocks are treated as block elements
            FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_)
            | FragmentNode::SnippetBlock(_) => false,
            // Special elements: most are block-level, a few are inline
            FragmentNode::SpecialElement(el) => {
                use crate::ast::internal::SpecialElementKind;
                matches!(
                    el.kind,
                    SpecialElementKind::SlotElement
                        | SpecialElementKind::SvelteFragment
                        | SpecialElementKind::SvelteComponent { .. }
                        | SpecialElementKind::SvelteSelf
                )
            }
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
