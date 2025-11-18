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

        // Print prelude based on type
        match &atrule.prelude {
            internal::PreludeValue::Values { values, .. } if !values.is_empty() => {
                self.write(" ");
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(" ");
                    }
                    // Use semantic formatting to normalize quotes and spacing
                    self.print_css_value_semantic(value);
                }
            }
            internal::PreludeValue::Raw { content, .. } if !content.is_empty() => {
                self.write(" ");
                let normalized = self.normalize_comment_spacing(content);
                self.write(&normalized);
            }
            internal::PreludeValue::Selectors { root, limit, .. } => {
                // @scope selector lists: @scope (root) to (limit)
                self.write(" (");
                self.print_selector_list(root);
                self.write(")");
                if let Some(limit_selectors) = limit {
                    self.write(" to (");
                    self.print_selector_list(limit_selectors);
                    self.write(")");
                }
            }
            _ => {}
        }

        if let Some(block) = &atrule.block {
            self.write(" {\n");
            self.indent_level += 1;

            let mut i = 0;
            while i < block.children.len() {
                let child = &block.children[i];

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
                            let prev_child = &block.children[i - 1];
                            let has_blank_line = {
                                let source = self.source;
                                let prev_end = prev_child.span().end as usize;
                                let curr_start = child.span().start as usize;
                                if prev_end < curr_start && curr_start <= source.len() {
                                    let between = &source[prev_end..curr_start];
                                    between.matches('\n').count() >= 2
                                } else {
                                    false
                                }
                            };

                            // Declarations end with \n, but rules/at-rules end with }
                            // So only add separator newline if prev is not a declaration
                            let prev_is_declaration =
                                matches!(prev_child, internal::CssBlockChild::Declaration(_));

                            if !prev_is_declaration {
                                self.write("\n"); // Separator
                            }
                            if has_blank_line {
                                self.write("\n"); // Blank line
                            }
                        }
                    }
                }

                // Format the child with appropriate indentation handling
                match child {
                    internal::CssBlockChild::Declaration(_) => {
                        // Declaration will write its own indentation
                        self.print_atrule_block_child(child);
                    }
                    internal::CssBlockChild::Rule(_) | internal::CssBlockChild::Atrule(_) => {
                        // Rules and at-rules need indentation
                        self.write_indent();
                        self.print_atrule_block_child(child);

                        // Check if next child is an inline comment (no newline between } and /*)
                        if let Some(internal::CssBlockChild::Comment(next_comment)) =
                            block.children.get(i + 1)
                        {
                            let child_end = child.span().end as usize;
                            let comment_start = next_comment.span.start as usize;

                            // Extract text between closing brace and comment
                            let has_newline_between = if child_end < comment_start && comment_start <= self.source.len() {
                                self.source[child_end..comment_start].contains('\n')
                            } else {
                                false
                            };

                            // Only print inline if there's NO newline between them in source
                            // This preserves user's formatting choice (inline vs standalone)
                            if !has_newline_between {
                                // Print comment inline after closing brace
                                self.write(" /*");
                                self.write(&next_comment.content);
                                self.write("*/");
                                i += 1; // Skip the comment in the next iteration
                            }
                        }
                    }
                    internal::CssBlockChild::Comment(_) => {
                        // Standalone comment
                        self.write_indent();
                        self.print_atrule_block_child(child);
                    }
                }

                i += 1;
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
                    // Check if comment is on same line as selector AND before the opening brace
                    // If there's a '{' between selector and comment, the comment is inside the block, not after selector
                    if self.is_same_line(rule.selector.span.end, comment.span.start)
                        && !self.has_opening_brace_between(rule.selector.span.end, comment.span.start)
                    {
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
                                && self.is_same_line(decl.span.end, next_comment.span.start)
                            {
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
                            // Check if there's a blank line before this comment in source
                            if i > start_index
                                && let Some(prev_child) = rule.declarations.get(i - 1)
                                && self.has_blank_line_between_spans(
                                    prev_child.span().end,
                                    comment.span.start,
                                )
                            {
                                // Source has blank line - add it
                                // Note: Previous element already ended with \n, so one more \n gives blank line
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_comment(comment);
                            self.write("\n");
                        }
                        internal::CssBlockChild::Rule(nested_rule) => {
                            // CSS Nesting Module - format nested rule inside at-rule block rule
                            let prev_is_comment = i > 0
                                && matches!(
                                    rule.declarations.get(i - 1),
                                    Some(internal::CssBlockChild::Comment(_))
                                );
                            if i > start_index && !prev_is_comment {
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_rule(nested_rule);

                            // Check for inline comment after nested rule's closing brace
                            let mut has_inline_comment = false;
                            if let Some(internal::CssBlockChild::Comment(next_comment)) =
                                rule.declarations.get(i + 1)
                                && self.is_same_line(nested_rule.span.end, next_comment.span.start)
                            {
                                self.write(" /*");
                                self.write(&next_comment.content);
                                self.write("*/");
                                has_inline_comment = true;
                            }

                            self.write("\n");

                            // Add blank line after nested rule if next sibling is a declaration
                            // (Don't add for comments - comment handles its own spacing)
                            if let Some(next_child) = rule.declarations.get(i + 1)
                                && let internal::CssBlockChild::Declaration(_) = next_child
                            {
                                self.write("\n");
                            }

                            if has_inline_comment {
                                i += 1;
                            }
                        }
                        internal::CssBlockChild::Atrule(nested_atrule) => {
                            // Nested at-rule inside rule
                            let prev_is_comment = i > 0
                                && matches!(
                                    rule.declarations.get(i - 1),
                                    Some(internal::CssBlockChild::Comment(_))
                                );
                            if i > start_index && !prev_is_comment {
                                self.write("\n");
                            }
                            self.write_indent();
                            self.print_css_atrule(nested_atrule);
                            self.write("\n");

                            // Add blank line after nested at-rule if next sibling is a declaration
                            // (Don't add for comments - comment handles its own spacing)
                            if let Some(next_child) = rule.declarations.get(i + 1)
                                && matches!(next_child, internal::CssBlockChild::Declaration(_))
                            {
                                self.write("\n");
                            }
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
