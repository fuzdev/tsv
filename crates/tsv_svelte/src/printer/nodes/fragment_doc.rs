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
use tsv_lang::doc::GroupId;
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

/// Helper to wrap body content in indent(), with optional hardline for leading whitespace.
///
/// This pattern is used consistently across all block types (if, each, await, snippet, key)
/// to ensure proper indentation of nested content when it breaks across lines.
fn indent_body(printer: &Printer, body_doc: DocId, has_leading_ws: bool) -> DocId {
    if has_leading_ws {
        let hardline = printer.d().hardline();
        let inner = printer.d().concat(&[hardline, body_doc]);
        printer.d().indent(inner)
    } else {
        printer.d().indent(body_doc)
    }
}

/// Build an await block section body with newline-based whitespace detection.
///
/// Returns `(body_doc, has_trailing)` — the indented body doc and whether the
/// fragment had trailing whitespace (needed for section separator logic).
fn build_await_section_body(printer: &Printer, fragment: &Fragment) -> (DocId, bool) {
    let has_leading = printer.fragment_has_leading_ws(fragment);
    let has_trailing = printer.fragment_has_trailing_ws(fragment);
    let force_break = printer.fragment_should_force_break_content(&fragment.nodes);
    let is_inline = !has_leading && !has_trailing && !force_break;
    let body_doc = if is_inline {
        printer.build_fragment_doc(fragment)
    } else {
        printer.build_nodes_doc_multiline(&fragment.nodes)
    };
    (indent_body(printer, body_doc, has_leading), has_trailing)
}

/// Build `indent([line, body_doc])` for space-only await blocks.
///
/// In flat mode (fits): ` body_doc` (space + content)
/// In break mode (exceeds print width): newline + indent + body_doc
fn indent_body_soft(printer: &Printer, body_doc: DocId) -> DocId {
    let line = printer.d().line();
    let inner = printer.d().concat(&[line, body_doc]);
    printer.d().indent(inner)
}

/// Split a raw parameter string at top-level commas, returning trimmed param strings.
///
/// Handles nesting for `()`, `[]`, `{}`, `<>`, and string literals (`'...'`, `"..."`).
/// E.g., `"a: A | 'x', b: B<C, D>"` → `["a: A | 'x'", "b: B<C, D>"]`.
fn split_raw_params_at_commas(raw: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b',' if depth == 0 => {
                result.push(raw[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let last = raw[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
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
    fn fragment_should_force_break_content(&self, nodes: &[FragmentNode]) -> bool {
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
    pub(crate) fn build_if_block_doc(&self, block: &internal::IfBlock) -> DocId {
        self.build_if_block_doc_with_context(block, false)
    }

    /// Build if block doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces expand.
    pub(crate) fn build_if_block_doc_with_context(
        &self,
        block: &internal::IfBlock,
        in_multiline_context: bool,
    ) -> DocId {
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
    ) -> DocId {
        let d = self.d();
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
        // Force non-inline when block elements among multiple children
        // (matches prettier's forceBreakContent + breakParent)
        let force_break = self.fragment_should_force_break_content(&block.consequent.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.consequent)
        } else {
            self.build_nodes_doc_multiline(&block.consequent.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation
        let indented_body = indent_body(self, body_doc, has_leading);

        let mut parts = vec![d.text("{#if "), expr_doc, d.text("}"), indented_body];

        // Handle alternate (else/else-if) and determine final trailing status
        let final_has_trailing = if let Some(alt) = &block.alternate {
            // Add break before alternate only if consequent has trailing ws
            if has_trailing {
                parts.push(d.hardline());
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
            parts.push(d.hardline());
        }

        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Check if a fragment can be flattened to an else-if.
    ///
    /// Returns the single IfBlock if the fragment contains exactly one IfBlock
    /// (plus optional whitespace) and can be flattened. Returns None if the
    /// fragment has multiple IfBlocks or other content that prevents flattening.
    pub(super) fn get_flattenable_else_if(alt: &Fragment) -> Option<&internal::IfBlock> {
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
    ) -> DocId {
        let d = self.d();
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
            let force_break = self.fragment_should_force_break_content(&else_if.consequent.nodes);
            let is_inline = !has_leading && !has_trailing && !force_break;
            let parent_inline = !parent_has_leading && !parent_has_trailing;
            let is_both_inline = is_inline && parent_inline;

            // For inline: use regular fragment doc (preserves spaces)
            // For multiline: use multiline doc (preserves line structure)
            let body_doc = if is_both_inline {
                self.build_fragment_doc(&else_if.consequent)
            } else {
                self.build_nodes_doc_multiline(&else_if.consequent.nodes)
            };

            let indented_body = indent_body(self, body_doc, has_leading);

            let mut parts = vec![d.text("{:else if "), expr_doc, d.text("}"), indented_body];

            // Handle nested alternate or trailing
            if let Some(nested_alt) = &else_if.alternate {
                // Add break before next alternate only if this branch has trailing ws
                if has_trailing {
                    parts.push(d.hardline());
                }
                parts.push(self.build_if_alternate_doc(
                    nested_alt,
                    has_leading,
                    has_trailing,
                    in_multiline_context,
                ));
            }

            return d.concat(&parts);
        }

        // Plain {:else}
        let (has_leading, has_trailing) = self.fragment_ws_status(alt, in_multiline_context);
        let force_break = self.fragment_should_force_break_content(&alt.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;
        let parent_inline = !parent_has_leading && !parent_has_trailing;
        let is_both_inline = is_inline && parent_inline;

        // For inline: use regular fragment doc (preserves spaces)
        // For multiline: use multiline doc
        let body_doc = if is_both_inline {
            self.build_nodes_doc(&alt.nodes)
        } else {
            self.build_nodes_doc_multiline(&alt.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        d.concat(&[d.text("{:else}"), indented_body])
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
    pub(crate) fn build_each_block_doc(&self, block: &internal::EachBlock) -> DocId {
        self.build_each_block_doc_with_context(block, false)
    }

    /// Build each block doc with multiline context awareness.
    pub(crate) fn build_each_block_doc_with_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_each_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build each block doc with full context (multiline + preceding content).
    fn build_each_block_doc_with_full_context(
        &self,
        block: &internal::EachBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
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

        let mut opening = vec![d.text("{#each "), expr_doc];

        // Pattern (context) - only add " as " when there's a context or index
        if let Some(context) = &block.context {
            opening.push(d.text(" as "));
            // Format pattern through TypeScript formatter for proper whitespace normalization
            let pattern_doc = self.build_pattern_doc(context);
            opening.push(pattern_doc);
            if let Some(index) = &block.index {
                opening.push(d.text(", "));
                opening.push(d.text_owned(index.clone()));
            }
        } else if let Some(index) = &block.index {
            // No context but has index: ", i" pattern
            opening.push(d.text(", "));
            opening.push(d.text_owned(index.clone()));
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
                self.build_ts_expression_doc(key)
            };
            opening.push(d.text(" ("));
            opening.push(key_doc);
            opening.push(d.text(")"));
        }

        opening.push(d.text("}"));

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.body, in_multiline_context);
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(&block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        let opening_concat = d.concat(&opening);
        let mut parts = vec![opening_concat, indented_body];

        // Determine final trailing status (from body or fallback if present)
        let final_has_trailing = if let Some(fallback) = &block.fallback {
            // Add break before {:else} only if body has trailing ws
            if has_trailing {
                parts.push(d.hardline());
            }

            let (fallback_has_leading, fallback_has_trailing) =
                self.fragment_ws_status(fallback, in_multiline_context);
            let fallback_force_break = self.fragment_should_force_break_content(&fallback.nodes);
            let fallback_inline =
                !fallback_has_leading && !fallback_has_trailing && !fallback_force_break;
            let is_both_inline = fallback_inline && is_inline;

            parts.push(d.text("{:else}"));

            // For inline: use regular fragment doc
            // For multiline: use multiline doc
            let fallback_doc = if is_both_inline {
                self.build_fragment_doc(fallback)
            } else {
                self.build_nodes_doc_multiline(&fallback.nodes)
            };

            let indented_fallback =
                indent_body(self, fallback_doc, fallback_has_leading || has_leading);
            parts.push(indented_fallback);

            fallback_has_trailing
        } else {
            has_trailing
        };

        // Add endline before {/each} only if final has trailing whitespace
        if final_has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }

    /// Build a doc for an await block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock) -> DocId {
        self.build_await_block_doc_with_context(block, false)
    }

    /// Build await block doc with multiline context awareness.
    pub(crate) fn build_await_block_doc_with_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_await_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build await block doc with full context (multiline + preceding content).
    fn build_await_block_doc_with_full_context(
        &self,
        block: &internal::AwaitBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_expression_doc_for_block(
            &block.expression,
            block.opening_tag_span.start + 8, // after "{#await "
            block.opening_tag_span.end - 1,   // before "}"
            8,                                // "{#await " = 8 chars
            allow_wrapping || in_multiline_context,
        );

        let mut parts = vec![d.text("{#await "), expr_doc];

        // Shorthand: {#await expr then value}...{/await}
        // Also handles: {#await expr then value}...{:catch error}...{/await}
        if let (Some(value), None) = (&block.value, &block.pending) {
            parts.push(d.text(" then "));
            parts.push(self.build_pattern_doc(value));
            parts.push(d.text("}"));

            // Check if any section has space-only whitespace
            let has_space_only = block
                .then
                .as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
                || block
                    .catch
                    .as_ref()
                    .is_some_and(|f| self.fragment_has_space_only_ws(f));

            if has_space_only {
                if let Some(then_block) = &block.then {
                    let body_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                if let Some(error) = &block.error {
                    parts.push(d.line());
                    parts.push(d.text("{:catch "));
                    parts.push(self.build_pattern_doc(error));
                    parts.push(d.text("}"));
                } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                    parts.push(d.line());
                    parts.push(d.text("{:catch}"));
                }
                if let Some(catch_block) = &block.catch {
                    let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                parts.push(d.line());
                parts.push(d.text("{/await}"));
                let concat = d.concat(&parts);
                return d.group(concat);
            }

            let mut prev_has_trailing = false;
            if let Some(then_block) = &block.then {
                let (body, trailing) = build_await_section_body(self, then_block);
                parts.push(body);
                prev_has_trailing = trailing;
            }

            // Optional {:catch} continuation after then-shorthand
            if block.catch.is_some() {
                if let Some(error) = &block.error {
                    if prev_has_trailing {
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("{:catch "));
                    parts.push(self.build_pattern_doc(error));
                    parts.push(d.text("}"));
                } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                    if prev_has_trailing {
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("{:catch}"));
                }
                if let Some(catch_block) = &block.catch {
                    let (body, trailing) = build_await_section_body(self, catch_block);
                    parts.push(body);
                    prev_has_trailing = trailing;
                }
            }

            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{/await}"));
            return d.concat(&parts);
        }

        // Shorthand: {#await expr catch error}
        if block.pending.is_none()
            && block.value.is_none()
            && let Some(error) = &block.error
        {
            parts.push(d.text(" catch "));
            parts.push(self.build_pattern_doc(error));
            parts.push(d.text("}"));

            // Check if any section has space-only whitespace
            let has_space_only = block
                .catch
                .as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f));

            if has_space_only {
                if let Some(catch_block) = &block.catch {
                    let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                    parts.push(indent_body_soft(self, body_doc));
                }
                parts.push(d.line());
                parts.push(d.text("{/await}"));
                let concat = d.concat(&parts);
                return d.group(concat);
            }

            if let Some(catch_block) = &block.catch {
                let (body, has_trailing) = build_await_section_body(self, catch_block);
                parts.push(body);
                if has_trailing {
                    parts.push(d.hardline());
                }
            }
            parts.push(d.text("{/await}"));
            return d.concat(&parts);
        }

        parts.push(d.text("}"));

        // Check if any section has space-only whitespace (spaces, no newlines).
        // Space-only await blocks stay inline when short but break when exceeding
        // print width. Use group+line so the renderer decides based on width.
        let has_space_only = [&block.pending, &block.then, &block.catch].iter().any(|f| {
            f.as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
        });

        if has_space_only {
            // Build all sections with line() docs — space in flat, newline in break.
            // All sections break together as a unit via the outer group.
            if let Some(pending) = &block.pending {
                let body_doc = self.build_nodes_doc_multiline(&pending.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            if let Some(value) = &block.value {
                parts.push(d.line());
                parts.push(d.text("{:then "));
                parts.push(self.build_pattern_doc(value));
                parts.push(d.text("}"));
            } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
                parts.push(d.line());
                parts.push(d.text("{:then}"));
            }
            if let Some(then_block) = &block.then {
                let body_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            if let Some(error) = &block.error {
                parts.push(d.line());
                parts.push(d.text("{:catch "));
                parts.push(self.build_pattern_doc(error));
                parts.push(d.text("}"));
            } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
                parts.push(d.line());
                parts.push(d.text("{:catch}"));
            }
            if let Some(catch_block) = &block.catch {
                let body_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                parts.push(indent_body_soft(self, body_doc));
            }

            parts.push(d.line());
            parts.push(d.text("{/await}"));
            let concat = d.concat(&parts);
            return d.group(concat);
        }

        // Track whitespace status for each section
        // The final section's trailing determines break before {/await}
        let mut final_has_trailing = false;
        let mut prev_has_trailing = false;

        // Pending - newline-based detection only (space-only handled above via group)
        if let Some(pending) = &block.pending {
            let (body, has_trailing) = build_await_section_body(self, pending);
            parts.push(body);
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Then
        if let Some(value) = &block.value {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:then "));
            parts.push(self.build_pattern_doc(value));
            parts.push(d.text("}"));
        } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:then}"));
        }
        if let Some(then_block) = &block.then {
            let (body, has_trailing) = build_await_section_body(self, then_block);
            parts.push(body);
            final_has_trailing = has_trailing;
            prev_has_trailing = has_trailing;
        }

        // Catch
        if let Some(error) = &block.error {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:catch "));
            parts.push(self.build_pattern_doc(error));
            parts.push(d.text("}"));
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            if prev_has_trailing {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:catch}"));
        }
        if let Some(catch_block) = &block.catch {
            let (body, has_trailing) = build_await_section_body(self, catch_block);
            parts.push(body);
            final_has_trailing = has_trailing;
        }

        // Add endline before {/await} only if final section has trailing whitespace
        if final_has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build a doc for a key block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock) -> DocId {
        self.build_key_block_doc_with_context(block, false)
    }

    /// Build key block doc with multiline context awareness.
    pub(crate) fn build_key_block_doc_with_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_key_block_doc_with_full_context(block, in_multiline_context, false)
    }

    /// Build key block doc with full context (multiline + preceding content).
    fn build_key_block_doc_with_full_context(
        &self,
        block: &internal::KeyBlock,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> DocId {
        let d = self.d();
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
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(&block.fragment.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.fragment)
        } else {
            self.build_nodes_doc_multiline(&block.fragment.nodes)
        };

        let indented_body = indent_body(self, body_doc, has_leading);

        let mut parts = vec![d.text("{#key "), expr_doc, d.text("}"), indented_body];

        // Add endline before {/key} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/key}"));
        d.concat(&parts)
    }

    /// Build a doc for a snippet block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    /// Opening tag uses group() for parameter wrapping when they exceed print width.
    pub(crate) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock) -> DocId {
        let d = self.d();
        // Extract snippet name from the identifier expression
        let name = self.extract_source_range(
            block.expression.span().start_usize(),
            block.expression.span().end_usize(),
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.body, false);
        let force_break = self.fragment_should_force_break_content(&block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // Type parameters (generics)
        let type_params_part = block.type_parameters.as_ref().map_or_else(
            || d.empty(),
            |tp| d.concat(&[d.text("<"), d.text_owned(tp.clone()), d.text(">")]),
        );

        // Parameters: use raw_parameters if available (preserves TypeScript types),
        // otherwise format individual params.
        // Split raw_parameters at top-level commas so each param gets its own
        // line when the group breaks (matching prettier's per-param wrapping).
        let params_docs: Vec<DocId> = if let Some(raw) = &block.raw_parameters {
            split_raw_params_at_commas(raw)
                .iter()
                .map(|s| d.text_owned(s.to_string()))
                .collect()
        } else {
            block
                .parameters
                .iter()
                .map(|p| {
                    // Format parameter through TypeScript formatter for proper normalization
                    self.build_ts_expression_doc_no_comments(p)
                })
                .collect()
        };

        // Build opening tag with group for parameter wrapping
        // When fits: {#snippet name(a, b, c)}
        // When wraps: {#snippet name(\n\ta,\n\tb,\n\tc,\n)}
        // Empty params: {#snippet name()} - no wrapping structure
        let opening_doc = if params_docs.is_empty() {
            // No params - simple structure that won't break incorrectly
            d.concat(&[
                d.text("{#snippet "),
                d.text_owned(name.to_string()),
                type_params_part,
                d.text("()}"),
            ])
        } else {
            // Build params doc with line() separators for wrapping
            // Pre-allocate: each param + separator (except first)
            let mut parts = Vec::with_capacity(params_docs.len() * 3);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(param_doc);
            }
            let params_doc = d.concat(&parts);

            let indent_sl = d.indent_softline(params_doc);
            let trailing = d.trailing_comma();
            let softline = d.softline();
            let inner = d.concat(&[
                d.text("{#snippet "),
                d.text_owned(name.to_string()),
                type_params_part,
                d.text("("),
                indent_sl,
                trailing,
                softline,
                d.text(")}"),
            ]);
            d.group(inner)
        };

        let mut parts = vec![opening_doc];

        // Body: inline hugs directly, multiline uses hardlines
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        parts.push(indent_body(self, body_doc, has_leading));

        // Add endline before {/snippet} only if trailing whitespace exists
        if has_trailing {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/snippet}"));
        d.concat(&parts)
    }

    //
    // Template tags
    //

    /// Build a doc for {@html expr}
    pub(crate) fn build_html_tag_doc(&self, tag: &internal::HtmlTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@html " (start + 7) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 7, // after "{@html "
            tag.span.end - 1,   // before "}"
        );

        // Assignment expressions need parens: {@html (a = b)}
        let expr_doc = if matches!(tag.expression, tsv_ts::Expression::AssignmentExpression(_)) {
            d.parens(expr_doc)
        } else {
            expr_doc
        };

        d.concat(&[d.text("{@html "), expr_doc, d.text("}")])
    }

    /// Build a doc for {@const declaration}
    ///
    /// Prettier formats @const as an AssignmentExpression, using its assignment
    /// layout to decide whether to break at `=`. Three layouts:
    /// - will_break: `{@const id = init}` (init has hardlines, keep together)
    /// - fluid: `{@const id = init}` or `{@const id =\n\tinit}` (marker group)
    /// - break-after-operator: `{@const id =\n\tinit}` (group with line at `=`)
    pub(crate) fn build_const_tag_doc(&self, tag: &internal::ConstTag) -> DocId {
        let d = self.d();
        let id_doc = self.build_ts_expression_doc_no_comments(&tag.id);
        // Build init with is_embedded_expression=false so binary chains use Grouped style
        // (not ContinuationIndent). The assignment layout handles indentation —
        // ContinuationIndent would double-indent continuation lines.
        let init_doc = self.build_const_init_doc(
            &tag.init,
            tag.init.span().start,
            tag.span.end - 1, // before "}"
        );

        // Choose layout matching prettier's assignment layout selection.
        if d.will_break(init_doc) {
            // Init has forced breaks (ternary, multi-line template, etc.)
            // Keep "= init" together — init's own breaks handle formatting.
            d.concat(&[
                d.text("{@const "),
                id_doc,
                d.text(" = "),
                init_doc,
                d.text("}"),
            ])
        } else if Self::const_should_break_after_op(&tag.init) {
            // Binary expressions, conditional with binary test, etc.
            // Break-after-operator: group with line at "=" so the doc printer
            // can break when the flat form exceeds print width.
            // Prettier ref: shouldBreakAfterOperator (assignment.js:196-259)
            let rhs = d.concat(&[d.line(), init_doc]);
            let rhs_indented = d.indent(rhs);
            let assignment = d.group(d.concat(&[d.text(" ="), rhs_indented, d.text("}")]));

            d.concat(&[d.text("{@const "), id_doc, assignment])
        } else {
            // Fluid layout: break at `=` only when the full line exceeds
            // print width. Uses indentIfBreak so the RHS is evaluated
            // independently — e.g., a ternary with identifier test stays
            // on the same line as `=` while its branches break below.
            // Prettier ref: "fluid" layout (assignment.js:59-67)
            d.concat(&[
                d.text("{@const "),
                id_doc,
                d.text(" ="),
                d.group_with_id(d.indent(d.line()), GroupId::Assignment),
                d.line_suffix_boundary(),
                d.indent_if_break(init_doc, GroupId::Assignment, false),
                d.text("}"),
            ])
        }
    }

    /// Check if a @const init expression needs break-after-operator layout.
    ///
    /// Matches prettier's `shouldBreakAfterOperator` for the expression types
    /// that appear in @const tags. Binary expressions and conditionals with
    /// binary tests break after `=`; other expressions use fluid layout.
    /// Prettier ref: assignment.js:196-226
    fn const_should_break_after_op(expr: &tsv_ts::Expression) -> bool {
        match expr {
            // Binary expressions break after `=`, UNLESS it's a logical expression
            // with a self-expanding RHS (non-empty object/array). In that case, the
            // RHS handles its own expansion: `= item || { ... }` not `=\n  item || {}`
            // Prettier ref: assignment.js:199 `isBinaryish && !shouldInlineLogicalExpression`
            tsv_ts::Expression::BinaryExpression(bin) => !Self::is_inline_logical(bin),
            tsv_ts::Expression::SequenceExpression(_) => true,
            tsv_ts::Expression::ConditionalExpression(cond) => {
                // Only break-after-operator when test is binary (and not inline logical).
                // Simple identifier tests (e.g., `cond ? a : b`) use fluid layout.
                // Prettier ref: assignment.js:216-219
                matches!(&*cond.test, tsv_ts::Expression::BinaryExpression(bin) if !Self::is_inline_logical(bin))
            }
            _ => false,
        }
    }

    /// Check if a binary expression is a logical expression with a self-expanding RHS.
    ///
    /// Logical operators (`&&`, `||`, `??`) with non-empty object or array on the
    /// right should NOT use break-after-operator — the RHS self-expands.
    /// Prettier ref: `shouldInlineLogicalExpression` (binaryish.js:361)
    fn is_inline_logical(bin: &tsv_ts::ast::internal::BinaryExpression) -> bool {
        if !bin.operator.is_logical() {
            return false;
        }
        match &*bin.right {
            tsv_ts::Expression::ObjectExpression(obj) => !obj.properties.is_empty(),
            tsv_ts::Expression::ArrayExpression(arr) => !arr.elements.is_empty(),
            _ => false,
        }
    }

    /// Build init expression doc for @const with assignment-appropriate config.
    ///
    /// Like `build_expression_with_comments_doc` but uses `first_line_offset = 0`
    /// so binary chains use Grouped style (not ContinuationIndent). The @const
    /// assignment layout handles indentation; ContinuationIndent would stack.
    fn build_const_init_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // mode defaults to Standalone: binary chains use Grouped style, not ContinuationIndent
        let embed = tsv_lang::EmbedContext {
            first_line_offset: 0,
            ..self.embed
        };

        let expr_doc = tsv_ts::build_expression_doc_with_comments(
            d,
            expr,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            &embed,
            self.comments,
            &self.line_breaks,
            tsv_ts::TsConfig::svelte(),
        );

        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            d.concat(&parts)
        }
    }

    /// Build a doc for {@debug vars}
    pub(crate) fn build_debug_tag_doc(&self, tag: &internal::DebugTag) -> DocId {
        let d = self.d();
        if tag.identifiers.is_empty() {
            d.text("{@debug}")
        } else {
            let idents: Vec<DocId> = tag
                .identifiers
                .iter()
                .map(|id| {
                    let name =
                        self.extract_source_range(id.span().start_usize(), id.span().end_usize());
                    d.text_owned(name.to_string())
                })
                .collect();

            d.concat(&[d.text("{@debug "), d.join(idents, ", "), d.text("}")])
        }
    }

    /// Build a doc for {@render snippet(args)}
    pub(crate) fn build_render_tag_doc(&self, tag: &internal::RenderTag) -> DocId {
        let d = self.d();
        // Build expression doc with surrounding comments
        // Span range: after "{@render " (start + 9) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 9, // after "{@render "
            tag.span.end - 1,   // before "}"
        );

        d.concat(&[d.text("{@render "), expr_doc, d.text("}")])
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
    pub(super) fn build_pattern_doc(&self, expr: &tsv_ts::Expression) -> DocId {
        let d = self.d();
        match expr {
            tsv_ts::Expression::ObjectPattern(obj) => {
                let mut parts = vec![d.text("{ ")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectPatternProperty::Property(p) => {
                            if p.shorthand {
                                // Shorthand: `{ k }` or `{ k = 1 }`
                                // Use build_pattern_doc for the value to handle
                                // AssignmentPattern (defaults) and preserve quotes
                                parts.push(self.build_pattern_doc(&p.value));
                            } else {
                                parts.push(self.build_ts_expression_doc_no_comments(&p.key));
                                parts.push(d.text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectPatternProperty::RestElement(r) => {
                            parts.push(d.text("..."));
                            parts.push(self.build_pattern_doc(&r.argument));
                        }
                    }
                }
                parts.push(d.text(" }"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ObjectExpression(obj) => {
                // Legacy AST - treat same as ObjectPattern
                let mut parts = vec![d.text("{ ")];
                for (i, prop) in obj.properties.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    match prop {
                        tsv_ts::ObjectProperty::Property(p) => {
                            if p.shorthand {
                                // Shorthand: `{ k }` or `{ k = 1 }`
                                parts.push(self.build_pattern_doc(&p.value));
                            } else {
                                parts.push(self.build_ts_expression_doc_no_comments(&p.key));
                                parts.push(d.text(": "));
                                parts.push(self.build_pattern_doc(&p.value));
                            }
                        }
                        tsv_ts::ObjectProperty::SpreadElement(s) => {
                            parts.push(d.text("..."));
                            parts.push(self.build_pattern_doc(&s.argument));
                        }
                    }
                }
                parts.push(d.text(" }"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ArrayPattern(arr) => {
                let mut parts = vec![d.text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(d.text("]"));
                d.concat(&parts)
            }
            tsv_ts::Expression::ArrayExpression(arr) => {
                // Legacy AST - treat same as ArrayPattern
                let mut parts = vec![d.text("[")];
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        parts.push(d.text(", "));
                    }
                    if let Some(e) = elem {
                        parts.push(self.build_pattern_doc(e));
                    }
                }
                parts.push(d.text("]"));
                d.concat(&parts)
            }
            tsv_ts::Expression::RestElement(rest) => {
                let dots = d.text("...");
                let arg = self.build_pattern_doc(&rest.argument);
                d.concat(&[dots, arg])
            }
            tsv_ts::Expression::AssignmentPattern(assign) => {
                let left = self.build_pattern_doc(&assign.left);
                let eq = d.text(" = ");
                let right = self.build_pattern_doc(&assign.right);
                d.concat(&[left, eq, right])
            }
            tsv_ts::Expression::AssignmentExpression(assign) => {
                // Legacy AST - treat same as AssignmentPattern
                let left = self.build_pattern_doc(&assign.left);
                let eq = d.text(" = ");
                let right = self.build_pattern_doc(&assign.right);
                d.concat(&[left, eq, right])
            }
            tsv_ts::Expression::Literal(lit) => {
                // Preserve source text for literals (maintains original quote style)
                let text = self.extract_source_range(lit.span.start_usize(), lit.span.end_usize());
                d.text_owned(text.to_string())
            }
            // Default: build doc directly in shared arena
            _ => self.build_ts_expression_doc_no_comments(expr),
        }
    }

    /// Build a doc for an expression with leading and trailing comments
    ///
    /// Looks up comments in the range [span_start, span_end] and includes them:
    /// - Leading comments: between span_start and expr.span().start
    /// - Expression doc
    /// - Trailing comments: between expr.span().end and span_end
    ///
    /// Builds the expression doc directly in the shared arena using
    /// `build_expression_doc_with_comments` with `is_embedded_expression = true`
    /// so binary chains use ContinuationIndent style. The surrounding Svelte doc tree
    /// (e.g., the closing `}`) provides natural lookahead for fits checks — no
    /// `suffix_width` estimation needed.
    fn build_expression_with_comments_doc(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments (between span_start and expression start)
        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // Embed for embedded expression context: binary chains use ContinuationIndent style.
        // first_line_offset estimates the column position for width calculations.
        let context_indent = tsv_lang::TAB_WIDTH;
        let opening_offset = 5; // typical tag prefix, e.g. `{#if `
        let first_line_offset = context_indent + opening_offset;
        let embed = tsv_lang::EmbedContext {
            first_line_offset,
            mode: tsv_lang::LayoutMode::Embedded,
            ..self.embed
        };

        // Build expression doc directly in the shared arena.
        // No suffix_width needed — the surrounding doc tree (closing `}`, etc.)
        // provides natural lookahead via arena_fits_with_lookahead's rest_commands.
        let expr_doc = tsv_ts::build_expression_doc_with_comments(
            d,
            expr,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
            &embed,
            self.comments,
            &self.line_breaks,
            tsv_ts::TsConfig::svelte(),
        );

        // Build docs for trailing comments (between expression end and span_end)
        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        // Combine: leading + expr + trailing
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            d.concat(&parts)
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
    ///   Uses `is_embedded_expression` for proper continuation indent on wrapped binary expressions.
    ///
    /// # Parameters
    /// - `opening_offset` - Characters before the expression (e.g., 5 for `{#if `). Used to
    ///   calculate `first_line_offset` for width estimation.
    /// - `in_multiline_context` - Whether the block is on its own line (multiline) or inline
    pub(super) fn build_expression_doc_for_block(
        &self,
        expr: &tsv_ts::Expression,
        span_start: u32,
        span_end: u32,
        opening_offset: usize,
        in_multiline_context: bool,
    ) -> DocId {
        let d = self.d();
        let expr_start = expr.span().start;
        let expr_end = expr.span().end;

        // Build docs for leading comments
        let leading_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, span_start, expr_start)
                .map(|c| self.build_leading_js_comment_doc(c))
                .collect();

        // In multiline contexts, set up embedded expression context so binary chains
        // use ContinuationIndent style. first_line_offset estimates the column position.
        let embed = if in_multiline_context {
            let context_indent = tsv_lang::TAB_WIDTH;
            let first_line_offset = context_indent + opening_offset;
            tsv_lang::EmbedContext {
                first_line_offset,
                mode: tsv_lang::LayoutMode::Embedded,
                ..self.embed
            }
        } else {
            self.embed
        };

        // Build expression doc tree
        // Assignment expressions need parens in block conditions: {#if (a = b)}
        let expr_doc = if matches!(expr, tsv_ts::Expression::AssignmentExpression(_)) {
            let inner = tsv_ts::build_expression_doc_with_comments(
                d,
                expr,
                self.source,
                Rc::clone(&self.interner),
                &self.config,
                &embed,
                self.comments,
                &self.line_breaks,
                tsv_ts::TsConfig::svelte(),
            );
            d.parens(inner)
        } else {
            tsv_ts::build_expression_doc_with_comments(
                d,
                expr,
                self.source,
                Rc::clone(&self.interner),
                &self.config,
                &embed,
                self.comments,
                &self.line_breaks,
                tsv_ts::TsConfig::svelte(),
            )
        };

        // Apply remove_lines() only in INLINE contexts to prevent the condition
        // from being the first thing to break when there's other content on the line.
        // In multiline contexts, the condition is on its own line and can wrap naturally.
        let expr_doc = if in_multiline_context {
            expr_doc
        } else {
            d.remove_lines(expr_doc)
        };

        // Build docs for trailing comments
        let trailing_docs: Vec<DocId> =
            tsv_lang::comments_in_range(self.comments, expr_end, span_end)
                .map(|c| self.build_trailing_js_comment_doc(c))
                .collect();

        // Combine: leading + expr + trailing
        if leading_docs.is_empty() && trailing_docs.is_empty() {
            expr_doc
        } else {
            let mut parts = Vec::with_capacity(leading_docs.len() + 1 + trailing_docs.len());
            parts.extend(leading_docs);
            parts.push(expr_doc);
            parts.extend(trailing_docs);
            d.concat(&parts)
        }
    }
}
