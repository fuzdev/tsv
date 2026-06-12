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

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::arena::DocId;

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

impl<'a> Printer<'a> {
    /// Build a doc for an entire fragment (sequence of nodes)
    ///
    /// This is the entry point for doc-based inline content formatting.
    /// The resulting doc includes all nodes, so fits() checks will
    /// naturally account for siblings.
    pub(crate) fn build_fragment_doc(&self, fragment: &Fragment) -> DocId {
        self.build_nodes_doc(&fragment.nodes)
    }

    /// Build a doc for a slice of fragment nodes
    ///
    /// Accepts a slice directly, avoiding Fragment allocation when caller
    /// already has a `&[FragmentNode]`.
    pub(crate) fn build_nodes_doc(&self, nodes: &[FragmentNode]) -> DocId {
        self.build_nodes_doc_with_context(nodes, false)
    }

    /// Build a doc for nodes with context about text trimming
    ///
    /// # Parameters
    /// - `trim_text`: If true, trim text completely (block context).
    ///   If false, preserve single space at boundaries (inline context).
    pub(crate) fn build_nodes_doc_with_context(
        &self,
        nodes: &[FragmentNode],
        trim_text: bool,
    ) -> DocId {
        let mut docs: Vec<DocId> = Vec::new();
        let mut prettier_ignore_next = false;
        for (i, node) in nodes.iter().enumerate() {
            // prettier-ignore: skip whitespace, emit raw source for ignored node
            if prettier_ignore_next {
                if let FragmentNode::Text(text) = node
                    && text.raw.is_whitespace_only()
                {
                    continue;
                }
                let raw = node.span().extract(self.source);
                docs.push(self.d().text_owned(raw.to_string()));
                prettier_ignore_next = false;
                continue;
            }
            if let FragmentNode::Comment(comment) = node
                && comment.content.trim() == "prettier-ignore"
            {
                if let Some(doc) = self.build_fragment_node_doc_with_context(node, trim_text) {
                    docs.push(doc);
                }
                prettier_ignore_next = true;
                continue;
            }

            // For control flow blocks, check if there's preceding breakable content
            let is_control_flow = matches!(
                node,
                FragmentNode::IfBlock(_)
                    | FragmentNode::EachBlock(_)
                    | FragmentNode::AwaitBlock(_)
                    | FragmentNode::KeyBlock(_)
            );
            let doc = if is_control_flow {
                let has_preceding_breakable = nodes[..i].iter().any(|n| {
                    matches!(
                        n,
                        FragmentNode::ExpressionTag(_)
                            | FragmentNode::Element(_)
                            | FragmentNode::SpecialElement(_)
                            | FragmentNode::HtmlTag(_)
                            | FragmentNode::RenderTag(_)
                    )
                });
                self.build_fragment_node_doc_with_preceding_context(
                    node,
                    trim_text,
                    has_preceding_breakable,
                )
            } else {
                self.build_fragment_node_doc_with_context(node, trim_text)
            };
            if let Some(doc) = doc {
                docs.push(doc);
            }
        }

        if docs.is_empty() {
            self.d().empty()
        } else {
            self.d().concat(&docs)
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
    ) -> DocId {
        let d = self.d();
        if nodes.is_empty() {
            return d.empty();
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
            return d.empty();
        }

        let trimmed_nodes = &nodes[start_idx..end_idx];
        let trimmed_len = trimmed_nodes.len();

        // Build docs matching prettier-plugin-svelte's structure:
        // - Each text node → fill([word, line, word, ...])
        // - Inline elements → wrapped with group([line, element]) or group([element, line])
        //   depending on surrounding whitespace
        let mut child_docs: Vec<DocId> = Vec::new();
        let mut handle_whitespace_of_prev_text = false;

        let mut prettier_ignore_next = false;
        for (i, node) in trimmed_nodes.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == trimmed_len - 1;

            // prettier-ignore: skip whitespace, emit raw source for ignored node
            if prettier_ignore_next {
                if let FragmentNode::Text(text) = node
                    && text.raw.is_whitespace_only()
                {
                    continue;
                }
                let raw = node.span().extract(self.source);
                child_docs.push(d.text_owned(raw.to_string()));
                handle_whitespace_of_prev_text = false;
                prettier_ignore_next = false;
                continue;
            }
            if let FragmentNode::Comment(comment) = node
                && comment.content.trim() == "prettier-ignore"
            {
                prettier_ignore_next = true;
            }

            if let FragmentNode::Text(text) = node {
                let prev_node = if i > 0 {
                    Some(&trimmed_nodes[i - 1])
                } else {
                    None
                };
                let prev_is_inline = prev_node.is_some_and(Self::is_inline_content);
                let prev_is_tag = prev_node.is_some_and(Self::is_expression_tag);
                let next_node = if i + 1 < trimmed_len {
                    Some(&trimmed_nodes[i + 1])
                } else {
                    None
                };
                let next_is_inline = next_node.is_some_and(Self::is_inline_content);
                let next_is_tag = next_node.is_some_and(Self::is_expression_tag);
                let position =
                    SiblingPosition::new(is_first, is_last, prev_is_inline, next_is_inline);
                self.handle_text_child(
                    &text.raw,
                    position,
                    trim_boundaries,
                    prev_is_tag,
                    next_is_tag,
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
                            | FragmentNode::HtmlTag(_)
                            | FragmentNode::RenderTag(_)
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
            d.empty()
        } else {
            d.concat(&child_docs)
        }
    }

    /// Check if a node is inline content (non-text node that participates in fill).
    ///
    /// This is NOT the same as `!tsv_html::is_block_element` which checks HTML classification.
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

    /// Check if a node is an expression-like tag (ExpressionTag, HtmlTag, RenderTag).
    ///
    /// These tags use the leading/trailing line fill approach instead of group wrapping,
    /// because group wrapping forces line breaks after multiline expressions.
    fn is_expression_tag(node: &FragmentNode) -> bool {
        matches!(
            node,
            FragmentNode::ExpressionTag(_) | FragmentNode::HtmlTag(_) | FragmentNode::RenderTag(_)
        )
    }

    /// Handle a text child node - matches prettier-plugin-svelte's handleTextChild
    #[allow(clippy::too_many_arguments)]
    fn handle_text_child(
        &self,
        raw: &str,
        position: SiblingPosition,
        trim_boundaries: bool,
        prev_is_tag: bool,
        next_is_tag: bool,
        child_docs: &mut Vec<DocId>,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let d = self.d();
        *handle_whitespace_of_prev_text = false;

        // ASCII whitespace class `[\t\n\f\r ]`, matching prettier-plugin-svelte's
        // text split (`splitTextToDocs`). A leading/trailing non-breaking space (or
        // any non-ASCII whitespace) is content, so a node made only of those is not
        // whitespace-only and is preserved verbatim.
        let has_leading_ws = raw.starts_with(|c: char| c.is_ascii_whitespace());
        let has_trailing_ws = raw.ends_with(|c: char| c.is_ascii_whitespace());
        let trimmed = raw.trim_ascii();

        let is_first = position.is_first();
        let is_last = position.is_last();

        if trimmed.is_empty() {
            // Whitespace-only text: behavior depends on position and parent type
            if (is_first || is_last) && !trim_boundaries {
                // Boundary whitespace in inline element: always output as single space
                // (normalizes both `<span> text` and `<span>\n\ttext` to `<span> text`)
                child_docs.push(d.text(" "));
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
        // trim the leading ws and wrap the previous element with a trailing line.
        //
        // For last child: match prettier's handleTextChild early return for idx===last
        // which does NOT wrap the previous element. Instead, the fill starts with a
        // line() element so it can continue on the expression's continuation line
        // (line() → space in flat mode) or break to a new line (line() → newline).
        //
        // For non-last child with breaking prev: skip wrapping because
        // group([breaking_element, line()]) forces the line() to break too,
        // incorrectly separating the closing tag from trailing text.
        let prev_will_break = child_docs.last().is_some_and(|&doc| d.will_break(doc));
        let mut leading_line = false;
        if has_leading_ws && !is_first && position.prev_is_inline() {
            if prev_is_tag && (is_last || !prev_will_break) {
                // Text after expression/html/render tag: use leading_line in fill instead
                // of wrapping the tag with group([tag, line()]). The group approach forces
                // line() to break after multiline tags, pushing text to a new line.
                // leading_line lets fill continue on the tag's continuation line
                // (line() → space in flat, newline in break).
                trim_left = true;
                add_leading_space = false;
                leading_line = true;
            } else if is_last && prev_will_break {
                // Last child after breaking element (e.g. multiline attrs):
                // skip wrapping because group([breaking_element, line()]) forces
                // line() to break too, incorrectly separating closing tag from text.
                // Note: non-last text after a breaking tag (prev_is_tag && !is_last
                // && prev_will_break) also falls through without action — group()
                // would force line() to break, and leading_line is only for
                // non-breaking continuation. The text's leading ws handles spacing.
            } else if !prev_will_break {
                trim_left = true;
                add_leading_space = false; // line() handles the space
                // Pop the last doc (the inline element) and wrap it with trailing line
                if let Some(last_doc) = child_docs.pop() {
                    let line = d.line();
                    let inner = d.concat(&[last_doc, line]);
                    child_docs.push(d.group(inner));
                }
            }
        }

        // If text ends with whitespace and next is inline element:
        // trim the trailing ws and either use trailing_line in fill or set flag for next element.
        //
        // For tags (ExpressionTag, HtmlTag, RenderTag): use trailing_line in the fill.
        // group([line, expr]) wrapping forces a newline before multiline expressions;
        // trailing_line lets fill decide whether to break (same approach as leading_line).
        //
        // For non-tag inline elements: set handle_whitespace_of_prev_text so the next
        // element gets wrapped with group([line, element]).
        let mut trailing_line = false;
        if has_trailing_ws && !is_last && position.next_is_inline() {
            if is_first || next_is_tag {
                // First child or middle child before tag: trailing line in fill
                add_trailing_space = false;
                trailing_line = true;
                if !is_first {
                    trim_right = true;
                }
            } else if !is_first {
                // Middle child before non-tag inline element: wrap next element
                trim_right = true;
                add_trailing_space = false;
                *handle_whitespace_of_prev_text = true;
            }
        }

        // Build fill for this text node's words.
        // leading_line: fill starts with line() (text after expression tag)
        // trailing_line: fill ends with line() (text before expression tag or first-child)
        if add_leading_space {
            child_docs.push(d.text(" "));
        }
        if let Some(fill_doc) = self.build_text_fill_doc_trimmed(
            raw,
            trim_left,
            trim_right,
            leading_line,
            trailing_line,
        ) {
            child_docs.push(fill_doc);
        }
        if add_trailing_space {
            child_docs.push(d.text(" "));
        }
    }

    /// Handle an inline child element - matches prettier-plugin-svelte's handleInlineChild
    fn handle_inline_child(
        &self,
        node: &FragmentNode,
        child_docs: &mut Vec<DocId>,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let d = self.d();
        if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, false) {
            if *handle_whitespace_of_prev_text {
                // Previous text had trailing whitespace - wrap element with leading line
                let line = d.line();
                let inner = d.concat(&[line, node_doc]);
                child_docs.push(d.group(inner));
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
    pub(super) fn build_nodes_doc_multiline(&self, nodes: &[FragmentNode]) -> DocId {
        let d = self.d();
        if nodes.is_empty() {
            return d.empty();
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
            return d.empty();
        }

        let trimmed_nodes = &nodes[start_idx..end_idx];

        // Check if we should split expressions to separate lines
        // This matches Prettier: multiple expressions with whitespace between them
        // get their own lines, but expressions with semantic text stay together
        let should_split_expressions = self.should_split_expressions_in_nodes(trimmed_nodes);

        // Use separate current_line and completed lines vectors to avoid unwrap calls.
        // The pattern: build current line, then push to lines when starting a new line.
        let mut lines: Vec<Vec<DocId>> = Vec::new();
        let mut current_line: Vec<DocId> = Vec::new();

        // Track if previous text ended with space (for inline-before-block pattern)
        let mut prev_text_has_trailing_space = false;

        let mut prettier_ignore_next = false;
        for (i, node) in trimmed_nodes.iter().enumerate() {
            // prettier-ignore: skip whitespace, emit raw source for ignored node
            if prettier_ignore_next {
                if let FragmentNode::Text(text) = node
                    && text.raw.is_whitespace_only()
                {
                    continue;
                }
                let raw = node.span().extract(self.source);
                let raw_doc = d.text_owned(raw.to_string());
                if !current_line.is_empty() {
                    lines.push(std::mem::take(&mut current_line));
                }
                current_line.push(raw_doc);
                // Don't close the line — let subsequent inline content stay on same line
                prev_text_has_trailing_space = false;
                prettier_ignore_next = false;
                continue;
            }
            if let FragmentNode::Comment(comment) = node
                && comment.content.trim() == "prettier-ignore"
            {
                prettier_ignore_next = true;
            }

            let is_block = self.is_block_fragment_node(node);

            if is_block {
                // Block element - check if it should stay on the same line as previous content
                //
                // Control flow blocks ({#if}, {#each}, etc.) can hug when:
                // 1. Directly adjacent to previous node (no whitespace between)
                // 2. Previous text had trailing space AND on same source line
                //
                // HTML block elements (<div>, <p>, etc.) can stay inline when:
                // - Block is the second trimmed child (i==1) AND first child is content text
                //   with trailing space AND on same source line.
                //   This matches prettier: only first-child text keeps blocks inline.
                //   When non-text nodes (elements, expressions) precede the whitespace,
                //   prettier adds softline + breakParent to force a line break.
                let is_control_flow = super::helpers::is_control_flow_block(node);
                let keep_inline_with_prev = if i > 0 {
                    let prev_span = trimmed_nodes[i - 1].span();
                    let curr_span = node.span();
                    // Directly adjacent: prev.end == curr.start (no text node between)
                    // Only control flow blocks can hug when directly adjacent
                    let directly_adjacent = is_control_flow && prev_span.end == curr_span.start;
                    // Previous text had trailing space and on same line
                    let text_with_space = prev_text_has_trailing_space
                        && tsv_lang::printing::spans_on_same_line(
                            self.source,
                            prev_span,
                            curr_span,
                        );
                    // For HTML block elements (not control flow), only keep inline
                    // when the block is the second child and the first child is
                    // content text. This matches prettier where forceBreakContent
                    // (breakParent) forces all softlines to break, and only
                    // first-child text avoids getting a softline before the block.
                    let text_with_space = if !is_control_flow && text_with_space {
                        i == 1
                            && matches!(
                                &trimmed_nodes[0],
                                FragmentNode::Text(t) if !t.raw.is_whitespace_only()
                            )
                    } else {
                        text_with_space
                    };
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
                            current_line.push(d.text(" "));
                        }
                        // Whitespace-only text counts as trailing space for inline-before-block
                        prev_text_has_trailing_space = true;
                    }
                } else if text.raw.contains('\n') {
                    // Text with newlines - split into lines at structural boundaries.
                    //
                    // Per-newline: content-flow (both sides have content) → collapse
                    // by joining parts with space into one fill doc for proper wrapping.
                    // Structural (either side whitespace-only) → preserve as line break.
                    //
                    // First pass: identify content-flow newlines and join those parts.
                    let parts: Vec<&str> = text.raw.split('\n').collect();
                    let mut merged_parts: Vec<String> = Vec::new();

                    for (idx, part) in parts.iter().enumerate() {
                        if idx > 0 {
                            let prev_has_content =
                                parts[idx - 1].contains(|c: char| !c.is_whitespace());
                            let curr_has_content = part.contains(|c: char| !c.is_whitespace());

                            if prev_has_content && curr_has_content {
                                // Content-flow: join with previous merged part
                                if let Some(last) = merged_parts.last_mut() {
                                    // Trim trailing ws from prev, add space, add curr trimmed
                                    let prev_trimmed = last.trim_end().to_string();
                                    *last = format!("{prev_trimmed} {}", part.trim_start());
                                }
                                continue;
                            }
                        }
                        merged_parts.push((*part).to_string());
                    }

                    // Second pass: process merged parts with original structural logic
                    let line_was_empty_before = current_line.is_empty();
                    let mut consecutive_blank_count = 0;
                    for (idx, part) in merged_parts.iter().enumerate() {
                        let should_skip =
                            idx == 0 || (idx == 1 && line_was_empty_before && parts[0].is_empty());
                        if !should_skip {
                            let is_pushing_blank = current_line.is_empty();
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
                            current_line.push(d.text(" "));
                        }
                        // Use fill for word-level breaking
                        if let Some(fill_doc) =
                            self.build_text_fill_doc_trimmed(part, true, true, false, false)
                        {
                            current_line.push(fill_doc);
                            consecutive_blank_count = 0;

                            let remaining_parts_have_content =
                                merged_parts[idx + 1..].iter().any(|p| !p.trim().is_empty());
                            let is_last_node = i == trimmed_nodes.len() - 1;
                            if part.ends_with(char::is_whitespace)
                                && (remaining_parts_have_content || !is_last_node)
                            {
                                current_line.push(d.text(" "));
                            }
                        }
                    }
                    // Last part's trailing space affects next node
                    prev_text_has_trailing_space = merged_parts
                        .last()
                        .is_some_and(|p| p.ends_with(char::is_whitespace));
                } else {
                    // No newlines - add to current line with fill for word-level breaking
                    let has_leading = text.raw.starts_with(char::is_whitespace);
                    let has_trailing = text.raw.ends_with(char::is_whitespace);

                    // Add leading space if source has it AND line has content
                    if has_leading && !current_line.is_empty() {
                        current_line.push(d.text(" "));
                    }

                    // Use fill for multi-word text to enable word-level line breaking
                    if let Some(fill_doc) =
                        self.build_text_fill_doc_trimmed(&text.raw, true, true, false, false)
                    {
                        current_line.push(fill_doc);
                    }

                    // Add trailing space if source has it, but NOT for the last node
                    // (boundary whitespace at end of fragment should be trimmed)
                    let is_last = i == trimmed_nodes.len() - 1;
                    if has_trailing && !is_last {
                        current_line.push(d.text(" "));
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
                docs.push(d.literalline());
            } else {
                // Content line - use hardline before it (except first)
                if !docs.is_empty() {
                    docs.push(d.hardline());
                }
                docs.push(d.concat(&line_docs));
                found_first_content = true;
            }
        }

        if docs.is_empty() {
            d.empty()
        } else {
            d.concat(&docs)
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
            FragmentNode::SpecialElement(el) => el.kind.is_block(),
            _ => super::helpers::is_control_flow_block(node),
        }
    }

    /// Check if fragment content should force breaking due to block elements.
    ///
    /// Matches prettier's `forceBreakContent`: when there are multiple non-whitespace
    /// children and at least one is a block element, content should break.
    /// This forces the multiline path even for "inline" Svelte block bodies.
    pub(super) fn fragment_should_force_break_content(&self, nodes: &[FragmentNode]) -> bool {
        let non_ws_count = nodes
            .iter()
            .filter(|n| !n.is_whitespace_only_text())
            .count();
        non_ws_count > 1 && nodes.iter().any(|n| self.is_block_fragment_node(n))
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
    ) -> Option<DocId> {
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
    ) -> Option<DocId> {
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
    ) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, trim_text, false, has_preceding_breakable)
    }

    fn build_fragment_node_doc_impl(
        &self,
        node: &FragmentNode,
        trim_text: bool,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Option<DocId> {
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
    fn build_text_doc(&self, text: &internal::Text, trim_completely: bool) -> Option<DocId> {
        let trimmed = text.raw.trim();
        if trimmed.is_empty() {
            // Pure whitespace: collapse to single space only in inline context
            if !trim_completely && text.raw.contains(char::is_whitespace) {
                Some(self.d().text(" "))
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
    fn build_text_fill_doc(&self, raw: &str, trim_completely: bool) -> Option<DocId> {
        self.build_text_fill_doc_trimmed(raw, trim_completely, trim_completely, false, false)
    }

    /// Build a fill doc for text with separate control over leading/trailing trimming.
    ///
    /// Used by build_nodes_doc_trimmed where first node trims leading, last trims trailing.
    /// When `leading_line` or `trailing_line` is true, the fill uses `line()` at the
    /// boundary instead of wrapping the adjacent expression in a group. This lets fill
    /// continue on the expression's continuation line rather than forcing a newline.
    fn build_text_fill_doc_trimmed(
        &self,
        raw: &str,
        trim_leading: bool,
        trim_trailing: bool,
        leading_line: bool,
        trailing_line: bool,
    ) -> Option<DocId> {
        let d = self.d();
        // ASCII whitespace only (matching the word split below): a boundary space
        // is emitted only when the split consumed an ASCII-whitespace run. A
        // boundary non-breaking space (U+00A0 / U+202F) stays attached to its word
        // and must not get a spurious regular space prepended/appended.
        let has_leading_ws = raw.starts_with(|c: char| c.is_ascii_whitespace());
        let has_trailing_ws = raw.ends_with(|c: char| c.is_ascii_whitespace());

        // Split on ASCII whitespace only and collect non-empty words. Prettier's
        // splitTextToDocs splits on `/[\t\n\f\r ]+/`, so non-breaking spaces
        // (U+00A0) and narrow non-breaking spaces (U+202F) stay attached to their
        // words — they are not break points and are preserved verbatim. Rust's
        // `split_whitespace` is Unicode-aware and would split (and thus drop) them.
        let words: Vec<&str> = raw.split_ascii_whitespace().collect();
        if words.is_empty() {
            return None;
        }

        // Single word: return text (with boundary handling)
        if words.len() == 1 && !leading_line {
            if trailing_line && has_trailing_ws {
                let word = if !trim_leading && has_leading_ws {
                    format!(" {}", words[0])
                } else {
                    words[0].to_string()
                };
                let parts = [d.text_owned(word), d.line()];
                return Some(d.fill(&parts));
            }
            let mut result = String::new();
            if !trim_leading && has_leading_ws {
                result.push(' ');
            }
            result.push_str(words[0]);
            if !trim_trailing && has_trailing_ws {
                result.push(' ');
            }
            return Some(d.text_owned(result));
        }

        // Multiple words (or leading_line): build fill parts
        // leading_line: [line, word, line, word, ...] — text after expression tag
        // trailing_line: [..., word, line] — text before expression tag
        // both: [line, word, line, ..., word, line]
        let prepend_space = !leading_line && !trim_leading && has_leading_ws;
        let append_space = !trim_trailing && has_trailing_ws && !trailing_line;
        let mut parts = Vec::with_capacity(words.len() * 2 + 2);

        if leading_line {
            parts.push(d.line());
        }

        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                parts.push(d.line());
            }
            if i == 0 && prepend_space {
                let mut s = String::with_capacity(1 + word.len());
                s.push(' ');
                s.push_str(word);
                parts.push(d.text_owned(s));
            } else if i == words.len() - 1 && append_space {
                let mut s = String::with_capacity(word.len() + 1);
                s.push_str(word);
                s.push(' ');
                parts.push(d.text_owned(s));
            } else {
                parts.push(d.text_owned((*word).to_string()));
            }
        }

        if trailing_line && has_trailing_ws {
            parts.push(d.line());
        }

        Some(d.fill(&parts))
    }

    //
    // Comment nodes
    //

    /// Build a doc for an HTML comment
    pub(crate) fn build_html_comment_doc(&self, comment: &internal::HtmlComment) -> DocId {
        let d = self.d();
        d.concat(&[
            d.text("<!--"),
            d.text_owned(comment.content.clone()),
            d.text("-->"),
        ])
    }

    //
    // Helper methods
    //
}
