// Script and Style section formatting for Svelte components
//
// Handles the top-level <script> and <style> sections in .svelte files.
// These sections contain TypeScript/JS and CSS respectively,
// which are formatted using their dedicated printers.

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::doc;

impl<'a> Printer<'a> {
    /// Format a Script tag
    ///
    /// Formats both regular `<script>` and `<script context="module">` tags.
    /// The TypeScript content is formatted using the TypeScript printer with indentation.
    ///
    /// Uses Doc-based integration instead of string post-processing. The Doc system
    /// naturally handles template literals correctly: `indent(doc)` only affects `Line` docs
    /// (hardline, softline, line), not newlines inside `text()` which output as-is.
    pub(super) fn print_script(&mut self, script: &internal::Script) {
        // Opening tag
        self.write("<script");

        // Format attributes (includes "context" if this is a module script)
        for attr in &script.attributes {
            self.write(" ");
            self.print_attribute_node(attr);
        }

        // Check if script had any original content (including whitespace)
        let had_content = script.content.span.start != script.content.span.end;

        if had_content {
            self.write(">\n");
        } else {
            self.write(">");
        }

        // Build Doc for script content
        // Width calculations are handled by:
        // - start_column for the first line
        // - start_indent_level for subsequent lines after hardline
        // Note: We use default config (base_indent_offset=0) for accurate width calculations.
        // Template indent fallback (when source has no whitespace) is handled separately
        // in the TypeScript printer with a hardcoded default of 1 for Svelte context.
        let config = tsv_lang::PrintConfig::default();
        let script_doc_id =
            tsv_ts::build_program_doc(self.d(), &script.content, self.source(), config);

        // Render with indent
        // The Doc system naturally handles template literals: text() newlines are NOT indented
        // We render at indent_level=1 so hardlines produce proper indentation
        // The first line needs manual indentation since there's no hardline before it
        //
        // start_column = tab_width (2) to account for the initial indent we'll add
        // start_indent_level = 1 to account for the Svelte wrapper indent
        let interner = script.content.interner.borrow();
        let output = doc::arena_print_doc_with_indent_resolved(
            self.d(),
            script_doc_id,
            &config,
            config.tab_width, // start column = 1 tab's visual width
            1,                // start indent level = 1 (accounts for Svelte wrapper)
            &*interner,
        );

        // Only write content if there is any (skip indent for empty scripts)
        // The output always ends with a hardline (\n), so non-empty content is at least "\n"
        // For empty content (just comments or empty body), output is just "\n"
        if !output.trim().is_empty() {
            // Write first line's indent manually (doc only indents after hardlines)
            self.indent_level += 1;
            self.write_indent();
            self.indent_level -= 1;
            self.write(&output);
        }

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

        // Check if there was any original content (including whitespace)
        let had_content = style.content_span.start != style.content_span.end;

        // Format CSS content if present (nodes or comments)
        if !style.css_stylesheet.nodes.is_empty() || !style.css_stylesheet.comments.is_empty() {
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
        } else if had_content {
            // Preserve block structure when original had whitespace-only content
            self.write("\n");
        }

        // Closing tag
        self.write("</style>\n");
    }
}
