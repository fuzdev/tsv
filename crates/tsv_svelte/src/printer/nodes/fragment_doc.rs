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
            .filter_map(|node| self.build_fragment_node_doc_with_context(node, trim_text))
            .collect();

        if docs.is_empty() {
            doc::text("")
        } else {
            doc::concat(docs)
        }
    }

    /// Build a doc for a node slice with boundary whitespace trimmed
    ///
    /// Matches Prettier's trimTextNodeLeft/trimTextNodeRight behavior:
    /// - Skip whitespace-only text at start and end
    /// - Trim leading whitespace from first text node with content
    /// - Trim trailing whitespace from last text node with content
    /// - Keep internal whitespace-only text as single space
    pub(crate) fn build_nodes_doc_trimmed(&self, nodes: &[FragmentNode]) -> Doc {
        if nodes.is_empty() {
            return doc::text("");
        }

        // Find first non-whitespace-only index
        let start_idx = nodes
            .iter()
            .position(|n| !n.is_whitespace_only_text())
            .unwrap_or(nodes.len());

        // Find last non-whitespace-only index (exclusive)
        let end_idx = nodes
            .iter()
            .rposition(|n| !n.is_whitespace_only_text())
            .map_or(0, |i| i + 1);

        if start_idx >= end_idx {
            return doc::text("");
        }

        // Build docs for the trimmed range
        let trimmed_nodes = &nodes[start_idx..end_idx];
        let trimmed_len = trimmed_nodes.len();
        let mut docs = Vec::with_capacity(trimmed_len);

        for (i, node) in trimmed_nodes.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == trimmed_len - 1;

            // For boundary text nodes, trim their leading/trailing whitespace
            if let FragmentNode::Text(text) = node {
                let trimmed = text.raw.trim();
                if trimmed.is_empty() {
                    // Internal whitespace-only node - preserve as single space
                    docs.push(doc::text(" "));
                } else {
                    // Text with content - normalize internal whitespace and trim boundaries as needed
                    let normalized = if is_first && is_last {
                        // Only text node - fully normalize (collapse all whitespace, trim both ends)
                        self.normalize_whitespace(&text.raw, true)
                    } else if is_first {
                        // Trim leading whitespace, preserve trailing
                        let s = text.raw.trim_start();
                        self.normalize_whitespace(s, false)
                    } else if is_last {
                        // Preserve leading, trim trailing
                        let s = text.raw.trim_end();
                        self.normalize_whitespace(s, false)
                    } else {
                        // Internal - preserve both
                        self.normalize_whitespace(&text.raw, false)
                    };
                    docs.push(doc::text_owned(normalized));
                }
            } else if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, false) {
                docs.push(node_doc);
            }
        }

        if docs.is_empty() {
            doc::text("")
        } else {
            doc::concat(docs)
        }
    }

    /// Build a doc for nodes in a multiline block context.
    ///
    /// Block children (div, p, etc.) and control flow blocks get their own lines.
    /// Text nodes with newlines split into separate lines, preserving source structure.
    pub(super) fn build_nodes_doc_multiline(&self, nodes: &[FragmentNode]) -> Doc {
        if nodes.is_empty() {
            return doc::text("");
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
            return doc::text("");
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
                // Block element - check if it should stay on the same line as previous text
                // Prettier keeps text+space+block together when source has them on same line
                let keep_inline_with_prev = prev_text_has_trailing_space && {
                    // Check if this block and previous content are on the same source line
                    if i > 0 {
                        let prev_span = self.get_node_span(&trimmed_nodes[i - 1]);
                        let curr_span = self.get_node_span(node);
                        tsv_lang::printing::spans_on_same_line(self.source, prev_span, curr_span)
                    } else {
                        false
                    }
                };

                if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, true) {
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
                    // New line after block
                    lines.push(std::mem::take(&mut current_line));
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
                    for (idx, part) in parts.iter().enumerate() {
                        // Push new line for each newline in the text, EXCEPT:
                        // - idx == 0: we're still on the "before first newline" part
                        // - idx == 1 AND line was empty before AND first part was empty:
                        //   reuse the empty line from previous block element
                        let should_skip =
                            idx == 0 || (idx == 1 && line_was_empty_before && parts[0].is_empty());
                        if !should_skip {
                            lines.push(std::mem::take(&mut current_line));
                        }
                        // Preserve leading space if part has it
                        if part.starts_with(char::is_whitespace) && !current_line.is_empty() {
                            current_line.push(doc::text(" "));
                        }
                        let trimmed = part.trim();
                        if !trimmed.is_empty() {
                            current_line.push(doc::text_owned(trimmed.to_string()));
                            // Preserve trailing space within the text
                            if part.ends_with(char::is_whitespace) {
                                current_line.push(doc::text(" "));
                            }
                        }
                    }
                    // Last part's trailing space affects next node
                    prev_text_has_trailing_space = parts
                        .last()
                        .is_some_and(|p| p.ends_with(char::is_whitespace));
                } else {
                    // No newlines - add to current line
                    // Preserve leading space if source has it AND line has content
                    // (don't add space at start of a new line after block element)
                    if text.raw.starts_with(char::is_whitespace) && !current_line.is_empty() {
                        current_line.push(doc::text(" "));
                    }
                    let trimmed = text.raw.trim();
                    if !trimmed.is_empty() {
                        current_line.push(doc::text_owned(trimmed.to_string()));
                        // Preserve trailing space if source has it
                        if text.raw.ends_with(char::is_whitespace) {
                            current_line.push(doc::text(" "));
                        }
                    }
                    // Track trailing space for inline-before-block pattern
                    prev_text_has_trailing_space = text.raw.ends_with(char::is_whitespace);
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
            doc::text("")
        } else {
            doc::concat(docs)
        }
    }

    /// Check if a fragment node is a block-level node (needs its own line)
    fn is_block_fragment_node(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::Element(el) => {
                let tag = self.resolve_symbol(el.name);
                tsv_html::is_block_element(&tag) || el.kind == internal::ElementKind::Component
            }
            FragmentNode::SpecialElement(_) => true,
            FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_)
            | FragmentNode::SnippetBlock(_) => true,
            _ => false,
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
                nodes[first..=last]
                    .iter()
                    .any(FragmentNode::is_whitespace_only_text)
            }
            _ => false,
        }
    }

    /// Get the span of a fragment node for source position checks.
    fn get_node_span(&self, node: &FragmentNode) -> tsv_lang::Span {
        match node {
            FragmentNode::Text(text) => text.span,
            FragmentNode::Element(el) => el.span,
            FragmentNode::SpecialElement(el) => el.span,
            FragmentNode::ExpressionTag(tag) => tag.span,
            FragmentNode::Comment(comment) => comment.span,
            FragmentNode::IfBlock(block) => block.span,
            FragmentNode::EachBlock(block) => block.span,
            FragmentNode::AwaitBlock(block) => block.span,
            FragmentNode::KeyBlock(block) => block.span,
            FragmentNode::SnippetBlock(block) => block.span,
            FragmentNode::HtmlTag(tag) => tag.span,
            FragmentNode::ConstTag(tag) => tag.span,
            FragmentNode::DebugTag(tag) => tag.span,
            FragmentNode::RenderTag(tag) => tag.span,
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
        match node {
            FragmentNode::Text(text) => self.build_text_doc(text, trim_text),
            FragmentNode::Element(element) => Some(self.build_element_doc(element)),
            FragmentNode::SpecialElement(element) => Some(self.build_special_element_doc(element)),
            FragmentNode::ExpressionTag(tag) => Some(self.build_expression_tag_doc(tag)),
            FragmentNode::Comment(comment) => Some(self.build_html_comment_doc(comment)),
            FragmentNode::IfBlock(block) => Some(self.build_if_block_doc(block)),
            FragmentNode::EachBlock(block) => Some(self.build_each_block_doc(block)),
            FragmentNode::AwaitBlock(block) => Some(self.build_await_block_doc(block)),
            FragmentNode::KeyBlock(block) => Some(self.build_key_block_doc(block)),
            FragmentNode::SnippetBlock(block) => Some(self.build_snippet_block_doc(block)),
            FragmentNode::HtmlTag(tag) => Some(self.build_html_tag_doc(tag)),
            FragmentNode::ConstTag(tag) => Some(self.build_const_tag_doc(tag)),
            FragmentNode::DebugTag(tag) => Some(self.build_debug_tag_doc(tag)),
            FragmentNode::RenderTag(tag) => Some(self.build_render_tag_doc(tag)),
        }
    }

    // =========================================================================
    // Text nodes
    // =========================================================================

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
            // Has content: normalize whitespace
            let normalized = self.normalize_whitespace(&text.raw, trim_completely);
            Some(doc::text_owned(normalized))
        }
    }

    // =========================================================================
    // Comment nodes
    // =========================================================================

    /// Build a doc for an HTML comment
    fn build_html_comment_doc(&self, comment: &internal::HtmlComment) -> Doc {
        doc::concat(vec![
            doc::text("<!--"),
            doc::text_owned(comment.content.clone()),
            doc::text("-->"),
        ])
    }

    // =========================================================================
    // Control flow blocks
    // =========================================================================

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
        // Calculate suffix_width for width-aware wrapping decisions
        // This accounts for `}`, inline body, and closing tag
        let suffix_width = self.calculate_if_suffix_width(block);

        // Build expression doc with surrounding comments and suffix_width awareness
        // Span range: after "{#if " (start + 5) to before "}" (end - 1)
        // opening_offset = 5 for "{#if "
        let expr_doc = self.build_expression_with_context_doc(
            &block.test,
            block.opening_tag_span.start + 5, // after "{#if "
            block.opening_tag_span.end - 1,   // before "}"
            5,                                // "{#if "
            suffix_width,
        );

        let is_inline = self.is_inline_fragment(&block.consequent);

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.consequent)
        } else {
            self.build_nodes_doc_multiline(&block.consequent.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation.
        // For inline blocks: no hardlines, content hugs the tags
        // For multiline blocks: hardlines force proper line breaks
        let indented_body = if is_inline {
            // Inline: indent the body but no hardlines (hugs directly)
            doc::indent(body_doc)
        } else {
            // Multiline: indent with hardlines for proper breaking
            doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
        };

        let mut parts = vec![doc::text("{#if "), expr_doc, doc::text("}"), indented_body];

        // Add endline only for multiline blocks
        if !is_inline {
            parts.push(doc::hardline());
        }

        // Handle alternate (else/else-if)
        if let Some(alt) = &block.alternate {
            parts.push(self.build_if_alternate_doc(alt, is_inline));
        }

        parts.push(doc::text("{/if}"));
        doc::concat(parts)
    }

    /// Build doc for if block alternate (else or else-if)
    ///
    /// Uses the parent's inline status to determine formatting.
    /// Always wraps body in indent() for proper internal break indentation.
    fn build_if_alternate_doc(&self, alt: &Fragment, parent_inline: bool) -> Doc {
        // Check if this is an else-if (alternate contains only an if block)
        let is_else_if = alt.nodes.iter().all(|n| match n {
            FragmentNode::IfBlock(_) => true,
            FragmentNode::Text(t) => t.raw.trim().is_empty(),
            _ => false,
        });

        if let Some(FragmentNode::IfBlock(else_if)) = alt
            .nodes
            .iter()
            .find(|n| matches!(n, FragmentNode::IfBlock(_)))
            && (else_if.elseif || is_else_if)
        {
            // {:else if condition}
            // Calculate suffix_width for width-aware wrapping decisions
            let suffix_width = self.calculate_if_suffix_width(else_if);

            // Build expression doc with surrounding comments and suffix_width awareness
            // The offset depends on whether this is a true {:else if} or a normalized {#if}:
            // - elseif=true: opening_tag_span is "{:else if ...}", offset = 10
            // - elseif=false: opening_tag_span is "{#if ...}", offset = 5
            let opening_offset: usize = if else_if.elseif { 10 } else { 5 };
            let expr_doc = self.build_expression_with_context_doc(
                &else_if.test,
                else_if.opening_tag_span.start + opening_offset as u32,
                else_if.opening_tag_span.end - 1,
                opening_offset,
                suffix_width,
            );

            let is_inline = self.is_inline_fragment(&else_if.consequent);
            let is_both_inline = is_inline && parent_inline;

            // For inline: use regular fragment doc
            // For multiline: use multiline doc (preserves line structure)
            let body_doc = if is_both_inline {
                self.build_fragment_doc(&else_if.consequent)
            } else {
                self.build_nodes_doc_multiline(&else_if.consequent.nodes)
            };

            // Always wrap in indent() for internal break indentation
            let indented_body = if is_both_inline {
                doc::indent(body_doc)
            } else {
                doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
            };

            let mut parts = vec![
                doc::text("{:else if "),
                expr_doc,
                doc::text("}"),
                indented_body,
            ];

            // Add endline only for multiline
            if !is_both_inline {
                parts.push(doc::hardline());
            }

            if let Some(nested_alt) = &else_if.alternate {
                parts.push(self.build_if_alternate_doc(nested_alt, is_both_inline));
            }

            return doc::concat(parts);
        }

        // Plain {:else}
        let is_inline = self.is_inline_fragment(alt);
        let is_both_inline = is_inline && parent_inline;

        // For inline: use regular fragment doc
        // For multiline: use multiline doc
        let body_doc = if is_both_inline {
            self.build_fragment_doc(alt)
        } else {
            self.build_nodes_doc_multiline(&alt.nodes)
        };

        // Always wrap in indent() for internal break indentation
        let indented_body = if is_both_inline {
            doc::indent(body_doc)
        } else {
            doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
        };

        if is_both_inline {
            doc::concat(vec![doc::text("{:else}"), indented_body])
        } else {
            doc::concat(vec![doc::text("{:else}"), indented_body, doc::hardline()])
        }
    }

    /// Build a doc for an each block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_each_block_doc(&self, block: &internal::EachBlock) -> Doc {
        // Calculate suffix_width for width-aware wrapping decisions
        let suffix_width = self.calculate_each_suffix_width(block);

        // Comment range for collection expression: "{#each " is 7 chars
        // End before "as" keyword (context start, or opening_tag_span.end if no context)
        let expr_comment_end = block
            .context
            .as_ref()
            .map_or(block.opening_tag_span.end - 1, |c| c.span().start);
        let expr_doc = self.build_expression_with_context_doc(
            &block.expression,
            block.opening_tag_span.start + 7, // after "{#each "
            expr_comment_end,
            7, // "{#each "
            suffix_width,
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
            // Use key_span for comment lookup if available (includes parentheses)
            let key_doc = if let Some(key_span) = block.key_span {
                self.build_expression_with_comments_doc(
                    key,
                    key_span.start + 1, // after "("
                    key_span.end - 1,   // before ")"
                )
            } else {
                tsv_ts::build_expression_doc(
                    key,
                    self.source,
                    Rc::clone(&self.interner),
                    &self.config,
                )
            };
            opening.push(doc::text(" ("));
            opening.push(key_doc);
            opening.push(doc::text(")"));
        }

        opening.push(doc::text("}"));

        let is_inline = self.is_inline_fragment(&block.body);

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(&block.body.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation.
        // For inline blocks: no hardlines, content hugs the tags
        // For multiline blocks: hardlines force proper line breaks
        let indented_body = if is_inline {
            doc::indent(body_doc)
        } else {
            doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
        };

        let mut parts = vec![doc::concat(opening), indented_body];

        // Add endline only for multiline blocks
        if !is_inline {
            parts.push(doc::hardline());
        }

        if let Some(fallback) = &block.fallback {
            let fallback_inline = self.is_inline_fragment(fallback);
            parts.push(doc::text("{:else}"));

            // For inline: use regular fragment doc
            // For multiline: use multiline doc
            let fallback_doc = if fallback_inline && is_inline {
                self.build_fragment_doc(fallback)
            } else {
                self.build_nodes_doc_multiline(&fallback.nodes)
            };

            let indented_fallback = if fallback_inline && is_inline {
                doc::indent(fallback_doc)
            } else {
                doc::indent(doc::concat(vec![doc::hardline(), fallback_doc]))
            };
            parts.push(indented_fallback);
            if !(fallback_inline && is_inline) {
                parts.push(doc::hardline());
            }
        }

        parts.push(doc::text("{/each}"));
        doc::concat(parts)
    }

    /// Build a doc for an await block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock) -> Doc {
        // Calculate suffix_width for width-aware wrapping decisions
        let suffix_width = self.calculate_await_suffix_width(block);

        // Build expression doc with surrounding comments and suffix_width awareness
        // Span range: after "{#await " (start + 8) to before "}" or " then" (end - 1)
        let expr_doc = self.build_expression_with_context_doc(
            &block.expression,
            block.opening_tag_span.start + 8, // after "{#await "
            block.opening_tag_span.end - 1,   // before "}"
            8,                                // "{#await "
            suffix_width,
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
                let is_inline = self.is_inline_fragment(then_block);
                if is_inline {
                    // Inline: no extra indentation/wrapping
                    parts.push(self.build_fragment_doc(then_block));
                } else {
                    // Multiline: use multiline doc with hardline
                    let then_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                    parts.push(doc::indent(doc::concat(vec![doc::hardline(), then_doc])));
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
                let is_inline = self.is_inline_fragment(catch_block);
                if is_inline {
                    parts.push(self.build_fragment_doc(catch_block));
                } else {
                    let catch_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                    parts.push(doc::indent(doc::concat(vec![doc::hardline(), catch_doc])));
                    parts.push(doc::hardline());
                }
            }
            parts.push(doc::text("{/await}"));
            return doc::concat(parts);
        }

        parts.push(doc::text("}"));

        // Track overall inline status for closing tag
        let is_overall_inline = block
            .pending
            .as_ref()
            .is_none_or(|p| self.is_inline_fragment(p));

        // Pending
        if let Some(pending) = &block.pending {
            let is_inline = self.is_inline_fragment(pending);
            if is_inline {
                parts.push(self.build_fragment_doc(pending));
            } else {
                let pending_doc = self.build_nodes_doc_multiline(&pending.nodes);
                parts.push(doc::indent(doc::concat(vec![doc::hardline(), pending_doc])));
                parts.push(doc::hardline());
            }
        }

        // Then
        if let Some(value) = &block.value {
            let value_pattern =
                self.extract_source_range(value.span().start_usize(), value.span().end_usize());
            parts.push(doc::text("{:then "));
            parts.push(doc::text_owned(value_pattern.to_string()));
            parts.push(doc::text("}"));
        } else if block.then.as_ref().is_some_and(|t| !t.nodes.is_empty()) {
            parts.push(doc::text("{:then}"));
        }
        if let Some(then_block) = &block.then {
            let is_inline = self.is_inline_fragment(then_block);
            if is_inline && is_overall_inline {
                parts.push(self.build_fragment_doc(then_block));
            } else {
                let then_doc = self.build_nodes_doc_multiline(&then_block.nodes);
                parts.push(doc::indent(doc::concat(vec![doc::hardline(), then_doc])));
                parts.push(doc::hardline());
            }
        }

        // Catch
        if let Some(error) = &block.error {
            let error_pattern =
                self.extract_source_range(error.span().start_usize(), error.span().end_usize());
            parts.push(doc::text("{:catch "));
            parts.push(doc::text_owned(error_pattern.to_string()));
            parts.push(doc::text("}"));
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            parts.push(doc::text("{:catch}"));
        }
        if let Some(catch_block) = &block.catch {
            let is_inline = self.is_inline_fragment(catch_block);
            if is_inline && is_overall_inline {
                parts.push(self.build_fragment_doc(catch_block));
            } else {
                let catch_doc = self.build_nodes_doc_multiline(&catch_block.nodes);
                parts.push(doc::indent(doc::concat(vec![doc::hardline(), catch_doc])));
                parts.push(doc::hardline());
            }
        }

        parts.push(doc::text("{/await}"));
        doc::concat(parts)
    }

    /// Build a doc for a key block
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock) -> Doc {
        // Calculate suffix_width for width-aware wrapping decisions
        let suffix_width = self.calculate_key_suffix_width(block);

        // Build expression doc with surrounding comments and suffix_width awareness
        // Span range: after "{#key " (start + 6) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_context_doc(
            &block.expression,
            block.opening_tag_span.start + 6, // after "{#key "
            block.opening_tag_span.end - 1,   // before "}"
            6,                                // "{#key "
            suffix_width,
        );

        let is_inline = self.is_inline_fragment(&block.fragment);

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.fragment)
        } else {
            self.build_nodes_doc_multiline(&block.fragment.nodes)
        };

        // Always wrap body in indent() for proper internal break indentation.
        // For inline blocks: no hardlines, content hugs the tags
        // For multiline blocks: hardlines force proper line breaks
        let indented_body = if is_inline {
            doc::indent(body_doc)
        } else {
            doc::indent(doc::concat(vec![doc::hardline(), body_doc]))
        };

        let mut parts = vec![doc::text("{#key "), expr_doc, doc::text("}"), indented_body];

        // Add endline only for multiline blocks
        if !is_inline {
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

        let is_inline = self.is_inline_fragment(&block.body);

        // Type parameters (generics)
        let type_params_part = block.type_parameters.as_ref().map_or_else(
            || doc::text(""),
            |tp| {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tp.clone()),
                    doc::text(">"),
                ])
            },
        );

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
                        tsv_ts::format_expression(p, self.source, Rc::clone(&self.interner));
                    doc::text_owned(formatted)
                })
                .collect()
        };

        // Build params doc with line() separators for wrapping
        let params_doc = if params_docs.is_empty() {
            doc::text("")
        } else {
            // Pre-allocate: each param + separator (except first)
            let mut parts = Vec::with_capacity(params_docs.len() * 3);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(doc::text(","));
                    parts.push(doc::line());
                }
                parts.push(param_doc);
            }
            doc::concat(parts)
        };

        // Build opening tag with group for parameter wrapping
        // When fits: {#snippet name(a, b, c)}
        // When wraps: {#snippet name(\n\ta,\n\tb,\n\tc,\n)}
        let opening_doc = doc::group(doc::concat(vec![
            doc::text("{#snippet "),
            doc::text_owned(name.to_string()),
            type_params_part,
            doc::text("("),
            doc::indent(doc::concat(vec![doc::softline(), params_doc])),
            doc::if_break(doc::text(","), doc::text("")),
            doc::softline(),
            doc::text(")}"),
        ]));

        let mut parts = vec![opening_doc];

        // Body: inline hugs directly, multiline uses hardlines
        if is_inline {
            parts.push(self.build_fragment_doc(&block.body));
        } else {
            let body_doc = self.build_nodes_doc_multiline(&block.body.nodes);
            parts.push(doc::indent(doc::concat(vec![doc::hardline(), body_doc])));
            parts.push(doc::hardline());
        }

        parts.push(doc::text("{/snippet}"));
        doc::concat(parts)
    }

    // =========================================================================
    // Template tags
    // =========================================================================

    /// Build a doc for {@html expr}
    fn build_html_tag_doc(&self, tag: &internal::HtmlTag) -> Doc {
        // Build expression doc with surrounding comments
        // Span range: after "{@html " (start + 7) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 7, // after "{@html "
            tag.span.end - 1,   // before "}"
        );

        doc::concat(vec![doc::text("{@html "), expr_doc, doc::text("}")])
    }

    /// Build a doc for {@const declaration}
    fn build_const_tag_doc(&self, tag: &internal::ConstTag) -> Doc {
        // Format both id (pattern) and init expression properly
        // Patterns like {a,b} need spacing: {a, b}
        // For @const, comments are typically after the init expression
        // Span range for init: after "= " to before "}" (end - 1)
        let id_doc = tsv_ts::build_expression_doc(
            &tag.id,
            self.source,
            Rc::clone(&self.interner),
            &self.config,
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
    fn build_debug_tag_doc(&self, tag: &internal::DebugTag) -> Doc {
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
    fn build_render_tag_doc(&self, tag: &internal::RenderTag) -> Doc {
        // Build expression doc with surrounding comments
        // Span range: after "{@render " (start + 9) to before "}" (end - 1)
        let expr_doc = self.build_expression_with_comments_doc(
            &tag.expression,
            tag.span.start + 9, // after "{@render "
            tag.span.end - 1,   // before "}"
        );

        doc::concat(vec![doc::text("{@render "), expr_doc, doc::text("}")])
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

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
                )),
            ]),
            // Default: format as regular expression
            _ => {
                let formatted =
                    tsv_ts::format_expression(expr, self.source, Rc::clone(&self.interner));
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
}
