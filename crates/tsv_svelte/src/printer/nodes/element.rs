// Element-specific formatting for Svelte templates
//
// Handles formatting of HTML/Svelte elements with context-aware decisions about
// whether to format children in multiline vs compact mode.
//
// ## Formatting Modes
//
// 1. **Void elements**: Self-closing (`<br />`)
// 2. **Empty components/foreign**: Preserve author's self-closing choice
// 3. **Hug mode**: Special `><` pattern for inline/component parents with block children
// 4. **Multiline**: Indented children with newlines
// 5. **Compact**: Single line

use crate::ast::internal::{self, ElementKind, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_html as html;
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Format a Svelte element with context-aware formatting
    pub fn print_element(&mut self, element: &internal::Element) {
        // Determine element characteristics
        let (is_block, preserves_ws, is_void, is_foreign) =
            self.with_resolved_symbol(element.name, |tag| {
                (
                    html::is_block_element(tag),
                    html::preserves_whitespace(tag),
                    html::is_void_element(tag),
                    html::is_foreign_element(tag),
                )
            });

        let is_component = element.kind == ElementKind::Component;
        let is_block = is_block || is_component;
        let is_block_for_hug = is_block && !is_component;

        // Opening tag
        let tag_name = self.resolve_symbol(element.name);
        self.write("<");
        self.write(&tag_name);

        // Attributes
        let attrs_multiline = self.print_attributes_with_wrapping(element, &tag_name, is_void);

        // Handle void elements
        if is_void {
            self.write_void_closing(attrs_multiline);
            return;
        }

        // Handle empty components/foreign elements (preserve author's self-closing choice)
        if (is_component || is_foreign)
            && element.fragment.nodes.is_empty()
            && self.was_self_closing(element)
        {
            self.write_self_closing(attrs_multiline);
            return;
        }

        // Handle nested <style> and <script> elements - format content as CSS/JS
        if tag_name == "style" || tag_name == "script" {
            self.print_raw_content_element(&tag_name, element, attrs_multiline);
            return;
        }

        // Determine formatting mode
        let use_hug_mode = self.needs_hug_mode(element, is_block, attrs_multiline);
        let hug_start = self.should_hug_start(element, is_block_for_hug);
        let hug_end = self.should_hug_end(element, is_block_for_hug);
        let multiline = !use_hug_mode
            && self.should_format_multiline(element, is_block, preserves_ws, is_foreign);

        // Format based on mode
        if use_hug_mode {
            self.print_hug_mode(
                &tag_name,
                element,
                preserves_ws,
                hug_start,
                hug_end,
                attrs_multiline,
            );
        } else {
            self.print_normal_mode(
                &tag_name,
                element,
                is_block,
                preserves_ws,
                multiline,
                attrs_multiline,
            );
        }
    }

    // =========================================================================
    // Closing tag helpers
    // =========================================================================

    /// Write void element closing: ` />` or `\n/>`
    fn write_void_closing(&mut self, attrs_multiline: bool) {
        if attrs_multiline {
            self.write("\n");
            self.write("/>");
        } else {
            self.write(" />");
        }
    }

    /// Write self-closing tag: ` />` or `\n\t/>`
    fn write_self_closing(&mut self, attrs_multiline: bool) {
        if attrs_multiline {
            self.write("\n");
            self.write_indent();
            self.write("/>");
        } else {
            self.write(" />");
        }
    }

    /// Check if element was self-closing in source
    fn was_self_closing(&self, element: &internal::Element) -> bool {
        element.span.extract(self.source).trim_end().ends_with("/>")
    }

    /// Write a closing tag: `</tag>`
    fn write_closing_tag(&mut self, tag_name: &str) {
        self.write("</");
        self.write(tag_name);
        self.write(">");
    }

    /// Write a split closing tag: `</tag\n>` (for hug end pattern)
    fn write_split_closing_tag(&mut self, tag_name: &str) {
        self.write("</");
        self.write(tag_name);
        self.write("\n");
        self.write_indent();
        self.write(">");
    }

    // =========================================================================
    // Hug mode formatting
    // =========================================================================

    /// Format element in hug mode (one of 4 patterns based on hug_start/hug_end)
    fn print_hug_mode(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
        hug_start: bool,
        hug_end: bool,
        attrs_multiline: bool,
    ) {
        match (hug_start, hug_end) {
            (true, true) => self.print_hug_both(tag_name, element, preserves_ws, attrs_multiline),
            (true, false) => self.print_hug_start_only(tag_name, element, preserves_ws),
            (false, true) => self.print_hug_end_only(tag_name, element, preserves_ws),
            (false, false) => self.print_hug_neither(tag_name, element, preserves_ws),
        }
    }

    /// Hug both: `><content</tag\n>`
    fn print_hug_both(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
        attrs_multiline: bool,
    ) {
        self.print_hug_mode_element(
            tag_name,
            &element.fragment.nodes,
            preserves_ws,
            true,
            true,
            attrs_multiline,
        );
    }

    /// Hug start only: `\n\t><content>\n</tag>`
    fn print_hug_start_only(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
    ) {
        // Opening: indented > on new line
        self.write("\n");
        self.indent_level += 1;
        self.write_indent();
        self.write(">");

        // Children: print inline, skipping whitespace-only text
        self.print_non_ws_children_inline(&element.fragment.nodes, preserves_ws);

        // Closing: normal closing tag on new line
        self.write("\n");
        self.indent_level -= 1;
        self.write_indent();
        self.write_closing_tag(tag_name);
    }

    /// Hug end only: `>\n\t<content></tag\n>`
    fn print_hug_end_only(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
    ) {
        // Opening: normal with newline
        self.write("\n");
        self.write(">");

        // Children: multiline but no trailing newline
        self.write("\n");
        self.indent_level += 1;
        self.print_children_no_trailing_newline(&element.fragment.nodes, preserves_ws);
        self.indent_level -= 1;

        // Closing: split closing tag
        self.write_split_closing_tag(tag_name);
    }

    /// Hug neither: `>\n\t<content>\n</tag>`
    fn print_hug_neither(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
    ) {
        self.write("\n");
        self.write(">");
        self.print_multiline_children(&element.fragment.nodes, preserves_ws);
        self.write_closing_tag(tag_name);
    }

    /// Print children inline, skipping whitespace-only text nodes
    fn print_non_ws_children_inline(&mut self, nodes: &[FragmentNode], preserves_ws: bool) {
        for node in nodes {
            if !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()) {
                self.print_fragment_node(node, false, preserves_ws);
            }
        }
    }

    /// Print children with indentation but no trailing newline (for hug end)
    fn print_children_no_trailing_newline(&mut self, nodes: &[FragmentNode], preserves_ws: bool) {
        let mut first = true;
        for (i, node) in nodes.iter().enumerate() {
            // Skip leading whitespace-only text
            if first && matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()) {
                continue;
            }
            first = false;

            self.write_indent();
            self.print_fragment_node(node, false, preserves_ws);

            // Newline before next node, but not after last
            if i < nodes.len() - 1 {
                self.write("\n");
            }
        }
    }

    // =========================================================================
    // Normal mode formatting
    // =========================================================================

    /// Format element in normal mode (multiline or compact)
    fn print_normal_mode(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        is_block: bool,
        preserves_ws: bool,
        multiline: bool,
        attrs_multiline: bool,
    ) {
        // Check for special "wrapped attrs + single block child" pattern
        let is_wrapped_single_block = self.is_wrapped_single_block(element, attrs_multiline);

        // Opening >
        if attrs_multiline && !is_wrapped_single_block {
            self.write("\n");
        }
        self.write(">");

        // Check for split closing tag patterns
        let use_split = is_wrapped_single_block
            || (multiline && self.has_opening_newline_only(&element.fragment.nodes));

        if use_split {
            self.print_split_closing(tag_name, element, preserves_ws, is_wrapped_single_block);
        } else if multiline {
            self.print_multiline_children(&element.fragment.nodes, preserves_ws);
            self.write_closing_tag(tag_name);
        } else {
            self.print_compact_children(&element.fragment, is_block, preserves_ws);
            self.write_closing_tag(tag_name);
        }
    }

    /// Check if element is "wrapped attrs + single native block child" pattern
    fn is_wrapped_single_block(&self, element: &internal::Element, attrs_multiline: bool) -> bool {
        if !attrs_multiline {
            return false;
        }

        // Check if source has opening newline (preserve author intent)
        let has_opening_newline = element.fragment.nodes.first().is_some_and(|node| {
            matches!(node, FragmentNode::Text(text)
                if text.raw.is_whitespace_only() && text.raw.contains('\n'))
        });

        if has_opening_newline {
            return false;
        }

        // Check for exactly 1 native block child
        let non_ws: Vec<_> = element
            .fragment
            .nodes
            .iter()
            .filter(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()))
            .collect();

        non_ws.len() == 1
            && matches!(non_ws.first(), Some(FragmentNode::Element(el)) if self.is_block_element(el))
    }

    /// Print with split closing tag pattern
    fn print_split_closing(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        preserves_ws: bool,
        is_wrapped_single_block: bool,
    ) {
        if !is_wrapped_single_block {
            self.write("\n");
            self.indent_level += 1;
            self.write_indent();
        }

        // Print content (skip leading whitespace)
        for (i, node) in element.fragment.nodes.iter().enumerate() {
            if i == 0 && matches!(node, FragmentNode::Text(t) if t.raw.is_whitespace_only()) {
                continue;
            }
            self.print_fragment_node(node, false, preserves_ws);
        }

        // Split closing tag
        self.write("</");
        self.write(tag_name);
        if !is_wrapped_single_block {
            self.indent_level -= 1;
        }
        self.write("\n");
        self.write_indent();
        self.write(">");
    }

    // =========================================================================
    // Formatting decision helpers
    // =========================================================================

    /// Determine if element children should be formatted multiline
    pub fn should_format_multiline(
        &self,
        element: &internal::Element,
        is_block: bool,
        preserves_ws: bool,
        is_foreign: bool,
    ) -> bool {
        if preserves_ws {
            return false;
        }
        if !is_block && !is_foreign {
            return false;
        }

        let fragment = &element.fragment;

        // Rule 1: Block parent with multiple block children or mixed block + text
        let element_count = fragment
            .nodes
            .iter()
            .filter(|n| matches!(n, FragmentNode::Element(_)))
            .count();
        let has_block_children = self.has_block_elements(fragment);
        let has_text = self.has_text_content(fragment);
        let needs_separation = has_block_children && (element_count > 1 || has_text);

        // Rule 2: Source has newlines before last content
        let has_source_breaks = {
            let last_idx = fragment
                .nodes
                .iter()
                .rposition(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()));
            last_idx.is_some_and(|idx| {
                fragment.nodes[..idx]
                    .iter()
                    .any(|n| matches!(n, FragmentNode::Text(t) if t.raw.contains('\n')))
            })
        };

        // Rule 3: Contains <script> or <style> with content (their formatted output is multiline)
        let has_script_or_style_with_content = fragment.nodes.iter().any(|n| {
            matches!(n, FragmentNode::Element(el) if {
                let tag = self.resolve_symbol(el.name);
                (tag == "script" || tag == "style") && !el.fragment.nodes.is_empty()
            })
        });

        needs_separation || has_source_breaks || has_script_or_style_with_content
    }

    /// Determine if element needs hug mode formatting
    pub fn needs_hug_mode(
        &self,
        element: &internal::Element,
        is_block: bool,
        attrs_wrapped: bool,
    ) -> bool {
        // Only inline elements and components can use hug mode
        if is_block && element.kind != ElementKind::Component {
            return false;
        }

        let non_ws: Vec<_> = element
            .fragment
            .nodes
            .iter()
            .filter(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()))
            .collect();

        if non_ws.is_empty() {
            return false;
        }

        let all_native_block = non_ws
            .iter()
            .all(|n| matches!(n, FragmentNode::Element(el) if self.is_block_element(el)));
        let all_components = non_ws
            .iter()
            .all(|n| matches!(n, FragmentNode::Element(el) if el.kind == ElementKind::Component));

        let should_hug = if all_native_block {
            non_ws.len() >= 2 || (non_ws.len() == 1 && attrs_wrapped)
        } else if all_components {
            attrs_wrapped && !non_ws.is_empty()
        } else {
            false
        };

        if !should_hug {
            return false;
        }

        // Only use hug mode if children are compact (no content newlines)
        !element.fragment.nodes.iter().any(|n| {
            matches!(n, FragmentNode::Text(t) if t.raw.contains('\n') && !t.raw.is_whitespace_only())
        })
    }

    /// Check if element has "opening newline only" pattern
    fn has_opening_newline_only(&self, nodes: &[FragmentNode]) -> bool {
        if nodes.is_empty() {
            return false;
        }
        let has_opening_nl = matches!(nodes.first(), Some(FragmentNode::Text(t))
            if t.raw.is_whitespace_only() && t.raw.contains('\n'));
        let has_trailing_text = matches!(nodes.last(), Some(FragmentNode::Text(_)));
        has_opening_nl && !has_trailing_text
    }

    /// Check if element should hug the start
    fn should_hug_start(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block || element.fragment.nodes.is_empty() {
            return !is_block;
        }
        match &element.fragment.nodes[0] {
            FragmentNode::Text(text) => {
                if text.raw.is_whitespace_only() {
                    !text.raw.contains('\n')
                } else {
                    !text.raw.starts_with(char::is_whitespace)
                }
            }
            _ => true,
        }
    }

    /// Check if element should hug the end
    fn should_hug_end(&self, element: &internal::Element, is_block: bool) -> bool {
        if is_block || element.fragment.nodes.is_empty() {
            return !is_block;
        }
        match element.fragment.nodes.last() {
            Some(FragmentNode::Text(text)) => {
                if text.raw.is_whitespace_only() {
                    !text.raw.contains('\n')
                } else {
                    !text.raw.ends_with(char::is_whitespace)
                }
            }
            _ => true,
        }
    }

    // =========================================================================
    // Special elements
    // =========================================================================

    /// Format a Svelte special element
    pub fn print_special_element(&mut self, element: &internal::SpecialElement) {
        use crate::ast::internal::SpecialElementKind;

        let tag_name = element.kind.tag_name();

        let is_typically_empty = matches!(
            element.kind,
            SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteElement { .. }
                | SpecialElementKind::SvelteSelf
        );

        let is_inline_element = matches!(
            element.kind,
            SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::TitleElement
        );

        let has_snippets = element
            .fragment
            .nodes
            .iter()
            .any(|n| matches!(n, FragmentNode::SnippetBlock(_)));
        let is_boundary_without_snippets =
            matches!(element.kind, SpecialElementKind::SvelteBoundary) && !has_snippets;

        // Opening tag
        self.write("<");
        self.write(tag_name);

        // Special `this` attributes
        self.print_special_this_attr(&element.kind);

        // Other attributes
        for attr in &element.attributes {
            self.write(" ");
            self.print_attribute_node(attr);
        }

        // Empty handling
        if element.fragment.nodes.is_empty() {
            if is_typically_empty {
                self.write(" />");
            } else {
                self.write("></");
                self.write(tag_name);
                self.write(">");
            }
            return;
        }

        self.write(">");

        // Determine formatting mode
        let has_block_children = element.fragment.nodes.iter().any(|n| {
            matches!(n, FragmentNode::Element(el) if self.is_block_element(el))
                || matches!(n, FragmentNode::SnippetBlock(_))
                || matches!(n, FragmentNode::SpecialElement(se)
                    if matches!(se.kind, SpecialElementKind::SvelteHead))
        });

        let is_simple = is_boundary_without_snippets
            || (!has_block_children
                && (is_inline_element
                    || element.fragment.nodes.iter().all(|n| match n {
                        FragmentNode::Text(_) | FragmentNode::ExpressionTag(_) => true,
                        FragmentNode::SpecialElement(se) => matches!(
                            se.kind,
                            SpecialElementKind::SlotElement | SpecialElementKind::SvelteFragment
                        ),
                        _ => false,
                    })));

        if is_simple {
            self.print_compact_children(&element.fragment, true, false);
        } else {
            self.print_multiline_children(&element.fragment.nodes, false);
        }

        self.write("</");
        self.write(tag_name);
        self.write(">");
    }

    /// Print special element's `this` attribute if applicable
    fn print_special_this_attr(&mut self, kind: &internal::SpecialElementKind) {
        use internal::SpecialElementKind;
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        match kind {
            SpecialElementKind::SvelteElement { tag } => {
                if let Expression::Literal(lit) = tag
                    && let LiteralValue::String { content, .. } = &lit.value
                {
                    self.write(" this=\"");
                    self.write(content);
                    self.write("\"");
                    return;
                }
                self.write(" this={");
                self.print_ts_expression(tag);
                self.write("}");
            }
            SpecialElementKind::SvelteComponent { expression } => {
                self.write(" this={");
                self.print_ts_expression(expression);
                self.write("}");
            }
            _ => {}
        }
    }

    // =========================================================================
    // Attribute wrapping
    // =========================================================================

    /// Format attributes with line wrapping support
    fn print_attributes_with_wrapping(
        &mut self,
        element: &internal::Element,
        tag_name: &str,
        is_void: bool,
    ) -> bool {
        use tsv_lang::doc;

        if element.attributes.is_empty() {
            return false;
        }

        // Build doc for all attributes
        let mut attr_docs = Vec::new();
        for attr in &element.attributes {
            attr_docs.push(doc::line());
            attr_docs.push(self.build_attribute_node_doc(attr));
        }
        attr_docs.push(doc::dedent(doc::line()));

        // Build complete doc for fits calculation
        let closing = if is_void {
            " />".to_string()
        } else if element.fragment.nodes.is_empty() {
            format!("></{tag_name}>")
        } else {
            ">".to_string()
        };

        let complete_doc = doc::group(doc::concat(vec![
            doc::text("<"),
            doc::text_owned(tag_name.to_string()),
            doc::indent(doc::group(doc::concat(attr_docs))),
            doc::text_owned(closing),
        ]));

        let fits = {
            let interner = self.interner.borrow();
            doc::fits_resolved(
                &complete_doc,
                self.config.print_width,
                doc::Mode::Flat,
                &self.config,
                &*interner,
            )
        };

        if fits {
            for attr in &element.attributes {
                self.write(" ");
                self.print_attribute_node(attr);
            }
            false
        } else {
            self.indent_level += 1;
            for attr in &element.attributes {
                self.write("\n");
                self.write_indent();
                self.print_attribute_node(attr);
            }
            self.indent_level -= 1;
            true
        }
    }

    // =========================================================================
    // Raw content elements (nested <style>/<script>)
    // =========================================================================

    /// Format a nested <style> or <script> element with CSS/JS formatting
    ///
    /// Per Svelte docs: "the <style> tag will be inserted as-is into the DOM"
    /// But prettier still formats the CSS/JS content inside these elements.
    fn print_raw_content_element(
        &mut self,
        tag_name: &str,
        element: &internal::Element,
        attrs_multiline: bool,
    ) {
        // Close opening tag
        if attrs_multiline {
            self.write("\n");
        }
        self.write(">");

        // Get raw content from the single Text child
        let content = element.fragment.nodes.first().and_then(|node| match node {
            FragmentNode::Text(text) => Some(text.data.as_str()),
            _ => None,
        });

        if let Some(content) = content {
            // Format content based on tag type
            let formatted = if tag_name == "style" {
                tsv_css::parse_css(content, 0).ok().map(|ast| {
                    let config = tsv_lang::PrintConfig {
                        base_indent_offset: self.indent_level + 1,
                        ..Default::default()
                    };
                    tsv_css::format_with_config(&ast, content, config)
                })
            } else {
                tsv_ts::parse(content).ok().map(|ast| {
                    let config = tsv_lang::PrintConfig {
                        base_indent_offset: self.indent_level + 1,
                        ..Default::default()
                    };
                    tsv_ts::format_with_config(&ast, content, config)
                })
            };

            if let Some(formatted) = formatted {
                self.write("\n");
                self.indent_level += 1;
                let mut in_template_literal = false;
                let mut is_continuation_line = false;
                for line in formatted.trim_end().lines() {
                    // Don't indent:
                    // - Blank lines
                    // - Lines inside template literals (whitespace is content)
                    // - Continuation lines (after backslash)
                    let should_indent =
                        !line.is_empty() && !in_template_literal && !is_continuation_line;
                    if should_indent {
                        self.write_indent();
                    }
                    self.write(line);
                    self.write("\n");

                    // Track state for next line
                    is_continuation_line = tsv_lang::printing::is_line_continuation_ending(line);
                    in_template_literal = crate::printer::helpers::is_inside_template_literal(
                        line,
                        in_template_literal,
                    );
                }
                self.indent_level -= 1;
                self.write_indent();
            } else {
                // Fallback: preserve raw content if parsing fails
                self.write(content);
            }
        }

        // Closing tag
        self.write("</");
        self.write(tag_name);
        self.write(">");
    }
}
