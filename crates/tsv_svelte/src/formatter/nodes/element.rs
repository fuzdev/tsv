// Element-specific formatting for Svelte templates
//
// Handles formatting of HTML/Svelte elements with context-aware decisions about
// whether to format children in multiline vs compact mode.

use crate::ast::internal::{self, FragmentNode};
use crate::formatter_core::Formatter;
use tsv_html as html;

impl Formatter {
    /// Format a Svelte element with context-aware formatting
    ///
    /// Block elements with multiple block children: multiline with indentation
    /// All other cases: compact (single line)
    pub fn format_element(&mut self, element: &internal::Element) {
        // Determine element characteristics (single symbol resolution for efficiency)
        let (is_block, preserves_ws, is_void) = self.with_resolved_symbol(element.name, |tag| {
            (
                html::is_block_element(tag),
                html::preserves_whitespace(tag),
                html::is_void_element(tag),
            )
        });

        // Decide if children should be multiline
        let multiline = self.should_format_multiline(element, is_block, preserves_ws);

        // Opening tag
        let tag_name = self.resolve_symbol(element.name);
        self.write("<");
        self.write(&tag_name);

        // Format attributes
        for attr in &element.attributes {
            self.write(" ");
            self.format_attribute(attr);
        }

        // Void elements are self-closing
        if is_void {
            self.write(" />");
            return; // No children or closing tag
        }

        self.write(">");

        // Format children
        if multiline {
            self.format_multiline_children(&element.fragment.nodes, preserves_ws);
        } else {
            self.format_compact_children(&element.fragment, is_block, preserves_ws);
        }

        // Closing tag
        self.write("</");
        self.write(&tag_name);
        self.write(">");
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

        // Rule 2: Newlines in source trigger multiline if multiple children
        // (single text node stays compact even with newlines - author intended single line)
        let total_children = fragment.nodes.len();
        let has_newlines = fragment
            .nodes
            .iter()
            .any(|node| matches!(node, FragmentNode::Text(text) if text.raw.contains('\n')));
        let has_source_line_breaks = has_newlines && total_children > 1;

        needs_block_separation || has_source_line_breaks
    }
}
