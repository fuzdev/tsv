// Svelte formatter - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates formatting of top-level sections
// - **nodes.rs**: Node-specific formatting (elements, expressions, control flow, etc.)
// - **text.rs**: Text content and whitespace normalization
// - **script_style.rs**: Script and style section formatting
// - **attributes.rs**: HTML attribute and directive formatting
//
// Language-level concerns (shared across tools) are in `crate::language::html`
//
// ## Design Principles
//
// 1. **Match Prettier**: Format output matches prettier-plugin-svelte for compatibility
// 2. **Preserve Semantics**: Never change HTML whitespace rendering semantics
// 3. **Source Layout**: Preserve authorial intent via inline run grouping
// 4. **Modularity**: Each module has single responsibility for future maintainability

mod attributes;
mod classification;
mod nodes;
mod script_style;
mod text;

use self::text::TextAnalysis;
use crate::ast::internal::{self, FragmentNode};
use crate::formatter::Formatter;

impl Formatter {
    /// Format a Svelte Root node
    ///
    /// Orchestrates formatting of the four main sections of a .svelte file:
    /// 1. Module script: `<script context="module">`
    /// 2. Instance script: `<script>`
    /// 3. Template: The HTML/Svelte template
    /// 4. Style: `<style>`
    ///
    /// Sections are ordered canonically and separated by blank lines.
    pub fn format_root(&mut self, root: &internal::Root) {
        let mut has_previous_section = false;

        // Format module script (if present)
        if let Some(script) = &root.module {
            self.format_script(script);
            has_previous_section = true;
        }

        // Format instance script (if present)
        if let Some(script) = &root.instance {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.format_script(script);
            has_previous_section = true;
        }

        // Format template fragment (if not empty)
        // Check if there are any non-whitespace nodes
        let has_content =
            root.fragment.nodes.iter().any(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            );

        if has_content {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.format_root_fragment(&root.fragment);
            self.write("\n"); // Template needs explicit newline
            has_previous_section = true;
        }

        // Format style (if present)
        if let Some(style) = &root.css {
            if has_previous_section {
                self.write("\n"); // Blank line between sections
            }
            self.format_style(style);
        }
    }

    /// Format a Fragment with blank lines between root-level block elements
    ///
    /// Root-level formatting has special rules:
    /// - Blank lines preserved from source (authorial intent for logical grouping)
    /// - Multiple blank lines collapse to single blank line
    /// - Whitespace between inline elements is preserved
    /// - Leading/trailing whitespace-only nodes are removed
    fn format_root_fragment(&mut self, fragment: &internal::Fragment) {
        let mut prev_was_block = false;
        let mut prev_had_blank_line = false;

        // Find first non-whitespace node index
        let first_non_ws_idx = fragment.nodes.iter().position(
            |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
        );

        for (i, node) in fragment.nodes.iter().enumerate() {
            match node {
                FragmentNode::Text(text) => {
                    // Skip leading whitespace-only text nodes at root level
                    if Some(i) < first_non_ws_idx {
                        continue;
                    }

                    // Check if this is a whitespace-only node
                    if text.raw.is_whitespace_only() {
                        // Check if it contains a blank line (2+ newlines)
                        if text.raw.has_blank_line() {
                            // Blank line found - skip this node entirely and remember the blank line
                            // The blank line will be added before the next element
                            prev_had_blank_line = true;
                            continue;
                        }

                        // Whitespace without blank line - skip if previous was block,
                        // otherwise preserve (semantically meaningful between inline elements)
                        if prev_was_block {
                            continue;
                        }
                    }

                    // Root-level text: normalize but preserve spacing
                    // parent_is_block: false, parent_preserves_ws: false
                    self.format_text(text, false, false);
                    prev_was_block = false;
                    prev_had_blank_line = false;
                }
                FragmentNode::Element(el) => {
                    let is_block = self.is_block_element(el);

                    // Add blank line if:
                    // 1. Previous whitespace had a blank line (preserve authorial intent), OR
                    // 2. Both current and previous are blocks (default spacing rule)
                    if prev_had_blank_line || (prev_was_block && is_block) {
                        self.write("\n\n");
                    }

                    self.format_element(el);
                    prev_was_block = is_block;
                    prev_had_blank_line = false;
                }
                FragmentNode::ExpressionTag(tag) => {
                    // Expression tags: blank line if source had one OR previous was block
                    if prev_had_blank_line || prev_was_block {
                        self.write("\n\n");
                    }

                    self.format_expression_tag(tag);
                    prev_was_block = false;
                    prev_had_blank_line = false;
                }
            }
        }
    }

    /// Format a single fragment node (dispatch to specific type)
    ///
    /// # Parameters
    /// - `parent_is_block`: Whether the parent element is a block element (affects text trimming)
    /// - `parent_preserves_ws`: Whether the parent element preserves whitespace (like `<pre>`)
    pub(crate) fn format_fragment_node(
        &mut self,
        node: &FragmentNode,
        parent_is_block: bool,
        parent_preserves_ws: bool,
    ) {
        match node {
            FragmentNode::Element(element) => self.format_element(element),
            FragmentNode::Text(text) => {
                self.format_text(text, parent_is_block, parent_preserves_ws)
            }
            FragmentNode::ExpressionTag(tag) => self.format_expression_tag(tag),
        }
    }
}
