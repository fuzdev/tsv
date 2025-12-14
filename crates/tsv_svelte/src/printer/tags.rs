//! Template tag printing (html, const, debug, render)

use std::rc::Rc;

use super::Printer;
use crate::ast::internal;
use tsv_lang::comments_in_range;

impl<'a> Printer<'a> {
    /// Format an html tag: {@html expr}
    pub(super) fn print_html_tag(&mut self, tag: &internal::HtmlTag) {
        self.write("{@html ");
        self.print_ts_expression(&tag.expression);
        self.write("}");
    }

    /// Format a const tag: {@const name = expr}
    pub(super) fn print_const_tag(&mut self, tag: &internal::ConstTag) {
        self.write("{@const ");
        // Format the id (pattern) with current indent level for multiline patterns
        let formatted_id = tsv_ts::format_expression_with_indent(
            &tag.id,
            self.source,
            Rc::clone(&self.interner),
            self.indent_level,
        );
        self.write(&formatted_id);
        self.write(" = ");
        // Format the init expression
        let formatted_init = tsv_ts::format_expression_with_indent(
            &tag.init,
            self.source,
            Rc::clone(&self.interner),
            self.indent_level,
        );
        self.write(&formatted_init);
        self.write("}");
    }

    /// Format a debug tag: {@debug} or {@debug x, y, z}
    ///
    /// Unlike Prettier (which strips comments), we preserve TS comments.
    /// Comments are looked up from Root.comments by span position.
    pub(super) fn print_debug_tag(&mut self, tag: &internal::DebugTag) {
        self.write("{@debug");

        // Get comments within the tag's content (after "{@debug" and before "}")
        // The tag span includes the full `{@debug ... }`, so we look inside
        let tag_comments: Vec<_> =
            comments_in_range(self.comments, tag.span.start, tag.span.end).collect();

        if tag.identifiers.is_empty() && tag_comments.is_empty() {
            // Just {@debug} with no identifiers or comments
            self.write("}");
            return;
        }

        self.write(" ");

        // Track position as we emit content
        // Start after "{@debug " (7 characters from tag start)
        let mut last_end = tag.span.start + 7; // "{@debug" = 7 chars

        for (i, id) in tag.identifiers.iter().enumerate() {
            if i > 0 {
                self.write(", ");
                last_end += 2; // ", "
            }

            // Emit any comments that appear before this identifier
            for comment in &tag_comments {
                if comment.span.start >= last_end && comment.span.end <= id.span().start {
                    if comment.is_block {
                        self.write("/*");
                        self.write(&comment.content);
                        self.write("*/ ");
                    } else {
                        self.write("//");
                        self.write(&comment.content);
                        self.write("\n");
                    }
                    last_end = comment.span.end;
                }
            }

            self.print_ts_expression(id);
            last_end = id.span().end;
        }

        // Emit any trailing comments (after last identifier)
        for comment in &tag_comments {
            if comment.span.start >= last_end {
                self.write(" ");
                if comment.is_block {
                    self.write("/*");
                    self.write(&comment.content);
                    self.write("*/");
                } else {
                    self.write("//");
                    self.write(&comment.content);
                }
            }
        }

        self.write("}");
    }

    /// Format a render tag: {@render fn()} or {@render fn?.()}
    pub(super) fn print_render_tag(&mut self, tag: &internal::RenderTag) {
        self.write("{@render ");
        self.print_ts_expression(&tag.expression);
        self.write("}");
    }
}
