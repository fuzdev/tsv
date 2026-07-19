// Element analysis and layout classification for doc building
//
// The analyze/classify half of element doc building: predicates that inspect an
// element's children and source boundaries to decide multiline-ness, boundary
// modes, and the overall layout. The shared types (`BoundaryMode`,
// `ElementLayout`, `ElementKind`, `ElementContext`) live in `element_doc.rs`
// alongside the build half that also consumes them.

use crate::ast::internal::FragmentNode;
use crate::printer::Printer;
use tsv_lang::doc::arena::DocId;
use tsv_ts::ast::internal::Expression;

use super::element_doc::{BoundaryMode, ElementContext, ElementKind, ElementLayout, ElementParts};

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

        // Inline elements: preserve multiline only when BOTH boundaries are newline-authored
        // (both-or-neither, same as components) and there are non-text children.
        // `<a>\n\t{expr}\n</a>` preserves (leading newline + trailing newline).
        // `<a>\n\t{expr} </a>` collapses (trailing space is render-free — not a second break).
        // `<a>\n\t{expr}</a>` collapses (leading newline but no trailing break).
        // `<a>\n  text<span>text</span></a>` collapses (no trailing break).
        // `<span>  \n  {expr}</span>` collapses (space before \n, not leading).
        // Fill mode (`{a} {b}`) stays inline even with both breaks.
        let first_text_starts_with_newline = nodes
            .first()
            .is_some_and(|n| matches!(n, FragmentNode::Text(t) if t.raw(source).starts_with('\n')));

        if first_text_starts_with_newline && self.nodes_boundary_newline(nodes, false) {
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

    /// Analyze an element to compute all formatting-relevant properties.
    ///
    /// Shared by regular and `svelte:*` elements — both project onto [`ElementParts`].
    pub(super) fn analyze_element(
        &self,
        parts: &ElementParts<'_>,
        attr_docs: &[DocId],
    ) -> ElementContext {
        let ElementParts {
            kind,
            can_self_close,
            nodes,
            span,
            ..
        } = *parts;

        // Check if self-closing
        let is_self_closing =
            can_self_close && nodes.is_empty() && self.span_was_self_closing(span);

        // Check if empty
        let is_empty = nodes.is_empty() || nodes.iter().all(FragmentNode::is_whitespace_only_text);

        // Source boundary breaks
        let source_has_leading_break = nodes.first().is_some_and(FragmentNode::is_boundary_break);
        let source_has_trailing_break =
            source_has_leading_break && nodes.last().is_some_and(FragmentNode::is_boundary_break);

        // Hug modes
        let hug_start = self.should_hug_start(nodes, kind.is_block());
        let hug_end = self.should_hug_end(nodes, kind.is_block());

        // Block flow children → whether they force multiline. Computed once here (a non-trivial
        // traversal) and cached, since `will_go_multiline`, `compute_needs_multiline`, and the
        // hug-both `force` all read exactly this combination.
        let has_block_flow_children = nodes.iter().any(super::helpers::is_control_flow_block);
        let block_flow_multiline =
            has_block_flow_children && self.block_flow_forces_multiline(nodes);

        // Any attribute doc that will_break (forces attr group break)
        let has_multiline_attr = attr_docs.iter().any(|&doc| self.d().will_break(doc));

        // Check if all content children are text nodes (no elements, expressions, blocks)
        let only_text_content =
            !is_empty && nodes.iter().all(|n| matches!(n, FragmentNode::Text(_)));

        // Compute needs_multiline
        let needs_multiline = self.compute_needs_multiline(
            nodes,
            MultilineInputs {
                kind,
                is_empty,
                source_has_leading_break,
                source_has_trailing_break,
                block_flow_multiline,
                only_text_content,
            },
        );

        ElementContext {
            is_self_closing,
            is_empty,
            hug_both: hug_start && hug_end,
            needs_multiline,
            has_multiline_attr,
        }
    }

    /// Compute whether children need multiline formatting
    fn compute_needs_multiline(&self, nodes: &[FragmentNode<'_>], inputs: MultilineInputs) -> bool {
        let MultilineInputs {
            kind,
            is_empty,
            source_has_leading_break,
            source_has_trailing_break,
            block_flow_multiline,
            only_text_content,
        } = inputs;

        if is_empty {
            return false;
        }

        // Multiple block children
        let block_child_count = nodes
            .iter()
            .filter(|n| self.is_block_element_child(n))
            .count();
        if block_child_count > 1 {
            return true;
        }

        // Mixed content (block + non-block children)
        let has_block_children = block_child_count > 0;
        if has_block_children {
            let has_non_block = nodes.iter().any(|n| match n {
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
                nodes,
                kind,
                source_has_leading_break,
                source_has_trailing_break,
            )
        {
            return true;
        }

        // Elements with expanding blocks (if/each/key, or those inside await) always expand to
        // block-style multiline — inline elements too, not just block. The expanding block forces
        // block-style layout in `build_hug_both_doc` regardless; matching `needs_multiline` here so
        // the children are *built* multiline (one node per line) keeps the expanding block from
        // overshooting printWidth when authored compactly (it would otherwise flow inline).
        // Note: await blocks alone do NOT force expansion.
        if super::helpers::has_any_expanding_blocks(nodes) {
            return true;
        }

        // await/snippet (which don't force-expand on their own) still go multiline when they
        // follow a sibling, so their body-drop matches if/each (via the multiline path) and
        // the sibling-`>` dangle / block-on-own-line separation resolves in one pass.
        if kind.is_block() && super::helpers::has_control_flow_after_sibling(nodes) {
            return true;
        }

        // Block flow forces multiline
        if block_flow_multiline {
            return true;
        }

        // Text with internal newlines
        // Skip for text-only content — newlines between words are just whitespace
        if !only_text_content && self.text_has_internal_newlines(nodes, source_has_leading_break) {
            return true;
        }

        false
    }

    /// Check if block flow children force parent to multiline
    fn block_flow_forces_multiline(&self, nodes: &[FragmentNode<'_>]) -> bool {
        // Check if any block has non-inline content
        let has_non_inline_block = nodes.iter().any(|n| match n {
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
        let has_expanding_blocks = nodes
            .iter()
            .any(super::helpers::is_expanding_control_flow_block);
        let source = self.source;
        let has_ws_around_blocks = has_expanding_blocks
            && nodes.iter().any(|n| {
                matches!(n, FragmentNode::Text(t) if t.is_ascii_ws_only && !t.raw(source).is_empty())
            });

        has_non_inline_block || has_ws_around_blocks
    }

    /// Check if text content has internal newlines
    fn text_has_internal_newlines(
        &self,
        nodes: &[FragmentNode<'_>],
        source_has_leading_break: bool,
    ) -> bool {
        let source = self.source;
        let has_leading_content_break = nodes.first().is_some_and(|n| {
            matches!(n, FragmentNode::Text(t) if { let r = t.raw(source); r.starts_with('\n') && !t.is_ascii_ws_only })
        });

        (source_has_leading_break || has_leading_content_break)
            && nodes.iter().any(
                |n| matches!(n, FragmentNode::Text(t) if t.raw(source).trim_ascii().contains('\n')),
            )
    }

    /// Compute element layout from analyzed context
    pub(super) fn compute_element_layout(
        &self,
        parts: &ElementParts<'_>,
        ctx: &ElementContext,
    ) -> ElementLayout {
        if parts.is_void || ctx.is_self_closing {
            return if parts.is_void {
                ElementLayout::Void
            } else {
                ElementLayout::SelfClosing
            };
        }

        if ctx.is_empty {
            return ElementLayout::Empty;
        }

        // Determine boundary modes.
        //
        // Content that goes multiline lays out block-style — both tags intact, content on its own
        // indented lines — never with a dangled delimiter. Content-boundary whitespace is
        // render-free under Svelte 5 (start/end-of-tag whitespace is removed at compile), so it
        // must not decide that layout; if it did, the render-identical authorings of one document
        // would each settle on a different stable form. Two rules follow:
        //
        // 1. Both boundaries move together. Hugging is all-or-nothing (`hug_both`): a ONE-SIDED
        //    hug used to give prettier's dangle (`<tag⏎\t>content` / `content</tag⏎>`), so a lone
        //    render-free boundary character selected the layout. A one-sided hug now falls through
        //    to the same Soft boundaries as no hug at all, which break block-style. Output is
        //    unchanged when the element fits inline (the Soft boundary reproduces the authored
        //    space flat); only the broken form converges.
        // 2. A boundary is Hard only when the content is multiline. A source break at just ONE
        //    boundary is not an expansion signal on its own — the component rule is
        //    both-or-neither (`has_source_breaks_in_content`), and a lone leading break used to
        //    harden the opening while leaving the children built inline, producing a third stable
        //    form (broken tags, children still flowing on one line).
        //
        // `<pre>`/`<textarea>` are dispatched to `build_whitespace_sensitive_element_doc` before
        // any of this — there boundary whitespace IS render-significant and the dangle is
        // mandatory. See conformance_prettier.md §Svelte: Inline content block-style.
        let mode = if ctx.needs_multiline {
            BoundaryMode::Hard
        } else if ctx.hug_both {
            BoundaryMode::Hug
        } else {
            BoundaryMode::Soft
        };

        ElementLayout::WithContent(mode)
    }
}
