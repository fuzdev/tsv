// Inline run grouping for Svelte template formatting
//
// Identifies and formats sequences of inline content that appear on the same
// source line, preserving authorial layout intent while normalizing whitespace.

use super::super::text::TextAnalysis;
use crate::ast::internal::FragmentNode;
use crate::printer::Printer;
use tsv_lang::printing;

impl<'a> Printer<'a> {
    /// Find the end index of an inline run starting at `start_idx`
    ///
    /// An inline run is a sequence of consecutive inline content nodes
    /// (text, inline elements, void elements, expressions) that are on the
    /// same source line. Whitespace-only text nodes are skipped but included in the run.
    ///
    /// Returns the index of the last node in the run.
    pub fn find_inline_run_end(&self, nodes: &[FragmentNode], start_idx: usize) -> usize {
        let mut run_end = start_idx;
        // Track the span of the last non-whitespace node to compare adjacency
        let mut last_content_span = self.get_content_span(&nodes[start_idx]);

        while run_end + 1 < nodes.len() {
            let next_node = &nodes[run_end + 1];

            // Skip whitespace-only text nodes when looking ahead
            // BUT: Stop run if whitespace contains blank line (2+ newlines)
            if let FragmentNode::Text(text) = next_node
                && text.raw.is_whitespace_only()
            {
                if text.raw.has_blank_line() {
                    break; // Blank line ends the run
                }
                run_end += 1;
                continue;
            }

            // Check if next node should end the run (compare to last content node, not first)
            if self.should_end_inline_run(next_node, last_content_span) {
                break;
            }

            // Update the last content span as we extend the run
            last_content_span = self.get_content_span(next_node);
            run_end += 1;

            // If text node has trailing blank line, end run here
            // (blank line separates this content from what follows)
            if let FragmentNode::Text(text) = next_node
                && text.raw.has_trailing_blank_line()
            {
                break;
            }
        }

        run_end
    }

    /// Check if a node should end an inline run
    ///
    /// Inline runs terminate when:
    /// - Next node is a block element (break in flow)
    /// - Next node is on a different source line from the previous content node
    ///
    /// The `prev_content_span` should be the span of the last non-whitespace node
    /// in the current run, not the first node. This ensures multiline expressions
    /// that are adjacent in the source stay grouped together.
    pub fn should_end_inline_run(
        &self,
        node: &FragmentNode,
        prev_content_span: tsv_lang::Span,
    ) -> bool {
        // Block element ends the run
        if !self.is_inline_node(node) {
            return true;
        }

        // Different source line ends the run (preserves source layout)
        let node_span = self.get_content_span(node);
        !printing::spans_on_same_line(self.source, prev_content_span, node_span)
    }

    /// Format an inline run (consecutive inline content on same source line)
    ///
    /// Trims layout whitespace from first and last nodes in the run to avoid
    /// inserting unwanted spaces from indentation.
    ///
    /// If `block_follows_same_line` is true, preserves trailing space in the last text node
    /// (semantic spacing before the following block element).
    pub fn print_inline_run(
        &mut self,
        nodes: &[FragmentNode],
        start: usize,
        end: usize,
        preserves_ws: bool,
        block_follows_same_line: bool,
    ) {
        // Find first and last non-whitespace nodes in the run
        let (first_content_idx, last_content_idx) = self.find_content_boundaries(nodes, start, end);

        // Format nodes in the run
        for (idx, run_node) in nodes[start..=end].iter().enumerate() {
            let j = start + idx;
            if let FragmentNode::Text(text) = run_node {
                // Skip leading/trailing whitespace-only nodes, but preserve ones between content
                // (they're semantic spacing, e.g., `{a} {b}` needs the space preserved)
                if text.raw.is_whitespace_only() {
                    let is_between_content = first_content_idx.is_some_and(|first| j > first)
                        && last_content_idx.is_some_and(|last| j < last);

                    if !is_between_content {
                        continue; // Skip leading/trailing whitespace
                    }
                    // Fall through to preserve semantic spacing between content
                }

                let is_first = Some(j) == first_content_idx;
                let is_last = Some(j) == last_content_idx;

                // Handle trimming with context-aware rules for semantic spacing
                let trimmed_text = if is_last && block_follows_same_line {
                    // Last node before block on same line: preserve trailing space, trim leading
                    if is_first {
                        text.raw.trim_start().to_string()
                    } else {
                        text.raw.to_string()
                    }
                } else {
                    // Normal trimming
                    self.trim_text_for_run_position(&text.raw, is_first, is_last)
                };

                // Normalize the trimmed text
                let normalized = self.normalize_whitespace(&trimmed_text, false);
                self.write(&normalized);
            } else {
                // Format inline elements/expressions
                self.print_fragment_node(run_node, false, preserves_ws);
            }
        }
    }

    /// Find the indices of the first and last non-whitespace-only nodes in a range
    ///
    /// Used to determine which text nodes need trimming for inline run boundaries.
    /// Returns `(first_index, last_index)` or `(None, None)` if no content found.
    pub fn find_content_boundaries(
        &self,
        nodes: &[FragmentNode],
        start: usize,
        end: usize,
    ) -> (Option<usize>, Option<usize>) {
        let mut first = None;
        let mut last = None;

        for (idx, node) in nodes[start..=end].iter().enumerate() {
            let j = start + idx;

            // Skip whitespace-only text nodes
            if let FragmentNode::Text(text) = node
                && text.raw.is_whitespace_only()
            {
                continue;
            }

            if first.is_none() {
                first = Some(j);
            }
            last = Some(j);
        }

        (first, last)
    }

    /// Trim text node based on its position in an inline run
    ///
    /// - First node: trim leading whitespace (layout indentation)
    /// - Last node: trim trailing whitespace (layout separation)
    /// - First and last: trim both sides
    /// - Middle nodes: leave as-is (may contain intentional spaces)
    pub fn trim_text_for_run_position(&self, text: &str, is_first: bool, is_last: bool) -> String {
        match (is_first, is_last) {
            (true, true) => text.trim().to_string(),
            (true, false) => text.trim_start().to_string(),
            (false, true) => text.trim_end().to_string(),
            (false, false) => text.to_string(),
        }
    }
}
