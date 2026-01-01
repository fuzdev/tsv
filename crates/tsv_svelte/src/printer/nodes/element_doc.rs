// Doc-based formatting for regular HTML/component elements
//
// Handles all element types except svelte:* special elements:
// - HTML elements (div, span, etc.)
// - Components (PascalCase)
// - Void elements (br, img, etc.)
// - Raw content elements (script, style)
// - Whitespace-sensitive elements (pre, textarea)

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use crate::printer::text::TextAnalysis;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build a doc for an element (regular HTML or component)
    ///
    /// Includes opening tag with attributes, children, and closing tag.
    /// Attribute wrapping is handled via group() with line separators.
    pub(crate) fn build_element_doc(&self, element: &internal::Element) -> Doc {
        let tag_name = self.resolve_symbol(element.name);
        let is_void = tsv_html::is_void_element(&tag_name);
        let is_foreign = tsv_html::is_foreign_element(&tag_name);
        let is_component = tag_name.starts_with(|c: char| c.is_ascii_uppercase())
            || tag_name.contains(':')
            || tag_name.contains('.');
        // Determine if children text should be trimmed:
        // - Only trim if parent is block/component AND children are ONLY text
        // - If children have expressions/elements mixed with text, preserve spacing
        let is_block_parent = is_component || !tsv_html::is_inline_element(&tag_name);
        let only_text_children = element
            .fragment
            .nodes
            .iter()
            .all(|n| matches!(n, FragmentNode::Text(_)));
        let trim_text_children = is_block_parent && only_text_children;

        // Check if self-closing (component/foreign with no children and was self-closing in source)
        let is_self_closing = (is_component || is_foreign)
            && element.fragment.nodes.is_empty()
            && self.was_self_closing_doc(element);

        // Build attribute docs
        let attr_docs = self.build_element_attrs_doc(&element.attributes);
        let has_attrs = !attr_docs.is_empty();

        // Special handling for <style> and <script> elements
        // These need their content formatted as CSS/JS, not as regular fragment nodes
        if tag_name == "style" || tag_name == "script" {
            return self.build_raw_content_element_doc(&tag_name, element, attr_docs);
        }

        // Whitespace-sensitive elements (pre, textarea, etc.) preserve content exactly
        if tsv_html::preserves_whitespace(&tag_name) {
            return self.build_whitespace_sensitive_element_doc(&tag_name, element, attr_docs);
        }

        if is_void || is_self_closing {
            // Self-closing: <tag attrs />
            // Wrap entire element in group so fits check includes tag + attrs + closing
            if attr_docs.is_empty() {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tag_name),
                    doc::text(" />"),
                ])
            } else {
                // Group wraps everything from < to />
                // When fits check runs, it checks: <tag + attrs + /> + siblings
                // - Inline (flat): attrs ` />` (space before />)
                // - Multiline (break): attrs on lines, newline + `/>` at element level
                // Note: Use line() not hardline() - hardline causes will_break() to return true
                doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tag_name),
                    doc::indent(doc::concat(attr_docs)),
                    // line() = space in flat mode, newline in break mode
                    doc::line(),
                    doc::text("/>"),
                ]))
            }
        } else {
            // Regular element: <tag attrs>children</tag>
            // Determine if this is a block element (for hug mode detection)
            let is_block = !tsv_html::is_inline_element(&tag_name) && !is_component;

            // Check hug mode
            let hug_start = self.should_hug_start(element, is_block);
            let hug_end = self.should_hug_end(element, is_block);
            let is_empty = element.fragment.nodes.is_empty()
                || element
                    .fragment
                    .nodes
                    .iter()
                    .all(FragmentNode::is_whitespace_only_text);
            // Detect if source was multiline (has newline at boundary)
            // If the first child is whitespace-only text with newline, source was multiline
            // This forces hardline instead of softline to preserve layout
            // Key: only force breaks if the LEADING boundary has newline (hug mode pattern)
            // Trailing-only breaks are just extra whitespace that should normalize away
            let source_has_leading_break = element.fragment.nodes.first().is_some_and(|n| {
                matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only() && t.raw.contains('\n'))
            });
            // Only check trailing break if we have a leading break (consistent hug pattern)
            let source_has_trailing_break = source_has_leading_break
                && element.fragment.nodes.last().is_some_and(|n| {
                    matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only() && t.raw.contains('\n'))
                });

            // Detect if content should be multiline:
            // 1. Block parent with multiple block children (always needs multiline)
            // 2. Source has newlines between content (preserve structure)
            // Note: Block FLOW (if, each, etc.) and COMPONENTS don't trigger multiline,
            // only HTML block ELEMENTS (div, p, etc.) do.
            let is_block_element_child = |n: &FragmentNode| -> bool {
                match n {
                    FragmentNode::Element(el) => {
                        // Only HTML block elements, not components
                        if el.kind == internal::ElementKind::Component {
                            return false;
                        }
                        let tag = self.resolve_symbol(el.name);
                        tsv_html::is_block_element(&tag)
                    }
                    FragmentNode::SpecialElement(_) => false, // svelte:* elements don't trigger multiline
                    // Block flow (if, each, etc.) is NOT counted as block for multiline
                    _ => false,
                }
            };
            let has_block_children = element.fragment.nodes.iter().any(&is_block_element_child);
            let has_multiple_block_children = has_block_children
                && element
                    .fragment
                    .nodes
                    .iter()
                    .filter(|n| is_block_element_child(n))
                    .count()
                    > 1;

            // Check for source breaks: newlines in content that indicate author's line structure
            // This includes:
            // 1. Whitespace-only text with newlines BETWEEN content nodes
            // 2. Content text that starts with a newline (e.g., "\n\ttext" after <input />)
            let has_source_breaks = {
                let first_content_idx = element
                    .fragment
                    .nodes
                    .iter()
                    .position(|n| !n.is_whitespace_only_text());
                let last_content_idx = element
                    .fragment
                    .nodes
                    .iter()
                    .rposition(|n| !n.is_whitespace_only_text());
                match (first_content_idx, last_content_idx) {
                    (Some(first), Some(last)) if first < last => {
                        // Check for newlines between content nodes OR embedded in content text
                        element.fragment.nodes[first..=last]
                            .iter()
                            .any(|n| match n {
                                // Whitespace-only text with newlines between content
                                FragmentNode::Text(t) if t.raw.is_whitespace_only() => {
                                    t.raw.contains('\n')
                                }
                                // Content text that starts with newline (after previous content)
                                FragmentNode::Text(t) => t.raw.starts_with('\n'),
                                _ => false,
                            })
                    }
                    _ => false,
                }
            };

            // Check for block flow children (if, each, await, key, snippet)
            // These force the parent element to break to multiline
            let has_block_flow_children = element.fragment.nodes.iter().any(|n| {
                matches!(
                    n,
                    FragmentNode::IfBlock(_)
                        | FragmentNode::EachBlock(_)
                        | FragmentNode::AwaitBlock(_)
                        | FragmentNode::KeyBlock(_)
                        | FragmentNode::SnippetBlock(_)
                )
            });

            // Multiline when:
            // 1. Multiple block children (each needs its own line)
            // 2. Block child mixed with non-block children (inline elements or text)
            // 3. Source has newlines between content (preserve author's line structure)
            // 4. Source has boundary breaks AND multiple expressions with whitespace between them
            // (Empty content stays compact)
            // Count non-whitespace, non-block-element children
            // This includes inline elements, text with content, and expressions
            // Block flow (if, each) is NOT counted here - they're handled separately
            let has_non_block_children = element.fragment.nodes.iter().any(|n| match n {
                FragmentNode::Text(t) => !t.raw.is_whitespace_only(),
                FragmentNode::Element(e) => !self.is_block_element(e),
                FragmentNode::SpecialElement(_) => false, // svelte:* elements are block-like
                FragmentNode::ExpressionTag(_) => true,
                FragmentNode::Comment(_) => false,
                // Block flow - not considered for mixed content with block elements
                FragmentNode::IfBlock(_)
                | FragmentNode::EachBlock(_)
                | FragmentNode::AwaitBlock(_)
                | FragmentNode::KeyBlock(_)
                | FragmentNode::SnippetBlock(_) => false,
                FragmentNode::HtmlTag(_)
                | FragmentNode::ConstTag(_)
                | FragmentNode::DebugTag(_)
                | FragmentNode::RenderTag(_) => true,
            });
            let has_mixed_content = has_block_children && has_non_block_children;

            // Check for expression splitting: leading boundary break + trailing whitespace + multiple expressions
            // Prettier splits expressions to separate lines when:
            // - Source has leading break (multiline intent)
            // - Source has trailing whitespace (space or newline) before closing
            // - Multiple expressions with whitespace between them
            // When content hugs the closing tag (no trailing whitespace), expressions stay inline
            let should_split = self.should_split_expressions_in_nodes(&element.fragment.nodes);
            let has_trailing_whitespace = !hug_end; // hug_end = false when last node ends with whitespace
            let needs_expression_split =
                source_has_leading_break && has_trailing_whitespace && should_split;

            // Block flow children (if, each, etc.) that have multiline content force parent to multiline
            // This ensures content after the block goes on its own line
            let block_flow_forces_multiline = has_block_flow_children && {
                element.fragment.nodes.iter().any(|n| match n {
                    FragmentNode::IfBlock(b) => !self.is_inline_fragment(&b.consequent),
                    FragmentNode::EachBlock(b) => !self.is_inline_fragment(&b.body),
                    FragmentNode::AwaitBlock(b) => {
                        b.pending
                            .as_ref()
                            .is_some_and(|f| !self.is_inline_fragment(f))
                            || b.then.as_ref().is_some_and(|f| !self.is_inline_fragment(f))
                            || b.catch
                                .as_ref()
                                .is_some_and(|f| !self.is_inline_fragment(f))
                    }
                    FragmentNode::KeyBlock(b) => !self.is_inline_fragment(&b.fragment),
                    FragmentNode::SnippetBlock(b) => !self.is_inline_fragment(&b.body),
                    _ => false,
                })
            };

            let needs_multiline = !is_empty
                && (has_multiple_block_children
                    || has_mixed_content
                    || has_source_breaks
                    || needs_expression_split
                    || block_flow_forces_multiline);

            // Build children doc based on hug mode and multiline needs
            // - Multiline: each child on its own line
            // - Hug both inline: no trimming (no boundary whitespace by definition)
            // - All other cases: trim boundary whitespace (Prettier's trimTextNodeLeft/Right)
            let should_trim_boundaries = !(hug_start && hug_end);
            let children_doc = if needs_multiline {
                // Multiline: each child on its own line
                self.build_nodes_doc_multiline(&element.fragment.nodes)
            } else if should_trim_boundaries {
                // Non-hug case: trim whitespace-only text at boundaries
                self.build_nodes_doc_trimmed(&element.fragment.nodes)
            } else {
                self.build_nodes_doc_with_context(&element.fragment.nodes, trim_text_children)
            };

            // Build opening tag with attrs
            // Attrs are wrapped in their own group inside indent, so when the outer element
            // group breaks, the attrs group can stay flat if they fit on one line.
            // Structure matches Prettier: indent(group([attrs, dedent(softline)]))
            let opening_tag = if attr_docs.is_empty() {
                doc::concat(vec![doc::text("<"), doc::text_owned(tag_name.clone())])
            } else {
                doc::concat(vec![
                    doc::text("<"),
                    doc::text_owned(tag_name.clone()),
                    doc::indent(doc::group(doc::concat(vec![
                        doc::concat(attr_docs),
                        // dedent(softline) puts the softline at original level
                        // so when breaking, newline is at element level, not attr level
                        if hug_start && !is_empty {
                            doc::text("") // No softline before > when hugging
                        } else {
                            doc::dedent(doc::softline())
                        },
                    ]))),
                ])
            };

            if hug_start && hug_end {
                self.build_hug_both_doc(
                    &tag_name,
                    element,
                    opening_tag,
                    children_doc,
                    has_attrs,
                    has_block_flow_children,
                    needs_multiline,
                    is_empty,
                    is_component,
                )
            } else if hug_start {
                // Hug start only: > hugs content, closing on normal line
                // Use hardline if source was multiline or attrs will wrap
                // Multiple attrs typically cause wrapping, which means element goes multiline
                let has_multiline_attrs = element.attributes.len() > 1;
                let force_break = source_has_leading_break || has_multiline_attrs;
                let leading_break = if force_break {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if source_has_trailing_break || has_multiline_attrs {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                doc::group(doc::concat(vec![
                    opening_tag,
                    doc::indent(doc::concat(vec![
                        leading_break,
                        doc::group(doc::concat(vec![doc::text(">"), children_doc])),
                    ])),
                    trailing_break,
                    doc::text("</"),
                    doc::text_owned(tag_name),
                    doc::text(">"),
                ]))
            } else if hug_end {
                // Hug end only: normal opening, content hugs closing tag
                // Use hardline if source was multiline to preserve layout
                let leading_break = if source_has_leading_break {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if source_has_trailing_break {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                doc::group(doc::concat(vec![
                    opening_tag,
                    doc::text(">"),
                    doc::indent(doc::concat(vec![
                        leading_break,
                        doc::group(doc::concat(vec![
                            children_doc,
                            doc::text("</"),
                            doc::text_owned(tag_name),
                        ])),
                    ])),
                    trailing_break,
                    doc::text(">"),
                ]))
            } else if is_empty {
                // Empty element - delegate to separate helper
                self.build_empty_element_doc(
                    &tag_name,
                    element,
                    opening_tag,
                    has_attrs,
                    is_component,
                )
            } else if needs_multiline {
                // Content needs multiline formatting (source multiline or multiple block children)
                // Use hardlines to force breaks and properly separate block children
                let multiline_children_doc =
                    self.build_nodes_doc_multiline(&element.fragment.nodes);
                doc::concat(vec![
                    opening_tag,
                    doc::text(">"),
                    doc::indent(doc::concat(vec![doc::hardline(), multiline_children_doc])),
                    doc::hardline(),
                    doc::text("</"),
                    doc::text_owned(tag_name),
                    doc::text(">"),
                ])
            } else {
                // No hugging: block element or whitespace at boundaries
                // Use hardline if source was multiline to preserve layout
                let leading_break = if source_has_leading_break {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if source_has_trailing_break {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                doc::group(doc::concat(vec![
                    opening_tag,
                    doc::text(">"),
                    doc::indent(doc::concat(vec![
                        leading_break,
                        doc::group(doc::concat(vec![children_doc])),
                    ])),
                    trailing_break,
                    doc::text("</"),
                    doc::text_owned(tag_name),
                    doc::text(">"),
                ]))
            }
        }
    }

    /// Build doc for hug-both mode (content hugs both opening and closing)
    #[allow(clippy::too_many_arguments)]
    fn build_hug_both_doc(
        &self,
        tag_name: &str,
        element: &internal::Element,
        _opening_tag: Doc,
        children_doc: Doc,
        has_attrs: bool,
        has_block_flow_children: bool,
        needs_multiline: bool,
        is_empty: bool,
        is_component: bool,
    ) -> Doc {
        // Hug both sides: ><content></tag\n>
        // When attrs break, > stays inline with last attr:
        //   <Comp
        //     attr="val"><body></Comp
        //   >
        // Structure: <tag, indent([attrs]), >, body, </tag, softline, >
        // The > immediately follows attrs (no softline before it)
        if !has_attrs {
            // No attrs - use standard indent pattern
            // Force multiline when:
            // - Block flow children (if, each, etc.)
            // - Content needs multiline (multiple blocks, mixed content, source breaks)
            let force_break = has_block_flow_children || needs_multiline;
            let leading_break = if force_break {
                doc::hardline()
            } else {
                doc::softline()
            };
            let trailing_break = if force_break {
                doc::hardline()
            } else {
                doc::softline()
            };
            let hugged_content = doc::concat(vec![
                leading_break,
                doc::group(doc::concat(vec![
                    doc::text(">"),
                    children_doc,
                    doc::text("</"),
                    doc::text_owned(tag_name.to_string()),
                ])),
            ]);

            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                if is_empty {
                    doc::group(hugged_content)
                } else {
                    doc::indent(hugged_content)
                },
                trailing_break,
                doc::text(">"),
            ]))
        } else {
            // With attrs - layout depends on whether there are block flow children
            // Rebuild attr_docs since we're in a different branch
            let hug_attr_docs = self.build_element_attrs_doc(&element.attributes);
            if has_block_flow_children {
                // Block flow forces multiline: > on new line after attrs
                // <span attr="val"
                //     >{#if ...}{/if}</span
                // >
                // Use nested group for opening tag to keep attrs flat
                doc::group(doc::concat(vec![
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::indent(doc::concat(hug_attr_docs)),
                    ])),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::text_owned(tag_name.to_string()),
                    ])),
                    doc::hardline(),
                    doc::text(">"),
                ]))
            } else {
                // No block flow - check if we need hug mode for inline elements
                // Inline elements with long attrs use hug mode: attrs inline, > on new line
                let is_inline_elem = tsv_html::is_inline_element(tag_name) || is_component;
                if is_inline_elem && is_empty {
                    // Use conditional_group for proper hug mode:
                    // 1. All inline: <tag attrs></tag>
                    // 2. Hug mode: <tag attrs\n></tag> (attrs inline, > on new line)
                    // 3. Full multiline: <tag\n\tattr\n></tag>
                    let closing = doc::concat(vec![
                        doc::text("></"),
                        doc::text_owned(tag_name.to_string()),
                        doc::text(">"),
                    ]);

                    // State 1: All inline
                    let inline_state = doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::indent(doc::concat(hug_attr_docs.clone())),
                        closing.clone(),
                    ]);

                    // State 2: Hug mode - attrs inline (space-separated), > on new line
                    let hug_space_attrs = self.build_element_attrs_doc_spaces(&element.attributes);
                    let hug_state = doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::concat(hug_space_attrs),
                        doc::hardline(),
                        closing.clone(),
                    ]);

                    // State 3: Full multiline - attrs on separate lines, > on new line
                    let multiline_state = doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::indent(doc::concat(hug_attr_docs)),
                        doc::hardline(),
                        closing,
                    ]);

                    doc::conditional_group(vec![inline_state, hug_state, multiline_state])
                } else {
                    // Block elements or elements with content - use nested groups
                    // like Prettier:
                    // - Outer group: controls whether content goes on new line
                    // - Inner group (around attrs): controls whether attrs break
                    //
                    // This produces 4 possible outputs:
                    // 1. Inline: <tag attrs>content</tag>
                    // 2. Content breaks: <tag attrs\n\t>content</tag\n>
                    // 3. Attrs break: <tag\n\tattrs>content</tag> (attrs break, body hugs)
                    // 4. Both break: <tag\n\tattrs\n\t>content</tag\n>
                    //
                    // The key insight: inner attrs group can stay flat even when
                    // outer element group breaks, allowing arrow functions to stay inline.
                    let body = doc::concat(vec![
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::text_owned(tag_name.to_string()),
                    ]);

                    // Prettier structure (hugStart && hugEnd case):
                    // group([
                    //   openingTag,  // includes indent(group([...attrs]))
                    //   group(indent([softline, group([">", body, "</tag"])])),
                    //   softline,
                    //   ">"
                    // ])
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::text_owned(tag_name.to_string()),
                        doc::indent(doc::group(doc::concat(hug_attr_docs))),
                        doc::group(doc::indent(doc::concat(vec![doc::softline(), body]))),
                        doc::softline(),
                        doc::text(">"),
                    ]))
                }
            }
        }
    }

    /// Build doc for empty element with no hugging
    fn build_empty_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element,
        opening_tag: Doc,
        has_attrs: bool,
        is_component: bool,
    ) -> Doc {
        // Empty element: <tag attrs></tag> with no content between > and </
        // For inline elements with attrs, use conditional_group for proper hug mode:
        // 1. All inline: <tag attrs></tag>
        // 2. Hug mode: <tag attrs\n></tag> (attrs inline, > on new line)
        // 3. Full multiline: <tag\n\tattr\n></tag> (attrs on separate lines)
        let is_inline = tsv_html::is_inline_element(tag_name) || is_component;
        if has_attrs && is_inline {
            // Build three alternative layouts for conditional_group
            let closing = doc::concat(vec![
                doc::text("></"),
                doc::text_owned(tag_name.to_string()),
                doc::text(">"),
            ]);

            // State 1: All inline (current structure with line separators)
            let inline_state = doc::concat(vec![opening_tag, closing.clone()]);

            // State 2: Hug mode - attrs inline (space-separated), > on new line
            let hug_attrs = self.build_element_attrs_doc_spaces(&element.attributes);
            let hug_state = doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::concat(hug_attrs),
                doc::hardline(),
                closing.clone(),
            ]);

            // State 3: Full multiline - attrs on separate lines (line-separated), > on new line
            let multiline_attrs = self.build_element_attrs_doc(&element.attributes);
            let multiline_state = doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::indent(doc::concat(multiline_attrs)),
                doc::hardline(),
                closing,
            ]);

            doc::conditional_group(vec![inline_state, hug_state, multiline_state])
        } else {
            // Block elements or no attrs - use simple structure
            doc::group(doc::concat(vec![
                opening_tag,
                doc::text("></"),
                doc::text_owned(tag_name.to_string()),
                doc::text(">"),
            ]))
        }
    }

    /// Build a doc for a nested <style> or <script> element with formatted CSS/JS content
    ///
    /// This handles nested style/script elements (inside other elements like `<div>`)
    /// that need their content formatted as CSS/JS rather than as regular fragment nodes.
    pub(super) fn build_raw_content_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element,
        attr_docs: Vec<Doc>,
    ) -> Doc {
        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::text(">"),
            ])
        } else {
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::indent(doc::group(doc::concat(vec![
                    doc::concat(attr_docs),
                    doc::dedent(doc::softline()),
                ]))),
                doc::text(">"),
            ]))
        };

        // Get raw content from the single Text child
        let content = element.fragment.nodes.first().and_then(|node| match node {
            FragmentNode::Text(text) => Some(text.data.as_str()),
            _ => None,
        });

        // Empty element or whitespace-only content
        let Some(content) = content.filter(|c| !c.trim().is_empty()) else {
            return doc::concat(vec![
                opening_tag,
                doc::text("</"),
                doc::text_owned(tag_name.to_string()),
                doc::text(">"),
            ]);
        };

        // Parse and format content based on tag type
        // Using base_indent_offset of 0 because we'll handle indentation in the doc structure
        let formatted = if tag_name == "style" {
            tsv_css::parse(content)
                .ok()
                .map(|ast| tsv_css::format(&ast, content))
        } else {
            tsv_ts::parse(content)
                .ok()
                .map(|ast| tsv_ts::format(&ast, content))
        };

        match formatted {
            Some(formatted) if !formatted.trim().is_empty() => {
                // Build doc with properly indented content
                // Each line of formatted content goes on its own line with indent
                let lines: Vec<&str> = formatted.trim_end().lines().collect();
                let mut content_lines = Vec::with_capacity(lines.len() * 2);
                for line in lines {
                    content_lines.push(doc::hardline());
                    if !line.is_empty() {
                        content_lines.push(doc::text_owned(line.to_string()));
                    }
                }

                doc::concat(vec![
                    opening_tag,
                    doc::indent(doc::concat(content_lines)),
                    doc::hardline(),
                    doc::text("</"),
                    doc::text_owned(tag_name.to_string()),
                    doc::text(">"),
                ])
            }
            _ => {
                // Fallback: preserve raw content if parsing fails
                doc::concat(vec![
                    opening_tag,
                    doc::text_owned(content.to_string()),
                    doc::text("</"),
                    doc::text_owned(tag_name.to_string()),
                    doc::text(">"),
                ])
            }
        }
    }

    /// Build doc for whitespace-sensitive elements (pre, textarea, etc.)
    ///
    /// These elements preserve their content exactly as-is without any formatting.
    pub(super) fn build_whitespace_sensitive_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element,
        attr_docs: Vec<Doc>,
    ) -> Doc {
        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::text(">"),
            ])
        } else {
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::text_owned(tag_name.to_string()),
                doc::indent(doc::group(doc::concat(vec![
                    doc::concat(attr_docs),
                    doc::dedent(doc::softline()),
                ]))),
                doc::text(">"),
            ]))
        };

        // Get raw content - preserve exact whitespace
        let raw_content = self.extract_raw_text_content(&element.fragment.nodes);

        doc::concat(vec![
            opening_tag,
            doc::text_owned(raw_content),
            doc::text("</"),
            doc::text_owned(tag_name.to_string()),
            doc::text(">"),
        ])
    }

    /// Extract raw text content from fragment nodes, preserving exact whitespace
    fn extract_raw_text_content(&self, nodes: &[FragmentNode]) -> String {
        let mut content = String::new();
        for node in nodes {
            if let FragmentNode::Text(text) = node {
                content.push_str(&text.raw);
            }
        }
        content
    }

    /// Build docs for element attributes (line-separated)
    pub(crate) fn build_element_attrs_doc(&self, attrs: &[internal::AttributeNode]) -> Vec<Doc> {
        self.build_element_attrs_doc_impl(attrs, doc::line())
    }

    /// Build docs for element attributes (space-separated, for hug mode)
    ///
    /// In hug mode, attributes stay on the same line with space separators.
    pub(crate) fn build_element_attrs_doc_spaces(
        &self,
        attrs: &[internal::AttributeNode],
    ) -> Vec<Doc> {
        self.build_element_attrs_doc_impl(attrs, doc::text(" "))
    }

    /// Build docs for element attributes with configurable separator
    fn build_element_attrs_doc_impl(
        &self,
        attrs: &[internal::AttributeNode],
        separator: Doc,
    ) -> Vec<Doc> {
        let mut docs = Vec::with_capacity(attrs.len() * 2);
        for attr in attrs {
            docs.push(separator.clone());
            docs.push(self.build_attribute_node_doc(attr));
        }
        docs
    }

    /// Check if element was self-closing in source (for doc building)
    pub(super) fn was_self_closing_doc(&self, element: &internal::Element) -> bool {
        element.span.extract(self.source).trim_end().ends_with("/>")
    }
}
