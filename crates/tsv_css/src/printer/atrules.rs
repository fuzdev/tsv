// CSS at-rule formatting
//
// Handles formatting of:
// - At-rules (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
// - At-rule blocks and their children (rules, declarations, nested at-rules)

use super::Printer;
use crate::ast::internal;

impl<'a> Printer<'a> {
    /// Format a CSS at-rule (@media, @keyframes, @supports, etc.)
    pub(super) fn print_css_atrule(&mut self, atrule: &internal::CssAtrule) {
        self.write("@");
        self.write(&atrule.name);

        // Only add space before prelude if prelude is not empty
        if !atrule.prelude.is_empty() {
            self.write(" ");
            self.write(&atrule.prelude);
        }

        if let Some(block) = &atrule.block {
            self.write(" {\n");
            self.indent_level += 1;

            for (i, child) in block.children.iter().enumerate() {
                // For non-first children, add newline before rules/at-rules/comments
                if i > 0 {
                    match child {
                        internal::CssBlockChild::Declaration(_) => {
                            // Declarations already end with \n, no extra newline needed
                        }
                        internal::CssBlockChild::Rule(_)
                        | internal::CssBlockChild::Atrule(_)
                        | internal::CssBlockChild::Comment(_) => {
                            // Check if there's a blank line between children in the source
                            let has_blank_line = {
                                let source = self.source;
                                let prev_child = &block.children[i - 1];
                                let prev_end = prev_child.span().end as usize;
                                let curr_start = child.span().start as usize;
                                if prev_end < curr_start && curr_start <= source.len() {
                                    let between = &source[prev_end..curr_start];
                                    between.matches('\n').count() >= 2
                                } else {
                                    false
                                }
                            };

                            if has_blank_line {
                                self.write("\n");
                            }
                            self.write("\n");
                        }
                    }
                }

                // Format the child with appropriate indentation handling
                match child {
                    internal::CssBlockChild::Declaration(_) => {
                        // Declaration will write its own indentation
                        self.print_atrule_block_child(child);
                    }
                    _ => {
                        // Rules, at-rules, and comments need indentation from here
                        self.write_indent();
                        self.print_atrule_block_child(child);
                    }
                }
            }

            self.indent_level -= 1;

            // Only write newline if the last child wasn't a declaration
            // (declarations end with \n already)
            if !matches!(
                block.children.last(),
                Some(internal::CssBlockChild::Declaration(_))
            ) {
                self.write("\n");
            }
            self.write_indent();
            self.write("}");
        } else {
            self.write(";");
        }
    }

    /// Format an at-rule block child (rule, declaration, or nested at-rule)
    fn print_atrule_block_child(&mut self, child: &internal::CssBlockChild) {
        match child {
            internal::CssBlockChild::Rule(rule) => {
                // Format rule selector and opening brace
                self.print_selector_list(&rule.selector);

                // Check if first child is a comment after selector (before {)
                let mut start_index = 0;
                if let Some(internal::CssBlockChild::Comment(comment)) = rule.declarations.first() {
                    // Check if comment is on same line as selector (inline after selector)
                    if self.is_same_line(rule.selector.span.end, comment.span.start) {
                        // Print comment inline after selector
                        self.write(" /*");
                        self.write(&comment.content);
                        self.write("*/");
                        start_index = 1; // Skip this comment when processing declarations
                    }
                }

                self.write(" {\n");

                // Format declarations and comments with proper indentation
                self.indent_level += 1;
                let mut i = start_index;
                while i < rule.declarations.len() {
                    let child = &rule.declarations[i];
                    match child {
                        internal::CssBlockChild::Declaration(decl) => {
                            self.print_css_declaration(decl);

                            // Check if next child is an inline comment
                            if let Some(internal::CssBlockChild::Comment(next_comment)) =
                                rule.declarations.get(i + 1)
                                && self.is_same_line(decl.span.end, next_comment.span.start) {
                                    // Print comment inline
                                    self.buffer_remove_trailing_newline();
                                    self.write(" /*");
                                    self.write(&next_comment.content);
                                    self.write("*/\n");
                                    i += 1; // Skip the comment in the next iteration
                                }
                        }
                        internal::CssBlockChild::Comment(comment) => {
                            // Standalone comment (not inline after a declaration)
                            self.write_indent();
                            self.print_css_comment(comment);
                            self.write("\n");
                        }
                        internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_) => {
                            // Nested rules not expected here
                        }
                    }
                    i += 1;
                }
                self.indent_level -= 1;

                // Closing brace at current indentation level (inside at-rule)
                self.write_indent();
                self.write("}");
            }
            internal::CssBlockChild::Declaration(decl) => self.print_css_declaration(decl),
            internal::CssBlockChild::Atrule(atrule) => self.print_css_atrule(atrule),
            internal::CssBlockChild::Comment(comment) => self.print_css_comment(comment),
        }
    }
}
