// Doc-based formatting for inline fragment content
//
// Builds Doc IR trees for fragment nodes, enabling proper fits() checks
// that account for siblings. This matches Prettier's architecture where
// the entire inline content is represented as a single doc tree.
//
// Used by `print_inline_children()` to format inline content with correct
// attribute wrapping decisions that consider what comes after each element.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use std::rc::Rc;

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

/// Position of a text node relative to its siblings.
///
/// Encodes both position (first/last/middle/only) and whether adjacent
/// siblings are inline content, which affects whitespace handling.
enum SiblingPosition {
    /// Only child (first AND last) - no siblings
    Only,
    /// First child with info about next sibling
    First { next_is_inline: bool },
    /// Last child with info about previous sibling
    Last { prev_is_inline: bool },
    /// Middle child with info about both neighbors
    Middle {
        prev_is_inline: bool,
        next_is_inline: bool,
    },
}

impl SiblingPosition {
    fn new(is_first: bool, is_last: bool, prev_is_inline: bool, next_is_inline: bool) -> Self {
        match (is_first, is_last) {
            (true, true) => Self::Only,
            (true, false) => Self::First { next_is_inline },
            (false, true) => Self::Last { prev_is_inline },
            (false, false) => Self::Middle {
                prev_is_inline,
                next_is_inline,
            },
        }
    }

    fn is_first(&self) -> bool {
        matches!(self, Self::Only | Self::First { .. })
    }

    fn is_last(&self) -> bool {
        matches!(self, Self::Only | Self::Last { .. })
    }

    fn prev_is_inline(&self) -> bool {
        match self {
            Self::Last { prev_is_inline } | Self::Middle { prev_is_inline, .. } => *prev_is_inline,
            _ => false,
        }
    }

    fn next_is_inline(&self) -> bool {
        match self {
            Self::First { next_is_inline } | Self::Middle { next_is_inline, .. } => *next_is_inline,
            _ => false,
        }
    }
}

/// Helper to wrap body content in indent(), with optional hardline for leading whitespace.
///
/// This pattern is used consistently across all block types (if, each, await, snippet, key)
/// to ensure proper indentation of nested content when it breaks across lines.
fn indent_body(body_doc: Doc, has_leading_ws: bool) -> Doc {
    if has_leading_ws {
        doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
    } else {
        doc::indent(body_doc)
    }
}

impl<'a> Printer<'a> {
    /// Build a doc for an entire fragment (sequence of nodes)
    ///
    /// This is the entry point for doc-based inline content formatting.
    /// The resulting doc includes all nodes, so fits() checks will
    /// naturally account for siblings.
    pub(crate) fn build_fragment_doc(&self, fragment: &Fragment) -> Doc {
        self.build_nodes_doc(&fragment.nodes)
    }

    /// Build a doc for a slice of fragment nodes
    ///
    /// Accepts a slice directly, avoiding Fragment allocation when caller
    /// already has a `&[FragmentNode]`.
    pub(crate) fn build_nodes_doc(&self, nodes: &[FragmentNode]) -> Doc {
        self.build_nodes_doc_with_context(nodes, false)
    }

    /// Build a doc for nodes with context about text trimming
    ///
    /// # Parameters
    /// - `trim_text`: If true, trim text completely (block context).
    ///   If false, preserve single space at boundaries (inline context).
    pub(super) fn build_nodes_doc_with_context(
        &self,
        nodes: &[FragmentNode],
        trim_text: bool,
    ) -> Doc {
        let docs: Vec<Doc> = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                // For control flow blocks, check if there's preceding breakable content
                let is_control_flow = matches!(
                    node,
                    FragmentNode::IfBlock(_)
                        | FragmentNode::EachBlock(_)
                        | FragmentNode::AwaitBlock(_)
                        | FragmentNode::KeyBlock(_)
                );
                if is_control_flow {
                    let has_preceding_breakable = nodes[..i].iter().any(|n| {
                        matches!(
                            n,
                            FragmentNode::ExpressionTag(_)
                                | FragmentNode::Element(_)
                                | FragmentNode::SpecialElement(_)
                        )
                    });
                    self.build_fragment_node_doc_with_preceding_context(
                        node,
                        trim_text,
                        has_preceding_breakable,
                    )
                } else {
                    self.build_fragment_node_doc_with_context(node, trim_text)
                }
            })
            .collect();

        if docs.is_empty() {
            doc::empty()
        } else {
            doc::concat(docs)
        }
    }

    /// Build a doc for a node slice with boundary whitespace trimmed
    ///
    /// Matches prettier-plugin-svelte's printChildren behavior:
    /// - Skip whitespace-only text at start and end
    /// - Each text node gets its own fill (for word-level breaking)
    /// - Whitespace between text and inline elements is handled via group([line, ...])
    /// - This allows fills to operate independently while still coordinating breaks
    ///
    /// The key insight from prettier-plugin-svelte:
    /// - Text ending with whitespace before inline element: trim ws, set flag
    /// - Inline element with flag: wrap as group([line, element])
    /// - Text starting with whitespace after inline element: trim ws, wrap prev element with line after
    ///
    /// # Parameters
    /// - `trim_boundaries`: If true, trim leading ws of first node and trailing ws of last node.
    ///   For block elements: true (boundary whitespace is not semantic).
    ///   For inline elements: false (boundary whitespace is semantic, preserve it).
    pub(crate) fn build_nodes_doc_trimmed(
        &self,
        nodes: &[FragmentNode],
        trim_boundaries: bool,
    ) -> Doc {
        if nodes.is_empty() {
            return doc::empty();
        }

        // Find boundary indices based on trim_boundaries setting:
        // - Block elements (trim_boundaries=true): skip whitespace-only text at boundaries
        // - Inline elements (trim_boundaries=false): keep whitespace (normalize to space in handle_text_child)
        //
        // Helper: should we skip this node at the boundary?
        let should_skip_at_boundary = |n: &FragmentNode| -> bool {
            if let FragmentNode::Text(text) = n {
                // Whitespace-only: skip only for block elements
                // Inline elements keep boundary whitespace (normalized to single space)
                text.raw.trim().is_empty() && trim_boundaries
            } else {
                false // Not text, don't skip
            }
        };

        let start_idx = nodes
            .iter()
            .position(|n| !should_skip_at_boundary(n))
            .unwrap_or(nodes.len());
        let end_idx = nodes
            .iter()
            .rposition(|n| !should_skip_at_boundary(n))
            .map_or(0, |i| i + 1);

        if start_idx >= end_idx {
            return doc::empty();
        }

        let trimmed_nodes = &nodes[start_idx..end_idx];
        let trimmed_len = trimmed_nodes.len();

        // Build docs matching prettier-plugin-svelte's structure:
        // - Each text node → fill([word, line, word, ...])
        // - Inline elements → wrapped with group([line, element]) or group([element, line])
        //   depending on surrounding whitespace
        let mut child_docs: Vec<Doc> = Vec::new();
        let mut handle_whitespace_of_prev_text = false;

        for (i, node) in trimmed_nodes.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == trimmed_len - 1;

            if let FragmentNode::Text(text) = node {
                let prev_is_inline = i > 0 && Self::is_inline_content(&trimmed_nodes[i - 1]);
                let next_is_inline =
                    i + 1 < trimmed_len && Self::is_inline_content(&trimmed_nodes[i + 1]);
                let position =
                    SiblingPosition::new(is_first, is_last, prev_is_inline, next_is_inline);
                self.handle_text_child(
                    &text.raw,
                    position,
                    trim_boundaries,
                    &mut child_docs,
                    &mut handle_whitespace_of_prev_text,
                );
            } else if Self::is_inline_content(node) {
                self.handle_inline_child(
                    node,
                    &mut child_docs,
                    &mut handle_whitespace_of_prev_text,
                );
            } else {
                // Other nodes (blocks, etc.)
                // Check if there's preceding breakable content (expression tags or elements)
                // This affects whether block conditions should use remove_lines() or not:
                // - With preceding breakable content: use remove_lines() so that content breaks first
                // - Without preceding breakable content: allow wrapping to respect print_width
                let has_preceding_breakable = trimmed_nodes[..i].iter().any(|n| {
                    matches!(
                        n,
                        FragmentNode::ExpressionTag(_)
                            | FragmentNode::Element(_)
                            | FragmentNode::SpecialElement(_)
                    )
                });
                if let Some(node_doc) = self.build_fragment_node_doc_with_preceding_context(
                    node,
                    false,
                    has_preceding_breakable,
                ) {
                    child_docs.push(node_doc);
                }
                handle_whitespace_of_prev_text = false;
            }
        }

        if child_docs.is_empty() {
            doc::empty()
        } else {
            doc::concat(child_docs)
        }
    }

    /// Check if a node is inline content (non-text node that participates in fill).
    ///
    /// This is NOT the same as `tsv_html::is_inline_element` which checks HTML classification.
    /// Here we check if a fragment node is a non-text element that appears inline with text
    /// (elements, expressions, tags) for the purpose of fill whitespace handling.
    fn is_inline_content(node: &FragmentNode) -> bool {
        matches!(
            node,
            FragmentNode::Element(_)
                | FragmentNode::SpecialElement(_)
                | FragmentNode::ExpressionTag(_)
                | FragmentNode::RenderTag(_)
                | FragmentNode::HtmlTag(_)
        )
    }

    /// Handle a text child node - matches prettier-plugin-svelte's handleTextChild
    fn handle_text_child(
        &self,
        raw: &str,
        position: SiblingPosition,
        trim_boundaries: bool,
        child_docs: &mut Vec<Doc>,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        *handle_whitespace_of_prev_text = false;

        let has_leading_ws = raw.starts_with(char::is_whitespace);
        let has_trailing_ws = raw.ends_with(char::is_whitespace);
        let trimmed = raw.trim();

        let is_first = position.is_first();
        let is_last = position.is_last();

        if trimmed.is_empty() {
            // Whitespace-only text: behavior depends on position and parent type
            if (is_first || is_last) && !trim_boundaries {
                // Boundary whitespace in inline element: always output as single space
                // (normalizes both `<span> text` and `<span>\n\ttext` to `<span> text`)
                child_docs.push(doc::text(" "));
            } else {
                // Middle whitespace or block element: signal separator needed
                *handle_whitespace_of_prev_text = true;
            }
            return;
        }

        // Determine what whitespace to trim
        // For block elements: always trim first/last boundaries
        // For inline elements: preserve space-only boundaries, normalize newline boundaries to space
        let has_leading_space_only = raw.has_leading_space_only();
        let has_trailing_space_only = raw.has_trailing_space_only();

        // Track if we need to add a space to replace trimmed newline whitespace (inline only)
        let mut add_leading_space = false;
        let mut add_trailing_space = false;

        let mut trim_left = if is_first {
            if trim_boundaries {
                true // Block: always trim
            } else if has_leading_space_only {
                false // Inline with space-only: preserve
            } else if has_leading_ws {
                add_leading_space = true; // Inline with newline: trim but add space
                true
            } else {
                false // No leading whitespace
            }
        } else {
            false
        };

        let mut trim_right = if is_last {
            if trim_boundaries {
                true // Block: always trim
            } else if has_trailing_space_only {
                false // Inline with space-only: preserve
            } else if has_trailing_ws {
                add_trailing_space = true; // Inline with newline: trim but add space
                true
            } else {
                false // No trailing whitespace
            }
        } else {
            false
        };

        // If text starts with whitespace and prev is inline element:
        // trim the leading ws and wrap the previous element with a trailing line
        if has_leading_ws && !is_first && position.prev_is_inline() {
            trim_left = true;
            add_leading_space = false; // line() handles the space
            // Pop the last doc (the inline element) and wrap it with trailing line
            if let Some(last_doc) = child_docs.pop() {
                child_docs.push(doc::group(doc::concat(vec![last_doc, doc::line()])));
            }
        }

        // If text ends with whitespace and next is inline element:
        // trim the trailing ws and set flag for the next element
        if has_trailing_ws && !is_last && position.next_is_inline() {
            trim_right = true;
            add_trailing_space = false; // next element's line() handles the space
            *handle_whitespace_of_prev_text = true;
        }

        // Build fill for this text node's words
        if add_leading_space {
            child_docs.push(doc::text(" "));
        }
        if let Some(fill_doc) = self.build_text_fill_doc_trimmed(raw, trim_left, trim_right) {
            child_docs.push(fill_doc);
        }
        if add_trailing_space {
            child_docs.push(doc::text(" "));
        }
    }

    /// Handle an inline child element - matches prettier-plugin-svelte's handleInlineChild
    fn handle_inline_child(
        &self,
        node: &FragmentNode,
        child_docs: &mut Vec<Doc>,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, false) {
            if *handle_whitespace_of_prev_text {
                // Previous text had trailing whitespace - wrap element with leading line
                child_docs.push(doc::group(doc::concat(vec![doc::line(), node_doc])));
            } else {
                child_docs.push(node_doc);
            }
        }
        *handle_whitespace_of_prev_text = false;
    }

    /// Build a doc for nodes in a multiline block context.
    ///
    /// Block children (div, p, etc.) and control flow blocks get their own lines.
    /// Text nodes with newlines split into separate lines, preserving source structure.
    pub(super) fn build_nodes_doc_multiline(&self, nodes: &[FragmentNode]) -> Doc {
        if nodes.is_empty() {
            return doc::empty();
        }

        // Find first and last non-whitespace indices
        let start_idx = nodes
            .iter()
            .position(|n| !n.is_whitespace_only_text())
            .unwrap_or(nodes.len());
        let end_idx = nodes
            .iter()
            .rposition(|n| !n.is_whitespace_only_text())
            .map_or(0, |i| i + 1);

        if start_idx >= end_idx {
            return doc::empty();
        }

        let trimmed_nodes = &nodes[start_idx..end_idx];

        // Check if we should split expressions to separate lines
        // This matches Prettier: multiple expressions with whitespace between them
        // get their own lines, but expressions with semantic text stay together
        let should_split_expressions = self.should_split_expressions_in_nodes(trimmed_nodes);

        // Use separate current_line and completed lines vectors to avoid unwrap calls.
        // The pattern: build current line, then push to lines when starting a new line.
        let mut lines: Vec<Vec<Doc>> = Vec::new();
        let mut current_line: Vec<Doc> = Vec::new();

        // Track if previous text ended with space (for inline-before-block pattern)
        let mut prev_text_has_trailing_space = false;

        for (i, node) in trimmed_nodes.iter().enumerate() {
            let is_block = self.is_block_fragment_node(node);

            if is_block {
                // Block element - check if it should stay on the same line as previous content
                //
                // Control flow blocks ({#if}, {#each}, etc.) can hug when:
                // 1. Directly adjacent to previous node (no whitespace between)
                // 2. Previous text had trailing space AND on same source line
                //
                // HTML block elements (<div>, <p>, etc.) can stay inline when:
                // - Previous text had trailing space AND on same source line
                // (But NOT when directly adjacent without whitespace)
                let is_control_flow = super::helpers::is_control_flow_block(node);
                let keep_inline_with_prev = if i > 0 {
                    let prev_span = trimmed_nodes[i - 1].span();
                    let curr_span = node.span();
                    // Directly adjacent: prev.end == curr.start (no text node between)
                    // Only control flow blocks can hug when directly adjacent
                    let directly_adjacent = is_control_flow && prev_span.end == curr_span.start;
                    // Previous text had trailing space and on same line - works for all blocks
                    let text_with_space = prev_text_has_trailing_space
                        && tsv_lang::printing::spans_on_same_line(
                            self.source,
                            prev_span,
                            curr_span,
                        );
                    directly_adjacent || text_with_space
                } else {
                    false
                };

                if let Some(node_doc) = self.build_fragment_node_doc_in_multiline(node, true) {
                    if keep_inline_with_prev {
                        // Add to current line (inline with preceding text)
                        current_line.push(node_doc);
                    } else if current_line.is_empty() {
                        // Current line is empty - just add the block
                        current_line.push(node_doc);
                    } else {
                        // Start a new line for the block
                        lines.push(std::mem::take(&mut current_line));
                        current_line.push(node_doc);
                    }

                    // Check if next node should hug the closing tag (no break)
                    //
                    // For control flow blocks, hug when:
                    // 1. Next non-text node is directly adjacent (no whitespace between)
                    // 2. Next text has content and is on same source line
                    //
                    // Whitespace-only text between nodes means NO hugging (expands to line break).
                    // For HTML block elements, always break after.
                    let next_hugs_closing = is_control_flow
                        && trimmed_nodes.get(i + 1).is_some_and(|next| {
                            let curr_span = node.span();
                            let next_span = next.span();

                            // Whitespace-only text means no hugging
                            if let FragmentNode::Text(next_text) = next {
                                if next_text.raw.trim().is_empty() {
                                    return false;
                                }
                                // Text with content - hug if on same source line as block
                                return tsv_lang::printing::spans_on_same_line(
                                    self.source,
                                    curr_span,
                                    next_span,
                                );
                            }
                            // Non-text node - check if directly adjacent
                            curr_span.end == next_span.start
                        });

                    if !next_hugs_closing {
                        // New line after block (unless next node hugs)
                        lines.push(std::mem::take(&mut current_line));
                    }
                }
                prev_text_has_trailing_space = false;
            } else if let FragmentNode::Text(text) = node {
                // Text - split on newlines to preserve source line structure
                if text.raw.is_whitespace_only() {
                    let newline_count = text.raw.chars().filter(|&c| c == '\n').count();
                    if newline_count > 0 {
                        // Whitespace with newlines - preserve ONE blank line (Prettier behavior)
                        // First newline ends the current line
                        if !current_line.is_empty() {
                            lines.push(std::mem::take(&mut current_line));
                        }
                        // At most one blank line (2+ newlines → 1 blank line)
                        if newline_count >= 2 {
                            lines.push(vec![]);
                        }
                        prev_text_has_trailing_space = false; // Newline resets trailing space
                    } else if should_split_expressions {
                        // When splitting expressions, whitespace between them becomes a line break
                        // instead of a space (matches Prettier's multiline expression handling)
                        if !current_line.is_empty() {
                            lines.push(std::mem::take(&mut current_line));
                        }
                        prev_text_has_trailing_space = false;
                    } else {
                        // Inline whitespace - add space if there's preceding content
                        if !current_line.is_empty() {
                            current_line.push(doc::text(" "));
                        }
                        // Whitespace-only text counts as trailing space for inline-before-block
                        prev_text_has_trailing_space = true;
                    }
                } else if text.raw.contains('\n') {
                    // Text with newlines - split into lines
                    // Track if current line was empty BEFORE we started (from previous block)
                    let line_was_empty_before = current_line.is_empty();
                    let parts: Vec<&str> = text.raw.split('\n').collect();
                    // Track consecutive blank lines to collapse them to max 1
                    // Only count as "had blank" when we actually push an empty line
                    let mut consecutive_blank_count = 0;
                    for (idx, part) in parts.iter().enumerate() {
                        // Push new line for each newline in the text, EXCEPT:
                        // - idx == 0: we're still on the "before first newline" part
                        // - idx == 1 AND line was empty before AND first part was empty:
                        //   reuse the empty line from previous block element
                        let should_skip =
                            idx == 0 || (idx == 1 && line_was_empty_before && parts[0].is_empty());
                        if !should_skip {
                            let is_pushing_blank = current_line.is_empty();
                            // Only allow one consecutive blank line - skip the push but NOT the content
                            let should_push = !(is_pushing_blank && consecutive_blank_count >= 1);
                            if should_push {
                                lines.push(std::mem::take(&mut current_line));
                                if is_pushing_blank {
                                    consecutive_blank_count += 1;
                                } else {
                                    consecutive_blank_count = 0;
                                }
                            }
                        }
                        // Preserve leading space if part has it
                        if part.starts_with(char::is_whitespace) && !current_line.is_empty() {
                            current_line.push(doc::text(" "));
                        }
                        // Use fill for word-level breaking
                        if let Some(fill_doc) = self.build_text_fill_doc_trimmed(part, true, true) {
                            current_line.push(fill_doc);
                            consecutive_blank_count = 0; // Content resets blank tracking

                            // Preserve trailing space if:
                            // - Part has trailing whitespace, AND
                            // - There's more content after (not just empty trailing parts), OR
                            // - This text node is followed by another node
                            let remaining_parts_have_content =
                                parts[idx + 1..].iter().any(|p| !p.trim().is_empty());
                            let is_last_node = i == trimmed_nodes.len() - 1;
                            if part.ends_with(char::is_whitespace)
                                && (remaining_parts_have_content || !is_last_node)
                            {
                                current_line.push(doc::text(" "));
                            }
                        }
                    }
                    // Last part's trailing space affects next node
                    prev_text_has_trailing_space = parts
                        .last()
                        .is_some_and(|p| p.ends_with(char::is_whitespace));
                } else {
                    // No newlines - add to current line with fill for word-level breaking
                    let has_leading = text.raw.starts_with(char::is_whitespace);
                    let has_trailing = text.raw.ends_with(char::is_whitespace);

                    // Add leading space if source has it AND line has content
                    if has_leading && !current_line.is_empty() {
                        current_line.push(doc::text(" "));
                    }

                    // Use fill for multi-word text to enable word-level line breaking
                    if let Some(fill_doc) = self.build_text_fill_doc_trimmed(&text.raw, true, true)
                    {
                        current_line.push(fill_doc);
                    }

                    // Add trailing space if source has it, but NOT for the last node
                    // (boundary whitespace at end of fragment should be trimmed)
                    let is_last = i == trimmed_nodes.len() - 1;
                    if has_trailing && !is_last {
                        current_line.push(doc::text(" "));
                    }

                    // Track trailing space for inline-before-block pattern
                    prev_text_has_trailing_space = has_trailing;
                }
            } else if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, false) {
                // Non-text inline content (expressions, etc.)
                current_line.push(node_doc);
            }
        }

        // Don't forget to push the final current_line if it has content
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Build output: join lines with hardlines, preserving blank lines
        // - Content lines: hardline (adds \n + indent)
        // - Blank lines: literalline (adds \n only, no indent)
        // Skip leading and trailing empty lines (boundaries handled by element structure)
        let mut docs = Vec::new();
        let total_lines = lines.len();
        let mut found_first_content = false;

        for (i, line_docs) in lines.into_iter().enumerate() {
            let is_empty = line_docs.is_empty();
            let is_last = i == total_lines - 1;

            // Skip leading empty lines (element structure adds hardline before content)
            if is_empty && !found_first_content {
                continue;
            }

            // Skip trailing empty lines (after last content)
            if is_empty && is_last {
                continue;
            }

            if is_empty {
                // Internal blank line - use literalline (just \n, no indentation)
                docs.push(doc::literalline());
            } else {
                // Content line - use hardline before it (except first)
                if !docs.is_empty() {
                    docs.push(doc::hardline());
                }
                docs.push(doc::concat(line_docs));
                found_first_content = true;
            }
        }

        if docs.is_empty() {
            doc::empty()
        } else {
            doc::concat(docs)
        }
    }

    /// Check if a fragment node is a block-level node (needs its own line)
    ///
    /// Components are NOT treated as blocks - like Prettier, they're printed inline.
    /// The line structure comes from whitespace in text nodes, not from node types.
    fn is_block_fragment_node(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::Element(el) => {
                let tag = self.resolve_symbol(el.name);
                // Only HTML block elements - components are inline
                tsv_html::is_block_element(&tag)
            }
            FragmentNode::SpecialElement(_) => true,
            _ => super::helpers::is_control_flow_block(node),
        }
    }

    /// Check if expressions should be split to separate lines in multiline mode.
    ///
    /// Returns true when:
    /// - There are 2+ expression tags
    /// - There's whitespace-only text BETWEEN expressions (layout whitespace)
    ///
    /// Returns false when:
    /// - Single expression
    /// - Expressions directly adjacent (no whitespace between)
    /// - Semantic text between expressions (e.g., `{'<'}div{'>'}`)
    pub(super) fn should_split_expressions_in_nodes(&self, nodes: &[FragmentNode]) -> bool {
        // Count expression nodes
        let expr_count = nodes
            .iter()
            .filter(|n| matches!(n, FragmentNode::ExpressionTag(_)))
            .count();

        if expr_count < 2 {
            return false;
        }

        // Find first and last expression indices
        let first_expr = nodes
            .iter()
            .position(|n| matches!(n, FragmentNode::ExpressionTag(_)));
        let last_expr = nodes
            .iter()
            .rposition(|n| matches!(n, FragmentNode::ExpressionTag(_)));

        match (first_expr, last_expr) {
            (Some(first), Some(last)) if first < last => {
                // Check if there's whitespace-only text between expressions
                // The decision to split is controlled by the outer condition
                // (source_has_leading_break && has_trailing_whitespace)
                nodes[first..=last]
                    .iter()
                    .any(FragmentNode::is_whitespace_only_text)
            }
            _ => false,
        }
    }

    /// Build a doc for a single fragment node with text trimming context
    ///
    /// Returns None for whitespace-only text nodes that should be skipped.
    fn build_fragment_node_doc_with_context(
        &self,
        node: &FragmentNode,
        trim_text: bool,
    ) -> Option<Doc> {
        self.build_fragment_node_doc_impl(node, trim_text, false, false)
    }

    /// Build a fragment node doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces
    /// (spaces but no newlines) will expand to multiline format.
    fn build_fragment_node_doc_in_multiline(
        &self,
        node: &FragmentNode,
        trim_text: bool,
    ) -> Option<Doc> {
        self.build_fragment_node_doc_impl(node, trim_text, true, false)
    }

    /// Build a fragment node doc with preceding content context.
    ///
    /// When `has_preceding_breakable` is true, block conditions will use remove_lines()
    /// to ensure earlier content breaks before the condition.
    fn build_fragment_node_doc_with_preceding_context(
        &self,
        node: &FragmentNode,
        trim_text: bool,
        has_preceding_breakable: bool,
    ) -> Option<Doc> {
        self.build_fragment_node_doc_impl(node, trim_text, false, has_preceding_breakable)
    }

    fn build_fragment_node_doc_impl(
        &self,
        node: &FragmentNode,
        trim_text: bool,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Option<Doc> {
        match node {
            FragmentNode::Text(text) => self.build_text_doc(text, trim_text),
            FragmentNode::Element(element) => Some(self.build_element_doc(element)),
            FragmentNode::SpecialElement(element) => Some(self.build_special_element_doc(element)),
            FragmentNode::ExpressionTag(tag) => Some(self.build_expression_tag_doc(tag)),
            FragmentNode::Comment(comment) => Some(self.build_html_comment_doc(comment)),
            FragmentNode::IfBlock(block) => Some(self.build_if_block_doc_with_full_context(
                block,
                in_multiline_context,
                has_preceding_breakable,
            )),
            FragmentNode::EachBlock(block) => Some(self.build_each_block_doc_with_full_context(
                block,
                in_multiline_context,
                has_preceding_breakable,
            )),
            FragmentNode::AwaitBlock(block) => Some(self.build_await_block_doc_with_full_context(
                block,
                in_multiline_context,
                has_preceding_breakable,
            )),
            FragmentNode::KeyBlock(block) => Some(self.build_key_block_doc_with_full_context(
                block,
                in_multiline_context,
                has_preceding_breakable,
            )),
            FragmentNode::SnippetBlock(block) => Some(self.build_snippet_block_doc(block)),
            FragmentNode::HtmlTag(tag) => Some(self.build_html_tag_doc(tag)),
            FragmentNode::ConstTag(tag) => Some(self.build_const_tag_doc(tag)),
            FragmentNode::DebugTag(tag) => Some(self.build_debug_tag_doc(tag)),
            FragmentNode::RenderTag(tag) => Some(self.build_render_tag_doc(tag)),
        }
    }

    //
    // Text nodes
    //

    /// Build a doc for a text node
    ///
    /// Returns None for whitespace-only text that should be skipped.
    /// For text with content, normalizes internal whitespace to single spaces.
    ///
    /// # Parameters
    /// - `trim_completely`: If true, trim leading/trailing whitespace (block context).
    ///   If false, preserve single space at boundaries (inline context).
    fn build_text_doc(&self, text: &internal::Text, trim_completely: bool) -> Option<Doc> {
        let trimmed = text.raw.trim();
        if trimmed.is_empty() {
            // Pure whitespace: collapse to single space only in inline context
            if !trim_completely && text.raw.contains(char::is_whitespace) {
                Some(doc::text(" "))
            } else {
                None
            }
        } else {
            // Has content: use fill() for word-level line breaking
            // This matches Prettier's splitTextToDocs behavior
            self.build_text_fill_doc(&text.raw, trim_completely)
        }
    }

    /// Build a fill doc for text content, enabling word-level line breaking.
    ///
    /// Splits text on whitespace into words, joining with line() docs.
    /// This allows fill() to break at word boundaries when lines exceed width.
    fn build_text_fill_doc(&self, raw: &str, trim_completely: bool) -> Option<Doc> {
        self.build_text_fill_doc_trimmed(raw, trim_completely, trim_completely)
    }

    /// Build a fill doc for text with separate control over leading/trailing trimming.
    ///
    /// Used by build_nodes_doc_trimmed where first node trims leading, last trims trailing.
    fn build_text_fill_doc_trimmed(
        &self,
        raw: &str,
        trim_leading: bool,
        trim_trailing: bool,
    ) -> Option<Doc> {
        let has_leading_ws = raw.starts_with(char::is_whitespace);
        let has_trailing_ws = raw.ends_with(char::is_whitespace);

        // Split on whitespace and collect non-empty words
        let words: Vec<&str> = raw.split_whitespace().collect();
        if words.is_empty() {
            return None;
        }

        // Single word: just return text (with boundary handling)
        if words.len() == 1 {
            let mut result = String::new();
            if !trim_leading && has_leading_ws {
                result.push(' ');
            }
            result.push_str(words[0]);
            if !trim_trailing && has_trailing_ws {
                result.push(' ');
            }
            return Some(doc::text_owned(result));
        }

        // Multiple words: build fill parts [word, line, word, line, ...]
        let mut parts = Vec::with_capacity(words.len() * 2);

        // Handle leading whitespace
        if !trim_leading && has_leading_ws {
            parts.push(doc::text(" "));
        }

        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                parts.push(doc::line());
            }
            parts.push(doc::text_owned((*word).to_string()));
        }

        // Handle trailing whitespace
        if !trim_trailing && has_trailing_ws {
            parts.push(doc::text(" "));
        }

        Some(doc::fill(parts))
    }

    //
    // Comment nodes
    //

    /// Build a doc for an HTML comment
    pub(crate) fn build_html_comment_doc(&self, comment: &internal::HtmlComment) -> Doc {
        doc::concat(vec![
            doc::text("<!--"),
            doc::text_owned(comment.content.clone()),
            doc::text("-->"),
        ])
    }

    //
    // Control flow blocks
    //

    /// Build a doc for an if block
    ///
    /// For inline blocks (no leading/trailing whitespace in body), hugs content directly:
    ///   {#if cond}content{/if}
    ///
    /// For multiline blocks (has whitespace boundaries), uses hardlines:
    ///   {#if cond}\n  content\n{/if}
    ///
    /// Note: Body is always wrapped in indent() so any internal breaks (like component
    /// attr wrapping) get proper indentation relative to the if block.
    pub(crate) fn build_if_block_doc(&self, block: &internal::IfBlock) -> Doc {
        self.build_if_block_doc_with_context(block, false)
    }

    /// Build if block doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces expand.
    pub(crate) fn build_if_block_doc_with_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> Doc {
        self.build_if_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build if block doc with full context (multiline + preceding content).
    ///
    /// `has_preceding_breakable`: If true, there's breakable content before this block,
    /// so use remove_lines() to ensure that content breaks first.
    fn build_if_block_doc_with_full_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Doc {
        // Build expression doc with context-dependent behavior
        // Use remove_lines only if there's preceding breakable content (so it breaks first).
        // Otherwise, allow natural wrapping to respect print_width.
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.test,
            block.opening_tag_span.start + 5, // after "{#if "
            block.opening_tag_span.end - 1,   // before "}"
            5,                                // "{#if " = 5 chars
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        // E.g., `{#if a} content {/if}` or `{#if a} content{/if}` → expand to multiline.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.consequent, in_multiline_context);
        let is_inline = !has_leading && !has_trailing;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.consequent)
        } else {
            self.build_nodes_doc_multiline(&block.consequent.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation
        let indented_body = indent_body(body_doc, has_leading);

        let mut parts = vec![doc::text("{#if "), expr_doc, doc::text("}"), indented_body];

        // Handle alternate (else/else-if) and determine final trailing status
        let final_has_trailing = if let Some(alt) = &block.alternate {
            // Add break before alternate only if consequent has trailing ws
            if has_trailing {
                parts.push(doc::hardline());
            }
            parts.push(self.build_if_alternate_doc(
                alt,
                has_leading,
                has_trailing,
                in_multiline_context,
            ));
            // Get trailing status from the final branch
            self.get_final_branch_trailing(block, in_multiline_context)
        } else {
            has_trailing
        };

        // Add endline before {/if} only if final branch has trailing whitespace
        if final_has_trailing {
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/if}"));
        doc::concat(parts)
    }

    /// Check if a fragment can be flattened to an else-if.
    ///
    /// Returns the single IfBlock if the fragment contains exactly one IfBlock
    /// (plus optional whitespace) and can be flattened. Returns None if the
    /// fragment has multiple IfBlocks or other content that prevents flattening.
    fn get_flattenable_else_if(alt: &Fragment) -> Option<&internal::IfBlock> {
        let mut if_block: Option<&internal::IfBlock> = None;

        for node in &alt.nodes {
            match node {
                FragmentNode::IfBlock(b) => {
                    if if_block.is_some() {
                        // Multiple IfBlocks - can't flatten
                        return None;
                    }
                    if_block = Some(b);
                }
                FragmentNode::Text(t) if t.raw.trim().is_empty() => {
                    // Whitespace-only text is OK
                }
                _ => {
                    // Non-whitespace content - can't flatten
                    return None;
                }
            }
        }

        if_block
    }

    /// Build doc for if block alternate (else or else-if)
    ///
    /// Uses separate leading/trailing whitespace handling for proper hugging.
    /// `parent_has_leading` - whether parent had leading ws (break after opening)
    /// `parent_has_trailing` - whether parent had trailing ws (break before this alternate)
    /// `in_multiline_context` - whether we're in a multiline parent context
    ///
    /// Returns (doc, final_has_trailing) where final_has_trailing indicates whether
    /// the last branch of this alternate chain has trailing whitespace.
    fn build_if_alternate_doc(
        &self,
        alt: &Fragment,
        parent_has_leading: bool,
        parent_has_trailing: bool,
        in_multiline_context: bool,
    ) -> Doc {
        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // {:else if condition}
            // The span offset depends on whether this is a true {:else if} or a normalized {#if}
            let opening_offset: usize = if else_if.elseif { 10 } else { 5 };
            let expr_doc = self.build_expression_doc_for_block(
                &else_if.test,
                else_if.opening_tag_span.start + opening_offset as u32,
                else_if.opening_tag_span.end - 1,
                opening_offset,
                in_multiline_context,
            );

            // Check this branch's own leading/trailing whitespace
            let (has_leading, has_trailing) =
                self.fragment_ws_status(&else_if.consequent, in_multiline_context);
            let is_inline = !has_leading && !has_trailing;
            let parent_inline = !parent_has_leading && !parent_has_trailing;
            let is_both_inline = is_inline && parent_inline;

            // For inline: use regular fragment doc (preserves spaces)
            // For multiline: use multiline doc (preserves line structure)
            let body_doc = if is_both_inline {
                self.build_fragment_doc(&else_if.consequent)
            } else {
                self.build_nodes_doc_multiline(&else_if.consequent.nodes)
            };

            let indented_body = indent_body(body_doc, has_leading);

            let mut parts = vec![
                doc::text("{:else if "),
                expr_doc,
                doc::text("}"),
                indented_body,
            ];

            // Handle nested alternate or trailing
            if let Some(nested_alt) = &else_if.alternate {
                // Add break before next alternate only if this branch has trailing ws
                if has_trailing {
                    parts.push(doc::hardline());
                }
                parts.push(self.build_if_alternate_doc(
                    nested_alt,
                    has_leading,
                    has_trailing,
                    in_multiline_context,
                ));
            }

            return doc::concat(parts);
        }

        // Plain {:else}
        let (has_leading, has_trailing) = self.fragment_ws_status(alt, in_multiline_context);
        let is_inline = !has_leading && !has_trailing;
        let parent_inline = !parent_has_leading && !parent_has_trailing;
        let is_both_inline = is_inline && parent_inline;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc
        let body_doc = if is_both_inline {
            self.build_nodes_doc(&alt.nodes)
        } else {
            self.build_nodes_doc_multiline(&alt.nodes)
        };

        let indented_body = indent_body(body_doc, has_leading);

        doc::concat(vec![doc::text("{:else}"), indented_body])
    }

    /// Get the trailing whitespace status of the final branch in an if-block.
    ///
    /// This walks the alternate chain to find the last branch and returns
    /// whether it has trailing whitespace (for placing `{/if}`).
    fn get_final_branch_trailing(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> bool {
        // If no alternate, use the consequent's trailing
        let Some(alt) = &block.alternate else {
            let (_, has_trailing) =
                self.fragment_ws_status(&block.consequent, in_multiline_context);
            return has_trailing;
        };

        // Check if this is an else-if chain
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // Recurse into else-if
            return self.get_final_branch_trailing(else_if, in_multiline_context);
        }

        // Plain {:else} - use its trailing
        let (_, has_trailing) = self.fragment_ws_status(alt, in_multiline_context);
        has_trailing
    }

    /// Build a doc for an each block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_each_block_doc(&self, block: &internal::EachBlock) -> Doc {
        self.build_each_block_doc_with_context(block, false)
    }

    /// Build each block doc with multiline context awareness.
    pub(crate) fn build_each_block_doc_with_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
    ) -> Doc {
        self.build_each_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build each block doc with full context (multiline + preceding content).
    fn build_each_block_doc_with_full_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Doc {
        // Build expression doc with context-dependent behavior
        // Comment range: after "{#each " to before "as" keyword (or end if no context)
        let allow_wrapping = !has_preceding_breakable;
        let expr_comment_end = block
            .context
            .as_ref()
            .map_or(block.opening_tag_span.end - 1, |c| c.span().start);
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + 7, // after "{#each "
            expr_comment_end,
            7, // "{#each " = 7 chars
            allow_wrapping || in_multiline_context,
        );

        let mut opening = vec![doc::text("{#each "), expr_doc];

        // Pattern (context) - only add " as " when there's a context or index
        if let Some(context) = &block.context {
            opening.push(doc::text(" as "));
            // Format pattern through TypeScript formatter for proper whitespace normalization
            let pattern_doc = self.build_pattern_doc(context);
            opening.push(pattern_doc);
            if let Some(index) = &block.index {
                opening.push(doc::text(", "));
                opening.push(doc::text_owned(index.clone()));
            }
        } else if let Some(index) = &block.index {
            // No context but has index: ", i" pattern
            opening.push(doc::text(", "));
            opening.push(doc::text_owned(index.clone()));
        }

        if let Some(key) = &block.key {
            // Build key doc with context-dependent behavior
            // The key expression is inside parens, so opening offset accounts for that
            let key_doc = if let Some(key_span) = block.key_span {
                self.build_expression_doc_for_block(
                    key,
                    key_span.start + 1, // after "("
                    key_span.end - 1,   // before ")"
                    1,                  // "(" = 1 char (key is inside parens)
                    allow_wrapping || in_multiline_context,
                )
            } else {
                // No key_span: build doc directly
                tsv_ts::build_expression_doc_with_comments(
                    key,
                    self.source,
                    Rc::clone(&self.interner),
                    &self.config,
                    self.comments,
                    &self.line_breaks,
                )
            };
            opening.push(doc::text(" ("));
            opening.push(key_doc);
            opening.push(doc::text(")"));
        }

        opening.push(doc::text("}"));

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.body, in_multiline_context);
        let is_inline = !has_leading && !has_trailing;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        let indented_body = indent_body(body_doc, has_leading);

        let mut parts = vec![doc::concat(opening), indented_body];

        // Determine final trailing status (from body or fallback if present)
        let final_has_trailing = if let Some(fallback) = &block.fallback {
            // Add break before {:else} only if body has trailing ws
            if has_trailing {
                parts.push(doc::hardline());
            }

            let (fallback_has_leading, fallback_has_trailing) =
                self.fragment_ws_status(fallback, in_multiline_context);
            let fallback_inline = !fallback_has_leading && !fallback_has_trailing;
            let is_both_inline = fallback_inline && is_inline;

            parts.push(doc::text("{:else}"));

            // For inline: use regular fragment doc
            // For multiline: use multiline doc
            let fallback_doc = if is_both_inline {
                self.build_fragment_doc(fallback)
            } else {
                self.build_nodes_doc_multiline(&fallback.nodes)
            };

            let indented_fallback = indent_body(fallback_doc, fallback_has_leading || has_leading);
            parts.push(indented_fallback);

            fallback_has_trailing
        } else {
            has_trailing
        };

        // Add endline before {/each} only if final has trailing whitespace
        if final_has_trailing {
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/each}"));
        doc::concat(parts)
    }

    /// Build a doc for an await block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock) -> Doc {
        self.build_await_block_doc_with_context(block, false)
    }

    /// Build await block doc with multiline context awareness.
    pub(crate) fn build_await_block_doc_with_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
    ) -> Doc {
        self.build_await_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build await block doc with full context (multiline + preceding content).
    fn build_await_block_doc_with_full_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Doc {
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + 8, // after "{#await "
            block.opening_tag_span.end - 1,   // before "}"
            8,                                // "{#await " = 8 chars
            allow_wrapping || in_multiline_context,
        );

        let mut parts = vec![doc::text("{#await "), expr_doc];

        // Shorthand: {#await expr then value}
        if let (Some(value), None) = (&block.value, &block.pending) {
            let value_pattern =
                self.extract_source_range(value.span().start_usize(), value.span().end_usize());
            parts.push(doc::text(" then "));
            parts.push(doc::text_owned(value_pattern.to_string()));
            parts.push(doc::text("}"));
            if let Some(then_block) = &block.then {
                // Await shorthands: only use newline-based detection (not space-only)
                // Prettier keeps shorthand forms inline even with symmetric spaces
                let has_leading = self.fragment_has_leading_ws(then_block);
                let has_trailing = self.fragment_has_trailing_ws(then_block);
                let is_inline = !has_leading && !has_trailing;
                let body_doc = if is_inline {
                    self.build_fragment_doc(then_block)
                } else {
                    self.build_nodes_doc_multiline(&then_block.nodes)
                };
                parts.push(indent_body(body_doc, has_leading));
                if has_trailing {
                    parts.push(doc::hardline());
                }
            }
            parts.push(doc::text("{/await}"));
            return doc::concat(parts);
        }

        // Shorthand: {#await expr catch error}
        if block.pending.is_none()
            && block.value.is_none()
            && let Some(error) = &block.error
        {
            let error_pattern =
                self.extract_source_range(error.span().start_usize(), error.span().end_usize());
            parts.push(doc::text(" catch "));
            parts.push(doc::text_owned(error_pattern.to_string()));
            parts.push(doc::text("}"));
            if let Some(catch_block) = &block.catch {
                // Await shorthands: only use newline-based detection (not space-only)
                let has_leading = self.fragment_has_leading_ws(catch_block);
                let has_trailing = self.fragment_has_trailing_ws(catch_block);
                let is_inline = !has_leading && !has_trailing;
                let body_doc = if is_inline {
                    self.build_fragment_doc(catch_block)
                } else {
                    self.build_nodes_doc_multiline(&catch_block.nodes)
                };
                parts.push(indent_body(body_doc, has_leading));
                if has_trailing {
                    parts.push(doc::hardline());
                }
            }
            parts.push(doc::text("{/await}"));
            return doc::concat(parts);
        }

        parts.push(doc::text("}"));

        // Track whitespace status for each section
        // The final section's trailing determines break before {/await}
        let mut final_has_trailing = false;
        let mut prev_has_trailing = false;

        // Pending - await blocks only use newline-based detection (NOT space-only)
        // Unlike if/each/key, await blocks stay inline even with symmetric spaces:
        // `{#await p} text {/await}` stays inline, not multiline
        if let Some(pending) = &block.pending {
            let has_leading = self.fragment_has_leading_ws(pending);
            let has_trailing = self.fragment_has_trailing_ws(pending);
            let is_inline = !has_leading && !has_trailing;
            let body_doc = if is_inline {
                self.build_fragment_doc(pending)
            } else {
                self.build_nodes_doc_multiline(&pending.nodes)
            };
            parts.push(indent_body(body_doc, has_leading));
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Then
        if let Some(value) = &block.value {
            if prev_has_trailing {
                parts.push(doc::hardline());
            }
            let value_pattern =
                self.extract_source_range(value.span().start_usize(), value.span().end_usize());
            parts.push(doc::text("{:then "));
            parts.push(doc::text_owned(value_pattern.to_string()));
            parts.push(doc::text("}"));
        } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(doc::hardline());
            }
            parts.push(doc::text("{:then}"));
        }
        if let Some(then_block) = &block.then {
            // Await blocks only use newline-based detection
            let has_leading = self.fragment_has_leading_ws(then_block);
            let has_trailing = self.fragment_has_trailing_ws(then_block);
            let is_inline = !has_leading && !has_trailing;
            let body_doc = if is_inline {
                self.build_fragment_doc(then_block)
            } else {
                self.build_nodes_doc_multiline(&then_block.nodes)
            };
            parts.push(indent_body(body_doc, has_leading));
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Catch
        if let Some(error) = &block.error {
            if prev_has_trailing {
                parts.push(doc::hardline());
            }
            let error_pattern =
                self.extract_source_range(error.span().start_usize(), error.span().end_usize());
            parts.push(doc::text("{:catch "));
            parts.push(doc::text_owned(error_pattern.to_string()));
            parts.push(doc::text("}"));
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(doc::hardline());
            }
            parts.push(doc::text("{:catch}"));
        }
        if let Some(catch_block) = &block.catch {
            // Await blocks only use newline-based detection
            let has_leading = self.fragment_has_leading_ws(catch_block);
            let has_trailing = self.fragment_has_trailing_ws(catch_block);
            let is_inline = !has_leading && !has_trailing;
            let body_doc = if is_inline {
                self.build_fragment_doc(catch_block)
            } else {
                self.build_nodes_doc_multiline(&catch_block.nodes)
            };
            parts.push(indent_body(body_doc, has_leading));
            final_has_trailing = has_trailing;
        }

        // Add endline before {/await} only if final section has trailing whitespace
        if final_has_trailing {
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/await}"));
        doc::concat(parts)
    }

    /// Build a doc for a key block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock) -> Doc {
        self.build_key_block_doc_with_context(block, false)
    }

    /// Build key block doc with multiline context awareness.
    pub(crate) fn build_key_block_doc_with_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
    ) -> Doc {
        self.build_key_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build key block doc with full context (multiline + preceding content).
    fn build_key_block_doc_with_full_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Doc {
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + 6, // after "{#key "
            block.opening_tag_span.end - 1,   // before "}"
            6,                                // "{#key " = 6 chars
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.fragment, false);
        let is_inline = !has_leading && !has_trailing;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.fragment)
        } else {
            self.build_nodes_doc_multiline(&block.fragment.nodes)
        };

        let indented_body = indent_body(body_doc, has_leading);

        let mut parts = vec![doc::text("{#key "), expr_doc, doc::text("}"), indented_body];

        // Add endline before {/key} only if trailing whitespace exists
        if has_trailing {
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/key}"));
        doc::concat(parts)
    }

    /// Build a doc for a snippet block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    /// Opening tag uses group() for parameter wrapping when they exceed print width.
    pub(crate) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock) -> Doc {
        // Extract snippet name from the identifier expression
        let name = self.extract_source_range(
            block.expression.span().start_usize(),
            block.expression.span().end_usize(),
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.body, false);
        let is_inline = !has_leading && !has_trailing;

        // Type parameters (generics)
        let type_params_part = block
            .type_parameters
            .as_ref()
            .map_or_else(doc::empty, |tp| {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tp.clone()),
                    doc::text(">"),
                ])
            });

        // Parameters: use raw_parameters if available (preserves TypeScript types),
        // otherwise format individual params
        let params_docs: Vec<Doc> = if let Some(raw) = &block.raw_parameters {
            vec![doc::text_owned(raw.clone())]
        } else {
            block
                .parameters
                .iter()
                .map(|p| {
                    // Format parameter through TypeScript formatter for proper normalization
                    let formatted =
                        tsv_ts::format_expression(p, self.source, Rc::clone(&self.interner), &self.line_breaks);
                    doc::text_owned(formatted)
                })
                .collect()
        };

        // Build opening tag with group for parameter wrapping
        // When fits: {#snippet name(a, b, c)}
        // When wraps: {#snippet name(\n\ta,\n\tb,\n\tc,\n)}
        // Empty params: {#snippet name()} - no wrapping structure
        let opening_doc = if params_docs.is_empty() {
            // No params - simple structure that won't break incorrectly
            doc::concat(vec![
                doc::text("{#snippet "),
                doc::text_owned(name.to_string()),
                type_params_part,
                doc::text("()}"),
            ])
        } else {
            // Build params doc with line() separators for wrapping
            // Pre-allocate: each param + separator (except first)
            let mut parts = Vec::with_capacity(params_docs.len() * 3);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(doc::text(","));
                    parts.push(doc::line());
                }
                parts.push(param_doc);
            }
            let params_doc = doc::concat(parts);

            doc::group(doc::concat(vec![
                doc::text("{#snippet "),
                doc::text_owned(name.to_string()),
                type_params_part,
                doc::text("("),
                doc::indent_softline(params_doc),
                doc::trailing_comma(),
                doc::softline(),
                doc::text(")}"),
            ]))
        };

        let mut parts = vec![opening_doc];

        // Body: inline hugs directly, multiline uses hardlines
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        parts.push(indent_body(body_doc, has_leading));

        // Add endline before {/snippet} only if trailing whitespace exists
        if has_trailing {
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/snippet}"));
        doc::concat(parts)
    }

    //
    // Template tags
    //

    /// Build a doc for {@html expr}
    pub(crate) fn build_html_tag_doc(&self, tag: &internal::HtmlTag) -> Doc {
        // Build expression doc with surrounding comments
        // Span range: after "{@html " (start + 7) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 7, // after "{@html "
            tag.span.end - 1,   // before "}"
        );

        // Assignment expressions need parens: {@html (a = b)}
        let expr_doc = if matches!(tag.expression, tsv_ts::Expression::AssignmentExpression(_)) {
            doc::parens(expr_doc)
        } else {
            expr_doc
        };

        doc::concat(vec![doc::text("{@html "), expr_doc, doc::text("}")])
    }

    /// Build a doc for {@const declaration}
    pub(crate) fn build_const_tag_doc(&self, tag: &internal::ConstTag) -> Doc {
        // Format both id (pattern) and init expression properly
        // Patterns like {a,b} need spacing: {a, b}
        // For @const, comments are typically after the init expression
        // Span range for init: after "= " to before "}" (end - 1)
        let id_doc = tsv_ts::build_expression_doc(
            &tag.id,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            &self.line_breaks,
        );
        // Build init with comments (comments are typically trailing after init)
        let init_doc = self.build_expression_with_comments_doc(
            &tag.init,
            tag.init.span().start,
            tag.span.end - 1, // before "}"
        );

        doc::concat(vec![
            doc::text("{@const "),
            id_doc,
            doc::text(" = "),
            init_doc,
            doc::text("}"),
        ])
    }

    /// Build a doc for {@debug vars}
    pub(crate) fn build_debug_tag_doc(&self, tag: &internal::DebugTag) -> Doc {
        if tag.identifiers.is_empty() {
            doc::text("{@debug}")
        } else {
            let idents: Vec<Doc> = tag
                .identifiers
                .iter()
                .map(|id| {
                    let name =
                        self.extract_source_range(id.span().start_usize(), id.span().end_usize());
                    doc::text_owned(name.to_string())
                })
                .collect();

            doc::concat(vec![
                doc::text("{@debug "),
                doc::join(idents, ", "),
                doc::text("}"),
            ])
        }
    }

    /// Build a doc for {@render snippet(args)}
    pub(crate) fn build_render_tag_doc(&self, tag: &internal::RenderTag) -> Doc {
        // Build expression doc with surrounding comments
        // Span range: after "{@render " (start + 9) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 9, // after "{@render "
            tag.span.end - 1,   // before "}"
        );

        doc::concat(vec![doc::text("{@render "), expr_doc, doc::text("}")])
    }

    //
    // Helper methods
    //

    /// Extract source range as string slice
    fn extract_source_range(&self, start: usize, end: usize) -> &str {
        &self.source[start..end]
    }

    /// Build a doc for a pattern (destructuring context)
    ///
    /// Patterns use specific whitespace rules:
    /// - Object patterns: `{ a, b }` (spaces inside braces)
    /// - Array patterns: `[a, b]` (no spaces inside brackets)
    ///
    /// Used for `{#each ... as pattern}` contexts.
    fn build_pattern_doc(&self, expr: &tsv_ts::Expression) -> Doc {
        match expr {
            tsv_ts::Expression::ObjectPattern(obj) => {
                let mut parts = vec![doc::text("{ ")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectPatternProperty::Property(p) => {
                            let key = tsv_ts::format_expression(
                                &p.key,
                                self.source,
                                Rc::clone(&self.interner),
                                &self.line_breaks,
                            );
                            parts.push(doc::text_owned(key));
                            if !p.shorthand {
                                parts.push(doc::text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectPatternProperty::RestElement(r) => {
                            parts.push(doc::text("..."));
                            parts.push(self.build_pattern_doc(&r.argument));
                        }
                    }
                }
                parts.push(doc::text(" }"));
                doc::concat(parts)
            }
            tsv_ts::Expression::ObjectExpression(obj) => {
                // Legacy AST - treat same as ObjectPattern
                let mut parts = vec![doc::text("{ ")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectProperty::Property(p) => {
                            let key = tsv_ts::format_expression(
                                &p.key,
                                self.source,
                                Rc::clone(&self.interner),
                                &self.line_breaks,
                            );
                            parts.push(doc::text_owned(key));
                            if !p.shorthand {
                                parts.push(doc::text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectProperty::SpreadElement(s) => {
                            parts.push(doc::text("..."));
                            parts.push(self.build_pattern_doc(&s.argument));
                        }
                    }
                }
                parts.push(doc::text(" }"));
                doc::concat(parts)
            }
            tsv_ts::Expression::ArrayPattern(arr) => {
                let mut parts = vec![doc::text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(doc::text("]"));
                doc::concat(parts)
            }
            tsv_ts::Expression::ArrayExpression(arr) => {
                // Legacy AST - treat same as ArrayPattern
                let mut parts = vec![doc::text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(doc::text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(doc::text("]"));
                doc::concat(parts)
            }
            tsv_ts::Expression::RestElement(rest) => doc::concat(vec![
                doc::text("..."),
                self.build_pattern_doc(&rest.argument),
            ]),
            tsv_ts::Expression::AssignmentPattern(assign) => doc::concat(vec![
                self.build_pattern_doc(&assign.left),
                doc::text(" = "),
                doc::text_owned(tsv_ts::format_expression(
                    &assign.right,
                    self.source,
                    Rc::clone(&self.interner),
                    &self.line_breaks,
                )),
            ]),
            // Default: format as regular expression
            _ => {
                let formatted =
                    tsv_ts::format_expression(expr, self.source, Rc::clone(&self.interner), &self.line_breaks);
                doc::text_owned(formatted)
            }
        }
    }

    /// Build a doc for an expression with leading and trailing comments
    ///
    /// Looks up comments in the range [span_start, span_end] and includes them:
    /// - Leading comments: between span_start and expr.span().start
    /// - Expression doc
    /// - Trailing comments: between expr.span().end and span_end
    ///
    /// This matches the imperative `print_ts_expression_with_comments()` behavior.
    /// For expressions that need suffix_width awareness (block conditions), use
    /// `build_expression_with_suffix_doc()` instead.
    fn build_expression_with_comments_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) -> Doc {
        // Default suffix_width=1 for closing `}`, opening_offset=5 for typical tags
        self.build_expression_with_context_doc(expr, span_start, span_end, 5, 1)
    }

    /// Build a doc for an expression with width-aware context for proper chain wrapping.
    ///
    /// Used for expressions in block directives (if, each, await, key) where method chains
    /// should wrap with proper continuation indent.
    ///
    /// # Arguments
    /// * `opening_offset` - Characters before the expression (e.g., 5 for `{#if `)
    /// * `suffix_width` - Characters after the expression (e.g., closing tag + body width)
    ///
    /// The width parameters help the TypeScript formatter make correct wrapping decisions.
    /// When the expression contains newlines (from wrapped chains), they are converted to
    /// `doc::hardline()` with `doc::indent()` so the doc rendering respects the context.
    fn build_expression_with_context_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
        opening_offset: usize,
        suffix_width: usize,
    ) -> Doc {
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments (between span_start and expression start)
        let leading_docs: Vec<Doc> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(Self::build_leading_js_comment_doc)
                .collect();

        // Format expression with width-aware context so method chains break
        // at the right points. We use first_line_offset to tell the formatter
        // where it starts on the line for proper width calculations.
        //
        // The formatted string may contain newlines for wrapped chains. We convert
        // these to doc::hardline() so the doc rendering engine applies proper
        // indentation based on the surrounding context.
        //
        // Important: first_line_offset must include an estimate of the context indent.
        // Since we don't track indent level during doc building, we estimate based on
        // the nesting depth. Each level of nesting adds tab_width to the offset.
        // We add 1 extra level to account for being inside at least one element.
        let context_indent = self.config.tab_width;
        let first_line_offset = context_indent + opening_offset;
        let config = tsv_lang::PrintConfig {
            first_line_offset,
            suffix_width,
            ..self.config
        };
        let formatted = tsv_ts::format_expression_with_config(
            expr,
            self.source,
            Rc::clone(&self.interner),
            self.comments,
            config,
            &self.line_breaks,
        );

        // Convert newlines to hardlines for proper doc-based rendering.
        //
        // The TypeScript formatter returns expressions with absolute indentation
        // (relative to column 0). Each continuation line may have tabs for:
        // - Being inside parentheses/call (1 tab)
        // - Arrow function body (2 tabs)
        // - etc.
        //
        // When rendered in doc context, hardline() adds context indentation (e.g.,
        // 1 tab for being inside <div>). We preserve the TS formatter's indentation
        // as-is, so total indent = context + TS tabs.
        //
        // Example: inside <div> with TS output `\t(item) =>`:
        //   hardline() → context indent (1 tab)
        //   + "\t(item) =>" → TS indent (1 tab)
        //   = 2 tabs total ✓
        let expr_doc = if formatted.contains('\n') {
            let lines: Vec<&str> = formatted.split('\n').collect();
            // First line stays at base level (inline with opening tag content)
            let first_line = doc::text_owned(lines[0].to_string());

            // Continuation lines: hardline for context indent + preserve TS indentation
            let mut continuation_parts = Vec::with_capacity((lines.len() - 1) * 2);
            for line in &lines[1..] {
                continuation_parts.push(doc::hardline());
                if !line.trim().is_empty() {
                    // Preserve the line as-is, including its leading tabs
                    continuation_parts.push(doc::text_owned((*line).to_string()));
                }
            }

            doc::concat(vec![first_line, doc::concat(continuation_parts)])
        } else {
            doc::text_owned(formatted)
        };

        // Build docs for trailing comments (between expression end and span_end)
        let trailing_docs: Vec<Doc> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(Self::build_trailing_js_comment_doc)
                .collect();

        // Combine: leading + expr + trailing
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            doc::concat(parts)
        }
    }

    /// Build expression doc for block expressions (if, each, await, key).
    ///
    /// # Context-dependent behavior
    ///
    /// - **Inline context** (`in_multiline_context=false`): Applies `remove_lines()` to prevent
    ///   the block condition from breaking. When the line exceeds print_width, EARLIER content
    ///   should break instead. Example: `{expr}{#if cond}` - expr breaks, cond stays flat.
    ///
    /// - **Multiline context** (`in_multiline_context=true`): The condition is on its own line.
    ///   No `remove_lines()` is applied, allowing long chains to wrap naturally.
    ///   Uses `first_line_offset` to get proper continuation indent for wrapped binary expressions.
    ///
    /// # Parameters
    /// - `opening_offset` - Characters before the expression (e.g., 5 for `{#if `). Used to
    ///   calculate `first_line_offset` which triggers continuation indent for binary expressions.
    /// - `in_multiline_context` - Whether the block is on its own line (multiline) or inline
    fn build_expression_doc_for_block(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
        opening_offset: usize,
        in_multiline_context: bool,
    ) -> Doc {
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments
        let leading_docs: Vec<Doc> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(Self::build_leading_js_comment_doc)
                .collect();

        // Set up config with first_line_offset to trigger continuation indent in the
        // TypeScript formatter. The formatter checks `first_line_offset > 0` to decide
        // whether to use continuation indent for binary expressions.
        // Only apply in multiline context where wrapping is allowed.
        let config = if in_multiline_context {
            let context_indent = self.config.tab_width;
            let first_line_offset = context_indent + opening_offset;
            tsv_lang::PrintConfig {
                first_line_offset,
                ..self.config
            }
        } else {
            self.config
        };

        // Build expression doc tree
        // Assignment expressions need parens in block conditions: {#if (a = b)}
        let expr_doc = if matches!(expr, tsv_ts::Expression::AssignmentExpression(_)) {
            doc::parens(tsv_ts::build_expression_doc_with_comments(
                expr,
                self.source,
                Rc::clone(&self.interner),
                &config,
                self.comments,
                &self.line_breaks,
            ))
        } else {
            tsv_ts::build_expression_doc_with_comments(
                expr,
                self.source,
                Rc::clone(&self.interner),
                &config,
                self.comments,
                &self.line_breaks,
            )
        };

        // Apply remove_lines() only in INLINE contexts to prevent the condition
        // from being the first thing to break when there's other content on the line.
        // In multiline contexts, the condition is on its own line and can wrap naturally.
        let expr_doc = if in_multiline_context {
            expr_doc
        } else {
            doc::remove_lines(expr_doc)
        };

        // Build docs for trailing comments
        let trailing_docs: Vec<Doc> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(Self::build_trailing_js_comment_doc)
                .collect();

        // Combine: leading + expr + trailing
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            doc::concat(parts)
        }
    }
}
