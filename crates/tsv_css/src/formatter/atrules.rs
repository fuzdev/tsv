// CSS at-rule formatting
//
// Handles formatting of:
// - At-rules (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
// - At-rule blocks and their children (rules, declarations, nested at-rules)

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format a CSS at-rule (@media, @keyframes, @supports, etc.)
    pub(super) fn format_css_atrule(&mut self, atrule: &internal::CssAtrule) {
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
                            let has_blank_line = if let Some(source) = &self.source {
                                let prev_child = &block.children[i - 1];
                                let prev_end = prev_child.span().end as usize;
                                let curr_start = child.span().start as usize;
                                if prev_end < curr_start && curr_start <= source.len() {
                                    let between = &source[prev_end..curr_start];
                                    between.matches('\n').count() >= 2
                                } else {
                                    false
                                }
                            } else {
                                false
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
                        self.format_atrule_block_child(child);
                    }
                    _ => {
                        // Rules, at-rules, and comments need indentation from here
                        self.write_indent();
                        self.format_atrule_block_child(child);
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
    fn format_atrule_block_child(&mut self, child: &internal::CssBlockChild) {
        match child {
            internal::CssBlockChild::Rule(rule) => {
                // Format rule selector and opening brace
                self.format_selector_list(&rule.selector);
                self.write(" {\n");

                // Format declarations with proper indentation
                self.indent_level += 1;
                for decl in &rule.declarations {
                    self.format_css_declaration(decl);
                }
                self.indent_level -= 1;

                // Closing brace at current indentation level (inside at-rule)
                self.write_indent();
                self.write("}");
            }
            internal::CssBlockChild::Declaration(decl) => self.format_css_declaration(decl),
            internal::CssBlockChild::Atrule(atrule) => self.format_css_atrule(atrule),
            internal::CssBlockChild::Comment(comment) => self.format_css_comment(comment),
        }
    }
}
