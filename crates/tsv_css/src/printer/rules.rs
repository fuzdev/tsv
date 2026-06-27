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
use tsv_lang::is_format_ignore_directive;

impl<'a> Printer<'a> {
    /// Format a CSS rule (selector + declarations block)
    pub(super) fn print_css_rule(&mut self, rule: &internal::CssRule<'_>) {
        // Format selector (uses selectors module)
        self.print_selector_list(&rule.selector);

        // Drop a hex escape's terminator whitespace (`.\1F600 ` before `{`) so it
        // doesn't double with the block separator written below.
        self.pop_selector_terminator();

        // Inline-print any comments between the selector and opening brace.
        // block_span.start is the position of the opening brace, so a leading
        // child whose span precedes it is a pre-brace comment.
        let mut start_index = 0;
        while let Some(internal::CssBlockChild::Comment(comment)) =
            rule.declarations.get(start_index)
        {
            if comment.span.start >= rule.block_span.start {
                break;
            }
            // Always add space before comment for readability (normalize)
            // This is an intentional divergence from prettier (which preserves no-space)
            self.write(" /*");
            self.write(comment.content(self.source));
            self.write("*/");
            start_index += 1; // Skip this comment when processing declarations
        }

        self.write(" {\n");

        // Format declarations and comments with indentation
        self.indent_level += 1;
        let mut i = start_index;
        let mut format_ignore_next = false;
        while i < rule.declarations.len() {
            let child = &rule.declarations[i];
            match child {
                internal::CssBlockChild::Declaration(decl) => {
                    // Preserve blank line before declaration if source has one
                    if i > start_index && self.has_blank_line_before_child(rule.declarations, i) {
                        self.write("\n");
                    }
                    let format_ignore = format_ignore_next;
                    format_ignore_next = false;
                    i += self.print_decl_with_inline_comments(
                        rule.declarations,
                        i,
                        decl,
                        format_ignore,
                    );
                }
                internal::CssBlockChild::Comment(comment) => {
                    // Standalone comment (not inline after a declaration)
                    // Preserve blank line before comment if present in source
                    if i > start_index && self.has_blank_line_before_child(rule.declarations, i) {
                        self.write("\n");
                    }

                    // Check for a format-ignore directive
                    if is_format_ignore_directive(comment.content(self.source)) {
                        format_ignore_next = true;
                    }

                    self.write_indent();
                    self.print_css_comment(comment);
                    self.write("\n");
                }
                internal::CssBlockChild::Rule(nested_rule) => {
                    // CSS Nesting Module - format nested rule
                    // Add blank line before nested rule if source has one (preserve author intent)
                    if i > start_index && self.has_blank_line_before_child(rule.declarations, i) {
                        self.write("\n");
                    }
                    self.write_indent();
                    if format_ignore_next {
                        self.write(nested_rule.span.extract(self.source));
                        format_ignore_next = false;
                    } else {
                        self.print_css_rule(nested_rule);
                    }

                    // Check for inline comment after nested rule's closing brace
                    let inline_count =
                        self.try_print_inline_comments(rule.declarations, i, nested_rule.span.end);

                    self.write("\n");

                    i += inline_count;
                }
                internal::CssBlockChild::Atrule(nested_atrule) => {
                    // Nested at-rule (e.g., @media inside a rule)
                    // Add blank line before nested at-rule only if source had one
                    if i > start_index && self.has_blank_line_before_child(rule.declarations, i) {
                        self.write("\n");
                    }
                    self.write_indent();
                    if format_ignore_next {
                        self.write(nested_atrule.span.extract(self.source));
                        format_ignore_next = false;
                    } else {
                        self.print_css_atrule(nested_atrule);
                    }
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
