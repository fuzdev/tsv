// Element analysis and layout classification for doc building
//
// The analyze/classify half of element doc building: predicates that inspect an
// element's children and source boundaries to decide multiline-ness, boundary
// modes, and the overall layout. The shared types (`BoundaryMode`,
// `ElementLayout`, `ElementKind`, `ElementContext`) live in `element_doc.rs`
// alongside the build half that also consumes them.

use crate::ast::internal::{self, FragmentNode};
use crate::printer::Printer;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::arena::DocId;
use tsv_ts::ast::internal::Expression;

use super::element_doc::{BoundaryMode, ElementContext, ElementKind, ElementLayout};

/// Inputs to the [`Printer::compute_needs_multiline`] decision.
///
/// Bundles the per-element flags the predicate reads so they pass by name
/// rather than as positional bools that are easy to misorder at the call site.
/// Mirrors the corresponding [`ElementContext`] fields — both are built from
/// the same locals.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct MultilineInputs {
    /// Element type classification
    kind: ElementKind,
    /// Whether element has no meaningful content
    is_empty: bool,
    /// Whether content should hug the closing tag
    hug_end: bool,
    /// Whether source has newline at opening boundary
    source_has_leading_break: bool,
    /// Whether source has newline at closing boundary
    source_has_trailing_break: bool,
    /// Whether block-flow children force this element multiline (cached, mirrors
    /// [`ElementContext::block_flow_multiline`])
    block_flow_multiline: bool,
    /// Whether all content children are text nodes
    only_text_content: bool,
}

impl<'a> Printer<'a> {
    /// Check if an expression has internal break points (ternary, &&, ||, +, etc.)
    ///
    /// When true, the expression can break internally before the containing element
    /// needs to break its tags. This enables the "hug mode" divergence where we keep
    /// `<tag>` together and let expressions break, reducing indentation drift.
    pub(super) fn expression_has_break_points(expr: &Expression<'_>) -> bool {
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
            Expression::JsdocCast(cast) => Self::expression_has_break_points(cast.inner),
            Expression::ParenthesizedExpression(paren) => {
                Self::expression_has_break_points(paren.expression)
            }
            Expression::TSAsExpression(e) => Self::expression_has_break_points(e.expression),
            Expression::TSSatisfiesExpression(e) => Self::expression_has_break_points(e.expression),
            Expression::TSNonNullExpression(e) => Self::expression_has_break_points(e.expression),
            Expression::TSTypeAssertion(e) => Self::expression_has_break_points(e.expression),
            Expression::AwaitExpression(e) => Self::expression_has_break_points(e.argument),
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
            | Expression::ThisExpression(_)
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

    /// Whether any direct child expression tag (`{expr}`) can break internally
    /// (ternary, binary, call, …). Mirrors the hug-both wrapper's hard-width
    /// divergence: when true, the children builder must keep those expression
    /// groups breakable, so boundary text adjacent to them is emitted as plain
    /// spaces rather than `fill` `line`s — otherwise a `line` in fits()-Break
    /// mode short-circuits the preceding expression group's width check, leaving
    /// it flat and overshooting printWidth (the `fill_multiple_expr_long` case).
    pub(super) fn nodes_have_breakable_expression(nodes: &[FragmentNode<'_>]) -> bool {
        nodes.iter().any(|n| {
            if let FragmentNode::ExpressionTag(tag) = n {
                Self::expression_has_break_points(&tag.expression)
            } else {
                false
            }
        })
    }

    /// Check if a fragment node is an HTML block element (not component, not control flow)
    ///
    /// Used to detect when parent elements need multiline formatting due to
    /// block-level children. Components and control flow blocks don't trigger
    /// this - only actual HTML block elements like `<div>`, `<p>`, etc.
    fn is_block_element_child(&self, node: &FragmentNode<'_>) -> bool {
        match node {
            // Defer to the one block-element adapter (component + script/style overlay).
            FragmentNode::Element(el) => self.is_block_element(el),
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
        nodes: &[FragmentNode<'_>],
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

        let source = self.source;

        // Find first and last non-whitespace content indices
        let first_content_idx = nodes.iter().position(|n| !n.is_whitespace_only_text());
        let last_content_idx = nodes.iter().rposition(|n| !n.is_whitespace_only_text());

        let (Some(first), Some(last)) = (first_content_idx, last_content_idx) else {
            return false;
        };

        // Inline elements: preserve multiline when content starts with newline AND ends
        // with any whitespace (space, tab, or newline), and has non-text children.
        // `<a>\n\t{expr}\n</a>` preserves (leading newline + trailing newline).
        // `<a>\n\t{expr} </a>` preserves (leading newline + trailing space).
        // `<a>\n\t{expr}</a>` collapses (leading newline but no trailing whitespace).
        // `<a>\n  text<span>text</span></a>` collapses (no trailing whitespace).
        // `<span>  \n  {expr}</span>` collapses (space before \n, not leading).
        // Fill mode (`{a} {b}`) stays inline even with both breaks.
        let first_text_starts_with_newline = nodes
            .first()
            .is_some_and(|n| matches!(n, FragmentNode::Text(t) if t.raw(source).starts_with('\n')));
        let last_text_ends_with_whitespace = nodes.last().is_some_and(
            |n| matches!(n, FragmentNode::Text(t) if t.raw(source).ends_with(|c: char| c.is_ascii_whitespace())),
        );

        if first_text_starts_with_newline && last_text_ends_with_whitespace {
            let has_nontext_content = nodes[first..=last]
                .iter()
                .any(|n| !matches!(n, FragmentNode::Text(_)));

            // Check if content is in fill mode: expressions separated by space-only text
            let is_fill_mode = nodes[first..=last].windows(2).any(|w| {
                !matches!(w[0], FragmentNode::Text(_))
                    && matches!(&w[1], FragmentNode::Text(t) if { let r = t.raw(source); !r.is_empty() && r.bytes().all(|b| b == b' ') })
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

            let raw = t.raw(source);
            if kind.preserves_boundary_breaks() {
                // Block/component: any newline triggers source break
                t.has_newline()
            } else if t.is_ascii_ws_only {
                // Inline, whitespace-only: newlines are separators
                t.has_newline()
            } else {
                // Inline, text with content: exclude boundary (ASCII) whitespace.
                // A non-breaking space is content, so trim_ascii keeps it attached.
                let is_first_content = i == 0;
                let is_last_content = i == last - first;
                let check_str = match (is_first_content, is_last_content) {
                    (true, true) => raw.trim_ascii(),
                    (true, false) => raw.trim_ascii_start(),
                    (false, true) => raw.trim_ascii_end(),
                    (false, false) => raw,
                };
                check_str.contains('\n')
            }
        })
    }

    /// Analyze an element to compute all formatting-relevant properties
    pub(super) fn analyze_element(
        &self,
        element: &internal::Element<'_>,
        attr_docs: &[DocId],
    ) -> ElementContext {
        // Resolve the tag once inside the borrow and derive everything that needs
        // it there — avoids allocating a `String` per element on the hot path.
        let (is_void, is_foreign, kind) = self.with_resolved_symbol(element.name, |tag_name| {
            let is_void = tsv_html::is_void_element(tag_name);
            let is_foreign = tsv_html::is_foreign_element(tag_name);

            // Determine element kind
            // Matches prettier-plugin-svelte: isInlineElement = !isBlockElement
            // Elements NOT in the block list (including table cells) use inline formatting.
            let kind = if tag_name.starts_with(|c: char| c.is_ascii_uppercase())
                || tag_name.contains(':')
                || tag_name.contains('.')
            {
                ElementKind::Component
            } else if tsv_html::is_block_element(tag_name) {
                ElementKind::Block
            } else {
                ElementKind::Inline
            };

            (is_void, is_foreign, kind)
        });

        // Check if self-closing
        let is_self_closing = (kind.is_component() || is_foreign)
            && element.fragment.nodes.is_empty()
            && self.span_was_self_closing(element.span);

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

        // Block flow children → whether they force multiline. Computed once here (a non-trivial
        // traversal) and cached, since `will_go_multiline`, `compute_needs_multiline`, and the
        // hug-both `force` all read exactly this combination.
        let has_block_flow_children = element
            .fragment
            .nodes
            .iter()
            .any(super::helpers::is_control_flow_block);
        let block_flow_multiline =
            has_block_flow_children && self.block_flow_forces_multiline(element);

        // Any attribute doc that will_break (forces attr group break + trim_boundaries)
        let has_multiline_attr = attr_docs.iter().any(|&doc| self.d().will_break(doc));

        // Check if all content children are text nodes (no elements, expressions, blocks)
        let only_text_content = !is_empty
            && element
                .fragment
                .nodes
                .iter()
                .all(|n| matches!(n, FragmentNode::Text(_)));

        // Compute needs_multiline
        let needs_multiline = self.compute_needs_multiline(
            element,
            MultilineInputs {
                kind,
                is_empty,
                hug_end,
                source_has_leading_break,
                source_has_trailing_break,
                block_flow_multiline,
                only_text_content,
            },
        );

        // Compute trim_boundaries
        let will_go_multiline = element.attributes.len() > 1
            || block_flow_multiline
            || super::helpers::has_nested_block_flow(element.fragment.nodes)
            || has_multiline_attr;
        let trim_boundaries = !kind.is_inline() || will_go_multiline;

        ElementContext {
            kind,
            is_void,
            is_self_closing,
            is_empty,
            source_has_leading_break,
            source_has_trailing_break,
            hug_start,
            hug_end,
            needs_multiline,
            block_flow_multiline,
            trim_boundaries,
            has_multiline_attr,
            only_text_content,
        }
    }

    /// Compute whether children need multiline formatting
    fn compute_needs_multiline(
        &self,
        element: &internal::Element<'_>,
        inputs: MultilineInputs,
    ) -> bool {
        let MultilineInputs {
            kind,
            is_empty,
            hug_end,
            source_has_leading_break,
            source_has_trailing_break,
            block_flow_multiline,
            only_text_content,
        } = inputs;

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
                FragmentNode::Text(t) => !t.is_ascii_ws_only,
                FragmentNode::Element(e) => !self.is_block_element(e),
                FragmentNode::ExpressionTag(_) => true,
                FragmentNode::HtmlTag(_)
                | FragmentNode::ConstTag(_)
                | FragmentNode::DeclarationTag(_)
                | FragmentNode::DebugTag(_)
                | FragmentNode::RenderTag(_) => true,
                _ => !super::helpers::is_control_flow_block(n),
            });
            if has_non_block {
                return true;
            }
        }

        // Source breaks in content
        // Skip for block elements with text-only content — whitespace newlines between
        // text words collapse to spaces, so the group mechanism should decide layout
        // based on whether the joined text fits inline.
        if !only_text_content
            && self.has_source_breaks_in_content(
                element.fragment.nodes,
                kind,
                source_has_leading_break,
                source_has_trailing_break,
            )
        {
            return true;
        }

        // Expression splitting forces an element multiline when authored with a leading break,
        // a non-hugged trailing boundary, and 2+ spaced `{expr}` siblings — a multiline-*entry*
        // trigger (distinct from sibling separation, which `build_nodes_doc_multiline` handles).
        // Load-bearing for the component case (`components/multi_expressions_multiline`): without
        // it such a `<Comp>` would stay inline. The only remaining use of
        // `should_split_expressions_in_nodes` now that the sibling-break caller is retired.
        let should_split = self.should_split_expressions_in_nodes(element.fragment.nodes);
        let has_trailing_ws = !hug_end;
        if source_has_leading_break && has_trailing_ws && should_split {
            return true;
        }

        // Elements with expanding blocks (if/each/key, or those inside await) always expand to
        // block-style multiline — inline elements too, not just block. The expanding block forces
        // block-style layout in `build_hug_both_doc` regardless; matching `needs_multiline` here so
        // the children are *built* multiline (one node per line) keeps the expanding block from
        // overshooting printWidth when authored compactly (it would otherwise flow inline).
        // Note: await blocks alone do NOT force expansion.
        if super::helpers::has_any_expanding_blocks(element.fragment.nodes) {
            return true;
        }

        // await/snippet (which don't force-expand on their own) still go multiline when they
        // follow a sibling, so their body-drop matches if/each (via the multiline path) and
        // the sibling-`>` dangle / block-on-own-line separation resolves in one pass.
        if kind.is_block() && super::helpers::has_control_flow_after_sibling(element.fragment.nodes)
        {
            return true;
        }

        // Block flow forces multiline
        if block_flow_multiline {
            return true;
        }

        // Text with internal newlines
        // Skip for text-only content — newlines between words are just whitespace
        if !only_text_content && self.text_has_internal_newlines(element, source_has_leading_break)
        {
            return true;
        }

        false
    }

    /// Check if block flow children force parent to multiline
    fn block_flow_forces_multiline(&self, element: &internal::Element<'_>) -> bool {
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
        let source = self.source;
        let has_ws_around_blocks = has_expanding_blocks
            && element.fragment.nodes.iter().any(|n| {
                matches!(n, FragmentNode::Text(t) if t.is_ascii_ws_only && !t.raw(source).is_empty())
            });

        has_non_inline_block || has_ws_around_blocks
    }

    /// Check if text content has internal newlines
    fn text_has_internal_newlines(
        &self,
        element: &internal::Element<'_>,
        source_has_leading_break: bool,
    ) -> bool {
        let source = self.source;
        let has_leading_content_break = element.fragment.nodes.first().is_some_and(|n| {
            matches!(n, FragmentNode::Text(t) if { let r = t.raw(source); r.starts_with('\n') && !t.is_ascii_ws_only })
        });

        (source_has_leading_break || has_leading_content_break)
            && element.fragment.nodes.iter().any(
                |n| matches!(n, FragmentNode::Text(t) if t.raw(source).trim_ascii().contains('\n')),
            )
    }

    /// Compute element layout from analyzed context
    pub(super) fn compute_element_layout(&self, ctx: &ElementContext) -> ElementLayout {
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
        // Text-only block content uses soft boundaries so the group can collapse to
        // inline when content fits (e.g., `<p>text1 text2</p>` instead of multiline).
        let preserve_breaks = ctx.kind.preserves_boundary_breaks() && !ctx.only_text_content;
        let start_mode = if ctx.hug_start {
            BoundaryMode::Hug
        } else if ctx.needs_multiline || (preserve_breaks && ctx.source_has_leading_break) {
            BoundaryMode::Hard
        } else {
            BoundaryMode::Soft
        };

        let end_mode = if ctx.hug_end {
            BoundaryMode::Hug
        } else if ctx.needs_multiline || (preserve_breaks && ctx.source_has_trailing_break) {
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
