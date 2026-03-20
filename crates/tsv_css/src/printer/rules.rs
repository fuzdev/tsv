//! CSS rule printing
//!
//! Handles formatting of:
//! - CSS rules (selector + declarations block)
//! - Comment placement within rules
//! - Blank line preservation between declarations
//!
//! Declaration and value printing are handled by separate modules.
//!
//! ## Architecture
//!
//! This module uses doc builders where practical. The complex block child
//! handling with inline comments and blank line preservation is still
//! imperative for clarity.

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule) {
        // Format selector (uses selectors module)
        self.print_selector_list(&rule.selector);

        // Check if first child is a comment between selector and opening brace
        let mut start_index = 0;
        if let Some(internal::CssBlockChild::Comment(comment)) = rule.declarations.first() {
            // Check if comment is before the opening brace (between selector and {)
            // block_span.start is the position of the opening brace
            if comment.span.start < rule.block_span.start {
                // Comment is between selector and brace - print inline
                // Always add space before comment for readability (normalize)
                // This is an intentional divergence from prettier (which preserves no-space)
                self.write(" /*");
                self.write(&comment.content);
                self.write("*/");
                start_index = 1; // Skip this comment when processing declarations
            }
        }

        self.write(" {\n");

        // Format declarations and comments with indentation
        self.indent_level += 1;
        let mut i = start_index;
        while i < rule.declarations.len() {
            let child = &rule.declarations[i];
            match child {
                internal::CssBlockChild::Declaration(decl) => {
                    // Preserve blank line before declaration if source has one
                    if i > start_index && self.has_blank_line_before_child(&rule.declarations, i) {
                        self.write("\n");
                    }
                    self.print_css_declaration(decl);

                    // Check for inline comments after the declaration
                    let inline_count = self.try_print_inline_comments_after_decl(
                        &rule.declarations,
                        i,
                        decl.span.end,
                    );
                    if inline_count > 0 {
                        i += inline_count;
                    }
                }
                internal::CssBlockChild::Comment(comment) => {
                    // Standalone comment (not inline after a declaration)
                    // Preserve blank line before comment if present in source
                    if i > start_index && self.has_blank_line_before_child(&rule.declarations, i) {
                        self.write("\n");
                    }

                    self.write_indent();
                    self.print_css_comment(comment);
                    self.write("\n");
                }
                internal::CssBlockChild::Rule(nested_rule) => {
                    // CSS Nesting Module - format nested rule
                    // Add blank line before nested rule if source has one (preserve author intent)
                    if i > start_index && self.has_blank_line_before_child(&rule.declarations, i) {
                        self.write("\n");
                    }
                    self.write_indent();
                    self.print_css_rule(nested_rule);

                    // Check for inline comment after nested rule's closing brace
                    let inline_count =
                        self.try_print_inline_comments(&rule.declarations, i, nested_rule.span.end);

                    self.write("\n");

                    i += inline_count;
                }
                internal::CssBlockChild::Atrule(nested_atrule) => {
                    // Nested at-rule (e.g., @media inside a rule)
                    // Add blank line before nested at-rule only if source had one
                    if i > start_index && self.has_blank_line_before_child(&rule.declarations, i) {
                        self.write("\n");
                    }
                    self.write_indent();
                    self.print_css_atrule(nested_atrule);
                    self.write("\n");
                }
            }
            i += 1;
        }
        self.indent_level -= 1;

        self.write_indent();
        self.write("}");
    }
}
