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

/// Wrap mode for special element attributes
#[derive(Clone, Copy, PartialEq, Eq)]
enum SpecialAttrWrapMode {
    /// All attrs + children + closing fit on one line
    Inline,
    /// Attrs + content fit but final `>` must split: `attrs>{content}</tag\n>`
    SplitFinalBracket,
    /// Attrs fit at column 0 but not with children; attrs inline, `>` on new line indented
    Hug,
    /// Attrs don't fit at column 0; each attr on its own line
    FullMultiline,
    /// Attrs have internal breaks (e.g., arrow body wraps); `>` hugs with `}`, but use split closing
    InternalBreak,
}

impl SpecialAttrWrapMode {
    /// Returns true if attrs stay on the same line (not wrapped to separate lines)
    fn is_attrs_inline(self) -> bool {
        matches!(
            self,
            Self::Inline | Self::SplitFinalBracket | Self::InternalBreak
        )
    }
}

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

        // Determine if this is a self-closing component/foreign element
        let is_self_closing_component = (is_component || is_foreign)
            && element.fragment.nodes.is_empty()
            && self.was_self_closing(element);

        // Attributes
        let attr_wrap_mode = self.print_attributes_with_wrapping(
            element,
            &tag_name,
            is_void,
            is_self_closing_component,
            is_block,
        );
        let attrs_multiline = !attr_wrap_mode.is_attrs_inline();

        // Handle void elements
        if is_void {
            self.write_void_closing(attrs_multiline);
            return;
        }

        // Handle empty components/foreign elements (preserve author's self-closing choice)
        if is_self_closing_component {
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
                attr_wrap_mode,
            );
        } else {
            self.print_normal_mode(
                &tag_name,
                element,
                is_block,
                preserves_ws,
                multiline,
                attr_wrap_mode,
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
            self.write_indent();
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

    /// Write a split closing tag: `</tag\n\t>` (for hug end pattern, with indent)
    fn write_split_closing_tag(&mut self, tag_name: &str) {
        self.write("</");
        self.write(tag_name);
        self.write("\n");
        self.write_indent();
        self.write(">");
    }

    /// Write a split closing tag with bracket at column 0: `</tag\n>`
    fn write_split_closing_bracket(&mut self, tag_name: &str) {
        self.write("</");
        self.write(tag_name);
        self.write("\n");
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
        attr_wrap_mode: SpecialAttrWrapMode,
    ) {
        let attrs_multiline = attr_wrap_mode != SpecialAttrWrapMode::Inline;
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
        attr_wrap_mode: SpecialAttrWrapMode,
    ) {
        let attrs_multiline = attr_wrap_mode != SpecialAttrWrapMode::Inline;

        // Check for special "wrapped attrs + single block child" pattern
        let is_wrapped_single_block = self.is_wrapped_single_block(element, attrs_multiline);

        // Check if element has non-whitespace content
        let has_content = element
            .fragment
            .nodes
            .iter()
            .any(|node| !matches!(node, FragmentNode::Text(t) if t.raw.is_whitespace_only()));

        // Split closing tag rule: inline elements with content + multiline attrs
        // Block elements never use split closing (Prettier behavior)
        let use_split_closing = !is_block && has_content && attrs_multiline;

        // Handle SplitFinalBracket: attrs + content inline, only final > splits
        // Pattern: `<tag attrs>{content}</tag\n>`
        if attr_wrap_mode == SpecialAttrWrapMode::SplitFinalBracket && !multiline {
            self.write(">");
            self.print_compact_children(&element.fragment, is_block, preserves_ws);
            self.write_split_closing_bracket(tag_name);
            return;
        }

        // Handle InternalBreak: attrs have internal breaks (like arrow body wraps)
        // Opening > hugs with closing }
        if attr_wrap_mode == SpecialAttrWrapMode::InternalBreak && !multiline {
            self.write(">");
            self.print_compact_children(&element.fragment, is_block, preserves_ws);
            if use_split_closing {
                self.write_split_closing_bracket(tag_name);
            } else {
                self.write_closing_tag(tag_name);
            }
            return;
        }

        // Handle Hug/FullMultiline with non-multiline children
        if (attr_wrap_mode == SpecialAttrWrapMode::Hug
            || attr_wrap_mode == SpecialAttrWrapMode::FullMultiline)
            && !multiline
            && !is_wrapped_single_block
        {
            if use_split_closing {
                // Inline element with content: > indented, content inline, split closing
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.write(">");

                self.print_compact_children(&element.fragment, is_block, preserves_ws);

                self.write("</");
                self.write(tag_name);
                self.indent_level -= 1;
                self.write("\n");
                self.write(">");
            } else {
                // Block element or empty: > at base indent, normal closing
                self.write("\n");
                self.write_indent();
                self.write(">");

                self.print_compact_children(&element.fragment, is_block, preserves_ws);

                self.write_closing_tag(tag_name);
            }
            return;
        }

        // Opening >
        if attrs_multiline && !is_wrapped_single_block {
            self.write("\n");
            self.write_indent();
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
            // Check if total element width exceeds print_width - if so, use block mode
            // This handles cases like: <p>{long expression}</p> → <p>\n\t{expr}\n</p>
            // Skip for whitespace-preserving elements (pre, textarea, etc.)
            let use_width_block_mode = !preserves_ws
                && self.should_use_width_block_mode(&element.fragment.nodes, tag_name);

            if use_width_block_mode {
                // Element block mode: content on indented line
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.print_compact_children(&element.fragment, is_block, preserves_ws);
                self.indent_level -= 1;
                self.write("\n");
                self.write_indent();
                self.write_closing_tag(tag_name);
            } else {
                self.print_compact_children(&element.fragment, is_block, preserves_ws);
                self.write_closing_tag(tag_name);
            }
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

    /// Determine if element should use width-based block mode.
    ///
    /// This handles cases where the total element width exceeds print_width,
    /// even though the content structure would normally be compact.
    /// Example: `<p>{long expression}</p>` → `<p>\n\t{expr}\n</p>`
    ///
    /// Only applies to simple content (text, expressions, comments).
    /// Elements with nested child elements use other formatting rules.
    fn should_use_width_block_mode(&self, nodes: &[FragmentNode], tag_name: &str) -> bool {
        // Only apply to simple content - if there are nested elements, let
        // the regular formatting rules handle it
        let has_nested_elements = nodes.iter().any(|n| {
            matches!(
                n,
                FragmentNode::Element(_)
                    | FragmentNode::SpecialElement(_)
                    | FragmentNode::IfBlock(_)
                    | FragmentNode::EachBlock(_)
                    | FragmentNode::AwaitBlock(_)
                    | FragmentNode::KeyBlock(_)
                    | FragmentNode::SnippetBlock(_)
            )
        });
        if has_nested_elements {
            return false;
        }

        // Calculate content's flat length
        // If content has internal breaks (groups that don't fit flat), element needs block mode
        let Some(content_len) = self.calculate_inline_content_length(nodes) else {
            return true;
        };

        // Empty content doesn't need block mode
        if content_len == 0 {
            return false;
        }

        // Calculate remaining width needed: content + </tag>
        // We already wrote `<tag attrs>` so use current column position
        let current_col = self.buffer.current_column(self.config.tab_width);
        let closing_tag_len = 2 + tag_name.len() + 1; // `</tag>`
        let total_len = current_col + content_len + closing_tag_len;

        // Use block mode if total exceeds print_width
        total_len > self.config.print_width
    }

    /// Calculate the length of inline content for fits calculation.
    /// Returns Some(length) for content that can be measured inline, None for complex content
    /// that would cause multiline formatting anyway.
    fn calculate_inline_content_length(&self, nodes: &[FragmentNode]) -> Option<usize> {
        use tsv_lang::doc;

        // Skip leading/trailing whitespace-only text nodes
        let non_ws_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()))
            .collect();

        if non_ws_nodes.is_empty() {
            return Some(0);
        }

        // Build docs for content that may have internal breaks (expressions),
        // and track fixed-width content separately for efficiency
        let mut content_docs = Vec::new();
        let mut fixed_width = 0usize;
        let len = non_ws_nodes.len();

        for (i, node) in non_ws_nodes.iter().enumerate() {
            let is_first = i == 0;
            let is_last = i == len - 1;

            match node {
                FragmentNode::Text(text) => {
                    // Text content - normalize whitespace (collapse newlines to spaces)
                    let normalized = self.normalize_whitespace(&text.raw, false);
                    // Only trim leading/trailing at boundaries, preserve interior spaces
                    let processed = if is_first && is_last {
                        normalized.trim()
                    } else if is_first {
                        normalized.trim_start()
                    } else if is_last {
                        normalized.trim_end()
                    } else {
                        &normalized
                    };
                    fixed_width += processed.len();
                }
                FragmentNode::ExpressionTag(tag) => {
                    // Expression tags may have internal breaks - build doc for accurate measurement
                    content_docs.push(self.build_expression_tag_doc(tag));
                }
                FragmentNode::Comment(comment) => {
                    // HTML comment: <!--content-->
                    fixed_width += 7 + comment.content.len(); // "<!--" + "-->"
                }
                // Inline tags - use source span width
                FragmentNode::HtmlTag(tag) => {
                    fixed_width += (tag.span.end - tag.span.start) as usize;
                }
                FragmentNode::RenderTag(tag) => {
                    fixed_width += (tag.span.end - tag.span.start) as usize;
                }
                FragmentNode::DebugTag(tag) => {
                    fixed_width += (tag.span.end - tag.span.start) as usize;
                }
                FragmentNode::ConstTag(tag) => {
                    fixed_width += (tag.span.end - tag.span.start) as usize;
                }
                // Block structures (if, each, etc.) cause multiline - can't estimate
                _ => return None,
            }
        }

        // If we have expression tags, render their docs to check for breaks
        if !content_docs.is_empty() {
            let content_doc = doc::concat(content_docs);
            let interner = self.interner.borrow();
            let rendered = doc::print_doc_resolved(&content_doc, &self.config, &*interner);

            // If expressions have newlines, content has internal breaks
            if rendered.contains('\n') {
                return None;
            }

            Some(fixed_width + rendered.len())
        } else {
            Some(fixed_width)
        }
    }

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

    /// Check if element has "opening newline only" pattern.
    ///
    /// This pattern applies when:
    /// - First node is whitespace-only with newline (indentation)
    /// - Last node is not text (no trailing whitespace)
    /// - No block children (if/each/await/key blocks)
    ///
    /// Block children require proper newlines between them, which print_split_closing
    /// doesn't provide. For those cases, use standard multiline formatting instead.
    fn has_opening_newline_only(&self, nodes: &[FragmentNode]) -> bool {
        if nodes.is_empty() {
            return false;
        }

        // Must have opening newline
        let has_opening_nl = matches!(nodes.first(), Some(FragmentNode::Text(t))
            if t.raw.is_whitespace_only() && t.raw.contains('\n'));
        if !has_opening_nl {
            return false;
        }

        // Must not have trailing text
        let has_trailing_text = matches!(nodes.last(), Some(FragmentNode::Text(_)));
        if has_trailing_text {
            return false;
        }

        // Must not have any block children (if/each/await/key blocks, etc.)
        // print_split_closing doesn't add newlines between children, so block
        // children would be merged incorrectly. Use standard multiline formatting instead.
        let has_block_children = nodes.iter().any(|n| {
            matches!(
                n,
                FragmentNode::IfBlock(_)
                    | FragmentNode::EachBlock(_)
                    | FragmentNode::AwaitBlock(_)
                    | FragmentNode::KeyBlock(_)
                    | FragmentNode::SnippetBlock(_)
            )
        });
        !has_block_children
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

        // Determine if this is a self-closing element
        let is_self_closing = element.fragment.nodes.is_empty() && is_typically_empty;

        // svelte:boundary with snippets should not use hug mode
        let is_boundary_with_snippets =
            matches!(element.kind, SpecialElementKind::SvelteBoundary) && has_snippets;

        // Estimate inline content width for fits calculation
        // This includes children text/expressions and closing tag
        // For boundary with snippets, don't estimate (always format as standard block)
        let inline_content_estimate =
            if element.fragment.nodes.is_empty() || is_boundary_with_snippets {
                0
            } else {
                self.estimate_inline_content_width(&element.fragment.nodes, tag_name)
            };

        // Opening tag
        self.write("<");
        self.write(tag_name);

        // Attributes with wrapping support
        let wrap_mode = self.print_special_element_attributes_with_wrapping(
            element,
            tag_name,
            is_self_closing,
            inline_content_estimate,
            is_boundary_with_snippets,
        );

        // Empty handling
        if element.fragment.nodes.is_empty() {
            if is_typically_empty {
                if wrap_mode != SpecialAttrWrapMode::Inline {
                    self.write("\n");
                    self.write_indent();
                    self.write("/>");
                } else {
                    self.write(" />");
                }
            } else {
                self.write("></");
                self.write(tag_name);
                self.write(">");
            }
            return;
        }

        // Close opening tag with proper indentation based on wrap mode
        match wrap_mode {
            SpecialAttrWrapMode::Hug => {
                // Hug mode: newline + indent + `>`
                self.write("\n");
                self.indent_level += 1;
                self.write_indent();
                self.write(">");
            }
            SpecialAttrWrapMode::FullMultiline => {
                // Full multiline: just `>` followed by multiline children
                self.write("\n");
                self.write_indent();
                self.write(">");
            }
            SpecialAttrWrapMode::Inline
            | SpecialAttrWrapMode::SplitFinalBracket
            | SpecialAttrWrapMode::InternalBreak => {
                // Inline/SplitFinalBracket/InternalBreak: just `>`
                self.write(">");
            }
        }

        // Determine formatting mode for children based on wrap mode
        match wrap_mode {
            SpecialAttrWrapMode::Hug => {
                // Hug mode: count non-whitespace children
                let non_ws_count = element
                    .fragment
                    .nodes
                    .iter()
                    .filter(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()))
                    .count();

                if non_ws_count <= 1 {
                    // Single child: print inline with `>` and closing tag
                    self.print_compact_children(&element.fragment, true, false);
                } else {
                    // Multiple children: each child on its own line, closing tag hugged with last
                    for (i, node) in element.fragment.nodes.iter().enumerate() {
                        if matches!(node, FragmentNode::Text(t) if t.raw.is_whitespace_only()) {
                            continue;
                        }
                        self.print_fragment_node(node, false, false);
                        // Add newline before next non-ws child (but not after last)
                        if i < element.fragment.nodes.len() - 1 {
                            let has_more_non_ws = element.fragment.nodes[i + 1..]
                                .iter()
                                .any(|n| !matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only()));
                            if has_more_non_ws {
                                self.write("\n");
                                self.write_indent();
                            }
                        }
                    }
                }
            }
            SpecialAttrWrapMode::FullMultiline => {
                // Full multiline: use standard multiline children formatting
                self.print_multiline_children(&element.fragment.nodes, false);
            }
            SpecialAttrWrapMode::Inline
            | SpecialAttrWrapMode::SplitFinalBracket
            | SpecialAttrWrapMode::InternalBreak => {
                // Inline/SplitFinalBracket/InternalBreak: use standard formatting rules
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
                                    SpecialElementKind::SlotElement
                                        | SpecialElementKind::SvelteFragment
                                ),
                                _ => false,
                            })));

                if is_simple {
                    self.print_compact_children(&element.fragment, true, false);
                } else {
                    self.print_multiline_children(&element.fragment.nodes, false);
                }
            }
        }

        // Closing tag based on wrap mode
        match wrap_mode {
            SpecialAttrWrapMode::Hug => {
                // Hug mode: split closing tag (</tag\n>) with indent adjustment
                self.write("</");
                self.write(tag_name);
                self.indent_level -= 1;
                self.write("\n");
                self.write_indent();
                self.write(">");
            }
            SpecialAttrWrapMode::SplitFinalBracket => {
                // Split just the final > (no indent adjustment needed)
                self.write_split_closing_bracket(tag_name);
            }
            SpecialAttrWrapMode::FullMultiline
            | SpecialAttrWrapMode::Inline
            | SpecialAttrWrapMode::InternalBreak => {
                self.write_closing_tag(tag_name);
            }
        }
    }

    /// Estimate width of inline children content plus closing tag
    /// Used for accurate fits calculation in attribute wrapping
    fn estimate_inline_content_width(&self, nodes: &[FragmentNode], tag_name: &str) -> usize {
        let mut width = 0;

        for node in nodes {
            match node {
                FragmentNode::Text(text) => {
                    // Use actual text length, trimmed for compact mode
                    width += text.raw.trim().len();
                }
                FragmentNode::ExpressionTag(expr) => {
                    // Use source span width as approximation
                    let span_width = (expr.span.end - expr.span.start) as usize;
                    width += span_width;
                }
                FragmentNode::Element(el) => {
                    // Use source span width as approximation
                    let span_width = (el.span.end - el.span.start) as usize;
                    width += span_width;
                }
                FragmentNode::SpecialElement(se) => {
                    // Use source span width as approximation
                    let span_width = (se.span.end - se.span.start) as usize;
                    width += span_width;
                }
                _ => {
                    // For other node types (blocks, etc.), use span if available
                    // or estimate conservatively
                    width += 50; // Conservative estimate for complex nodes
                }
            }
        }

        // Add closing tag length: </tag_name>
        width += 3 + tag_name.len(); // "</" + tag_name + ">"

        width
    }

    /// Print special element's `this` attribute if applicable (without leading space)
    fn print_special_this_attr(&mut self, kind: &internal::SpecialElementKind) {
        use internal::SpecialElementKind;
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        match kind {
            SpecialElementKind::SvelteElement { tag } => {
                if let Expression::Literal(lit) = tag
                    && let LiteralValue::String { content, .. } = &lit.value
                {
                    self.write("this=\"");
                    self.write(content);
                    self.write("\"");
                    return;
                }
                self.write("this={");
                self.print_ts_expression(tag);
                self.write("}");
            }
            SpecialElementKind::SvelteComponent { expression } => {
                self.write("this={");
                self.print_ts_expression(expression);
                self.write("}");
            }
            _ => {}
        }
    }

    /// Build a Doc for special element's `this` attribute
    fn build_special_this_attr_doc(
        &self,
        kind: &internal::SpecialElementKind,
    ) -> Option<tsv_lang::doc::Doc> {
        use internal::SpecialElementKind;
        use std::rc::Rc;
        use tsv_lang::doc;
        use tsv_ts::ast::internal::{Expression, LiteralValue};

        // Helper to build `this={expr}` doc
        let build_this_expr_doc = |expr: &Expression| {
            let expr_doc = tsv_ts::build_expression_doc_with_comments(
                expr,
                self.source,
                Rc::clone(&self.interner),
                &self.config,
                self.comments,
            );
            doc::concat(vec![doc::text("this={"), expr_doc, doc::text("}")])
        };

        match kind {
            SpecialElementKind::SvelteElement { tag } => {
                // String literal: this="tag"
                if let Expression::Literal(lit) = tag
                    && let LiteralValue::String { content, .. } = &lit.value
                {
                    Some(doc::text_owned(format!("this=\"{content}\"")))
                } else {
                    // Expression: this={tag}
                    Some(build_this_expr_doc(tag))
                }
            }
            SpecialElementKind::SvelteComponent { expression } => {
                Some(build_this_expr_doc(expression))
            }
            _ => None,
        }
    }

    /// Format special element attributes with line wrapping support
    ///
    /// Returns the wrap mode used for attributes.
    ///
    /// # Modes
    /// - **Inline**: All attrs + children + closing fit on one line
    /// - **Hug mode**: Attrs fit at column 0 but not with children; attrs inline, `>` on new line
    /// - **Full multiline**: Attrs don't fit at column 0; each attr on its own line
    ///
    /// The `inline_content_estimate` parameter should include the estimated width of
    /// inline children plus closing tag (e.g., `text</svelte:element>`).
    ///
    /// The `disable_hug_mode` parameter forces full multiline when attrs don't fit,
    /// bypassing hug mode (used for svelte:boundary with snippets).
    fn print_special_element_attributes_with_wrapping(
        &mut self,
        element: &internal::SpecialElement,
        tag_name: &str,
        is_self_closing: bool,
        inline_content_estimate: usize,
        disable_hug_mode: bool,
    ) -> SpecialAttrWrapMode {
        use tsv_lang::doc;

        let this_attr_doc = self.build_special_this_attr_doc(&element.kind);
        let has_this_attr = this_attr_doc.is_some();
        let has_regular_attrs = !element.attributes.is_empty();

        // No attributes at all
        if !has_this_attr && !has_regular_attrs {
            return SpecialAttrWrapMode::Inline;
        }

        // Build doc for all attributes (this attr + regular attrs)
        let mut attr_docs = Vec::new();
        let mut any_attr_will_break = false;

        // Add this attr doc first
        if let Some(this_doc) = &this_attr_doc {
            attr_docs.push(doc::line());
            if doc::will_break(this_doc) {
                any_attr_will_break = true;
            }
            attr_docs.push(this_doc.clone());
        }

        // Add regular attributes
        for attr in &element.attributes {
            attr_docs.push(doc::line());
            let attr_doc = self.build_attribute_node_doc(attr);
            if doc::will_break(&attr_doc) {
                any_attr_will_break = true;
            }
            attr_docs.push(attr_doc);
        }

        // Use softline so it disappears in flat mode (no space before `>`)
        attr_docs.push(doc::dedent(doc::softline()));

        // If any attribute will break, force full multiline formatting
        if any_attr_will_break {
            self.indent_level += 1;
            if has_this_attr {
                self.write("\n");
                self.write_indent();
                self.print_special_this_attr(&element.kind);
            }
            for attr in &element.attributes {
                self.write("\n");
                self.write_indent();
                self.print_attribute_node(attr);
            }
            self.indent_level -= 1;
            return SpecialAttrWrapMode::FullMultiline;
        }

        // Build complete doc for fits calculation
        // For elements with children, include estimated content width
        let closing = if is_self_closing {
            " />".to_string()
        } else if element.fragment.nodes.is_empty() {
            format!("></{tag_name}>")
        } else {
            // Include `>` + estimated inline content for accurate fits check
            format!(">{}", "x".repeat(inline_content_estimate))
        };

        // Check if hug mode is possible (only for non-empty, non-self-closing elements)
        // Disabled for svelte:boundary with snippets
        let can_use_hug_mode =
            !is_self_closing && !element.fragment.nodes.is_empty() && !disable_hug_mode;

        // Clone attr_docs only if we might need hug mode check
        let attr_docs_for_hug = if can_use_hug_mode {
            Some(attr_docs.clone())
        } else {
            None
        };

        let complete_doc = doc::group(doc::concat(vec![
            doc::text("<"),
            doc::text_owned(tag_name.to_string()),
            doc::indent(doc::group(doc::concat(attr_docs))),
            doc::text_owned(closing),
        ]));

        // Account for current indent level when checking if content fits
        let available_width = self
            .config
            .print_width
            .saturating_sub(self.indent_level * self.config.tab_width);

        let interner = self.interner.borrow();

        // Check: Does everything fit at effective width (with indent)?
        let fits_effective = doc::fits_resolved(
            &complete_doc,
            available_width,
            doc::Mode::Flat,
            &self.config,
            &*interner,
        );

        if fits_effective {
            // Mode 1: Everything fits on one line
            drop(interner);
            if has_this_attr {
                self.write(" ");
                self.print_special_this_attr(&element.kind);
            }
            for attr in &element.attributes {
                self.write(" ");
                self.print_attribute_node(attr);
            }
            SpecialAttrWrapMode::Inline
        } else {
            match attr_docs_for_hug {
                None => {
                    // Self-closing/empty elements: no hug mode, go straight to full multiline
                    drop(interner);
                    self.indent_level += 1;
                    if has_this_attr {
                        self.write("\n");
                        self.write_indent();
                        self.print_special_this_attr(&element.kind);
                    }
                    for attr in &element.attributes {
                        self.write("\n");
                        self.write_indent();
                        self.print_attribute_node(attr);
                    }
                    self.indent_level -= 1;
                    SpecialAttrWrapMode::FullMultiline
                }
                Some(attr_docs_for_hug) => {
                    // Elements with children: check if attr line (without >) fits at column 0
                    // If so, use hug mode. Otherwise, full multiline.

                    // Build doc for attr line only (tag + attrs, no closing >)
                    let attr_line_doc = doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::indent(doc::group(doc::concat(attr_docs_for_hug))),
                    ]));

                    // Check if attr line fits at column 0 (full print_width)
                    // Prettier uses hug mode when attrs exactly equal print_width
                    let attr_line_fits = doc::fits_resolved(
                        &attr_line_doc,
                        self.config.print_width,
                        doc::Mode::Flat,
                        &self.config,
                        &*interner,
                    );

                    drop(interner);

                    if attr_line_fits {
                        // Mode 2: Hug mode - attr line fits at column 0 but total doesn't with indent
                        // Keep attrs on one line, > will be put on new line by caller
                        if has_this_attr {
                            self.write(" ");
                            self.print_special_this_attr(&element.kind);
                        }
                        for attr in &element.attributes {
                            self.write(" ");
                            self.print_attribute_node(attr);
                        }
                        SpecialAttrWrapMode::Hug
                    } else {
                        // Mode 3: Full multiline - attr line exceeds at column 0
                        // Each attr on its own line
                        self.indent_level += 1;
                        if has_this_attr {
                            self.write("\n");
                            self.write_indent();
                            self.print_special_this_attr(&element.kind);
                        }
                        for attr in &element.attributes {
                            self.write("\n");
                            self.write_indent();
                            self.print_attribute_node(attr);
                        }
                        self.indent_level -= 1;
                        SpecialAttrWrapMode::FullMultiline
                    }
                }
            }
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
        is_self_closing_component: bool,
        is_block: bool,
    ) -> SpecialAttrWrapMode {
        use tsv_lang::doc;

        if element.attributes.is_empty() {
            return SpecialAttrWrapMode::Inline;
        }

        // Build doc for all attributes
        let mut attr_docs = Vec::new();
        let mut any_attr_will_break = false;
        for attr in &element.attributes {
            attr_docs.push(doc::line());
            let attr_doc = self.build_attribute_node_doc(attr);
            // Check if any attribute value contains hardlines (forces multiline)
            if doc::will_break(&attr_doc) {
                any_attr_will_break = true;
            }
            attr_docs.push(attr_doc);
        }
        // Use softline so it disappears in flat mode (no space before `>`)
        // but becomes a newline in break mode
        attr_docs.push(doc::dedent(doc::softline()));

        // If any attribute will break (contains hardlines like block bodies),
        // force multiline formatting with each attr on its own line.
        if any_attr_will_break {
            // Check if this is an inline element with content
            let has_content =
                element.fragment.nodes.iter().any(
                    |node| !matches!(node, FragmentNode::Text(t) if t.raw.is_whitespace_only()),
                );
            let use_internal_break = !is_block && has_content;

            self.indent_level += 1;
            for attr in &element.attributes {
                self.write("\n");
                self.write_indent();
                self.print_attribute_node(attr);
            }
            self.indent_level -= 1;

            // For inline elements with content, > hugs with } (InternalBreak)
            // For block elements or empty inline, > on own line (FullMultiline)
            return if use_internal_break {
                SpecialAttrWrapMode::InternalBreak
            } else {
                SpecialAttrWrapMode::FullMultiline
            };
        }

        // Build complete doc for fits calculation
        // For block elements with content, children go multiline anyway, so only check opening tag
        // For inline elements, include content estimate for accurate line length checking
        let closing = if is_void || is_self_closing_component {
            " />".to_string()
        } else if element.fragment.nodes.is_empty() {
            format!("></{tag_name}>")
        } else if is_block {
            // Block elements: children go multiline, just check opening tag
            ">".to_string()
        } else {
            // Inline elements with content: include content in fits check
            let content_estimate = self.calculate_inline_content_length(&element.fragment.nodes);
            if let Some(len) = content_estimate {
                // Content is simple enough to estimate - include it
                format!(">{}</{tag_name}>", "x".repeat(len))
            } else {
                // Complex inline content - use minimal closing
                // Hug mode check will catch cases where content doesn't fit
                format!("></{tag_name}>")
            }
        };

        // Check if hug mode is possible (only for inline elements that aren't self-closing)
        let can_use_hug_mode = !is_void && !is_self_closing_component && !is_block;

        // Clone attr_docs only if we might need hug mode check
        let attr_docs_for_hug = if can_use_hug_mode {
            Some(attr_docs.clone())
        } else {
            None
        };

        let complete_doc = doc::group(doc::concat(vec![
            doc::text("<"),
            doc::text_owned(tag_name.to_string()),
            doc::indent(doc::group(doc::concat(attr_docs))),
            doc::text_owned(closing),
        ]));

        // Account for current indent level when checking if content fits
        let available_width = self
            .config
            .print_width
            .saturating_sub(self.indent_level * self.config.tab_width);

        let interner = self.interner.borrow();

        // Check: Does everything fit at effective width (with indent)?
        let fits_effective = doc::fits_resolved(
            &complete_doc,
            available_width,
            doc::Mode::Flat,
            &self.config,
            &*interner,
        );

        if fits_effective {
            // Mode 1: Everything fits on one line
            drop(interner);
            for attr in &element.attributes {
                self.write(" ");
                self.print_attribute_node(attr);
            }
            return SpecialAttrWrapMode::Inline;
        }

        // For inline elements with content, check SplitFinalBracket mode
        // This is when `<tag attrs>{content}</tag` fits but final `>` doesn't
        if can_use_hug_mode
            && !element.fragment.nodes.is_empty()
            && let Some(content_len) = self.calculate_inline_content_length(&element.fragment.nodes)
        {
            let closing_no_bracket = format!(">{}</{tag_name}", "x".repeat(content_len));

            let doc_no_final_bracket = doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::indent(doc::group(doc::concat(
                    attr_docs_for_hug.clone().unwrap_or_default(),
                ))),
                doc::text_owned(closing_no_bracket),
            ]));

            let fits_without_bracket = doc::fits_resolved(
                &doc_no_final_bracket,
                available_width,
                doc::Mode::Flat,
                &self.config,
                &*interner,
            );

            if fits_without_bracket {
                // Mode 2: Split only the final >
                drop(interner);
                for attr in &element.attributes {
                    self.write(" ");
                    self.print_attribute_node(attr);
                }
                return SpecialAttrWrapMode::SplitFinalBracket;
            }
        }

        // Continue with hug mode checks
        match attr_docs_for_hug {
            None => {
                // Self-closing/void/block elements: no hug mode, go straight to full multiline
                drop(interner);
                self.indent_level += 1;
                for attr in &element.attributes {
                    self.write("\n");
                    self.write_indent();
                    self.print_attribute_node(attr);
                }
                self.indent_level -= 1;
                SpecialAttrWrapMode::FullMultiline
            }
            Some(attr_docs_for_hug) => {
                // Inline elements: check if attr line (without >) fits at column 0
                // If so, use hug mode. Otherwise, full multiline.

                // Check if the attr content ALONE would break:
                // Build doc for just the attrs (without tag name)
                let attrs_only: Vec<_> = attr_docs_for_hug
                    .iter()
                    .skip(1) // Skip the first doc::line()
                    .cloned()
                    .collect();
                let attrs_only_doc = doc::group(doc::concat(attrs_only));

                // Build doc for attr line only (tag + attrs, no closing >)
                let attr_line_doc = doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tag_name.to_string()),
                    doc::indent(doc::group(doc::concat(attr_docs_for_hug))),
                ]));

                let attr_line_fits_flat = doc::fits_resolved(
                    &attr_line_doc,
                    self.config.print_width,
                    doc::Mode::Flat,
                    &self.config,
                    &*interner,
                );
                // Check attrs at the indent level where they'll be printed (current + 1)
                // This accounts for the indentation when attrs go to separate lines
                let attrs_indent = (self.indent_level + 1) * self.config.tab_width;
                let attrs_available_width = self.config.print_width.saturating_sub(attrs_indent);

                let attrs_fit_flat = doc::fits_resolved(
                    &attrs_only_doc,
                    attrs_available_width,
                    doc::Mode::Flat,
                    &self.config,
                    &*interner,
                );

                // For InternalBreak mode (where > hugs with }), we need to check
                // that the wrapped content fits with room to spare. If any line
                // is exactly at print_width, Prettier uses FullMultiline instead.
                // Render the attr to actually measure max line width.
                let attrs_have_internal_breaks = if attrs_fit_flat {
                    false
                } else {
                    // Render the attr doc and check max line width
                    let rendered = doc::print_doc_with_indent_resolved(
                        &attrs_only_doc,
                        &self.config,
                        0,                     // start at column 0
                        self.indent_level + 1, // attrs are at this indent
                        &*interner,
                    );
                    let max_line_width = rendered
                        .lines()
                        .map(|line| {
                            // Calculate visual width considering tabs
                            let mut width = 0;
                            for c in line.chars() {
                                if c == '\t' {
                                    width += self.config.tab_width;
                                } else {
                                    width += 1;
                                }
                            }
                            width
                        })
                        .max()
                        .unwrap_or(0);
                    // Only use InternalBreak if ALL lines are < print_width
                    // (not <=, since exactly at boundary means no room for >)
                    max_line_width < self.config.print_width
                };

                drop(interner);

                if attr_line_fits_flat {
                    // Mode 2: Hug mode - attr line fits at column 0 but total doesn't with indent
                    // Keep attrs on one line, just put > on new line
                    for attr in &element.attributes {
                        self.write(" ");
                        self.print_attribute_node(attr);
                    }
                    SpecialAttrWrapMode::Hug
                } else if attrs_have_internal_breaks {
                    // Mode 4: InternalBreak - attrs have internal breaks (e.g., arrow body wraps)
                    // Attrs on separate lines, but > hugs with closing }
                    self.indent_level += 1;
                    for attr in &element.attributes {
                        self.write("\n");
                        self.write_indent();
                        self.print_attribute_node(attr);
                    }
                    self.indent_level -= 1;
                    SpecialAttrWrapMode::InternalBreak
                } else {
                    // Mode 3: Full multiline - attr line exceeds at column 0 even in break mode
                    // Each attr on its own line
                    self.indent_level += 1;
                    for attr in &element.attributes {
                        self.write("\n");
                        self.write_indent();
                        self.print_attribute_node(attr);
                    }
                    self.indent_level -= 1;
                    SpecialAttrWrapMode::FullMultiline
                }
            }
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
                tsv_css::parse(content).ok().map(|ast| {
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
