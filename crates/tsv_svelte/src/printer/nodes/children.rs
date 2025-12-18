// Child formatting for Svelte elements
//
// Handles formatting of element children in both multiline and compact modes,
// with support for blank line preservation and inline run grouping.

use super::super::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use tsv_lang::printing;

impl<'a> Printer<'a> {
    /// Format children in multiline mode with inline run grouping
    ///
    /// Groups consecutive inline content nodes that are on the same source line
    /// to preserve authorial intent and maintain semantic whitespace.
    pub fn print_multiline_children(&mut self, nodes: &[FragmentNode], preserves_ws: bool) {
        self.indent_level += 1;

        let mut i = 0;
        let mut had_blank_line = false;
        let mut has_output_content = false; // Track if we've output any elements yet
        let mut prev_block_inline = false; // Track if previous node was block formatted inline

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
            // ALWAYS add newline for text nodes, even after inline block
            // (the text's leading space will be preserved by the printer)
            if let FragmentNode::Text(_) = node {
                // Always add newline before text
                if had_blank_line {
                    self.write("\n\n");
                    had_blank_line = false;
                } else {
                    self.write("\n");
                }
                self.write_indent();
            } else if !prev_block_inline {
                // For non-text nodes, skip newline if previous was inline block
                if had_blank_line {
                    self.write("\n\n");
                    had_blank_line = false;
                } else {
                    self.write("\n");
                }
                self.write_indent();
            }
            has_output_content = true; // Mark that we've output content

            // Check if this is the start of an inline run
            if self.is_inline_node(node) {
                let run_end = self.find_inline_run_end(nodes, i);

                // Check if inline run is followed by a block element on the same source line
                // with semantic spacing (trailing space in last node before block)
                let mut next_block_inline = false;
                if run_end + 1 < nodes.len() {
                    let next_node = &nodes[run_end + 1];
                    if !self.is_inline_node(next_node) {
                        // It's a block element - check if on same line AND preceded by space
                        let run_span = self.get_content_span(&nodes[run_end]);
                        let next_span = self.get_content_span(next_node);
                        if printing::spans_on_same_line(self.source, run_span, next_span) {
                            // Check if last content in run ends with space
                            if let FragmentNode::Text(text) = &nodes[run_end] {
                                let trimmed = text.raw.trim_end();
                                if trimmed.len() < text.raw.len() {
                                    // Has trailing whitespace before block
                                    // Keep text+space+block on same line, ALWAYS
                                    next_block_inline = true;
                                }
                            }
                        }
                    }
                }

                // Count expression nodes in the run (not regular elements)
                let expr_count = nodes[i..=run_end]
                    .iter()
                    .filter(|n| matches!(n, FragmentNode::ExpressionTag(_)))
                    .count();

                // Check if expressions should be split to separate lines
                // Split ONLY if there's whitespace-only text BETWEEN expressions
                // (layout whitespace like `{a} {b}` should split to separate lines)
                // Keep together if:
                // - Semantic text between (e.g., `{'<'}div{'>'}`)
                // - Directly adjacent (e.g., `{'{'}{'}'}`)
                // - Only trailing whitespace after last expression (not between)
                let should_split_expressions =
                    expr_count > 1 && has_whitespace_between_expressions(&nodes[i..=run_end]);

                // Multiple expressions in multiline mode: format each on its own line
                // (Regular elements stay together, only expressions get broken)
                // BUT: only if no semantic text binds them together
                if expr_count > 1 && should_split_expressions {
                    let mut first = true;
                    for run_node in &nodes[i..=run_end] {
                        // Skip whitespace-only text nodes
                        if let FragmentNode::Text(text) = run_node
                            && text.raw.is_whitespace_only()
                        {
                            continue;
                        }

                        // Add newline before each node except the first
                        if !first {
                            self.write("\n");
                            self.write_indent();
                        }
                        first = false;

                        // Format the node
                        self.print_fragment_node(run_node, false, preserves_ws);
                    }
                } else {
                    // Single element or compact run: use normal inline formatting
                    self.print_inline_run(nodes, i, run_end, preserves_ws, next_block_inline);
                }

                // If block element follows on same line with space before it, format it inline
                if next_block_inline && run_end + 1 < nodes.len() {
                    self.print_fragment_node(&nodes[run_end + 1], false, preserves_ws);
                    prev_block_inline = true;
                    i = run_end + 2;
                } else {
                    // Check if the last node in the run ended with a blank line
                    // This sets up blank line for the NEXT iteration
                    if let FragmentNode::Text(text) = &nodes[run_end]
                        && text.raw.has_trailing_blank_line()
                    {
                        had_blank_line = true;
                    }
                    prev_block_inline = false;
                    i = run_end + 1;
                }
            } else {
                // Block element: format on its own line
                self.print_fragment_node(node, true, preserves_ws);
                prev_block_inline = false;
                i += 1;
            }
        }

        self.indent_level -= 1;
        self.write("\n");
        self.write_indent();
    }

    /// Format children in compact (single-line) mode
    pub fn print_compact_children(
        &mut self,
        fragment: &internal::Fragment,
        is_block: bool,
        preserves_ws: bool,
    ) {
        // Determine whether to treat children as inline or block context:
        // - Block element with ONLY text → block context (trim completely)
        // - Block element with expressions/inline elements → inline context (preserve spacing between them)
        // - Inline elements → always inline context
        let only_text = fragment
            .nodes
            .iter()
            .all(|node| matches!(node, FragmentNode::Text(_)));
        let has_inline_or_expr = fragment.nodes.iter().any(|node| match node {
            FragmentNode::Element(el) => self.is_inline_element(el),
            FragmentNode::ExpressionTag(_) => true,
            FragmentNode::Comment(_) => true, // Comments are inline-like for spacing
            _ => false,
        });

        // Treat as inline context if: has inline elements/expressions OR parent is inline
        // BUT: if only text nodes, use block context to trim completely
        let treat_as_inline = (has_inline_or_expr || !is_block) && !only_text;
        let child_parent_is_block = is_block && !treat_as_inline;

        // In block context with expressions/inline elements, skip leading/trailing whitespace-only text nodes
        // This handles cases like: <Comp>  {expr}  </Comp> → <Comp>{expr}</Comp>
        let should_skip_ws_boundaries = is_block && has_inline_or_expr && !only_text;

        if should_skip_ws_boundaries {
            // Find content boundaries (first/last non-whitespace-only nodes)
            let (first_content_idx, last_content_idx) =
                find_content_boundary_indices(&fragment.nodes);

            // Print nodes within content boundaries, with special handling for boundary text
            if let (Some(first), Some(last)) = (first_content_idx, last_content_idx) {
                for (i, node) in fragment.nodes.iter().enumerate() {
                    // Skip nodes outside content boundaries
                    if i < first || i > last {
                        continue;
                    }

                    // Special handling for boundary text nodes to trim leading/trailing whitespace
                    if let FragmentNode::Text(text) = node {
                        let is_first = i == first;
                        let is_last = i == last;

                        if is_first || is_last {
                            // Normalize whitespace, then trim boundaries as needed
                            let normalized = self.normalize_whitespace(&text.raw, false);
                            let trimmed = normalized.trim();

                            // Write with appropriate boundary handling
                            if is_first && is_last {
                                self.write(trimmed);
                            } else if is_first {
                                // Trim start, preserve trailing
                                let trimmed_start = normalized.trim_start();
                                self.write(trimmed_start);
                            } else {
                                // Preserve leading, trim end
                                let trimmed_end = normalized.trim_end();
                                self.write(trimmed_end);
                            }
                            continue;
                        }
                    }

                    self.print_fragment_node(node, child_parent_is_block, preserves_ws);
                }
            }
        } else {
            // Normal mode: print all nodes
            for node in &fragment.nodes {
                self.print_fragment_node(node, child_parent_is_block, preserves_ws);
            }
        }
    }

    /// Format element in "hug mode" (Prettier's `><` pattern)
    ///
    /// Used for inline elements and components with block children.
    /// Pattern: <Tag\n\t><child1>\n\t<child2></Tag\n>
    ///
    /// Example:
    /// ```svelte
    /// <Comp
    ///   ><div>a</div>
    ///   <div>b</div></Comp
    /// >
    /// ```
    pub fn print_hug_mode_element(
        &mut self,
        tag_name: &str,
        nodes: &[FragmentNode],
        preserves_ws: bool,
        _hug_start: bool,
        _hug_end: bool,
        attrs_multiline: bool,
    ) {
        // This function now ONLY handles the hug_both case (hugStart && hugEnd)
        // Pattern depends on whether attributes wrapped:
        // - With wrapped attrs: ><content</Tag\n> (tight opening, prettier lines 1180-1189)
        // - Without attrs: \n\t><content</Tag\n> (indented opening)
        //
        // Other hug combinations are handled in separate code paths in element.rs (lines 104-167)
        // We tried unifying all 4 cases here but it led to bugs - keep them separate!
        // TODO: Consider renaming to `print_hug_both_element()` to make this clearer

        // Opening: conditionally add newline+indent before >
        // When attrs wrap, they already added the newline/indent, so > comes immediately after
        // When no attrs (or attrs didn't wrap), we need to add newline+indent
        if !attrs_multiline {
            self.write("\n");
        }

        // Increment indent for children (and for > in no-attrs case)
        self.indent_level += 1;

        if !attrs_multiline {
            self.write_indent();
        }
        self.write(">");

        // Print all children with proper indentation
        for (i, node) in nodes.iter().enumerate() {
            // Skip whitespace-only text nodes
            if let FragmentNode::Text(text) = node
                && text.raw.is_whitespace_only()
            {
                continue;
            }

            // Print the node (inline, not multiline)
            self.print_fragment_node(node, false, preserves_ws);

            // Add newline and indent before next child (if not last)
            if i < nodes.len() - 1 {
                // Check if next node is not whitespace-only
                let has_next_content = nodes[i + 1..]
                    .iter()
                    .any(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()));

                if has_next_content {
                    self.write("\n");
                    self.write_indent();
                }
            }
        }

        // Closing: </Tag\n> (split closing, always for hug_both case)
        self.write("</");
        self.write(tag_name);
        self.indent_level -= 1;
        self.write("\n");
        self.write(">");
    }
}

/// Find the first and last indices of non-whitespace-only content nodes.
///
/// Returns (first_content_idx, last_content_idx) where:
/// - Text nodes with only whitespace are skipped
/// - Non-text nodes (elements, expressions, comments) always count as content
/// - Text nodes with non-whitespace content count as content
fn find_content_boundary_indices(nodes: &[FragmentNode]) -> (Option<usize>, Option<usize>) {
    let is_content = |node: &FragmentNode| {
        if let FragmentNode::Text(text) = node {
            text.raw.has_content() // Uses TextAnalysis trait
        } else {
            true // Non-text nodes are always content
        }
    };

    let first = nodes.iter().position(is_content);
    let last = nodes.iter().rposition(is_content);

    (first, last)
}

/// Check if there's whitespace-only text BETWEEN expressions in a run.
///
/// This is used to decide whether to split expressions to separate lines in multiline mode.
/// Layout whitespace like `{a} {b}` should split, but semantic patterns like `{'<'}div{'>'}`
/// should stay together because the text between expressions is meaningful content.
///
/// Only checks whitespace between the first and last expression - trailing whitespace
/// after the last expression doesn't count as "between".
fn has_whitespace_between_expressions(nodes: &[FragmentNode]) -> bool {
    let first_expr = nodes
        .iter()
        .position(|n| matches!(n, FragmentNode::ExpressionTag(_)));
    let last_expr = nodes
        .iter()
        .rposition(|n| matches!(n, FragmentNode::ExpressionTag(_)));

    match (first_expr, last_expr) {
        (Some(first), Some(last)) if first < last => {
            // Check only nodes between first and last expression
            nodes[first..=last]
                .iter()
                .any(|n| matches!(n, FragmentNode::Text(text) if text.raw.is_whitespace_only()))
        }
        _ => false,
    }
}
