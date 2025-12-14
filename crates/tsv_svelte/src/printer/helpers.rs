//! Fragment analysis and child printing helpers

use super::Printer;
use crate::ast::internal::{Fragment, FragmentNode};

/// Check if we're inside a template literal after processing this line.
///
/// Template literals span multiple lines when they contain actual newlines.
/// Lines that are inside a template literal should not be re-indented
/// because the whitespace is part of the template content.
///
/// This function counts unescaped backticks to track template literal state.
/// It also ignores backticks inside string literals (single/double quoted).
pub(super) fn is_inside_template_literal(line: &str, was_inside: bool) -> bool {
    let mut in_template = was_inside;
    let mut in_string: Option<char> = None;
    let mut chars = line.chars();

    while let Some(c) = chars.next() {
        // Handle escape sequences
        if c == '\\' {
            chars.next(); // Skip the escaped character
            continue;
        }

        // Handle string literals
        if in_string.is_none() && (c == '"' || c == '\'') {
            in_string = Some(c);
            continue;
        }
        if Some(c) == in_string {
            in_string = None;
            continue;
        }

        // Track template literals (only when not in a string)
        if in_string.is_none() && c == '`' {
            in_template = !in_template;
        }
    }

    in_template
}

impl<'a> Printer<'a> {
    /// Check if a fragment's content is inline (no newlines in source)
    pub(super) fn is_inline_fragment(&self, fragment: &Fragment) -> bool {
        let (Some(first), Some(last)) = (fragment.nodes.first(), fragment.nodes.last()) else {
            return true;
        };
        let first_start = first.span().start_usize();
        let last_end = last.span().end_usize();
        let content = &self.source[first_start..last_end];
        !content.contains('\n')
    }

    /// Print children inline (no newlines added)
    pub(super) fn print_inline_children(&mut self, fragment: &Fragment) {
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    // For inline, preserve trimmed text
                    let trimmed = text.raw.trim();
                    if !trimmed.is_empty() {
                        self.write(trimmed);
                    }
                }
                _ => {
                    self.print_fragment_node(node, false, false);
                }
            }
        }
    }

    /// Helper to print children of a block with proper indentation
    pub(super) fn print_block_children(&mut self, fragment: &Fragment) {
        // For now, just print each child on a new line with indentation
        if fragment.nodes.is_empty() {
            return;
        }
        self.indent_level += 1;
        for node in &fragment.nodes {
            match node {
                FragmentNode::Text(text) => {
                    if !text.raw.trim().is_empty() {
                        self.write("\n");
                        self.write_indent();
                        self.write(text.raw.trim());
                    }
                }
                _ => {
                    self.write("\n");
                    self.write_indent();
                    self.print_fragment_node(node, true, false);
                }
            }
        }
        self.indent_level -= 1;
    }
}
