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

use super::element_doc::{
    BoundaryMode, ElementContext, ElementKind, ElementLayout, ElementParts, MultilineCause,
};

/// Whether each edge of an element's content is newline-authored.
///
/// Two INDEPENDENT facts, both from the one boundary-authoring question
/// ([`Printer::nodes_boundary_newline`]). They are kept apart because the layout
/// rules read them differently — a block expands on the leading edge alone, while
/// components and inline elements are both-or-neither ([`Self::both`]) — and
/// folding them into a single "does it break" flag hides which rule is in play.
#[derive(Clone, Copy)]
struct BoundaryBreaks {
    /// Source has a newline in the run at the opening boundary
    leading: bool,
    /// Source has a newline in the run at the closing boundary
    trailing: bool,
}

impl BoundaryBreaks {
    /// Both edges newline-authored — the both-or-neither expansion signal.
    ///
    /// A lone leading break is not an expansion signal on its own: it used to
    /// harden the opening tag while leaving the children built inline, producing
    /// a third stable form. See [`Printer::compute_element_layout`].
    fn both(self) -> bool {
        self.leading && self.trailing
    }
}

/// Inputs to the [`Printer::compute_multiline_cause`] decision.
///
/// Bundles the per-element flags the predicate reads so they pass by name
/// rather than as positional bools that are easy to misorder at the call site.
/// Mirrors the corresponding [`ElementContext`] fields — both are built from
/// the same locals.
#[derive(Clone, Copy)]
struct MultilineInputs {
    /// Element type classification
    kind: ElementKind,
    /// Whether element has no meaningful content
    is_empty: bool,
    /// Whether each content boundary is newline-authored
    boundary: BoundaryBreaks,
    /// Whether block-flow children force this element multiline (cached, mirrors
    /// [`ElementContext::block_flow_multiline`])
    block_flow_multiline: bool,
    /// Whether all content children are text nodes
    only_text_content: bool,
}

/// What [`Printer::has_source_breaks_in_content`] learned about an element's content: whether the
/// author's newlines force multiline, and whether that content is a reflowable `fill`.
///
/// The two travel together because they are one traversal and, more importantly, one *question* —
/// see [`Printer::content_is_reflowable_fill`] for why every reader of the first must share the
/// second rather than re-derive it.
#[derive(Clone, Copy)]
struct ContentBreaks {
    /// The author's newlines force multiline layout
    multiline: bool,
    /// The content is a reflowable `fill`. Reported `false` on the two early returns that answer
    /// before the run is scanned (no content at all, and the block/component boundary rule) —
    /// neither has a reader for it, since the caller's third reader needs `boundary.leading`,
    /// which for a block is exactly the case that already returned multiline.
    is_fill: bool,
}

/// Whether `raw` holds an authored blank line — two newlines separated by horizontal whitespace
/// only.
///
/// ⚠️ **Not interchangeable with the two `has_blank_line`s already in the crate**
/// (`printer::text::TextAnalysis` on `str`, and `internal::Text`), which both answer
/// `newline_count >= 2` — a *total*, which two SEPARATE single breaks also reach
/// (`\n\tfoo bar\n\t`). Only the consecutive pair is the Tier-2 authoring signal, so this
/// predicate scans for the run rather than counting. The distinction is live, not theoretical:
/// `Printer::range_trailing_separator` reads the total and injects a blank line after a
/// `format-ignore` range whose trailing text merely spans two lines.
fn has_authored_blank_line(raw: &str) -> bool {
    let mut newlines = 0u32;
    for b in raw.bytes() {
        match b {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b' ' | b'\t' | b'\r' | b'\x0c' => {}
            _ => newlines = 0,
        }
    }
    false
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
    /// (ternary, binary, call, …). Mirrors the collapsible wrapper's hard-width
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

    /// Whether the element's content is a **`fill` to reflow into** — the thing whose presence
    /// makes the render-free content boundary stop selecting the layout.
    ///
    /// [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
    /// states the rule as "the presence of a `fill` to reflow into, **not the shape of the
    /// separator node**". A fill needs two things, and both are load-bearing:
    ///
    /// - **prose to pack** — a content text, asked through the flow rule's own
    ///   [`Printer::is_run_prose`] so the two rules cannot drift (an NBSP-only node is a
    ///   separator wearing content's clothing and is not prose);
    /// - **a whitespace seam to reflow at** — glued content (`<a>{expr}text</a>`) is a single
    ///   unbreakable unit. With nothing to reflow, the boundary is the only signal the author
    ///   has, so it keeps its authored lines — `elements/inline_multiline_nontext`, where
    ///   prettier agrees.
    ///
    /// The `{a} {b}` arm — a space-only separator standing between two non-text siblings — is a
    /// disjunct rather than a case of the above: such a run holds no prose, but the author put
    /// the siblings on one line themselves, so the boundary newline is not air there either.
    ///
    /// Both are reached only once the content is ONE run, which is the flow rule's own run
    /// boundary ([`Printer::breaks_inline_run`], reused so the two cannot drift): a **comment**
    /// (its line is authorship — §Comment Position Philosophy), a **control-flow block**, a
    /// **block element**, or an authored **blank line** each end the run, and content holding one
    /// is structured rather than flowed. Skipping that check flattened a `<!-- … -->` and the
    /// text below it onto one line, and deleted an authored blank line — two content-preservation
    /// breaks, not layout choices. The blank-line arm is asked of content texts too, since
    /// `breaks_inline_run` sees a blank only in a whitespace-ONLY node while `modern⏎⏎<Checkbox/>`
    /// carries it in a content text's trailing run.
    ///
    /// Because both conjuncts are about the *run*, the answer is independent of the separator's
    /// spelling and of how many siblings the run holds. That is the point: without it a prose
    /// separator (`<Comp /> text1 <Comp />`) and a space one (`<Comp /> <Comp />`) — one document
    /// under Svelte 5's whitespace collapse — reached two different layouts.
    ///
    /// ⚠️ **Every reader of "did the author break this content?" must consult this one answer.**
    /// The predicate *suppresses* a multiline signal, and a suppressed element lays its content
    /// out as one fill — which, once the content is wide, wraps a prose text ACROSS LINES and so
    /// writes the very newline the other readers key on. A reader left out therefore reports
    /// [`MultilineCause::SourceBreaks`] on the next pass; multiline mode freezes the authored
    /// newlines as hardlines, and the two arrangements alternate forever (an F1 break on real
    /// prose tables). There are exactly three readers: the `boundary.both()` arm and the
    /// content-text arm of [`Self::has_source_breaks_in_content`], and
    /// [`Self::text_has_internal_newlines`] at the [`Self::compute_multiline_cause`] call site.
    ///
    /// Takes the already-trimmed content run (the [`ContentBreaks`] producer owns that trim), so
    /// the boundary-trim scan is not repeated per reader.
    fn content_is_reflowable_fill(&self, run: &[FragmentNode<'_>]) -> bool {
        let source = self.source;

        // One run, or nothing to reflow as one — see the doc comment.
        if run.iter().any(|n| {
            self.breaks_inline_run(n)
                || matches!(n, FragmentNode::Text(t) if has_authored_blank_line(t.raw(source)))
        }) {
            return false;
        }

        // `{a} {b}` — a space-only separator between two non-text siblings.
        let spaced_siblings = run.windows(2).any(|w| {
            !matches!(w[0], FragmentNode::Text(_))
                && matches!(&w[1], FragmentNode::Text(t) if { let r = t.raw(source); !r.is_empty() && r.bytes().all(|b| b == b' ') })
        });
        if spaced_siblings {
            return true;
        }

        run.iter().any(|n| self.is_run_prose(n))
            && run.windows(2).any(|w| {
                matches!(&w[0], FragmentNode::Text(t) if t.raw(source).ends_with(|c: char| c.is_ascii_whitespace()))
                    || matches!(&w[1], FragmentNode::Text(t) if t.raw(source).starts_with(|c: char| c.is_ascii_whitespace()))
            })
    }

    /// Check if element content has source breaks (newlines) that should trigger multiline.
    ///
    /// Only reached for content with a non-text child — the caller
    /// (`compute_multiline_cause`) skips it for text-only content, where newlines are word
    /// separators and width alone decides layout (`<p>\ntext\n</p>` glues when it fits).
    ///
    /// The logic differs by element type:
    /// - **Blocks**: Leading boundary break triggers multiline (preserves `<div>\n<span>x</span>\n</div>`)
    /// - **Components**: Require BOTH leading AND trailing break (expressions hug when only leading)
    /// - **Inline**: Exclude boundary whitespace newlines (they normalize to spaces)
    ///
    /// Returns [`ContentBreaks`]: the answer, plus the [`Self::content_is_reflowable_fill`] answer
    /// this had to compute anyway, so the caller's third reader shares it rather than re-deriving
    /// it — see that predicate's warning for why one shared answer is load-bearing.
    fn has_source_breaks_in_content(
        &self,
        nodes: &[FragmentNode<'_>],
        kind: ElementKind,
        boundary: BoundaryBreaks,
    ) -> ContentBreaks {
        // Blocks: leading break alone triggers multiline
        // Components: require both boundaries
        //
        // Neither kind consults the fill answer (this returns before it is asked, and the caller's
        // third reader is unreachable for them — it needs `boundary.leading`, which is exactly the
        // block case here), so they never pay for the run scan.
        if (kind.is_block() && boundary.leading) || (kind.is_component() && boundary.both()) {
            return ContentBreaks {
                multiline: true,
                is_fill: false,
            };
        }

        let source = self.source;

        // Find first and last non-whitespace content indices
        let first_content_idx = nodes.iter().position(|n| !n.is_whitespace_only_text());
        let last_content_idx = nodes.iter().rposition(|n| !n.is_whitespace_only_text());

        let (Some(first), Some(last)) = (first_content_idx, last_content_idx) else {
            return ContentBreaks {
                multiline: false,
                is_fill: false,
            };
        };

        // Inline elements: preserve multiline only when BOTH boundaries are newline-authored
        // (both-or-neither, same as components) and there are non-text children.
        // `<a>\n\t{expr}\n</a>` preserves (leading newline + trailing newline).
        // `<a>\n\t{expr} </a>` collapses (trailing space is render-free — not a second break).
        // `<a>\n\t{expr}</a>` collapses (leading newline but no trailing break).
        // `<a>\n  text<span>text</span></a>` collapses (no trailing break).
        // `<span>  \n  {expr}</span>` collapses — but because it has no TRAILING break, not
        // because of the leading spaces: both boundaries route through the same run predicate.
        // A fill ([`Self::content_is_reflowable_fill`]) stays inline even with both breaks —
        // `{a} {b}`, and equally `<Comp /> text1 <Comp />` or `text1 <code>a</code>`, since what
        // makes the boundary inert is the fill, not the separator's shape.
        //
        // Both edges come from `nodes_boundary_newline` — the single boundary-authoring
        // question. It is a RUN predicate (does the edge whitespace run contain a newline?),
        // so horizontal whitespace before the newline is not authoring:
        // `<span>␣␣\n␣␣{x}\n</span>` and its mirror `<span>\n{x}␣␣\n␣␣</span>` are the same
        // document and must reach one form. A strict `starts_with('\n')` on the leading edge
        // alone made them settle on two, which is also what prettier's own run-based
        // `startsWithLinebreak`/`endsWithLinebreak` (`^([\t\f\r ]*\n)` / `(\n[\t\f\r ]*)$`)
        // rules out. Pinned by `elements/boundary_newline_padded`.
        let run = &nodes[first..=last];
        // The one fill answer, computed on the trim this function already owns.
        let is_fill = self.content_is_reflowable_fill(run);

        if boundary.both() {
            let has_nontext_content = run.iter().any(|n| !matches!(n, FragmentNode::Text(_)));

            if has_nontext_content && !is_fill {
                return ContentBreaks {
                    multiline: true,
                    is_fill,
                };
            }
        }

        if first >= last {
            return ContentBreaks {
                multiline: false,
                is_fill,
            };
        }

        // Check for newlines in content between first and last non-whitespace nodes
        let breaks = run.iter().any(|n| {
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
                // Inline, text with content: exclude the boundary (ASCII) whitespace runs on
                // BOTH edges, whatever the node's position. A non-breaking space is content, so
                // trim_ascii keeps it attached.
                //
                // The edge run is a *separator* between this text and its neighbour, and the
                // fill owns it either way — it reflows to a space when the run fits and to a
                // break when it does not. So its spelling is not the element-expansion signal;
                // only a newline strictly INSIDE the text's own content is. Trimming just the
                // fragment-edge sides (the old `is_first_content`/`is_last_content` match) left a
                // middle text's separator run counted, which made `<span><code>a</code> b,⏎<code>c
                // </code></span>` report SourceBreaks: the element went block-style on pass 1, the
                // fill then reflowed that very newline away, and pass 2 — seeing no newline left —
                // collapsed it inline. Two mechanisms reading one newline and answering
                // differently, the same class [`MultilineCause`] closed at the separator-flow site
                // (conformance_prettier.md §Svelte: Inline content block-style).
                //
                // The same argument reaches one step further inside a FILL, where the fill owns
                // the text's interior too: a newline there is one the fill itself wrapped in on a
                // previous pass, so reading it back as an expansion signal is the same
                // two-mechanisms-one-newline bug, merely relocated from the edge run to the
                // middle of a sentence. That is the F1 break the suppression exists to stop —
                // see [`Self::content_is_reflowable_fill`].
                !is_fill && raw.trim_ascii().contains('\n')
            }
        });
        ContentBreaks {
            multiline: breaks,
            is_fill,
        }
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
            collapses_child_ws,
            nodes,
            span,
            ..
        } = *parts;

        // Check if self-closing
        let is_self_closing =
            can_self_close && nodes.is_empty() && self.span_was_self_closing(span);

        // Check if empty
        let is_empty = nodes.is_empty() || nodes.iter().all(FragmentNode::is_whitespace_only_text);

        // Source boundary breaks — the same run predicate every other boundary question uses
        // (`nodes_boundary_newline`), not the stricter whitespace-only-node test. The two agree
        // wherever it matters (verified byte-identical over the fixture suite and 1200 real
        // components), and one question wants one predicate. Each edge is asked on its own;
        // the both-or-neither rules combine them via `BoundaryBreaks::both`.
        let boundary = BoundaryBreaks {
            leading: self.nodes_boundary_newline(nodes, true),
            trailing: self.nodes_boundary_newline(nodes, false),
        };

        // Block flow children → whether they force multiline. Computed once here (a non-trivial
        // traversal) and cached, since `will_go_multiline` and `compute_multiline_cause` both
        // read exactly this combination.
        let has_block_flow_children = nodes.iter().any(super::helpers::is_control_flow_block);
        let block_flow_multiline =
            has_block_flow_children && self.block_flow_forces_multiline(nodes);

        // Any attribute doc that will_break (forces attr group break)
        let has_multiline_attr = attr_docs.iter().any(|&doc| self.d().will_break(doc));

        // Check if all content children are text nodes (no elements, expressions, blocks)
        let only_text_content =
            !is_empty && nodes.iter().all(|n| matches!(n, FragmentNode::Text(_)));

        // The multiline decision. A whitespace-collapsing container (`<table>`, `<select>`, …)
        // with content always lays out block-style: its inter-sibling whitespace is render-free
        // (the compiler removes it), so the children sit one-per-line with the space trimmed, the
        // same block-style stance every other render-free boundary takes. `build_content_element_doc`
        // reads `collapses_child_ws` off the same `parts` to build that trimmed one-per-line content.
        // That is a property of the tag, not of the authoring, so it is `Structural`.
        let multiline = if collapses_child_ws && !is_empty {
            MultilineCause::Structural
        } else {
            self.compute_multiline_cause(
                nodes,
                MultilineInputs {
                    kind,
                    is_empty,
                    boundary,
                    block_flow_multiline,
                    only_text_content,
                },
            )
        };

        ElementContext {
            is_self_closing,
            is_empty,
            multiline,
            has_multiline_attr,
        }
    }

    /// Compute whether children need multiline formatting, and why.
    ///
    /// Every **structural** trigger is tested before either authoring-derived one, so an element
    /// that would go multiline regardless of its source newlines reports
    /// [`MultilineCause::Structural`] even when it also happens to be authored across lines. That
    /// ordering is what makes the cause meaningful to read — see [`MultilineCause`]. It is
    /// otherwise inert: every arm answers the same "multiline", so the *fact* is order-independent.
    fn compute_multiline_cause(
        &self,
        nodes: &[FragmentNode<'_>],
        inputs: MultilineInputs,
    ) -> MultilineCause {
        let MultilineInputs {
            kind,
            is_empty,
            boundary,
            block_flow_multiline,
            only_text_content,
        } = inputs;

        if is_empty {
            return MultilineCause::None;
        }

        // Multiple block children
        let block_child_count = nodes
            .iter()
            .filter(|n| self.is_block_element_child(n))
            .count();
        if block_child_count > 1 {
            return MultilineCause::Structural;
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
                return MultilineCause::Structural;
            }
        }

        // Elements with expanding blocks (if/each/key, or those inside await) always expand to
        // block-style multiline — inline elements too, not just block. The expanding block forces
        // block-style layout in `build_collapsible_element_doc` regardless; matching the multiline
        // decision here so the children are *built* multiline (one node per line) keeps the
        // expanding block from overshooting printWidth when authored compactly (it would otherwise
        // flow inline). Note: await blocks alone do NOT force expansion.
        if super::helpers::has_any_expanding_blocks(nodes) {
            return MultilineCause::Structural;
        }

        // await/snippet (which don't force-expand on their own) still go multiline when they
        // follow a sibling, so their body-drop matches if/each (via the multiline path) and
        // the sibling-`>` dangle / block-on-own-line separation resolves in one pass.
        if kind.is_block() && super::helpers::has_control_flow_after_sibling(nodes) {
            return MultilineCause::Structural;
        }

        // Block flow forces multiline
        if block_flow_multiline {
            return MultilineCause::Structural;
        }

        // The authoring-derived triggers. Both are skipped for text-only content — whitespace
        // newlines between text words collapse to spaces, so the group mechanism decides layout
        // on whether the joined text fits inline — and both read the SAME question about the
        // author's newlines, so they share one [`Self::content_is_reflowable_fill`] answer,
        // computed here. Splitting that answer across the readers is an F1 break, not a style
        // point: see the predicate's warning.
        if !only_text_content {
            // Source breaks in content
            let content = self.has_source_breaks_in_content(nodes, kind, boundary);
            if content.multiline {
                return MultilineCause::SourceBreaks;
            }

            // Text with internal newlines
            if !content.is_fill && self.text_has_internal_newlines(nodes, boundary.leading) {
                return MultilineCause::SourceBreaks;
            }
        }

        MultilineCause::None
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

        // Determine the boundary mode.
        //
        // Content that goes multiline lays out block-style — both tags intact, content on its own
        // indented lines — never with a dangled delimiter. Content-boundary whitespace is
        // render-free under Svelte 5 (start/end-of-tag whitespace is removed at compile), so it
        // must not decide that layout; if it did, the render-identical authorings of one document
        // would each settle on a different stable form.
        //
        // So the boundary run does not enter this decision at all: multiline-ness is the whole
        // question. `Hard` is exactly the multiline case; every inline case is `Soft`, whose
        // softlines reproduce the glued form flat and break block-style when the content doesn't
        // fit. A glued authoring and a spaced one therefore build the identical doc — which is why
        // there is no separate hug mode (the boundary run is trimmed either way), and why a source
        // break at just ONE boundary is not an expansion signal on its own: the rule is
        // both-or-neither (`has_source_breaks_in_content`). A lone leading break used to harden the
        // opening while leaving the children built inline, producing a third stable form (broken
        // tags, children still flowing on one line).
        //
        // `<pre>`/`<textarea>` are dispatched to `build_whitespace_sensitive_element_doc` before
        // any of this — there boundary whitespace IS render-significant and the dangle is
        // mandatory. See conformance_prettier.md §Svelte: Inline content block-style.
        let mode = if ctx.multiline.is_multiline() {
            BoundaryMode::Hard
        } else {
            BoundaryMode::Soft
        };

        ElementLayout::WithContent(mode)
    }
}
