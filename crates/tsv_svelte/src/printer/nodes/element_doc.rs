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
use crate::printer::helpers::has_multiline_template_literal;
use crate::printer::text::TextAnalysis;
use tsv_lang::doc::{self, Doc};
use tsv_lang::{SymbolResolver, SymbolToU32};

/// How content relates to an element boundary (opening or closing tag)
///
/// This determines what separator (if any) appears between the tag and content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryMode {
    /// Content touches tag directly, no separator
    /// Example: `<span>text` or `text</span>`
    Hug,
    /// Hardline separator - preserves source structure
    /// Example: `<p>\n  text` (source had newline, preserve it)
    Hard,
    /// Softline separator - collapses or breaks based on fit
    /// Example: `<span> text` where space can collapse if needed
    Soft,
}

/// Element layout classification for doc building
///
/// Determines which doc structure to use based on element type and content.
#[derive(Debug)]
enum ElementLayout {
    /// Void element: `<br>`, `<img>`, etc. - no closing tag
    Void,
    /// Self-closing: `<Component />` - explicit self-close
    SelfClosing,
    /// Empty element with optional softline: `<div></div>`
    Empty,
    /// Element with content and boundary modes
    WithContent {
        start: BoundaryMode,
        end: BoundaryMode,
        /// Whether children need multiline formatting (each on own line)
        multiline_children: bool,
    },
}

/// Element type classification
///
/// Determines whitespace handling and formatting behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementKind {
    /// Svelte component (PascalCase or namespaced like `svelte:component`)
    Component,
    /// HTML block element (div, p, section, etc.)
    Block,
    /// HTML inline element (span, a, strong, etc.)
    Inline,
}

impl ElementKind {
    fn is_component(self) -> bool {
        matches!(self, ElementKind::Component)
    }

    fn is_block(self) -> bool {
        matches!(self, ElementKind::Block)
    }

    fn is_inline(self) -> bool {
        matches!(self, ElementKind::Inline)
    }

    /// Whether this element type preserves source structure at boundaries
    fn preserves_boundary_breaks(self) -> bool {
        matches!(self, ElementKind::Block | ElementKind::Component)
    }
}

/// Analysis context for element formatting decisions
///
/// Computed once per element, used to determine layout and build docs.
/// The bools capture orthogonal properties needed by different builder methods.
#[allow(clippy::struct_excessive_bools)]
struct ElementContext {
    /// Tag name resolved from symbol
    tag_name: String,
    /// Element type classification
    kind: ElementKind,
    /// Whether element is void (br, img, etc.)
    is_void: bool,
    /// Whether element was self-closing in source
    is_self_closing: bool,
    /// Whether element has no meaningful content
    is_empty: bool,
    /// Whether source has newline at opening boundary
    source_has_leading_break: bool,
    /// Whether source has newline at closing boundary
    source_has_trailing_break: bool,
    /// Whether content should hug the opening tag
    hug_start: bool,
    /// Whether content should hug the closing tag
    hug_end: bool,
    /// Whether children need multiline formatting
    needs_multiline: bool,
    /// Whether element has block flow children (if, each, etc.)
    has_block_flow_children: bool,
    /// Whether to trim boundary whitespace from children
    trim_boundaries: bool,
    /// Whether any attribute contains a template literal with embedded newlines
    has_multiline_template_attr: bool,
}

impl<'a> Printer<'a> {
    /// Build a doc for an element (regular HTML or component)
    ///
    /// Uses a three-phase approach:
    /// 1. Analyze: Compute all formatting-relevant properties
    /// 2. Classify: Determine layout strategy (void, empty, hug modes, etc.)
    /// 3. Build: Construct doc based on layout
    pub(crate) fn build_element_doc(&self, element: &internal::Element) -> Doc {
        let tag_name = self.resolve_symbol(element.name);
        let tag_sym = element.name.to_u32();

        // Build attribute docs (needed for all paths)
        let attr_docs = self.build_element_attrs_doc(&element.attributes);

        // Special handling for <style> and <script> elements
        if tag_name == "style" || tag_name == "script" {
            return self.build_raw_content_element_doc(&tag_name, element, attr_docs);
        }

        // Whitespace-sensitive elements (pre, textarea, etc.)
        if tsv_html::preserves_whitespace(&tag_name) {
            return self.build_whitespace_sensitive_element_doc(&tag_name, element, attr_docs);
        }

        // Phase 1: Analyze element
        let ctx = self.analyze_element(element);

        // Phase 2: Compute layout
        let layout = self.compute_element_layout(&ctx);

        // Phase 3: Build doc based on layout
        match layout {
            ElementLayout::Void | ElementLayout::SelfClosing => {
                self.build_void_element_doc(tag_sym, attr_docs, &element.attributes)
            }
            ElementLayout::Empty => {
                let opening_tag = self.build_opening_tag(
                    tag_sym,
                    &attr_docs,
                    false,
                    ctx.is_empty,
                    ctx.has_multiline_template_attr,
                );
                self.build_empty_element_doc(
                    &ctx.tag_name,
                    element,
                    opening_tag,
                    !attr_docs.is_empty(),
                    ctx.kind.is_component(),
                )
            }
            ElementLayout::WithContent {
                start,
                end,
                multiline_children,
            } => self.build_content_element_doc(
                element,
                &ctx,
                &attr_docs,
                start,
                end,
                multiline_children,
            ),
        }
    }

    /// Build doc for void or self-closing element
    ///
    /// When any attribute contains a template literal with embedded newlines,
    /// forces attributes to break across multiple lines to match Prettier behavior.
    fn build_void_element_doc(
        &self,
        tag_sym: u32,
        attr_docs: Vec<Doc>,
        attrs: &[internal::AttributeNode],
    ) -> Doc {
        if attr_docs.is_empty() {
            doc::concat(vec![doc::text("<"), doc::symbol(tag_sym), doc::text(" />")])
        } else {
            // Check if any attribute contains a template literal with embedded newlines
            let has_multiline_template = attrs
                .iter()
                .any(|a| has_multiline_template_literal(a.span().extract(self.source)));

            let inner = doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::indent(doc::concat(attr_docs)),
                doc::line(),
                doc::text("/>"),
            ]);

            if has_multiline_template {
                doc::group_break(inner)
            } else {
                doc::group(inner)
            }
        }
    }

    /// Build opening tag with attributes
    ///
    /// When `force_break` is true (e.g., template literal with embedded newlines),
    /// forces attributes to break across multiple lines.
    fn build_opening_tag(
        &self,
        tag_sym: u32,
        attr_docs: &[Doc],
        hug_start: bool,
        is_empty: bool,
        force_break: bool,
    ) -> Doc {
        if attr_docs.is_empty() {
            doc::concat(vec![doc::text("<"), doc::symbol(tag_sym)])
        } else {
            let inner = doc::concat(vec![
                doc::concat(attr_docs.to_vec()),
                if hug_start && !is_empty {
                    doc::empty()
                } else {
                    doc::dedent(doc::softline())
                },
            ]);
            let attr_group = if force_break {
                doc::group_break(inner)
            } else {
                doc::group(inner)
            };
            doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::indent(attr_group),
            ])
        }
    }

    /// Build doc for element with content using boundary modes
    fn build_content_element_doc(
        &self,
        element: &internal::Element,
        ctx: &ElementContext,
        attr_docs: &[Doc],
        start_mode: BoundaryMode,
        end_mode: BoundaryMode,
        multiline_children: bool,
    ) -> Doc {
        let tag_sym = element.name.to_u32();

        // Build children doc
        let children_doc = if multiline_children {
            self.build_nodes_doc_multiline(&element.fragment.nodes)
        } else if !(start_mode == BoundaryMode::Hug && end_mode == BoundaryMode::Hug) {
            self.build_nodes_doc_trimmed(&element.fragment.nodes, ctx.trim_boundaries)
        } else {
            // Hug both: determine if we should trim
            let is_block_parent = !ctx.kind.is_inline();
            let only_text_children = element
                .fragment
                .nodes
                .iter()
                .all(|n| matches!(n, FragmentNode::Text(_)));
            let trim_text = is_block_parent && only_text_children;
            self.build_nodes_doc_with_context(&element.fragment.nodes, trim_text)
        };

        // Build opening tag
        let opening_tag = self.build_opening_tag(
            tag_sym,
            attr_docs,
            start_mode == BoundaryMode::Hug,
            ctx.is_empty,
            ctx.has_multiline_template_attr,
        );

        // Build doc structure based on boundary modes
        match (start_mode, end_mode) {
            (BoundaryMode::Hug, BoundaryMode::Hug) => {
                self.build_hug_both_doc(element, ctx, children_doc)
            }
            (BoundaryMode::Hug, _) => {
                // Hug start: > hugs content
                let has_multiline_attrs = element.attributes.len() > 1;
                let leading_break = if ctx.source_has_leading_break || has_multiline_attrs {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if end_mode == BoundaryMode::Hard {
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
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ]))
            }
            (_, BoundaryMode::Hug) => {
                // Hug end: content hugs closing tag
                let leading_break = if start_mode == BoundaryMode::Hard {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if ctx.needs_multiline
                    || (ctx.kind.preserves_boundary_breaks() && ctx.source_has_trailing_break)
                {
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
                            doc::symbol(tag_sym),
                        ])),
                    ])),
                    trailing_break,
                    doc::text(">"),
                ]))
            }
            (BoundaryMode::Hard, BoundaryMode::Hard) => {
                // Full multiline
                let multiline_children_doc =
                    self.build_nodes_doc_multiline(&element.fragment.nodes);
                doc::concat(vec![
                    opening_tag,
                    doc::text(">"),
                    doc::indent(doc::concat(vec![doc::hardline(), multiline_children_doc])),
                    doc::hardline(),
                    doc::text("</"),
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ])
            }
            _ => {
                // Standard: soft breaks that can harden based on source
                let leading_break = if start_mode == BoundaryMode::Hard {
                    doc::hardline()
                } else {
                    doc::softline()
                };
                let trailing_break = if end_mode == BoundaryMode::Hard {
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
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ]))
            }
        }
    }

    /// Build doc for hug-both mode (content hugs both opening and closing)
    fn build_hug_both_doc(
        &self,
        element: &internal::Element,
        ctx: &ElementContext,
        children_doc: Doc,
    ) -> Doc {
        let tag_sym = element.name.to_u32();
        let has_attrs = !element.attributes.is_empty();

        // Hug both sides: ><content></tag\n>
        // When attrs break, > stays inline with last attr:
        //   <Comp
        //     attr="val"><body></Comp
        //   >
        // Structure: <tag, indent([attrs]), >, body, </tag, softline, >
        // The > immediately follows attrs (no softline before it)
        if !has_attrs {
            // No attrs - structure depends on whether we need to force breaks
            // Check if children contain elements that will produce multiline output
            // (e.g., inner spans with block flow children)
            let has_multiline_element_children = element.fragment.nodes.iter().any(|n| {
                if let FragmentNode::Element(child_el) = n {
                    // Check if child element has block flow that forces multiline
                    child_el
                        .fragment
                        .nodes
                        .iter()
                        .any(super::helpers::is_control_flow_block)
                } else {
                    false
                }
            });

            // Force multiline when:
            // - Expanding control flow blocks (if, each, key) - always force break
            //   Note: await blocks do NOT force break - they stay inline in inline elements
            // - Expanding blocks nested inside await blocks also force break
            // - Snippet blocks only force break when their content is not inline
            // - Content needs multiline (multiple blocks, mixed content, source breaks)
            // - Source has leading break (preserve author's multiline structure)
            // - Children contain elements that will be multiline
            let has_expanding_blocks =
                super::helpers::has_any_expanding_blocks(&element.fragment.nodes);
            let snippet_forces_break =
                ctx.has_block_flow_children && self.block_flow_forces_multiline(element);
            let force_break = has_expanding_blocks
                || snippet_forces_break
                || ctx.needs_multiline
                || ctx.source_has_leading_break
                || has_multiline_element_children;

            if force_break {
                // Block flow or multiline: use hardlines with indent
                let hugged_content = doc::concat(vec![
                    doc::hardline(),
                    doc::group(doc::concat(vec![
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::symbol(tag_sym),
                    ])),
                ]);

                doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::symbol(tag_sym),
                    if ctx.is_empty {
                        doc::group(hugged_content)
                    } else {
                        doc::indent(hugged_content)
                    },
                    doc::hardline(),
                    doc::text(">"),
                ]))
            } else {
                // Check if any expression has internal break points (ternary, &&, ||, +, etc.)
                // When breakable expressions exist, keep opening bracket hugging so expression
                // breaks are preferred over bracket breaks (reduces indentation drift).
                // When expressions are simple or text-only, allow opening bracket to break.
                let has_breakable_expressions = element.fragment.nodes.iter().any(|n| {
                    if let FragmentNode::ExpressionTag(tag) = n {
                        Self::expression_has_break_points(&tag.expression)
                    } else {
                        false
                    }
                });

                // Check if content has any expressions (breakable or not)
                let has_expressions = element.fragment.nodes.iter().any(|n| {
                    matches!(
                        n,
                        FragmentNode::ExpressionTag(_)
                            | FragmentNode::HtmlTag(_)
                            | FragmentNode::RenderTag(_)
                    )
                });

                if has_breakable_expressions {
                    // Breakable expressions: keep opening hugging, expressions break internally
                    // This reduces indentation drift (1 less tab level) - intentional divergence
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::symbol(tag_sym),
                        doc::softline(),
                        doc::text(">"),
                    ]))
                } else if has_expressions {
                    // Simple expressions (identifiers, member access): Prettier-like structure
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::concat(vec![
                            doc::softline(),
                            doc::text(">"),
                            children_doc,
                            doc::text("</"),
                            doc::symbol(tag_sym),
                        ])),
                        doc::softline(),
                        doc::text(">"),
                    ]))
                } else {
                    // Text-only: inner group allows closing > to break independently
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::group(doc::concat(vec![
                            doc::softline(),
                            doc::text(">"),
                            children_doc,
                            doc::text("</"),
                            doc::symbol(tag_sym),
                        ]))),
                        doc::softline(),
                        doc::text(">"),
                    ]))
                }
            }
        } else {
            // With attrs - layout depends on whether there are block flow children
            // Rebuild attr_docs since we're in a different branch
            let hug_attr_docs = self.build_element_attrs_doc(&element.attributes);
            // Expanding blocks (if/each/key) always force multiline
            // Note: await blocks do NOT force multiline - they stay inline in inline elements
            // But expanding blocks nested inside await blocks DO force multiline
            // Snippet blocks only force multiline when content is not inline
            let has_expanding_blocks =
                super::helpers::has_any_expanding_blocks(&element.fragment.nodes);
            let snippet_forces_break =
                ctx.has_block_flow_children && self.block_flow_forces_multiline(element);
            if has_expanding_blocks || snippet_forces_break {
                // Block flow forces multiline: > on new line after attrs
                // <span attr="val"
                //     >{#if ...}{/if}</span
                // >
                // Use nested group for opening tag to keep attrs flat
                doc::group(doc::concat(vec![
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::concat(hug_attr_docs)),
                    ])),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::symbol(tag_sym),
                    ])),
                    doc::hardline(),
                    doc::text(">"),
                ]))
            } else {
                // No block flow - check if we need hug mode for inline elements
                // Inline elements with long attrs use hug mode: attrs inline, > on new line
                let is_inline_elem = ctx.kind.is_inline() || ctx.kind.is_component();
                if is_inline_elem && ctx.is_empty {
                    // Use conditional_group for proper hug mode:
                    // 1. All inline: <tag attrs></tag>
                    // 2. Hug mode: <tag attrs\n></tag> (attrs inline, > on new line)
                    // 3. Full multiline: <tag\n\tattr\n></tag>
                    let closing =
                        doc::concat(vec![doc::text("></"), doc::symbol(tag_sym), doc::text(">")]);

                    // State 1: All inline
                    let inline_state = doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::concat(hug_attr_docs.clone())),
                        closing.clone(),
                    ]);

                    // State 2: Hug mode - attrs inline (space-separated), > on new line
                    let hug_space_attrs = self.build_element_attrs_doc_spaces(&element.attributes);
                    let hug_state = doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::concat(hug_space_attrs),
                        doc::hardline(),
                        closing.clone(),
                    ]);

                    // State 3: Full multiline - attrs on separate lines, > on new line
                    let multiline_state = doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::concat(hug_attr_docs)),
                        doc::hardline(),
                        closing,
                    ]);

                    doc::conditional_group(vec![inline_state, hug_state, multiline_state])
                } else if ctx.kind.is_component() {
                    // Components with hugging content: structure like Prettier
                    //
                    // group([
                    //   <Name
                    //   indent(group(attrs))
                    //   group(indent([softline, group([> content </Name])])),
                    //   softline,
                    //   >
                    // ])
                    //
                    // When attrs break, softline before > becomes newline, putting > on its own line.
                    // When attrs fit, everything stays inline.
                    let hugged_content = doc::group(doc::indent(doc::concat(vec![
                        doc::softline(),
                        doc::group(doc::concat(vec![
                            doc::text(">"),
                            children_doc,
                            doc::text("</"),
                            doc::symbol(tag_sym),
                        ])),
                    ])));
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::group(doc::concat(hug_attr_docs))),
                        hugged_content,
                        doc::softline(),
                        doc::text(">"),
                    ]))
                } else {
                    // HTML elements with content - use nested groups for breaking:
                    // - Outer group: controls whether content goes on new line
                    // - Inner group (around attrs): controls whether attrs break
                    //
                    // This produces 4 possible outputs:
                    // 1. Inline: <tag attrs>content</tag>
                    // 2. Content breaks: <tag attrs\n\t>content</tag\n>
                    // 3. Attrs break: <tag\n\tattrs>content</tag> (attrs break, body hugs)
                    // 4. Both break: <tag\n\tattrs\n\t>content</tag\n>
                    //
                    // Note: body doesn't include trailing > since it's outside for hug mode
                    let html_body = doc::concat(vec![
                        doc::text(">"),
                        children_doc,
                        doc::text("</"),
                        doc::symbol(tag_sym),
                    ]);
                    doc::group(doc::concat(vec![
                        doc::text("<"),
                        doc::symbol(tag_sym),
                        doc::indent(doc::group(doc::concat(hug_attr_docs))),
                        doc::group(doc::indent_softline(html_body)),
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
        let tag_sym = element.name.to_u32();
        // Empty element: <tag attrs></tag> with no content between > and </
        // For inline elements with attrs, use conditional_group for proper hug mode:
        // 1. All inline: <tag attrs></tag>
        // 2. Hug mode: <tag attrs\n></tag> (attrs inline, > on new line)
        // 3. Full multiline: <tag\n\tattr\n></tag> (attrs on separate lines)
        let is_inline = tsv_html::is_inline_element(tag_name) || is_component;
        if has_attrs && is_inline {
            // Build three alternative layouts for conditional_group
            let closing = doc::concat(vec![doc::text("></"), doc::symbol(tag_sym), doc::text(">")]);

            // State 1: All inline (current structure with line separators)
            let inline_state = doc::concat(vec![opening_tag, closing.clone()]);

            // State 2: Hug mode - attrs inline (space-separated), > on new line
            let hug_attrs = self.build_element_attrs_doc_spaces(&element.attributes);
            let hug_state = doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::concat(hug_attrs),
                doc::hardline(),
                closing.clone(),
            ]);

            // State 3: Full multiline - attrs on separate lines (line-separated), > on new line
            let multiline_attrs = self.build_element_attrs_doc(&element.attributes);
            let multiline_state = doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
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
                doc::symbol(tag_sym),
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
        let tag_sym = element.name.to_u32();
        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            doc::concat(vec![doc::text("<"), doc::symbol(tag_sym), doc::text(">")])
        } else {
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
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
                doc::symbol(tag_sym),
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
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ])
            }
            _ => {
                // Fallback: preserve raw content if parsing fails
                doc::concat(vec![
                    opening_tag,
                    doc::text_owned(content.to_string()),
                    doc::text("</"),
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ])
            }
        }
    }

    /// Build doc for whitespace-sensitive elements (pre, textarea, etc.)
    ///
    /// These elements preserve text whitespace exactly as-is, but still format
    /// expressions, blocks, and other dynamic content normally.
    ///
    /// Behavior differs by element type:
    /// - Block elements (pre): hug `>` with last attr when attrs wrap
    /// - Inline elements (textarea):
    ///   - Empty: break `>` to own line when attrs wrap
    ///   - With content: keep attrs inline, wrap `>content</tag` together
    pub(super) fn build_whitespace_sensitive_element_doc(
        &self,
        tag_name: &str,
        element: &internal::Element,
        attr_docs: Vec<Doc>,
    ) -> Doc {
        let tag_sym = element.name.to_u32();
        let is_inline = tsv_html::is_inline_element(tag_name);
        let has_content = !element.fragment.nodes.is_empty();

        // Inline whitespace-sensitive elements with content and attrs (textarea with content)
        // have special formatting that depends on whether attrs fit on one line:
        // - If fits: <tag attrs>content</tag>
        // - If breaks: <tag attrs\n\t>content</tag\n>
        //
        // This preserves no leading whitespace before content while allowing attrs to stay inline when short.
        if is_inline && has_content && !attr_docs.is_empty() {
            let content_doc = self.build_whitespace_sensitive_content_doc(&element.fragment.nodes);
            // Rebuild as space-separated (caller passes line-separated which we can't use here)
            let space_attrs = self.build_element_attrs_doc_spaces(&element.attributes);

            return doc::group(doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::concat(space_attrs),
                // In flat mode: >content</tag>
                // In break mode: \n\t>content</tag\n>
                doc::if_break(
                    doc::concat(vec![
                        doc::indent(doc::concat(vec![
                            doc::hardline(),
                            doc::text(">"),
                            content_doc.clone(),
                            doc::text("</"),
                            doc::symbol(tag_sym),
                        ])),
                        doc::hardline(),
                        doc::text(">"),
                    ]),
                    doc::concat(vec![
                        doc::text(">"),
                        content_doc,
                        doc::text("</"),
                        doc::symbol(tag_sym),
                        doc::text(">"),
                    ]),
                ),
            ]));
        }

        // Block whitespace-sensitive elements with content and attrs (pre with content)
        // Divergence: When attrs wrap and `>{content}</tag>` would exceed print width, break `>` to new line.
        // This respects print width while preserving whitespace semantics (no text node added).
        //
        // Only apply this logic for simple content. For complex content that can break internally
        // (like function calls), use normal flow so content breaks first.
        if !is_inline && has_content && !attr_docs.is_empty() {
            // Check if content is "simple" - single expression tag without internal break points
            // Complex content (function calls, ternaries, etc.) should break internally first
            let is_simple_content = element.fragment.nodes.len() == 1
                && matches!(
                    &element.fragment.nodes[0],
                    FragmentNode::ExpressionTag(expr) if !Self::expression_has_break_points(&expr.expression)
                );

            if is_simple_content {
                let content_doc =
                    self.build_whitespace_sensitive_content_doc(&element.fragment.nodes);

                // Inner group decides if `>` needs to break to new line
                let closing_and_content = doc::group(doc::concat(vec![
                    doc::softline(),
                    doc::text(">"),
                    content_doc,
                    doc::text("</"),
                    doc::symbol(tag_sym),
                    doc::text(">"),
                ]));

                // Outer group decides if attrs need to break
                return doc::group(doc::concat(vec![
                    doc::text("<"),
                    doc::symbol(tag_sym),
                    doc::indent(doc::concat(vec![
                        doc::concat(attr_docs),
                        doc::dedent(closing_and_content),
                    ])),
                ]));
            }
            // Fall through to normal handling for complex content
        }

        // Build opening tag
        let opening_tag = if attr_docs.is_empty() {
            doc::concat(vec![doc::text("<"), doc::symbol(tag_sym), doc::text(">")])
        } else if is_inline {
            // Inline whitespace-sensitive elements (empty textarea):
            // Break `>` to own line when attrs wrap (like regular inline elements)
            // Use softline() so it's empty in flat mode, newline in break mode
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::indent(doc::concat(attr_docs)),
                doc::softline(),
                doc::text(">"),
            ]))
        } else {
            // Block whitespace-sensitive elements (pre):
            // Hug `>` with last attr when attrs wrap
            doc::group(doc::concat(vec![
                doc::text("<"),
                doc::symbol(tag_sym),
                doc::indent(doc::concat(attr_docs)),
                doc::text(">"),
            ]))
        };

        // Build content preserving text whitespace but formatting expressions/blocks
        let content_doc = self.build_whitespace_sensitive_content_doc(&element.fragment.nodes);

        doc::concat(vec![
            opening_tag,
            content_doc,
            doc::text("</"),
            doc::symbol(tag_sym),
            doc::text(">"),
        ])
    }

    /// Build content for whitespace-sensitive elements (pre, textarea).
    ///
    /// Text nodes preserve their exact whitespace (significant for pre/textarea).
    /// Expressions, blocks, and other dynamic content are formatted normally
    /// (their internal whitespace is not significant).
    fn build_whitespace_sensitive_content_doc(&self, nodes: &[FragmentNode]) -> Doc {
        doc::concat(
            nodes
                .iter()
                .map(|node| self.build_whitespace_sensitive_node_doc(node))
                .collect(),
        )
    }

    /// Build doc for a single node in whitespace-sensitive context.
    ///
    /// Text nodes preserve raw whitespace.
    /// Nested elements also use whitespace-sensitive formatting (e.g., <code> inside <pre>).
    /// Expressions and blocks are formatted normally WITH indent wrapper, so they get
    /// double-indented (once for being inside <pre>, once for their internal structure).
    fn build_whitespace_sensitive_node_doc(&self, node: &FragmentNode) -> Doc {
        match node {
            // Text: preserve exact whitespace (significant in pre/textarea)
            FragmentNode::Text(text) => doc::text_owned(text.raw.clone()),

            // Elements: recursively build as whitespace-sensitive (no indent wrapper needed -
            // the element's own indentation logic handles it)
            // This handles cases like <pre><code> where <code> inherits whitespace preservation
            FragmentNode::Element(element) => {
                let tag_name = self.resolve_symbol(element.name);
                // Always use whitespace-sensitive path when nested inside whitespace-sensitive elements
                let attr_docs = self.build_element_attrs_doc(&element.attributes);
                self.build_whitespace_sensitive_element_doc(&tag_name, element, attr_docs)
            }
            FragmentNode::SpecialElement(element) => {
                // Special elements in whitespace-sensitive context: format normally without indent
                self.build_special_element_doc(element)
            }

            // Expressions and blocks: format normally WITH indent wrapper
            // This gives them proper indentation (e.g., expression args inside <pre> get
            // double-indented: once for <pre>, once for call structure)
            FragmentNode::ExpressionTag(tag) => doc::indent(self.build_expression_tag_doc(tag)),
            FragmentNode::Comment(comment) => doc::indent(self.build_html_comment_doc(comment)),
            FragmentNode::IfBlock(block) => doc::indent(self.build_if_block_doc(block)),
            FragmentNode::EachBlock(block) => doc::indent(self.build_each_block_doc(block)),
            FragmentNode::AwaitBlock(block) => doc::indent(self.build_await_block_doc(block)),
            FragmentNode::KeyBlock(block) => doc::indent(self.build_key_block_doc(block)),
            FragmentNode::SnippetBlock(block) => doc::indent(self.build_snippet_block_doc(block)),
            FragmentNode::HtmlTag(tag) => doc::indent(self.build_html_tag_doc(tag)),
            FragmentNode::ConstTag(tag) => doc::indent(self.build_const_tag_doc(tag)),
            FragmentNode::DebugTag(tag) => doc::indent(self.build_debug_tag_doc(tag)),
            FragmentNode::RenderTag(tag) => doc::indent(self.build_render_tag_doc(tag)),
        }
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

    /// Check if an expression has internal break points (ternary, &&, ||, +, etc.)
    ///
    /// When true, the expression can break internally before the containing element
    /// needs to break its tags. This enables the "hug mode" divergence where we keep
    /// `<tag>` together and let expressions break, reducing indentation drift.
    fn expression_has_break_points(expr: &tsv_ts::ast::internal::Expression) -> bool {
        use tsv_ts::ast::internal::Expression;
        match expr {
            // Ternary always has break points
            Expression::ConditionalExpression(_) => true,
            // Binary expressions (includes &&, ||, +, -, etc.) have break points
            Expression::BinaryExpression(_) => true,
            // Sequence expressions (comma-separated) have break points
            Expression::SequenceExpression(_) => true,
            // Call expressions with multiple arguments can break
            Expression::CallExpression(call) => call.arguments.len() > 1,
            // New expressions with multiple arguments can break
            Expression::NewExpression(new) => new.arguments.len() > 1,
            // Template literals with expressions can break
            Expression::TemplateLiteral(tpl) => !tpl.expressions.is_empty(),
            // Array/object literals with multiple elements can break
            Expression::ArrayExpression(arr) => arr.elements.len() > 1,
            Expression::ObjectExpression(obj) => obj.properties.len() > 1,
            // Assignment expressions have break points
            Expression::AssignmentExpression(_) => true,
            // Wrapping expressions: check inner
            Expression::TSAsExpression(e) => Self::expression_has_break_points(&e.expression),
            Expression::TSSatisfiesExpression(e) => {
                Self::expression_has_break_points(&e.expression)
            }
            Expression::TSNonNullExpression(e) => Self::expression_has_break_points(&e.expression),
            Expression::TSTypeAssertion(e) => Self::expression_has_break_points(&e.expression),
            Expression::AwaitExpression(e) => Self::expression_has_break_points(&e.argument),
            Expression::YieldExpression(e) => e
                .argument
                .as_ref()
                .is_some_and(|a| Self::expression_has_break_points(a)),
            // Simple expressions without break points
            Expression::Literal(_)
            | Expression::Identifier(_)
            | Expression::MemberExpression(_)
            | Expression::PrivateIdentifier(_)
            | Expression::UnaryExpression(_)
            | Expression::UpdateExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ClassExpression(_)
            | Expression::SpreadElement(_)
            | Expression::TaggedTemplateExpression(_)
            | Expression::RegexLiteral(_)
            | Expression::Super(_)
            | Expression::ObjectPattern(_)
            | Expression::ArrayPattern(_)
            | Expression::AssignmentPattern(_)
            | Expression::RestElement(_)
            | Expression::TSInstantiationExpression(_)
            | Expression::TSParameterProperty(_)
            | Expression::ImportExpression(_)
            | Expression::MetaProperty(_) => false,
        }
    }

    /// Check if a fragment node is an HTML block element (not component, not control flow)
    ///
    /// Used to detect when parent elements need multiline formatting due to
    /// block-level children. Components and control flow blocks don't trigger
    /// this - only actual HTML block elements like `<div>`, `<p>`, etc.
    fn is_block_element_child(&self, node: &FragmentNode) -> bool {
        match node {
            FragmentNode::Element(el) => {
                // Only HTML block elements, not components
                if el.kind == internal::ElementKind::Component {
                    return false;
                }
                let tag = self.resolve_symbol(el.name);
                tsv_html::is_block_element(&tag)
            }
            // svelte:* elements and control flow don't trigger multiline
            _ => false,
        }
    }

    /// Check if element content has source breaks (newlines) that should trigger multiline.
    ///
    /// The logic differs by element type:
    /// - **Blocks**: Leading boundary break triggers multiline (preserves `<p>\ntext\n</p>`)
    /// - **Components**: Require BOTH leading AND trailing break (expressions hug when only leading)
    /// - **Inline**: Exclude boundary whitespace newlines (they normalize to spaces)
    fn has_source_breaks_in_content(
        &self,
        nodes: &[FragmentNode],
        kind: ElementKind,
        source_has_leading_break: bool,
        source_has_trailing_break: bool,
    ) -> bool {
        // Blocks: leading break alone triggers multiline
        // Components: require both boundaries
        if (kind.is_block() && source_has_leading_break)
            || (kind.is_component() && source_has_leading_break && source_has_trailing_break)
        {
            return true;
        }

        // Find first and last non-whitespace content indices
        let first_content_idx = nodes.iter().position(|n| !n.is_whitespace_only_text());
        let last_content_idx = nodes.iter().rposition(|n| !n.is_whitespace_only_text());

        let (Some(first), Some(last)) = (first_content_idx, last_content_idx) else {
            return false;
        };

        // Inline elements: preserve multiline when content starts with newline and has non-text.
        // `<span>\n\t{expr}</span>` preserves, `<span>  \n  {expr}</span>` collapses (space before \n).
        // Fill mode (`{a} {b}`) stays inline even with leading newline.
        let first_text_starts_with_newline = nodes
            .first()
            .is_some_and(|n| matches!(n, FragmentNode::Text(t) if t.raw.starts_with('\n')));

        if first_text_starts_with_newline {
            let has_nontext_content = nodes[first..=last]
                .iter()
                .any(|n| !matches!(n, FragmentNode::Text(_)));

            // Check if content is in fill mode: expressions separated by space-only text
            let is_fill_mode = nodes[first..=last].windows(2).any(|w| {
                !matches!(w[0], FragmentNode::Text(_))
                    && matches!(&w[1], FragmentNode::Text(t) if !t.raw.is_empty() && t.raw.bytes().all(|b| b == b' '))
            });

            if has_nontext_content && !is_fill_mode {
                return true;
            }
        }

        if first >= last {
            return false;
        }

        // Check for newlines in content between first and last non-whitespace nodes
        nodes[first..=last].iter().enumerate().any(|(i, n)| {
            let FragmentNode::Text(t) = n else {
                return false;
            };

            if kind.preserves_boundary_breaks() {
                // Block/component: any newline triggers source break
                t.raw.contains('\n')
            } else if t.raw.trim().is_empty() {
                // Inline, whitespace-only: newlines are separators
                t.raw.contains('\n')
            } else {
                // Inline, text with content: exclude boundary whitespace
                let is_first_content = i == 0;
                let is_last_content = i == last - first;
                let check_str = match (is_first_content, is_last_content) {
                    (true, true) => t.raw.trim(),
                    (true, false) => t.raw.trim_start(),
                    (false, true) => t.raw.trim_end(),
                    (false, false) => &t.raw,
                };
                check_str.contains('\n')
            }
        })
    }

    /// Analyze an element to compute all formatting-relevant properties
    fn analyze_element(&self, element: &internal::Element) -> ElementContext {
        let tag_name = self.resolve_symbol(element.name);
        let is_void = tsv_html::is_void_element(&tag_name);
        let is_foreign = tsv_html::is_foreign_element(&tag_name);

        // Determine element kind
        // Matches prettier-plugin-svelte: isInlineElement = !isBlockElement
        // Elements NOT in the block list (including table cells) use inline formatting.
        let kind = if tag_name.starts_with(|c: char| c.is_ascii_uppercase())
            || tag_name.contains(':')
            || tag_name.contains('.')
        {
            ElementKind::Component
        } else if tsv_html::is_block_element(&tag_name) {
            ElementKind::Block
        } else {
            ElementKind::Inline
        };

        // Check if self-closing
        let is_self_closing = (kind.is_component() || is_foreign)
            && element.fragment.nodes.is_empty()
            && self.was_self_closing_doc(element);

        // Check if empty
        let is_empty = element.fragment.nodes.is_empty()
            || element
                .fragment
                .nodes
                .iter()
                .all(FragmentNode::is_whitespace_only_text);

        // Source boundary breaks
        let source_has_leading_break = element
            .fragment
            .nodes
            .first()
            .is_some_and(FragmentNode::is_boundary_break);
        let source_has_trailing_break = source_has_leading_break
            && element
                .fragment
                .nodes
                .last()
                .is_some_and(FragmentNode::is_boundary_break);

        // Hug modes
        let hug_start = self.should_hug_start(element, kind.is_block());
        let hug_end = self.should_hug_end(element, kind.is_block());

        // Block flow children
        let has_block_flow_children = element
            .fragment
            .nodes
            .iter()
            .any(super::helpers::is_control_flow_block);

        // Opening tag multiline: any newline in attribute source (for trim_boundaries)
        let has_multiline_attr = element
            .attributes
            .iter()
            .any(|a| a.span().extract(self.source).contains('\n'));

        // Template literals with embedded newlines (for forcing attribute break)
        let has_multiline_template_attr = element
            .attributes
            .iter()
            .any(|a| has_multiline_template_literal(a.span().extract(self.source)));

        // Compute needs_multiline
        let needs_multiline = self.compute_needs_multiline(
            element,
            kind,
            is_empty,
            hug_end,
            source_has_leading_break,
            source_has_trailing_break,
            has_block_flow_children,
        );

        // Compute trim_boundaries
        let will_go_multiline = element.attributes.len() > 1
            || (has_block_flow_children && self.block_flow_forces_multiline(element))
            || super::helpers::has_nested_block_flow(&element.fragment.nodes)
            || has_multiline_attr;
        let trim_boundaries = !kind.is_inline() || will_go_multiline;

        ElementContext {
            tag_name,
            kind,
            is_void,
            is_self_closing,
            is_empty,
            source_has_leading_break,
            source_has_trailing_break,
            hug_start,
            hug_end,
            needs_multiline,
            has_block_flow_children,
            trim_boundaries,
            has_multiline_template_attr,
        }
    }

    /// Compute whether children need multiline formatting
    #[allow(clippy::too_many_arguments)]
    fn compute_needs_multiline(
        &self,
        element: &internal::Element,
        kind: ElementKind,
        is_empty: bool,
        hug_end: bool,
        source_has_leading_break: bool,
        source_has_trailing_break: bool,
        has_block_flow_children: bool,
    ) -> bool {
        if is_empty {
            return false;
        }

        // Multiple block children
        let block_child_count = element
            .fragment
            .nodes
            .iter()
            .filter(|n| self.is_block_element_child(n))
            .count();
        if block_child_count > 1 {
            return true;
        }

        // Mixed content (block + non-block children)
        let has_block_children = block_child_count > 0;
        if has_block_children {
            let has_non_block = element.fragment.nodes.iter().any(|n| match n {
                FragmentNode::Text(t) => !t.raw.is_whitespace_only(),
                FragmentNode::Element(e) => !self.is_block_element(e),
                FragmentNode::ExpressionTag(_) => true,
                FragmentNode::HtmlTag(_)
                | FragmentNode::ConstTag(_)
                | FragmentNode::DebugTag(_)
                | FragmentNode::RenderTag(_) => true,
                _ => !super::helpers::is_control_flow_block(n),
            });
            if has_non_block {
                return true;
            }
        }

        // Source breaks in content
        if self.has_source_breaks_in_content(
            &element.fragment.nodes,
            kind,
            source_has_leading_break,
            source_has_trailing_break,
        ) {
            return true;
        }

        // Expression splitting
        let should_split = self.should_split_expressions_in_nodes(&element.fragment.nodes);
        let has_trailing_ws = !hug_end;
        if source_has_leading_break && has_trailing_ws && should_split {
            return true;
        }

        // Block elements with expanding blocks (if/each/key, or those inside await) always expand
        // Note: await blocks alone do NOT force expansion in block elements
        if kind.is_block() && super::helpers::has_any_expanding_blocks(&element.fragment.nodes) {
            return true;
        }

        // Block flow forces multiline
        if has_block_flow_children && self.block_flow_forces_multiline(element) {
            return true;
        }

        // Text with internal newlines
        if self.text_has_internal_newlines(element, source_has_leading_break) {
            return true;
        }

        false
    }

    /// Check if block flow children force parent to multiline
    fn block_flow_forces_multiline(&self, element: &internal::Element) -> bool {
        // Check if any block has non-inline content
        let has_non_inline_block = element.fragment.nodes.iter().any(|n| match n {
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
        });

        // Check if there's whitespace around EXPANDING block flow children (if/each/key)
        // Await and snippet blocks don't force multiline when surrounded by whitespace
        let has_expanding_blocks = element
            .fragment
            .nodes
            .iter()
            .any(super::helpers::is_expanding_control_flow_block);
        let has_ws_around_blocks = has_expanding_blocks
            && element.fragment.nodes.iter().any(|n| {
                matches!(n, FragmentNode::Text(t) if t.raw.is_whitespace_only() && !t.raw.is_empty())
            });

        has_non_inline_block || has_ws_around_blocks
    }

    /// Check if text content has internal newlines
    fn text_has_internal_newlines(
        &self,
        element: &internal::Element,
        source_has_leading_break: bool,
    ) -> bool {
        let has_leading_content_break = element.fragment.nodes.first().is_some_and(|n| {
            matches!(n, FragmentNode::Text(t) if t.raw.starts_with('\n') && !t.raw.is_whitespace_only())
        });

        (source_has_leading_break || has_leading_content_break)
            && element
                .fragment
                .nodes
                .iter()
                .any(|n| matches!(n, FragmentNode::Text(t) if t.raw.trim().contains('\n')))
    }

    /// Compute element layout from analyzed context
    fn compute_element_layout(&self, ctx: &ElementContext) -> ElementLayout {
        if ctx.is_void || ctx.is_self_closing {
            return if ctx.is_void {
                ElementLayout::Void
            } else {
                ElementLayout::SelfClosing
            };
        }

        if ctx.is_empty {
            return ElementLayout::Empty;
        }

        // Determine boundary modes
        let start_mode = if ctx.hug_start {
            BoundaryMode::Hug
        } else if ctx.needs_multiline
            || (ctx.kind.preserves_boundary_breaks() && ctx.source_has_leading_break)
        {
            BoundaryMode::Hard
        } else {
            BoundaryMode::Soft
        };

        let end_mode = if ctx.hug_end {
            BoundaryMode::Hug
        } else if ctx.needs_multiline
            || (ctx.kind.preserves_boundary_breaks() && ctx.source_has_trailing_break)
        {
            BoundaryMode::Hard
        } else {
            BoundaryMode::Soft
        };

        ElementLayout::WithContent {
            start: start_mode,
            end: end_mode,
            multiline_children: ctx.needs_multiline,
        }
    }
}
