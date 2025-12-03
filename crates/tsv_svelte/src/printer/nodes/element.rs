// Element-specific formatting for Svelte templates
//
// Handles formatting of HTML/Svelte elements with context-aware decisions about
// whether to format children in multiline vs compact mode.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_html as html;
use tsv_lang::SymbolResolver;

impl<'a> Printer<'a> {
    /// Format a Svelte element with context-aware formatting
    ///
    /// Block elements with multiple block children: multiline with indentation
    /// All other cases: compact (single line)
    pub fn print_element(&mut self, element: &internal::Element) {
        // Determine element characteristics (single symbol resolution for efficiency)
        let (is_block, preserves_ws, is_void) = self.with_resolved_symbol(element.name, |tag| {
            (
                html::is_block_element(tag),
                html::preserves_whitespace(tag),
                html::is_void_element(tag),
            )
        });

        // Components are treated as block for general formatting but inline for hug logic
        use crate::ast::internal::ElementKind;
        let is_block = is_block || element.kind == ElementKind::Component;

        // For hug logic, Components are treated as inline (they can hug children)
        let is_block_for_hug = is_block && element.kind != ElementKind::Component;

        // Opening tag
        let tag_name = self.resolve_symbol(element.name);
        self.write("<");
        self.write(&tag_name);

        // Format attributes with line wrapping support (hybrid doc-builder approach)
        let attrs_multiline = self.print_attributes_with_wrapping(element, &tag_name, is_void);

        // Check if we need Prettier's "hug mode" (>< pattern) formatting
        // Must check AFTER attributes to know if opening tag wrapped
        // TODO: Rename `needs_hug_mode` → `should_break_children` or similar
        // The name "hug mode" conflates two concepts: break children vs whitespace detection
        let use_hug_mode = self.needs_hug_mode(element, is_block, attrs_multiline);

        // Determine hugging behavior based on semantic whitespace (prettier's approach)
        // hugStart: true if first child has NO leading whitespace
        // hugEnd: true if last child has NO trailing whitespace
        // Note: Use is_block_for_hug (Components are inline for hug logic)
        let hug_start = self.should_hug_start(element, is_block_for_hug);
        let hug_end = self.should_hug_end(element, is_block_for_hug);

        // Decide if children should be multiline (skip if using hug mode)
        let multiline =
            !use_hug_mode && self.should_format_multiline(element, is_block, preserves_ws);

        // Void elements are self-closing
        if is_void {
            // If attributes wrapped to multiple lines, closing tag goes at column 0
            // Otherwise, add a space before the closing tag
            if attrs_multiline {
                self.write("\n");
                self.write("/>");
            } else {
                self.write(" />");
            }
            return; // No children or closing tag
        }

        // Components with no children: preserve author's choice (self-closing vs explicit closing tag)
        if element.kind == ElementKind::Component && element.fragment.nodes.is_empty() {
            let source_slice = element.span.extract(self.source);
            let was_self_closing = source_slice.trim_end().ends_with("/>");

            if was_self_closing {
                // If attributes wrapped, add newline + indent before />; otherwise add space
                if attrs_multiline {
                    self.write("\n");
                    self.write_indent();
                    self.write("/>");
                } else {
                    self.write(" />");
                }
                return;
            }
            // Otherwise, fall through to write explicit closing tag below
        }

        // Format children based on mode and hugging behavior
        // Prettier has 4 separate code paths for hugStart/hugEnd combinations:
        // 1. hugStart && hugEnd (lines 1180-1189): Full hug with ><content</Tag\n>
        // 2. hugStart && !hugEnd (lines 1221-1227): Indented opening, normal closing
        // 3. !hugStart && hugEnd (lines 1229-1236): Normal opening, split closing
        // 4. !hugStart && !hugEnd (lines 1241-1247): Normal formatting
        //
        // TODO: Refactor into separate functions (see research notes):
        // - print_hug_both_element() - Case 1
        // - print_hug_start_element() - Case 2
        // - print_hug_end_element() - Case 3
        // - Normal flow for Case 4
        // This would reduce code duplication and make the logic clearer.
        if use_hug_mode && hug_start && hug_end {
            // Case 1: Full hug mode (prettier lines 1180-1189)
            // Pattern: ><content</Tag\n> (with wrapped attrs) or \n\t><content</Tag\n> (no attrs)
            // IMPORTANT: attrs_multiline parameter is critical - controls opening > indentation
            self.print_hug_mode_element(
                &tag_name,
                &element.fragment.nodes,
                preserves_ws,
                hug_start,
                hug_end,
                attrs_multiline,
            );
        } else if use_hug_mode && hug_start && !hug_end {
            // Case 2: Hug start only (prettier lines 1221-1227)
            // Pattern: \t><content>\n</Tag>
            // Opening: indented > on new line
            self.write("\n");
            self.indent_level += 1;
            self.write_indent();
            self.write(">");

            // Children: print inline, skipping whitespace-only text nodes
            for node in &element.fragment.nodes {
                if !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()) {
                    self.print_fragment_node(node, false, preserves_ws);
                }
            }

            // Closing: normal closing tag on new line with indent
            self.write("\n");
            self.indent_level -= 1;
            self.write_indent();
            self.write("</");
            self.write(&tag_name);
            self.write(">");
        } else if use_hug_mode && !hug_start && hug_end {
            // Case 3: Hug end only (prettier lines 1229-1236)
            // Pattern: >\n\t<content></Tag\n>
            // Opening: normal opening with newline
            self.write("\n");
            self.write(">");

            // Children: print with multiline formatting but no trailing newline
            self.write("\n");
            self.indent_level += 1;
            for (i, node) in element.fragment.nodes.iter().enumerate() {
                // Skip leading whitespace-only text node
                if i == 0
                    && matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())
                {
                    continue;
                }

                self.write_indent();
                self.print_fragment_node(node, false, preserves_ws);

                // Add newline before next node (but not after last)
                if i < element.fragment.nodes.len() - 1 {
                    self.write("\n");
                }
            }
            self.indent_level -= 1;

            // Closing: split closing tag (immediately after last child)
            self.write("</");
            self.write(&tag_name);
            self.write("\n");
            self.write_indent();
            self.write(">");
        } else if use_hug_mode && !hug_start && !hug_end {
            // Case 4: Neither hug (prettier lines 1241-1247)
            // Pattern: >\n\t<content>\n</Tag>
            // This is just normal multiline formatting
            self.write("\n");
            self.write(">");

            // Children: normal multiline formatting
            self.print_multiline_children(&element.fragment.nodes, preserves_ws);

            // Closing: normal closing tag
            self.write("</");
            self.write(&tag_name);
            self.write(">");
        } else {
            // Check if this is the "attrs wrapped + 1 native block child" case
            // This uses inline opening (no newline after >) + split closing tag
            // Only applies when source has NO opening newline (preserve author intent)
            let has_source_opening_newline = element.fragment.nodes.first().is_some_and(|node| {
                matches!(node, FragmentNode::Text(text)
                    if text.raw.is_whitespace_only() && text.raw.contains('\n'))
            });

            let is_wrapped_attrs_single_block = attrs_multiline
                && !has_source_opening_newline
                && {
                    let non_ws_children: Vec<_> = element.fragment.nodes.iter().filter(|node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())).collect();
                    non_ws_children.len() == 1
                        && matches!(non_ws_children.first(), Some(FragmentNode::Element(el)) if self.is_block_element(el))
                };

            // Add newline before > only if:
            // - Attributes wrapped AND
            // - NOT the special single-block case
            if attrs_multiline && !is_wrapped_attrs_single_block {
                self.write("\n");
            }
            self.write(">");

            // Check for split closing tag patterns:
            // 1. "wrapped attrs + single block" pattern: <Tag attrs...><content></Tag\n>
            // 2. "opening newline only" pattern: <Tag>\n\t{content}</Tag\n>
            let use_split_closing_tag = is_wrapped_attrs_single_block
                || (multiline && self.has_opening_newline_only(&element.fragment.nodes));

            if use_split_closing_tag {
                // Split closing tag with optional opening newline
                if !is_wrapped_attrs_single_block {
                    // Regular split closing: add opening newline + indent
                    self.write("\n");
                    self.indent_level += 1;
                    self.write_indent();
                }

                // Print content inline (skip leading whitespace, preserve rest)
                for (i, node) in element.fragment.nodes.iter().enumerate() {
                    // Skip first node if it's the leading whitespace
                    if i == 0
                        && matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only())
                    {
                        continue;
                    }
                    self.print_fragment_node(node, false, preserves_ws);
                }

                // Split closing tag
                self.write("</");
                self.write(&tag_name);
                if !is_wrapped_attrs_single_block {
                    self.indent_level -= 1;
                }
                self.write("\n");
                self.write_indent();
                self.write(">");
            } else if multiline {
                // Normal multiline formatting
                self.print_multiline_children(&element.fragment.nodes, preserves_ws);
                self.write("</");
                self.write(&tag_name);
                self.write(">");
            } else {
                // Compact formatting
                self.print_compact_children(&element.fragment, is_block, preserves_ws);
                self.write("</");
                self.write(&tag_name);
                self.write(">");
            }
        }
    }

    /// Determine if element children should be formatted multiline
    ///
    /// # Formatting Rules
    ///
    /// Multiline formatting is used when:
    /// 1. **Block parent with block children**: Multiple block children OR mixed block + text
    ///    - Example: `<div><div>a</div><div>b</div></div>` → multiline
    /// 2. **Source contains newlines + multiple children**: Preserves author's line breaks
    ///    - Example: `<div>\n  <span>a</span>\n  <span>b</span>\n</div>` → multiline
    ///
    /// Stays compact when:
    /// - Single block child (no text mixing)
    /// - All inline children (no separation needed)
    /// - Single text node (preserves author's single-line intent, even with newlines)
    /// - Whitespace-preserving parent (`<pre>`, `<textarea>`, etc.)
    ///
    /// # Rationale
    ///
    /// - **Rule 1** avoids putting block children on the same line (hurts readability)
    /// - **Rule 2** respects source layout for semantic significance (blank lines for grouping)
    /// - **Single text exception** handles cases like `<div>\n  text\n</div>` → stays `<div>text</div>`
    pub fn should_format_multiline(
        &self,
        element: &internal::Element,
        is_block: bool,
        preserves_ws: bool,
    ) -> bool {
        // Whitespace-preserving elements never go multiline
        if preserves_ws {
            return false;
        }

        // Only block elements can have multiline formatting
        if !is_block {
            return false;
        }

        let fragment = &element.fragment;

        // Rule 1: Block parent with block children needs separation
        let element_child_count = fragment
            .nodes
            .iter()
            .filter(|node| matches!(node, FragmentNode::Element(_)))
            .count();
        let has_block_children = self.has_block_elements(fragment);
        let has_text = self.has_text_content(fragment);
        let needs_block_separation = has_block_children && (element_child_count > 1 || has_text);

        // Rule 2: Newlines before last content trigger multiline (format preservation)
        // Simple rule: Check if ANY text node before the last content node has newlines
        // This naturally excludes trailing newlines (they're after last content)
        // Examples:
        //   <Comp>\n  {expr}\n</Comp>           → multiline (newline before expr)
        //   <Comp>  {\n  expr\n}\n</Comp>       → compact (only trailing newline)
        //   <div>\n\tText: {expr}\n</div>       → multiline (newline in text before expr)
        let has_source_line_breaks = {
            // Find the last non-whitespace-only content node
            let last_content_idx = fragment.nodes.iter().rposition(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            );

            if let Some(last_idx) = last_content_idx {
                // Check if ANY text node before the last content has newlines
                fragment.nodes[..last_idx]
                    .iter()
                    .any(|node| matches!(node, FragmentNode::Text(text) if text.raw.contains('\n')))
            } else {
                false // No content nodes at all
            }
        };

        needs_block_separation || has_source_line_breaks
    }

    /// Determine if element needs "hug mode" (`><` pattern) formatting
    ///
    /// Hug mode is Prettier's special formatting for inline/component parents with block children.
    /// Instead of standard multiline indentation, it uses the `><` pattern:
    ///
    /// ```svelte
    /// <Comp
    ///   ><div>a</div>
    ///   <div>b</div></Comp
    /// >
    /// ```
    ///
    /// This is triggered when:
    /// - Parent is an inline element OR component
    /// - Children are all block HTML elements OR all components
    /// - For native block children: 2+ children OR (1 child AND attrs wrapped)
    /// - For component children: attrs wrapped AND 1+ children
    /// - Children don't already have newlines (compact source)
    pub fn needs_hug_mode(
        &self,
        element: &internal::Element,
        is_block: bool,
        attrs_wrapped: bool,
    ) -> bool {
        use crate::ast::internal::ElementKind;

        // Only inline elements and components can use hug mode
        let is_inline_or_component = !is_block || element.kind == ElementKind::Component;

        if !is_inline_or_component {
            return false;
        }

        // Hug mode only applies when children are ALL block HTML elements
        // Mixed content (blocks + components/expressions) uses normal multiline
        let non_whitespace_children: Vec<_> = element
            .fragment
            .nodes
            .iter()
            .filter(
                |node| !matches!(node, FragmentNode::Text(text) if text.raw.is_whitespace_only()),
            )
            .collect();

        // Need at least 1 child for hug mode
        let child_count = non_whitespace_children.len();
        if child_count == 0 {
            return false;
        }

        // Hug mode rules:
        // 1. Native block children → hug mode if (2+ children) OR (1 child AND attrs wrapped)
        // 2. Component children → hug mode if (attrs wrapped AND 1+ children)
        // NOTE: Component *parents* with 1 native block child + wrapped attrs is a common case
        let all_native_block = non_whitespace_children
            .iter()
            .all(|node| matches!(node, FragmentNode::Element(el) if self.is_block_element(el)));

        let all_components = non_whitespace_children.iter().all(
            |node| matches!(node, FragmentNode::Element(el) if el.kind == ElementKind::Component),
        );

        // Determine if hug mode applies
        let should_use_hug = if all_native_block {
            // Native block children: 2+ children OR (1 child AND attrs wrapped)
            // The (1 child AND attrs wrapped) case is critical for Component parents
            child_count >= 2 || (child_count == 1 && attrs_wrapped)
        } else if all_components {
            // Component children: attrs wrapped AND 1+ children
            attrs_wrapped && child_count >= 1
        } else {
            // Mixed or non-element children: no hug mode
            false
        };

        if !should_use_hug {
            return false;
        }

        // Only use hug mode if children are compact (no newlines in content)
        // Ignore whitespace-only text nodes (they're formatting whitespace, not semantic content)
        let has_content_newlines = element.fragment.nodes.iter().any(|node| {
            matches!(node, FragmentNode::Text(text)
                if text.raw.contains('\n') && !text.raw.is_whitespace_only())
        });

        !has_content_newlines
    }

    /// Check if element has "opening newline only" pattern
    ///
    /// This pattern is when:
    /// - First child is whitespace text with newline (opening tag followed by newline)
    /// - Last child is NOT a text node at all (closing tag immediately after non-text content)
    ///
    /// Example: `<Comp>\n\t{count}</Comp>` (no text node after expression)
    ///
    /// Prettier uses split closing tag format for this: `</Tag\n>` instead of `\n</Tag>`
    fn has_opening_newline_only(&self, nodes: &[FragmentNode]) -> bool {
        if nodes.is_empty() {
            return false;
        }

        // Check if first child is whitespace text with newline
        let has_opening_newline = matches!(nodes.first(), Some(FragmentNode::Text(text))
            if text.raw.is_whitespace_only() && text.raw.contains('\n'));

        // Check if last child is a text node
        // If it is, then there's trailing content/whitespace, so NOT opening-newline-only pattern
        let has_trailing_text = matches!(nodes.last(), Some(FragmentNode::Text(_)));

        // Opening newline only pattern: opening has newline, NO trailing text node
        has_opening_newline && !has_trailing_text
    }

    /// Check if element should "hug" the start (opening tag hugs first child)
    ///
    /// Based on prettier-plugin-svelte's `shouldHugStart()` logic:
    /// - Block elements never hug
    /// - Inline elements hug if first child has NO leading whitespace
    /// - Returns true for ><child pattern, false for >\n\t<child pattern
    ///
    /// TODO: Consider extracting helper predicates:
    /// - `is_whitespace_node(&FragmentNode) -> bool`
    /// - `has_leading_whitespace(&Element) -> bool`
    /// - `has_trailing_whitespace(&Element) -> bool`
    fn should_hug_start(&self, element: &internal::Element, is_block: bool) -> bool {
        // Block elements never hug
        if is_block {
            return false;
        }

        // Empty elements hug by default
        if element.fragment.nodes.is_empty() {
            return true;
        }

        // Check if first child starts with whitespace
        let first_child = &element.fragment.nodes[0];
        match first_child {
            FragmentNode::Text(text) => {
                // If it's whitespace-only, check if it's formatting whitespace (newline)
                // If it has a newline, don't hug (spaced pattern)
                if text.raw.is_whitespace_only() {
                    !text.raw.contains('\n')
                } else {
                    // Text content with no leading whitespace → hug
                    !text.raw.starts_with(char::is_whitespace)
                }
            }
            // Non-text first child → hug
            _ => true,
        }
    }

    /// Check if element should "hug" the end (closing tag hugs last child)
    ///
    /// Based on prettier-plugin-svelte's `shouldHugEnd()` logic:
    /// - Block elements never hug
    /// - Inline elements hug if last child has NO trailing whitespace
    /// - Returns true for child</Tag\n> pattern, false for child\n</Tag> pattern
    fn should_hug_end(&self, element: &internal::Element, is_block: bool) -> bool {
        // Block elements never hug
        if is_block {
            return false;
        }

        // Empty elements hug by default
        if element.fragment.nodes.is_empty() {
            return true;
        }

        // Check if last child ends with whitespace
        let last_child = &element.fragment.nodes[element.fragment.nodes.len() - 1];
        match last_child {
            FragmentNode::Text(text) => {
                // If it's whitespace-only, check if it's formatting whitespace (newline)
                // If it has a newline, don't hug (spaced pattern)
                if text.raw.is_whitespace_only() {
                    !text.raw.contains('\n')
                } else {
                    // Text content with no trailing whitespace → hug
                    !text.raw.ends_with(char::is_whitespace)
                }
            }
            // Non-text last child → hug
            _ => true,
        }
    }

    /// Format a Svelte special element
    ///
    /// Special elements include: `<svelte:head>`, `<svelte:window>`, `<slot>`, etc.
    /// Similar to `print_element` but uses the kind's tag name.
    pub fn print_special_element(&mut self, element: &internal::SpecialElement) {
        use crate::ast::internal::{FragmentNode, SpecialElementKind};

        let tag_name = element.kind.tag_name();

        // Most special elements can have children, but some are empty by design
        let is_typically_empty = matches!(
            element.kind,
            SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteComponent
                | SpecialElementKind::SvelteSelf
        );

        // Determine if this is an inline-content element (like slot, svelte:fragment)
        let is_inline_element = matches!(
            element.kind,
            SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::SvelteComponent
                | SpecialElementKind::SvelteElement
                | SpecialElementKind::TitleElement
        );

        // svelte:boundary always formats inline (compacts newlines)
        let is_always_inline = matches!(element.kind, SpecialElementKind::SvelteBoundary);

        // Opening tag
        self.write("<");
        self.write(tag_name);

        // Special handling for svelte:element's `this` attribute
        if let Some(ref tag_expr) = element.tag {
            self.write(" this={");
            self.print_ts_expression(tag_expr);
            self.write("}");
        }

        // Special handling for svelte:component's `this` attribute
        if let Some(ref expr) = element.expression {
            self.write(" this={");
            self.print_ts_expression(expr);
            self.write("}");
        }

        // Format other attributes
        for attr in &element.attributes {
            self.write(" ");
            self.print_attribute_node(attr);
        }

        // Self-closing if empty
        if element.fragment.nodes.is_empty() {
            if is_typically_empty {
                // These elements are typically self-closing
                self.write(" />");
            } else {
                // Other elements with no children: use explicit closing tag
                self.write("></");
                self.write(tag_name);
                self.write(">");
            }
            return;
        }

        self.write(">");

        // Determine formatting mode based on content
        let has_block_children = element.fragment.nodes.iter().any(|node| {
            matches!(node, FragmentNode::Element(el) if self.is_block_element(el))
                || matches!(node, FragmentNode::SpecialElement(se) if matches!(
                    se.kind,
                    SpecialElementKind::SvelteHead
                ))
        });

        // Check if children are all simple text content (for compact formatting)
        // svelte:boundary always uses compact formatting; others check has_block_children
        let is_simple_inline_content = is_always_inline
            || (!has_block_children
                && (is_inline_element
                    || element.fragment.nodes.iter().all(|node| match node {
                        FragmentNode::Text(_) | FragmentNode::ExpressionTag(_) => true,
                        FragmentNode::SpecialElement(se) => matches!(
                            se.kind,
                            SpecialElementKind::SlotElement | SpecialElementKind::SvelteFragment
                        ),
                        _ => false,
                    })));

        if is_simple_inline_content {
            // Compact formatting for inline content
            self.print_compact_children(&element.fragment, true, false);
        } else {
            // Block formatting for complex content
            self.print_multiline_children(&element.fragment.nodes, false);
        }

        // Closing tag
        self.write("</");
        self.write(tag_name);
        self.write(">");
    }

    /// Format attributes with line wrapping support (hybrid doc-builder approach)
    ///
    /// Uses prettier's doc-builder pattern to decide whether to wrap attributes:
    /// - If all attributes fit on one line: print inline with spaces
    /// - If they don't fit: print each attribute on its own line with indentation
    ///
    /// Matches prettier's behavior from prettier-plugin-svelte:
    /// ```javascript
    /// group([
    ///     '<', node.name,
    ///     indent(group([
    ///         ...attributes,  // Each prepended with `line`
    ///         dedent(line()),
    ///     ])),
    ///     '/>'
    /// ])
    /// ```
    fn print_attributes_with_wrapping(
        &mut self,
        element: &internal::Element,
        tag_name: &str,
        is_void: bool,
    ) -> bool {
        use tsv_lang::doc;

        if element.attributes.is_empty() {
            return false; // No attributes, not multiline
        }

        // Build a doc for all attributes
        let mut attr_docs = Vec::new();
        for attr in &element.attributes {
            attr_docs.push(doc::line()); // Soft line before each attribute
            attr_docs.push(self.build_attribute_node_doc(attr));
        }

        // Add dedent(line()) at the end for proper spacing before closing tag
        // In flat mode: becomes a space
        // In break mode: becomes newline at dedented level
        attr_docs.push(doc::dedent(doc::line()));

        // For line length calculation, we need to include the tag name and closing
        // Build a complete doc to let the fits() algorithm make the right decision
        //
        // Prettier's behavior (from prettier-plugin-svelte):
        // - For empty elements: includes full `></tagname>` in fits calculation
        // - For elements with content: only checks opening tag with `>`
        // - For void/self-closing: uses `/>`
        let closing: String = if is_void {
            " />".to_string()
        } else if element.fragment.nodes.is_empty() {
            // Empty elements: include closing tag in calculation (matches prettier)
            // e.g., `></div>` for <div></div>
            format!("></{tag_name}>")
        } else {
            ">".to_string()
        };

        // Build the complete doc structure matching prettier
        // group([ '<', name, indent(group([ ...attributes ])), closing ])
        let complete_doc = doc::group(doc::concat(vec![
            doc::text("<"),
            doc::text(tag_name),
            doc::indent(doc::group(doc::concat(attr_docs))),
            doc::text(&closing),
        ]));

        // Check if the complete tag fits on one line
        let fits = doc::fits(
            &complete_doc,
            self.config.print_width,
            doc::Mode::Flat,
            &self.config,
        );

        if fits {
            // Print inline: space before each attribute
            for attr in &element.attributes {
                self.write(" ");
                self.print_attribute_node(attr);
            }
            false // Not multiline
        } else {
            // Print multiline: each attribute on its own line
            self.indent_level += 1;
            for attr in &element.attributes {
                self.write("\n");
                self.write_indent();
                self.print_attribute_node(attr);
            }
            self.indent_level -= 1;
            // NOTE: Newline before closing tag is handled by caller
            // (different behavior for hug mode vs normal mode)
            true // Multiline
        }
    }
}
