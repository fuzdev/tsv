// Element analysis and layout classification for doc building
//
// The analyze/classify half of element doc building: predicates that inspect an
// element's children and source boundaries to decide multiline-ness, boundary
// modes, and the overall layout. The shared types (`BoundaryMode`,
// `ElementLayout`, `ElementKind`, `ElementContext`) live in `element_doc.rs`
// alongside the build half that also consumes them.

use crate::ast::internal::{FragmentNode, is_collapsible_ws_char};
use crate::printer::Printer;
use crate::printer::text::has_authored_blank_line;
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
    /// A lone leading break is not an expansion signal on its own: hardening the
    /// opening tag on it while leaving the children built inline produces
    /// a third stable form. See [`Printer::compute_element_layout`].
    fn both(self) -> bool {
        self.leading && self.trailing
    }
}

/// Inputs to the [`Printer::compute_multiline_cause`] decision.
///
/// Bundles the per-element flags the predicate reads so they pass by name
/// rather than as positional bools that are easy to misorder at the call site.
/// Built from the same `analyze_element` locals that fill [`ElementContext`],
/// which keeps only the subset its own consumers need (`is_empty` is the one
/// field both carry).
#[derive(Clone, Copy)]
struct MultilineInputs {
    /// Element type classification
    kind: ElementKind,
    /// Whether element has no meaningful content
    is_empty: bool,
    /// Whether each content boundary is newline-authored
    boundary: BoundaryBreaks,
    /// Whether block-flow children force this element multiline —
    /// [`Printer::block_flow_forces_multiline`] gated on the element having any.
    /// Cached by `analyze_element` rather than asked here: it is a non-trivial
    /// traversal and `will_go_multiline` reads the same combination.
    block_flow_multiline: bool,
    /// Whether all content children are text nodes
    only_text_content: bool,
}

/// Whether every node here is a `Text` — content whose newlines are word separators, so width
/// alone decides its layout. `compute_multiline_cause` skips the authoring-derived trigger for
/// such content (the `only_text_content` gate), which is why a text-only element authored with
/// boundary air collapses back inline where any other content kind stays expanded. (The answer
/// is invariant under [`trimmed_content_run`]'s trim anyway — the trim removes only
/// whitespace-only `Text` nodes.)
///
/// It once had a second reader, a mirror answering what a width-broken element's OUTPUT
/// re-parses as, so that the tail boundary after such an element could pre-empt the next pass.
/// There is no such reader: the tail boundary's space spelling is decided per width at every
/// site, and its newline spelling reads the actual render (the flow probe), so nothing needs
/// to predict the re-parse.
fn content_is_text_only(nodes: &[FragmentNode<'_>]) -> bool {
    nodes.iter().all(|n| matches!(n, FragmentNode::Text(_)))
}

/// The content run between a fragment's first and last non-whitespace nodes — the slice every
/// content-shape question is asked of ([`Printer::has_source_breaks_in_content`]), so the
/// boundary-trim scan has one definition. `None` when there is no content at all.
fn trimmed_content_run<'n, 'x>(nodes: &'n [FragmentNode<'x>]) -> Option<&'n [FragmentNode<'x>]> {
    let first = nodes.iter().position(|n| !n.is_whitespace_only_text())?;
    let last = nodes.iter().rposition(|n| !n.is_whitespace_only_text())?;
    Some(&nodes[first..=last])
}

impl<'a> Printer<'a> {
    /// Check if an expression has internal break points (ternary, &&, ||, +, etc.)
    ///
    /// Sole consumer: the whitespace-sensitive element builder's simple-content
    /// check (`element_ws_sensitive_doc.rs`) — a `<pre>`/`<textarea>` whose only
    /// child is a single expression tag WITHOUT break points takes the
    /// `>`-dangle hug, while break-capable content uses normal flow so the
    /// expression breaks internally first.
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
    /// [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
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
    /// ⚠️ **A prose-free run reaches no fill answer here, and there is deliberately no third path
    /// for one.** A `{a} {b}` disjunct granting one — a one-line whitespace separator between
    /// two non-text siblings, in a run the author left on one line, on the reading that the author
    /// had put the siblings on one line themselves — cannot change an answer with the readers
    /// unified: this predicate is consulted only by the two
    /// interior-newline arms of [`Self::has_source_breaks_in_content`], and both ask it *of a text
    /// node that carries a newline*. A run that disjunct accepts has none — every newline-bearing
    /// node in it would have had to be prose, and a run with prose already reaches `true` through
    /// the prose path below (a one-line separator is itself a seam, so the seam conjunct is
    /// satisfied whenever the pair test is). So its whole distinct contribution was prose-free runs
    /// in which the arms were false regardless. The rule it stood for is unchanged and still
    /// enforced — a separator's *spelling* may not pick a layout, so space, tab and an
    /// entity-encoded tab are one document
    /// (`elements/inline_separator_tab_prettier_divergence`,
    /// `elements/inline_separator_entity_collapse_prettier_divergence`) — but that equality is
    /// carried by the boundary and separator paths, not by a fill answer for content that has no
    /// fill.
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
    /// The predicate *suppresses* a multiline signal: where it holds, an authored newline in the
    /// run belongs to the fill, which reflows it per width. A reader keying on the raw newline
    /// instead re-reads a wrap point as authoring and reports
    /// [`MultilineCause::SourceBreaks`] — and since block-style output emits boundary newlines
    /// the boundary arms honor, the wrong expansion is its own fixed point (where the interior
    /// would instead re-freeze the newline, it is an F1 2-cycle — a live break on real prose
    /// tables before the readers were unified). There are exactly two readers, both in
    /// [`Self::has_source_breaks_in_content`]'s interior scan: the content-text arm and the
    /// whitespace-ONLY-text arm.
    /// The whitespace-only arm was the last holdout, and it failed in the shape the other
    /// cannot reach: a run whose prose and whose newline live in DIFFERENT nodes
    /// (`<code>a</code>⏎<code>b</code> text1`), where the separator carrying the break holds
    /// nothing but whitespace. Its space twin already collapsed, so the two spellings of one
    /// document reached two layouts — `elements/inline_content_flow_collapse_prettier_divergence`
    /// carries the case.
    ///
    /// Takes the already-trimmed content run (the caller shares [`trimmed_content_run`]'s trim),
    /// so the boundary-trim scan is not repeated per reader.
    fn content_is_reflowable_fill(&self, run: &[FragmentNode<'_>]) -> bool {
        let source = self.source;

        // One run, or nothing to reflow as one — see the doc comment. The blank-line arm is the
        // CONTENT-text half of that question and says so: on a whitespace-only node
        // `breaks_inline_run` already answers it (`newline_count >= 2` is the same predicate there,
        // since every other byte of such a node is horizontal whitespace), so scanning those bytes
        // again finds nothing new on the commonest node in a fragment.
        if run.iter().any(|n| {
            self.breaks_inline_run(n)
                || matches!(n, FragmentNode::Text(t)
                    if !t.is_collapsible_ws_only && has_authored_blank_line(t.raw(source)))
        }) {
            return false;
        }

        // A pair is reflowable when the boundary between them is NOT glued — the whitespace lives on
        // one of the two texts' facing edges, so there is a break point to reflow at. Same predicate
        // the fragment path's glue decisions ask, negated (`Printer::text_glued_before` / `_after`).
        run.iter().any(|n| self.is_run_prose(n)) && run.windows(2).any(|w| {
            matches!(&w[0], FragmentNode::Text(t) if !Self::text_glued_after(t.raw(source)))
                || matches!(&w[1], FragmentNode::Text(t) if !Self::text_glued_before(t.raw(source)))
        })
    }

    /// Check if element content has source breaks (newlines) that should trigger multiline.
    ///
    /// Only reached for content with a non-text child — the caller
    /// (`compute_multiline_cause`) skips it for text-only content, where newlines are word
    /// separators and width alone decides layout (`<p>\ntext\n</p>` glues when it fits).
    ///
    /// The per-kind logic is the BOUNDARY rule alone:
    /// - **Blocks**: the leading boundary break alone triggers multiline (preserves `<div>\n<span>x</span>\n</div>`)
    /// - **Components / Inline**: require BOTH boundary breaks (both-or-neither)
    ///
    /// The interior scan is kind-independent: an authored newline inside the trimmed content
    /// run counts only where the run has no fill to reflow into — inside a fill it is the
    /// fill's own wrap point, whatever container holds it (the same reason the caller skips
    /// text-only content entirely). Which NODE a one-boundary newline lands in (a
    /// whitespace-only separator in front of an element vs the edge run inside a content text)
    /// is likewise not part of the question: the boundary arms read the fragment edge via
    /// `nodes_boundary_newline` and the interior scan trims a content text's edge runs, so a
    /// one-sided authoring answers by the boundary rule alone
    /// (`elements/boundary_air_one_sided_prettier_divergence`).
    ///
    /// There is deliberately no mirror predicate asking what a width-broken element's OUTPUT
    /// re-parses as (so the tail boundary after such an element could answer in advance what
    /// the next pass would) — a tail boundary's space spelling is
    /// decided per width from the closing tag's own column, and its newline spelling is
    /// layout-keyed at render (the flow probe — [`tsv_lang::doc::DocContext::flow_break_probe`]
    /// / `hold_line_after_broken_flow` — holds the tail's line exactly when the unit actually
    /// rendered multiline) — so no caller has to predict the re-parse and there is no second
    /// copy of these rules to keep in step.
    fn has_source_breaks_in_content(
        &self,
        nodes: &[FragmentNode<'_>],
        kind: ElementKind,
        boundary: BoundaryBreaks,
    ) -> bool {
        // Blocks: the leading break alone triggers multiline. A component's both() case is the
        // shared both-or-neither arm below — spelling it here too would be a second copy of one
        // rule.
        if kind.is_block() && boundary.leading {
            return true;
        }

        let source = self.source;

        let Some(run) = trimmed_content_run(nodes) else {
            return false;
        };

        // Components and inline elements: preserve multiline only when BOTH boundaries are
        // newline-authored (both-or-neither; a block's both() case was consumed by its leading
        // early return above, and every kind continues into the shared interior scan) and
        // there are non-text children.
        // `<a>\n\t{expr}\n</a>` preserves (leading newline + trailing newline).
        // `<a>\n\t{expr} </a>` collapses (trailing space is render-free — not a second break).
        // `<a>\n\t{expr}</a>` collapses (leading newline but no trailing break).
        // `<a>\n  text<span>text</span></a>` collapses (no trailing break).
        // `<span>  \n  {expr}</span>` collapses — but because it has no TRAILING break, not
        // because of the leading spaces: both boundaries route through the same run predicate.
        // A fill ([`Self::content_is_reflowable_fill`]) no longer changes that answer: the
        // boundary is the author's air whatever the content is made of, so `{a} {b}` and
        // `<Comp /> text1 <Comp />` preserve it exactly as glued content does. The fill answer
        // still governs the two INTERIOR arms below — a newline the fill itself wrapped in must
        // not be re-read as authoring — which is the distinction the old boundary conjunct
        // conflated.
        //
        // Both edges come from `nodes_boundary_newline` — the single boundary-authoring
        // question. It is a RUN predicate (does the edge whitespace run contain a newline?),
        // so horizontal whitespace before the newline is not authoring:
        // `<span>␣␣\n␣␣{x}\n</span>` and its mirror `<span>\n{x}␣␣\n␣␣</span>` are the same
        // document and must reach one form. A strict `starts_with('\n')` on the leading edge
        // alone made them settle on two, which is also what prettier's own run-based
        // `startsWithLinebreak`/`endsWithLinebreak` (`^([\t\f\r ]*\n)` / `(\n[\t\f\r ]*)$`)
        // rules out. Pinned by `elements/boundary_newline_padded`.
        //
        // The run is known to hold a non-text child, so the old `has_nontext_content` conjunct
        // here was dead: `compute_multiline_cause` reaches this function only past its `is_empty`
        // return and only when `!only_text_content`, i.e. only when some node is not a `Text` —
        // and the trim above drops whitespace-only *text* alone, so that node is inside `run`.
        // Both boundaries authored — the air is the author's, whatever the content is made of.
        // This is the SAME question the block arm above answers with `boundary.leading`, so
        // asking it identically for the other two kinds is what makes one
        // rule out of three: folding air at an inline element that a component in the identical
        // shape keeps would be a split keyed on the container's classification rather than on anything the
        // rule is about (`<span>` / `<a>` / `<td>` / `<label>` vs `<Comp>` / `<p>`).
        //
        // The conjunct removed here was `!is_fill`, on the reasoning that a run with a whitespace
        // seam has a fill to reflow into and therefore does not need its boundary preserved. That
        // is true of the run's INTERIOR — which is why `is_fill` still suppresses the two
        // interior-newline arms below — but not of the element's own boundary, where a newline is
        // the only air the author can express and prettier preserves it in every shape probed.
        if boundary.both() {
            return true;
        }

        if run.len() <= 1 {
            return false;
        }

        // The one fill answer, computed on the shared trim — consulted only by the two interior
        // arms, and computed only once one can be reached (every boundary-air case above returns
        // before the scan).
        let is_fill = self.content_is_reflowable_fill(run);

        // Check for newlines in content between first and last non-whitespace nodes
        run.iter().any(|n| {
            let FragmentNode::Text(t) = n else {
                return false;
            };

            let raw = t.raw(source);
            if t.is_collapsible_ws_only {
                // Whitespace-only: the node IS a separator, so its newline is an
                // expansion signal only where there is no fill for the run to reflow into.
                // Inside a fill it is the same newline the fill itself would wrap in, and
                // reading it back as authored is the two-mechanisms-one-newline bug the
                // content-text arm below documents — here with the prose and the break simply
                // living in different nodes. See [`Self::content_is_reflowable_fill`].
                !is_fill && t.has_newline()
            } else {
                // Text with content: exclude the boundary collapsible-whitespace runs
                // on BOTH edges, whatever the node's position. An NBSP or form feed is content,
                // so the trim keeps it attached.
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
                // (conformance_prettier_svelte.md §Svelte: Inline content block-style).
                //
                // The same argument reaches one step further inside a FILL, where the fill owns
                // the text's interior too: a newline there is one the fill itself wrapped in on a
                // previous pass, so reading it back as an expansion signal is the same
                // two-mechanisms-one-newline bug, merely relocated from the edge run to the
                // middle of a sentence. That is the F1 break the suppression exists to stop —
                // see [`Self::content_is_reflowable_fill`].
                !is_fill && raw.trim_matches(is_collapsible_ws_char).contains('\n')
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
        let only_text_content = !is_empty && content_is_text_only(nodes);

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
    /// Every **structural** trigger is tested before the authoring-derived one, so an element
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
                FragmentNode::Text(t) => !t.is_collapsible_ws_only,
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

        // A declaration that owns its own line is the same kind of child as a block element
        // here — its line is a break the content must have room for, so the element lays out
        // block-style rather than collapsing the declaration onto its neighbour. A LONE
        // declaration counts too (`Printer::has_own_line_declaration`): its boundaries are not
        // content, so it is not glued and the element still goes block-style.
        if self.has_own_line_declaration(nodes) {
            return MultilineCause::Structural;
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

        // The authoring-derived trigger, skipped for text-only content — whitespace newlines
        // between text words collapse to spaces, so the group mechanism decides layout on
        // whether the joined text fits inline. Every newline reading inside it shares one
        // [`Self::content_is_reflowable_fill`] answer: see the predicate's warning.
        if !only_text_content && self.has_source_breaks_in_content(nodes, kind, boundary) {
            return MultilineCause::SourceBreaks;
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
                matches!(n, FragmentNode::Text(t) if t.is_collapsible_ws_only && !t.raw(source).is_empty())
            });

        has_non_inline_block || has_ws_around_blocks
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
        // render-free under Svelte 5 (start/end-of-tag whitespace is removed at compile), so its
        // spelling must not decide the tags' layout; if it did, the render-identical authorings
        // of one document would each settle on a different stable form. (Whether the element is
        // multiline AT ALL is a different question, which a both-boundary newline does decide —
        // the Tier-2 air request `has_source_breaks_in_content` honors.)
        //
        // So the boundary run does not enter this decision at all: multiline-ness is the whole
        // question. `Hard` is exactly the multiline case; every inline case is `Soft`, whose
        // softlines reproduce the glued form flat and break block-style when the content doesn't
        // fit. A glued authoring and a spaced one therefore build the identical doc — which is why
        // there is no separate hug mode (the boundary run is trimmed either way), and why a source
        // break at just ONE boundary is not an expansion signal on its own: the rule is
        // both-or-neither (`has_source_breaks_in_content`). A lone leading break hardening the
        // opening while the children stay built inline would produce a third stable form (broken
        // tags, children still flowing on one line).
        //
        // `<pre>`/`<textarea>` are dispatched to `build_whitespace_sensitive_element_doc` before
        // any of this — there boundary whitespace IS render-significant and the dangle is
        // mandatory. See conformance_prettier_svelte.md §Svelte: Inline content block-style.
        let mode = if ctx.multiline.is_multiline() {
            BoundaryMode::Hard
        } else {
            BoundaryMode::Soft
        };

        ElementLayout::WithContent(mode)
    }
}
