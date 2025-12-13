//! CSS rule printing
//!
//! Handles formatting of:
//! - CSS rules (selector + declarations block)
//! - Comment placement within rules
//! - Blank line preservation between declarations
//!
//! Declaration and value printing are handled by separate modules.

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
                    let mut added_blank_line = false;
                    if i > start_index
                        && let Some(prev_child) = rule.declarations.get(i - 1)
                        && self
                            .has_blank_line_between_spans(prev_child.span().end, comment.span.start)
                    {
                        self.write("\n");
                        added_blank_line = true;
                    }

                    // Check if next sibling is a nested rule - if so, add blank line before comment
                    // (but only if we didn't already add one from source preservation)
                    if !added_blank_line
                        && let Some(next_child) = rule.declarations.get(i + 1)
                        && matches!(
                            next_child,
                            internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_)
                        )
                    {
                        // Comment before nested rule - add blank line before comment
                        if i > start_index {
                            self.write("\n");
                        }
                    }
                    self.write_indent();
                    self.print_css_comment(comment);
                    self.write("\n");
                }
                internal::CssBlockChild::Rule(nested_rule) => {
                    // CSS Nesting Module - format nested rule
                    // Add blank line before nested rule if:
                    // - Previous is a declaration (transition from declarations to rules)
                    // - OR source has blank line between consecutive nested rules (preserve author intent)
                    // Don't add blank line if previous is comment (handles its own spacing)
                    if i > start_index && !Self::prev_is_comment(&rule.declarations, i) {
                        if Self::prev_is_rule(&rule.declarations, i) {
                            // Consecutive rules: only add blank line if source had one
                            if let Some(prev_span_end) = Self::prev_span_end(&rule.declarations, i)
                                && self.has_blank_line_between_spans(
                                    prev_span_end,
                                    nested_rule.span.start,
                                )
                            {
                                self.write("\n");
                            }
                        } else {
                            // Previous is declaration: always add blank line
                            self.write("\n");
                        }
                    }
                    self.write_indent();
                    self.print_css_rule(nested_rule);

                    // Check for inline comment after nested rule's closing brace
                    let inline_count =
                        self.try_print_inline_comments(&rule.declarations, i, nested_rule.span.end);

                    self.write("\n");

                    // Add blank line after nested rule if next sibling is a declaration
                    // (Don't add for comments - comment handles its own spacing)
                    let next_idx = i + 1 + inline_count;
                    if let Some(internal::CssBlockChild::Declaration(_)) =
                        rule.declarations.get(next_idx)
                    {
                        self.write("\n");
                    }

                    i += inline_count;
                }
                internal::CssBlockChild::Atrule(nested_atrule) => {
                    // Nested at-rule (e.g., @media inside a rule)
                    // Add blank line before nested at-rule if there's a previous sibling
                    // UNLESS the previous sibling was a comment (blank line already added)
                    if i > start_index && !Self::prev_is_comment(&rule.declarations, i) {
                        self.write("\n");
                    }
                    self.write_indent();
                    self.print_css_atrule(nested_atrule);
                    self.write("\n");

                    // Add blank line after nested at-rule if next sibling exists and is a declaration/comment
                    if let Some(next_child) = rule.declarations.get(i + 1)
                        && matches!(
                            next_child,
                            internal::CssBlockChild::Declaration(_)
                                | internal::CssBlockChild::Comment(_)
                        )
                    {
                        self.write("\n");
                    }
                }
            }
            i += 1;
        }
        self.indent_level -= 1;

        self.write_indent();
        self.write("}");
    }
}
