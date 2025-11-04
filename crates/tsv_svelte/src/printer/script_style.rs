// Script and Style section formatting for Svelte components
//
// Handles the top-level <script> and <style> sections in .svelte files.
// These sections contain TypeScript/JS and CSS respectively,
// which are formatted using their dedicated printers.

use crate::ast::internal;
use crate::printer::Printer;

impl<'a> Printer<'a> {
    /// Format a Script tag
    ///
    /// Formats both regular `<script>` and `<script context="module">` tags.
    /// The TypeScript content is formatted using the TypeScript printer with indentation.
    pub(super) fn print_script(&mut self, script: &internal::Script) {
        // Opening tag
        self.write("<script");

        // Format attributes (includes "context" if this is a module script)
        for attr in &script.attributes {
            self.write(" ");
            self.print_attribute(attr);
        }

        self.write(">\n");

        // Format TypeScript content with indentation
        // Use the TypeScript printer from tsv_ts crate
        // IMPORTANT: TypeScript AST was parsed with base_offset, so spans are absolute
        // positions in the full Svelte source. Pass the full source for correct slicing.
        let formatted_content = tsv_ts::format(&script.content, self.source());

        // Indent each line - trim trailing newline first to avoid extra blank lines
        let content_trimmed = formatted_content.trim_end_matches('\n');
        self.indent_level += 1;
        for line in content_trimmed.lines() {
            // Don't indent blank lines (prettier outputs truly blank lines)
            if !line.is_empty() {
                self.write_indent();
            }
            self.write(line);
            self.write("\n");
        }
        self.indent_level -= 1;

        // Closing tag
        self.write("</script>\n");
    }

    /// Format a Style tag
    ///
    /// Formats the `<style>` tag with its CSS content.
    /// The CSS is formatted using the CSS printer with indentation.
    pub(super) fn print_style(&mut self, style: &internal::Style) {
        // Opening tag
        self.write("<style");

        // Format attributes
        for attr in &style.attributes {
            self.write(" ");
            self.print_attribute(attr);
        }

        self.write(">");

        // Format CSS content if present
        if !style.css_nodes.is_empty() {
            self.write("\n");

            // Pass the entire source to CSS printer (CSS node spans are absolute)
            // The CSS printer will use the spans to detect blank lines correctly
            let formatted_css = tsv_css::format(&style.css_nodes, self.source());

            // Indent each line - trim trailing newline first to avoid extra blank lines
            let css_trimmed = formatted_css.trim_end_matches('\n');
            self.indent_level += 1;
            let mut in_multiline_comment = false;
            for line in css_trimmed.lines() {
                let trimmed = line.trim_start();

                // Check if this is a comment continuation BEFORE updating state
                // (prettier preserves exact spacing in comment continuations)
                let is_comment_continuation = in_multiline_comment && !trimmed.starts_with("/*");

                // Track if we're inside a multi-line comment
                if trimmed.starts_with("/*") && !trimmed.contains("*/") {
                    in_multiline_comment = true;
                } else if in_multiline_comment && trimmed.contains("*/") {
                    in_multiline_comment = false;
                }

                // Don't indent blank lines or multi-line comment continuation lines
                if !line.is_empty() && !is_comment_continuation {
                    self.write_indent();
                }
                self.write(line);
                self.write("\n");
            }
            self.indent_level -= 1;
        }

        // Closing tag
        self.write("</style>\n");
    }
}
