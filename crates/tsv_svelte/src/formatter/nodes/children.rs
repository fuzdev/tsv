// Child formatting for Svelte elements
//
// Handles formatting of element children in both multiline and compact modes,
// with support for blank line preservation and inline run grouping.

use super::super::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use crate::formatter_core::Formatter;

impl Formatter {
    /// Format children in multiline mode with inline run grouping
    ///
    /// Groups consecutive inline content nodes that are on the same source line
    /// to preserve authorial intent and maintain semantic whitespace.
    pub fn format_multiline_children(&mut self, nodes: &[FragmentNode], preserves_ws: bool) {
        self.indent_level += 1;

        let mut i = 0;
        let mut had_blank_line = false;
        let mut has_output_content = false; // Track if we've output any elements yet

        while i < nodes.len() {
            let node = &nodes[i];

            // Skip whitespace-only text nodes in block context
            // But check for blank lines (2+ newlines) to preserve authorial intent
            if let FragmentNode::Text(text) = node
                && text.raw.is_whitespace_only()
            {
                // Only preserve blank lines BETWEEN elements, not before first element
                if has_output_content && text.raw.has_blank_line() {
                    had_blank_line = true;
                }
                i += 1;
                continue;
            }

            // Check for blank line at the START of non-whitespace text nodes
            // (after layout whitespace, before content)
            if let FragmentNode::Text(text) = node
                && has_output_content
            {
                // Only between elements
                let after_layout_ws = text.raw.trim_start();
                let leading_part = &text.raw[..text.raw.len() - after_layout_ws.len()];
                if leading_part.has_blank_line() {
                    had_blank_line = true;
                }
            }

            // Add newline before node (extra newline if there was a blank line)
            if had_blank_line {
                self.write("\n\n");
                had_blank_line = false;
            } else {
                self.write("\n");
            }
            self.write_indent();
            has_output_content = true; // Mark that we've output content

            // Check if this is the start of an inline run
            if self.is_inline_node(node) {
                let run_end = self.find_inline_run_end(nodes, i);
                self.format_inline_run(nodes, i, run_end, preserves_ws);

                // Check if the last node in the run ended with a blank line
                // This sets up blank line for the NEXT iteration
                if let FragmentNode::Text(text) = &nodes[run_end] {
                    let after_layout_ws = text.raw.trim_end();
                    let trailing_part = &text.raw[after_layout_ws.len()..];
                    if trailing_part.has_blank_line() {
                        had_blank_line = true;
                    }
                }

                i = run_end + 1;
            } else {
                // Block element: format on its own line
                self.format_fragment_node(node, true, preserves_ws);
                i += 1;
            }
        }

        self.indent_level -= 1;
        self.write("\n");
        self.write_indent();
    }

    /// Format children in compact (single-line) mode
    pub fn format_compact_children(
        &mut self,
        fragment: &internal::Fragment,
        is_block: bool,
        preserves_ws: bool,
    ) {
        // Determine whether to treat children as inline or block context:
        // - Block element with ONLY text → block context (trim completely)
        // - Block element with text + elements/expressions → inline context (preserve spacing)
        // - Inline elements → always inline context
        let only_text = fragment
            .nodes
            .iter()
            .all(|node| matches!(node, FragmentNode::Text(_)));
        let has_inline_or_expr = fragment.nodes.iter().any(|node| match node {
            FragmentNode::Element(el) => self.is_inline_element(el),
            FragmentNode::ExpressionTag(_) => true,
            _ => false,
        });

        // Treat as inline context if: has inline elements/expressions OR parent is inline
        // BUT: if only text nodes, use block context to trim completely
        let treat_as_inline = (has_inline_or_expr || !is_block) && !only_text;
        let child_parent_is_block = is_block && !treat_as_inline;

        for node in &fragment.nodes {
            self.format_fragment_node(node, child_parent_is_block, preserves_ws);
        }
    }
}
