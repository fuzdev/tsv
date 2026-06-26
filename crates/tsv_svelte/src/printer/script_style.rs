// Script and Style section formatting for Svelte components
//
// Handles the top-level <script> and <style> sections in .svelte files.
// These sections contain TypeScript/JS and CSS respectively,
// which are formatted using their dedicated printers.

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::TAB_WIDTH;
use tsv_lang::doc::{self, arena::DocId};

impl<'a> Printer<'a> {
    /// Format a Script tag
    ///
    /// Formats both regular `<script>` and `<script context="module">` tags.
    /// The TypeScript content is formatted using the TypeScript printer with indentation.
    ///
    /// Uses Doc-based integration instead of string post-processing. The Doc system
    /// naturally handles template literals correctly: `indent(doc)` only affects `Line` docs
    /// (hardline, softline, line), not newlines inside `text()` which output as-is.
    pub(super) fn print_script(&mut self, script: &internal::Script<'_>) {
        // Check if script had any original content (including whitespace)
        let had_content = script.content.span.start != script.content.span.end;

        // Opening tag with doc-based attribute wrapping
        self.write_section_opening_tag("script", script.attributes, had_content);

        if had_content {
            self.write("\n");
        }

        // Build Doc for script content
        // Width calculations are handled by:
        // - start_column for the first line
        // - start_indent_level for subsequent lines after hardline
        // Note: We use default embed (base_indent_offset=0) for accurate width calculations.
        // Template indent fallback (when source has no whitespace) is handled separately
        // in the TypeScript printer with a hardcoded default of 1 for Svelte context.
        let embed = tsv_lang::EmbedContext::default();
        let script_doc_id =
            tsv_ts::build_program_doc(self.d(), &script.content, self.source(), embed);

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
            &embed,
            TAB_WIDTH, // start column = 1 tab's visual width
            1,         // start indent level = 1 (accounts for Svelte wrapper)
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

    /// Get the `lang` or `type` attribute value from element attributes.
    /// Strips `text/` prefix (e.g., `type="text/less"` → `"less"`).
    /// Returns `None` if no `lang`/`type` attribute is present.
    pub(crate) fn get_lang_attribute(
        &self,
        attributes: &[internal::AttributeNode<'_>],
    ) -> Option<String> {
        let interner = self.interner.borrow();
        for attr_node in attributes {
            if let internal::AttributeNode::Attribute(attr) = attr_node {
                let name = interner.resolve(attr.name).unwrap_or("");
                if (name == "lang" || name == "type")
                    && let Some(value_parts) = attr.value
                {
                    for part in value_parts {
                        if let internal::AttributeValue::Text(text) = part {
                            let lang = text.raw(self.source).trim();
                            let lang = lang.strip_prefix("text/").unwrap_or(lang);
                            return Some(lang.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Format a Style tag
    ///
    /// Formats the `<style>` tag with its CSS content.
    /// The CSS is formatted using the CSS printer with indentation.
    /// For non-CSS languages (less, scss, etc.), content is preserved raw.
    pub(super) fn print_style(&mut self, style: &internal::Style<'_>) {
        // Check if there was any original content (including whitespace)
        let had_content = style.content_span.start != style.content_span.end;

        // Opening tag with doc-based attribute wrapping
        self.write_section_opening_tag("style", style.attributes, had_content);

        // Foreign languages (less, scss, etc.) — preserve content raw but normalize indentation
        if self
            .get_lang_attribute(style.attributes)
            .is_some_and(|l| l != "css")
        {
            if had_content {
                let content = style.content_span.extract(self.source()).to_string();
                // Collect non-empty lines (skip leading/trailing blank lines)
                let all_lines: Vec<&str> = content.lines().collect();
                let start = all_lines.iter().position(|l| !l.trim().is_empty());
                let end = all_lines.iter().rposition(|l| !l.trim().is_empty());
                match (start, end) {
                    (Some(start), Some(end)) => {
                        let lines = &all_lines[start..=end];

                        let leading_ws =
                            |line: &&str| -> usize { line.len() - line.trim_start().len() };

                        // Indentation levels from non-empty lines
                        let indents: Vec<usize> = lines
                            .iter()
                            .filter(|line| !line.trim().is_empty())
                            .map(leading_ws)
                            .collect();

                        let min_indent = indents.iter().copied().min().unwrap_or(0);

                        // Detect indent unit: smallest indent level above the base
                        let indent_unit = indents
                            .iter()
                            .copied()
                            .filter(|&i| i > min_indent)
                            .map(|i| i - min_indent)
                            .min()
                            .unwrap_or_else(|| 1.max(min_indent));

                        self.write("\n");
                        self.indent_level += 1;
                        for line in lines {
                            if line.trim().is_empty() {
                                self.write("\n");
                            } else {
                                let extra_levels =
                                    (leading_ws(line).saturating_sub(min_indent)) / indent_unit;
                                self.write_indent();
                                for _ in 0..extra_levels {
                                    self.write("\t");
                                }
                                self.write(line.trim_start());
                                self.write("\n");
                            }
                        }
                        self.indent_level -= 1;
                    }
                    _ => {
                        // Whitespace-only content — preserve block structure
                        self.write("\n");
                    }
                }
            }
            self.write("</style>\n");
            return;
        }

        // Format CSS content if present (nodes or comments)
        if !style.css_stylesheet.nodes.is_empty() || !style.css_stylesheet.comments.is_empty() {
            self.write("\n");

            // Pass the entire source to CSS printer (CSS node spans are absolute)
            // The CSS printer will use the spans to detect blank lines correctly
            // Use base_indent_offset=1 to account for the Svelte wrapper indent
            let embed = tsv_lang::EmbedContext {
                base_indent_offset: 1,
                ..tsv_lang::EmbedContext::default()
            };
            let formatted_css =
                tsv_css::format_embedded(&style.css_stylesheet, self.source(), embed);

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

    /// Build indented attribute docs and detect multiline values.
    ///
    /// Returns `(indent([line, attr1, line, attr2, ...]), has_multiline)`.
    /// Used by `write_section_opening_tag` (script/style) and `print_svelte_options`.
    pub(crate) fn build_indented_attrs_doc(
        &self,
        attributes: &[internal::AttributeNode<'_>],
    ) -> (DocId, bool) {
        let d = self.d();
        let mut parts = Vec::with_capacity(attributes.len() * 2);
        let mut has_multiline = false;
        for attr in attributes {
            parts.push(d.line());
            let attr_doc = self.build_attribute_node_doc(attr, false);
            if d.will_break(attr_doc) {
                has_multiline = true;
            }
            parts.push(attr_doc);
        }
        let concat = d.concat(&parts);
        (d.indent(concat), has_multiline)
    }

    /// Build and render an opening tag for `<script>` or `<style>` with doc-based
    /// attribute wrapping. Attributes wrap when any value contains embedded newlines
    /// or the total line width exceeds print_width.
    ///
    /// `has_content`: when false, the closing tag follows on the same line
    /// (e.g., `<script lang="ts"></script>`), so suffix_width accounts for it.
    ///
    /// Flat: `<tag attr1 attr2>`
    /// Break: `<tag\n\tattr1\n\tattr2\n>`
    fn write_section_opening_tag(
        &mut self,
        tag_name: &str,
        attributes: &[internal::AttributeNode<'_>],
        has_content: bool,
    ) {
        if attributes.is_empty() {
            self.write("<");
            self.write(tag_name);
            self.write(">");
            return;
        }

        let d = self.d();
        let (attr_indent, has_multiline) = self.build_indented_attrs_doc(attributes);
        let softline = d.softline();
        let tag_text = d.text_owned(format!("<{tag_name}"));
        let inner = d.concat(&[tag_text, attr_indent, softline, d.text(">")]);

        let group = if has_multiline {
            d.group_break(inner)
        } else {
            d.group(inner)
        };

        // When empty (no content), the closing tag follows on the same line:
        // `<script lang="ts"></script>`. Account for it via suffix_width so
        // fits() breaks when the full line exceeds print_width.
        let closing_tag_width = if !has_content {
            // "</tag>" = 3 + tag_name.len()
            3 + tag_name.len()
        } else {
            0
        };
        let col = self.buffer.current_column(TAB_WIDTH);
        let embed = tsv_lang::EmbedContext {
            suffix_width: closing_tag_width,
            ..self.embed
        };
        let output = {
            let interner = self.interner.borrow();
            doc::arena_print_doc_with_indent_resolved_preserve_whitespace(
                self.arena,
                group,
                &embed,
                col,
                self.indent_level,
                &*interner,
            )
        };
        self.write(&output);
    }
}
