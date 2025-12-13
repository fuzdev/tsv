// Script and Style section formatting for Svelte components
//
// Handles the top-level <script> and <style> sections in .svelte files.
// These sections contain TypeScript/JS and CSS respectively,
// which are formatted using their dedicated printers.

use crate::ast::internal;
use crate::printer::Printer;
use crate::printer::helpers::is_inside_template_literal;

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
            self.print_attribute_node(attr);
        }

        self.write(">\n");

        // Format TypeScript content with indentation
        // Use the TypeScript printer from tsv_ts crate
        // IMPORTANT: TypeScript AST was parsed with base_offset, so spans are absolute
        // positions in the full Svelte source. Pass the full source for correct slicing.
        // Use base_indent_offset=1 to account for the Svelte wrapper indent (width calculations)
        let config = tsv_lang::PrintConfig {
            base_indent_offset: 1,
            ..Default::default()
        };
        let formatted_content = tsv_ts::format_with_config(&script.content, self.source(), config);

        // Indent each line - trim trailing newline first to avoid extra blank lines
        // IMPORTANT: Don't add indentation to:
        // 1. String continuation lines (backslash + newline)
        // 2. Lines inside template literals (they're part of the template content)
        // 3. Multi-line block comment continuation lines (prettier preserves original spacing)
        // These must be preserved exactly as they are.
        let content_trimmed = formatted_content.trim_end_matches('\n');
        self.indent_level += 1;
        let mut is_continuation_line = false;
        let mut in_template_literal = false;
        let mut in_multiline_comment = false;
        for line in content_trimmed.lines() {
            let trimmed = line.trim_start();

            // Check if this is a comment line that should NOT be indented:
            // - We're inside a multi-line comment
            // - The line starts at column 0 (no leading whitespace)
            // - This includes closing `*/` lines that start at column 0
            // This preserves prettier's behavior where comment lines at column 0 stay at column 0
            // NOTE: Check this BEFORE updating in_multiline_comment state
            let is_unindented_comment_line = in_multiline_comment && line == trimmed;

            // Track if we're inside a multi-line comment
            // A multi-line comment starts when we see /* without a matching */ on the same line
            // Note: This is a simplified check that doesn't handle nested comments
            // or comments inside strings/templates, but works for typical cases
            if !in_template_literal && trimmed.contains("/*") && !trimmed.contains("*/") {
                in_multiline_comment = true;
            } else if in_multiline_comment && trimmed.contains("*/") {
                in_multiline_comment = false;
            }

            // Don't indent:
            // - Blank lines
            // - Continuation lines inside strings
            // - Lines that are inside template literals (actual newlines in template content)
            // - Multi-line block comment lines that start at column 0 (no leading whitespace)
            let should_indent = !line.is_empty()
                && !is_continuation_line
                && !in_template_literal
                && !is_unindented_comment_line;
            if should_indent {
                self.write_indent();
            }
            self.write(line);
            self.write("\n");

            // Check if this line ends with a line continuation (backslash at end)
            // If so, the next line is a continuation and should NOT be indented
            is_continuation_line = tsv_lang::printing::is_line_continuation_ending(line);

            // Track template literal state by counting unescaped backticks
            // Lines inside multi-line template literals should not be indented
            in_template_literal = is_inside_template_literal(line, in_template_literal);
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
            self.print_attribute_node(attr);
        }

        self.write(">");

        // Format CSS content if present
        if !style.css_stylesheet.nodes.is_empty() {
            self.write("\n");

            // Pass the entire source to CSS printer (CSS node spans are absolute)
            // The CSS printer will use the spans to detect blank lines correctly
            // Use base_indent_offset=1 to account for the Svelte wrapper indent
            let config = tsv_lang::PrintConfig {
                base_indent_offset: 1,
                ..Default::default()
            };
            let formatted_css =
                tsv_css::format_with_config(&style.css_stylesheet, self.source(), config);

            // Indent each line - trim trailing newlines first to avoid extra blank lines
            // Note: CSS formatter adds trailing newline, we need to remove it before line processing
            let css_trimmed = formatted_css.trim_end();
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
