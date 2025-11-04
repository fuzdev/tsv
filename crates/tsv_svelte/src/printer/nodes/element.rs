// Element-specific formatting for Svelte templates
//
// Handles formatting of HTML/Svelte elements with context-aware decisions about
// whether to format children in multiline vs compact mode.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_html as html;

impl<'a> Printer<'a> {
    /// Format a Svelte element with context-aware formatting
    ///
    /// Block elements with multiple block children: multiline with indentation
    /// All other cases: compact (single line)
    pub fn print_element(&mut self, element: &internal::Element) {
        // Determine element characteristics (single symbol resolution for efficiency)
        let (is_block, preserves_ws, is_void) = self.with_resolved_symbol(element.name, |tag| {
            (
                html::is_block_element(tag),
                html::preserves_whitespace(tag),
                html::is_void_element(tag),
            )
        });

        // Components are always treated as block elements (matches prettier behavior)
        use crate::ast::internal::ElementKind;
        let is_block = is_block || element.kind == ElementKind::Component;

        // Check if we need Prettier's "hug mode" (>< pattern) formatting
        let use_hug_mode = self.needs_hug_mode(element, is_block);

        // Decide if children should be multiline (skip if using hug mode)
        let multiline =
            !use_hug_mode && self.should_format_multiline(element, is_block, preserves_ws);

        // Opening tag
        let tag_name = self.resolve_symbol(element.name);
        self.write("<");
        self.write(&tag_name);

        // Format attributes
        for attr in &element.attributes {
            self.write(" ");
            self.print_attribute(attr);
        }

        // Void elements are self-closing
        if is_void {
            self.write(" />");
            return; // No children or closing tag
        }

        // Components with no children: preserve author's choice (self-closing vs explicit closing tag)
        if element.kind == ElementKind::Component && element.fragment.nodes.is_empty() {
            let source_slice = &self.source[element.span.start as usize..element.span.end as usize];
            let was_self_closing = source_slice.trim_end().ends_with("/>");

            if was_self_closing {
                self.write(" />");
                return;
            }
            // Otherwise, fall through to write explicit closing tag below
        }

        // Format children based on mode
        if use_hug_mode {
            // Hug mode: <Tag\n\t><child></Tag\n>
            self.print_hug_mode_element(&tag_name, &element.fragment.nodes, preserves_ws);
        } else {
            // Check for "opening newline only" pattern (prettier's split closing tag format)
            // Pattern: opening tag followed by newline, but closing tag NOT preceded by newline
            // Output: <Tag>\n\t{content}</Tag\n>
            let use_split_closing_tag =
                multiline && self.has_opening_newline_only(&element.fragment.nodes);

            // Normal mode
            self.write(">");

            if use_split_closing_tag {
                // Opening newline only: content on one indented line, split closing tag
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();

                // Print content inline (skip leading whitespace, preserve rest)
                for (i, node) in element.fragment.nodes.iter().enumerate() {
                    // Skip first node if it's the leading whitespace
                    if i == 0
                        && matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())
                    {
                        continue;
                    }
                    self.print_fragment_node(node, false, preserves_ws);
                }

                // Split closing tag
                self.write("</");
                self.write(&tag_name);
                self.indent_level -= 1;
                self.write("\n");
                self.write_indent();
                self.write(">");
            } else if multiline {
                // Normal multiline formatting
                self.print_multiline_children(&element.fragment.nodes, preserves_ws);
                self.write("</");
                self.write(&tag_name);
                self.write(">");
            } else {
                // Compact formatting
                self.print_compact_children(&element.fragment, is_block, preserves_ws);
                self.write("</");
                self.write(&tag_name);
                self.write(">");
            }
        }
    }

    /// Determine if element children should be formatted multiline
    ///
    /// # Formatting Rules
    ///
    /// Multiline formatting is used when:
    /// 1. **Block parent with block children**: Multiple block children OR mixed block + text
    ///    - Example: `<div><div>a</div><div>b</div></div>` → multiline
    /// 2. **Source contains newlines + multiple children**: Preserves author's line breaks
    ///    - Example: `<div>\n  <span>a</span>\n  <span>b</span>\n</div>` → multiline
    ///
    /// Stays compact when:
    /// - Single block child (no text mixing)
    /// - All inline children (no separation needed)
    /// - Single text node (preserves author's single-line intent, even with newlines)
    /// - Whitespace-preserving parent (`<pre>`, `<textarea>`, etc.)
    ///
    /// # Rationale
    ///
    /// - **Rule 1** avoids putting block children on the same line (hurts readability)
    /// - **Rule 2** respects source layout for semantic significance (blank lines for grouping)
    /// - **Single text exception** handles cases like `<div>\n  text\n</div>` → stays `<div>text</div>`
    pub fn should_format_multiline(
        &self,
        element: &internal::Element,
        is_block: bool,
        preserves_ws: bool,
    ) -> bool {
        // Whitespace-preserving elements never go multiline
        if preserves_ws {
            return false;
        }

        // Only block elements can have multiline formatting
        if !is_block {
            return false;
        }

        let fragment = &element.fragment;

        // Rule 1: Block parent with block children needs separation
        let element_child_count = fragment
            .nodes
            .iter()
            .filter(|node| matches!(node, FragmentNode::Element(_)))
            .count();
        let has_block_children = self.has_block_elements(fragment);
        let has_text = self.has_text_content(fragment);
        let needs_block_separation = has_block_children && (element_child_count > 1 || has_text);

        // Rule 2: Newlines before last content trigger multiline (format preservation)
        // Simple rule: Check if ANY text node before the last content node has newlines
        // This naturally excludes trailing newlines (they're after last content)
        // Examples:
        //   <Comp>\n  {expr}\n</Comp>           → multiline (newline before expr)
        //   <Comp>  {\n  expr\n}\n</Comp>       → compact (only trailing newline)
        //   <div>\n\tText: {expr}\n</div>       → multiline (newline in text before expr)
        let has_source_line_breaks = {
            // Find the last non-whitespace-only content node
            let last_content_idx = fragment.nodes.iter().rposition(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            );

            if let Some(last_idx) = last_content_idx {
                // Check if ANY text node before the last content has newlines
                fragment.nodes[..last_idx]
                    .iter()
                    .any(|node| matches!(node, FragmentNode::Text(text) if text.raw.contains('\n')))
            } else {
                false // No content nodes at all
            }
        };

        needs_block_separation || has_source_line_breaks
    }

    /// Determine if element needs "hug mode" (`><` pattern) formatting
    ///
    /// Hug mode is Prettier's special formatting for inline/component parents with block children.
    /// Instead of standard multiline indentation, it uses the `><` pattern:
    ///
    /// ```svelte
    /// <Comp
    ///   ><div>a</div>
    ///   <div>b</div></Comp
    /// >
    /// ```
    ///
    /// This is triggered when:
    /// - Parent is an inline element OR component
    /// - Children contain at least one block element
    /// - Children don't already have newlines (compact source)
    pub fn needs_hug_mode(&self, element: &internal::Element, is_block: bool) -> bool {
        use crate::ast::internal::ElementKind;

        // Only inline elements and components can use hug mode
        let is_inline_or_component = !is_block || element.kind == ElementKind::Component;

        if !is_inline_or_component {
            return false;
        }

        // Hug mode only applies when children are ALL block HTML elements
        // Mixed content (blocks + components/expressions) uses normal multiline
        let non_whitespace_children: Vec<_> = element
            .fragment
            .nodes
            .iter()
            .filter(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            )
            .collect();

        // Need at least 2 children for hug mode
        if non_whitespace_children.len() < 2 {
            return false;
        }

        // All children must be block HTML elements (not components, not expressions, not text)
        let all_block_elements = non_whitespace_children
            .iter()
            .all(|node| matches!(node, FragmentNode::Element(el) if self.is_block_element(el)));

        if !all_block_elements {
            return false;
        }

        // Only use hug mode if children are compact (no newlines in content)
        // Ignore whitespace-only text nodes (they're formatting whitespace, not semantic content)
        let has_content_newlines = element.fragment.nodes.iter().any(|node| {
            matches!(node, FragmentNode::Text(text)
                if text.raw.contains('\n') && !text.raw.is_whitespace_only())
        });

        !has_content_newlines
    }

    /// Check if element has "opening newline only" pattern
    ///
    /// This pattern is when:
    /// - First child is whitespace text with newline (opening tag followed by newline)
    /// - Last child is NOT a text node at all (closing tag immediately after non-text content)
    ///
    /// Example: `<Comp>\n\t{count}</Comp>` (no text node after expression)
    ///
    /// Prettier uses split closing tag format for this: `</Tag\n>` instead of `\n</Tag>`
    fn has_opening_newline_only(&self, nodes: &[FragmentNode]) -> bool {
        if nodes.is_empty() {
            return false;
        }

        // Check if first child is whitespace text with newline
        let has_opening_newline = matches!(nodes.first(), Some(FragmentNode::Text(text))
            if text.raw.is_whitespace_only() && text.raw.contains('\n'));

        // Check if last child is a text node
        // If it is, then there's trailing content/whitespace, so NOT opening-newline-only pattern
        let has_trailing_text = matches!(nodes.last(), Some(FragmentNode::Text(_)));

        // Opening newline only pattern: opening has newline, NO trailing text node
        has_opening_newline && !has_trailing_text
    }
}
