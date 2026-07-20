//! Which host comments carry into the synthetic program, and which classes
//! refuse.
//!
//! Server-specific: every rule here is reasoning about the *oracle's printer*
//! (esrap) — where its single `comment_index` sits when a given block opens, and
//! which synthetic span windows a carried comment would fall into. A client
//! transform emits different blocks and would need its own answer, so this is
//! deliberately not filed with the target-independent script analyses.
//!
//! See [`crate::transform_server`] for the orchestration that calls this, and
//! `../../docs/checklist_svelte_compiler.md` for the probed boundaries.

use tsv_svelte::ast::internal::{ElementKind, FragmentNode, Root};
use tsv_ts::ast::internal::Statement;

use crate::analyze::{is_effect_call, is_inspect_call};
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// Collect the comments carried into the synthetic program: exactly the host
/// comments inside the instance script's content span. Classes that can't
/// converge refuse:
///
/// - comments outside the script (template-expression comments) — the emitters
///   don't thread them yet;
/// - a fragment node *before* the script end (template-before-script) — the
///   `$.escape`/`$.html` wrapper windows would sweep script comments;
/// - format-ignore directives — they'd switch the printer to raw-source
///   emission of synthetic spans.
///
/// A comment inside the **module** script's content span is DROPPED (skipped, not
/// carried and not refused) when the module script comes **first**: the oracle's
/// printer has already advanced its comment index past it, so emitting the module
/// body comment-free reproduces the drop as parity. With the module script
/// **second** the index is re-seeked backward onto the comment and the oracle
/// re-attaches it into a template expression, so that ordering refuses
/// ([`Refusal::ModuleCommentAfterInstanceScript`]).
///
/// ⚠️ That module-first drop is parity only while nothing ELSE re-seeks the index.
/// A second, INDEPENDENT axis does: a block-bearing statement earlier in the module
/// body carries a `loc`, so opening it seeks the index back over the comment and the
/// oracle emits it. The refusal keys on script ORDER and does not cover this — a
/// module-first document, with or without an instance script, still mismatches when
/// a `function` / `class` / `if (1) {}` precedes the comment. See
/// `docs/checklist_svelte_compiler.md` §The open half for the probed boundary; do not
/// re-derive it from the ordering rule above.
pub(crate) fn collect_script_comments(
    root: &Root<'_>,
    source: &str,
    instance_body: &[Statement<'_>],
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    // The oracle drops module-script comments — a comment fully within the module
    // content span never carries and never refuses.
    // TODO: this unconditional skip is the open half of the module-script comment
    // class. It holds only while no earlier module-body statement carries a `{ … }`
    // block; one that does re-seeks esrap's comment index back over the comment and
    // the oracle EMITS it, which tsv drops → MISMATCH. Orthogonal to the ordering
    // refusal below and to instance-script presence. Boundary + corpus exposure:
    // `docs/checklist_svelte_compiler.md` §The open half.
    let module_content = root.module.map(|module| module.content.span);
    let in_module = |comment: &tsv_lang::Comment| {
        module_content.is_some_and(|m| comment.span.start >= m.start && comment.span.end <= m.end)
    };
    let Some(script) = root.instance else {
        // No instance script to carry into: any comment that is not a (dropped)
        // module comment is a template comment we don't thread — refuse.
        for comment in &root.comments {
            if !in_module(comment) {
                return Err(unsupported(Refusal::TemplateComments));
            }
        }
        return Ok(Vec::new());
    };
    let content = script.content.span;
    // Source order of the two scripts — the whole trigger for
    // [`Refusal::ModuleCommentAfterInstanceScript`] below. The tags cannot nest,
    // so comparing content starts is a total order.
    let module_after_instance = module_content.is_some_and(|m| m.start > content.start);
    // A comment at or past the last SURVIVING statement has no statement left to
    // lead — an `import` hoists to the comment-free module program and a
    // statement-position `$effect`/`$inspect` drops, so neither anchors one. The
    // bound is the last surviving statement's end, `content.start` when nothing
    // survives (an import-only script).
    //
    // Such a comment still carries: the oracle re-attaches it into the template
    // (trailing the final push, or nested inside the next emitted node — an
    // `{#if}` condition, an `$.ensure_array_like(…)` / `$.attr(…)` argument) while
    // tsv's printer lands it at the end of the synthetic function body (the body
    // block's span runs `[content.start, rbrace_end)`, so the block's trailing
    // window captures it exactly once). The placements differ, but the parity bar
    // grades comment DROP / COUNT / CONTENT, not position.
    //
    // The one shape that does NOT converge is a template emitting a nested block —
    // see [`template_emits_nested_block`].
    let survives = |stmt: &Statement<'_>| match stmt {
        Statement::ImportDeclaration(_) => false,
        Statement::ExpressionStatement(expr_stmt) => {
            is_effect_call(&expr_stmt.expression, source).is_none()
                && is_inspect_call(&expr_stmt.expression, source).is_none()
        }
        _ => true,
    };
    let last_stmt_end = instance_body
        .iter()
        .filter(|stmt| survives(stmt))
        .map(|stmt| stmt.span().end)
        .max()
        .unwrap_or(content.start);
    let nested_block = template_emits_nested_block(root.fragment.nodes);
    // A leading comment glued to the `<script>` line (no newline before it) shares
    // its source line with the function's synthetic opening brace, so the printer
    // trails it after the `{` instead of onto its own line — refuse the class
    // (prettier-formatted input always puts a leading comment on its own line, so
    // the covered fixtures are unaffected).
    let first_stmt_start = instance_body
        .first()
        .map_or(content.end, |stmt| stmt.span().start);
    let mut comments = Vec::with_capacity(root.comments.len());
    for comment in &root.comments {
        // A module-script comment drops — but ONLY when the module script comes
        // FIRST. The oracle's drop is not a rule about module scripts; it is
        // where esrap's single comment index happens to be. The component body
        // block carries the instance script's `loc`, and opening it re-seeks that
        // index ABSOLUTELY — forward past a comment that precedes the instance
        // script (the drop tsv reproduces), but BACKWARD onto one that follows it.
        // A recovered comment is then flushed into the next loc-bearing node the
        // printer reaches, which is a template expression it has nothing to do
        // with. tsv drops it either way, so the second ordering is a comment
        // PRESENCE difference — a mismatch, not a tolerated position one.
        if in_module(comment) {
            if module_after_instance {
                return Err(unsupported(Refusal::ModuleCommentAfterInstanceScript));
            }
            continue;
        }
        if comment.span.start < content.start || comment.span.end > content.end {
            return Err(unsupported(Refusal::TemplateComments));
        }
        // A multi-line block comment carries verbatim, but the oracle (esrap)
        // re-indents its interior lines to the emit position, so the two diverge on
        // any interior line whose source indentation differs from the target — refuse
        // until the printer re-indents block-comment interiors to match. Checked
        // before the after-last rule below so this independent gate keeps its own
        // refusal bucket whatever the template emits.
        if comment.multiline {
            return Err(unsupported(Refusal::MultilineBlockComment));
        }
        if nested_block && comment.span.start >= last_stmt_end {
            return Err(unsupported(Refusal::CommentAfterLastStatementWithBlock));
        }
        if comment.span.end <= first_stmt_start {
            let gap = &source[content.start as usize..comment.span.start as usize];
            if !gap.contains('\n') {
                return Err(unsupported(Refusal::LeadingCommentGluedToScript));
            }
        }
        let text = comment.content(source);
        if text.contains("prettier-ignore") || text.contains("format-ignore") {
            return Err(unsupported(Refusal::FormatIgnoreComment));
        }
        let mut comment = comment.clone();
        // Release a JSDoc cast's comment back to the positional machinery. `tsv_ts`
        // binds it to its `JsdocCast` node (`Comment::owned_by_node`) so a synthesized
        // paren can't land between the comment and the `(` it glues to — the owning
        // node becomes the only thing that prints it, and the range lookups skip it.
        // Erasure unwraps *every* `JsdocCast` (the compile path matches the oracle,
        // which has no such node and drops the parens), so in the emitted program that
        // owner does not exist: left owned, the comment is printed by nothing and
        // silently dropped. Un-owned, it prints from its gap exactly as the oracle
        // prints it — `const x = /** @type {number} */ 1`.
        comment.owned_by_node = false;
        comments.push(comment);
    }
    for node in root.fragment.nodes {
        // A whitespace-only text node — e.g. the run between a module `</script>`
        // and the instance `<script>`, or leading/trailing template whitespace —
        // is not real markup, so it doesn't force the refusal. Any genuine
        // element / expression / comment / block before the instance script's end
        // still refuses (its emitter's comment window would sweep the carried
        // script comments). A Unicode-whitespace-only text (`is_ascii_ws_only ==
        // false`) is content and correctly still refuses.
        if let FragmentNode::Text(text) = node
            && text.is_ascii_ws_only
        {
            continue;
        }
        if node.span().start < content.end {
            return Err(unsupported(Refusal::CommentsWithTemplateBeforeScript));
        }
    }
    Ok(comments)
}

/// Does the template emit a **synthetic block** — a `{ … }` body the oracle
/// builds with no source `loc`?
///
/// This decides whether a comment past the last surviving script statement can be
/// carried. The oracle's printer (esrap) walks one `comment_index` over the comment
/// list, and `body()` opens every block with `reset_comment_index(node)`. That reset
/// has two arms: a block with **no** `loc` sets the index to `comments.length`,
/// **discarding every comment not yet written**; a block that **has** a `loc`
/// re-seeks the index absolutely (`comments.findIndex(…)`), which can move it
/// **backward**. So a loc-less block annihilates the index and the next loc-bearing
/// block **recovers** it.
///
/// That recovery — not an exemption — is what carries an after-last comment through
/// to the component body. The body block is assigned the instance script's `loc`
/// (the transform's "trick esrap into including comments" line), and when the
/// component needs a context wrapper the transform **reassigns** `component_block`
/// to a fresh loc-LESS block around that loc-bearing one. The wrapper does annihilate
/// the index; the inner block then seeks back over the comment, so it still reaches
/// the body's closing flush. A template block gets no such recovery — it is loc-less,
/// reached after the body has already seeked, with nothing loc-bearing behind it to
/// seek back — so the comment is DROPPED, a divergence the parity bar grades, unlike
/// a mere position difference.
///
/// The scan is deliberately blunt: it answers "does a synthetic block exist
/// anywhere", not "is one reached before the comment would flush". A loc-bearing
/// expression emitted first (an `{#if}` test, an `{#each}` expression) flushes the
/// comment ahead of the block and the oracle keeps it — so `{#if x}` with an
/// after-last comment converges in practice, and this scan over-refuses it.
/// Tightening that costs an ordered next-emitted-node walk plus the oracle's
/// fold/rewrite rules for which expressions keep a `loc`; a safe over-refusal is
/// preferred to guessing.
///
/// The [`FragmentNode::SpecialElement`] arm is intentionally blanket-TRUE for the
/// same reason. Several kinds do emit a block and genuinely drop the comment
/// (`<svelte:head>`, `<svelte:element>`, `<svelte:boundary>`), but `<svelte:window>`
/// and `<slot>` emit no block at all and are knowingly over-refused — the blanket arm
/// buys a conservative safety margin, not a claim that every kind drops.
///
/// ⚠️ This TRUE/FALSE split is keyed to the **pinned** oracle's `reset_comment_index`
/// behavior (esrap 2.2.12, via the pinned Svelte compiler). If that pin moves, re-probe
/// the split against the new oracle rather than assuming it carries over.
///
/// Exhaustively matched so a new [`FragmentNode`] variant fails compilation here
/// rather than silently defaulting to "no block".
fn template_emits_nested_block(nodes: &[FragmentNode<'_>]) -> bool {
    nodes.iter().any(|node| match node {
        // Leaves and the tags that emit a bare call — no block.
        FragmentNode::Text(_)
        | FragmentNode::Comment(_)
        | FragmentNode::ExpressionTag(_)
        | FragmentNode::HtmlTag(_)
        | FragmentNode::RenderTag(_) => false,
        // Every block/closure emitter: `{#if}`/`{#each}` bodies, the `$.await` and
        // `$.head`/`$.element` closures, a `{#snippet}` function.
        FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_)
        | FragmentNode::SnippetBlock(_)
        | FragmentNode::SpecialElement(_)
        // Refused elsewhere; counted here so the scan never under-reports.
        | FragmentNode::ConstTag(_)
        | FragmentNode::DeclarationTag(_)
        | FragmentNode::DebugTag(_) => true,
        FragmentNode::Element(element) => match element.kind {
            // A component's children become a `children: ($$renderer) => { … }`
            // snippet prop — a block. Childless (or whitespace-only), it is a bare
            // `Foo($$renderer, {…})` call.
            ElementKind::Component => {
                element.fragment.nodes.iter().any(|child| {
                    !matches!(child, FragmentNode::Text(text) if text.is_ascii_ws_only)
                })
            }
            ElementKind::Html => template_emits_nested_block(element.fragment.nodes),
        },
    })
}
