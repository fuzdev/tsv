//! The per-fragment walk and its whitespace normalization.
//!
//! **The hub of the emission layer** — the tree recursion every other emitter
//! hangs off. [`emit_fragment`] is the core loop: decode nodes into [`CleanNode`],
//! normalize whitespace per the oracle's `clean_nodes` rules, then dispatch each
//! surviving node to its per-node emitter ([`crate::element`],
//! [`crate::blocks`], [`crate::snippet_emit`]), which recurse back through
//! [`emit_child_body`]. That mutual recursion is the tree's own shape and is the
//! only cycle here: the *primitives* the spokes share — the template accumulator
//! ([`crate::body_builder`]), the value rewrite ([`crate::template_value`]), the
//! dropped-region guards ([`crate::dropped`]), the special-element table
//! ([`crate::special_element_kind`]) — live in their own modules, and nothing in
//! them calls back into this walk.
//!
//! **Single source of truth** for the oracle's static-emission normalization:
//! boundary whitespace-only text dropped and edge runs trimmed per fragment, an
//! edge run abutting a non-text node collapsed to one space, `<pre>`/`<textarea>`
//! preserved, and the text-first `<!---->` anchor. Those rules were probe-derived
//! against Svelte's own `clean_nodes`/`escape_html` and are keyed on a node's
//! *neighbors*, so a second copy at any emitter would see a different neighbor
//! list and silently change the rendered output.
//!
//! See [`crate::transform_server`] for the orchestration that drives the root
//! fragment.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_svelte::ast::internal::{
    AwaitBlock, ConstTag, EachBlock, Element, ElementKind, ExpressionTag, Fragment, FragmentNode,
    HtmlTag, IfBlock, KeyBlock, RenderTag, SnippetBlock, SpecialElement, SpecialElementKind,
    SpecialThis,
};
use tsv_ts::ast::internal::{Expression, ExpressionStatement, Statement};

use crate::analyze::{ScopeEntry, evaluate, stringify_value};
use crate::blocks::{
    emit_await_block, emit_boundary, emit_const_tag, emit_each_block, emit_if_block,
    emit_key_block, emit_svelte_head,
};
use crate::body_builder::BodyBuilder;
use crate::build::escape_template_text;
use crate::component::component_is_standalone_eligible;
use crate::dropped::guard_inert_special_element;
use crate::element::{emit_element, emit_svelte_element};
use crate::namespace::{ChildNamespace, Namespace, infer_namespace};
use crate::snippet_emit::{
    emit_render_tag, emit_snippet, render_callee_dynamic, render_callee_name,
};
use crate::special_element_kind::special_element_refusal_kind;
use crate::template_value::wrap_value_expr;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Svelte's template whitespace class (`[ \t\r\n]` — the compiler's
/// `regex_*_whitespaces` patterns; deliberately narrower than Unicode
/// whitespace, so e.g. a decoded `&nbsp;` is content).
fn is_template_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn is_ws_only(s: &str) -> bool {
    s.chars().all(is_template_ws)
}

/// Replace the leading `[ \t\r\n]+` run with `replacement` (no-op without one).
fn replace_leading_ws(s: &str, replacement: &str) -> String {
    let trimmed = s.trim_start_matches(is_template_ws);
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!("{replacement}{trimmed}")
    }
}

/// Replace the trailing `[ \t\r\n]+` run with `replacement` (no-op without one).
fn replace_trailing_ws(s: &str, replacement: &str) -> String {
    let trimmed = s.trim_end_matches(is_template_ws);
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!("{trimmed}{replacement}")
    }
}

/// HTML-escape text content the way the oracle does (`escape_html`, `[&<]`).
fn escape_html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// A fragment child after comment-dropping, const-tag hoisting, and text
/// decoding, mutable for the whitespace normalization pass. Blocks are non-text
/// nodes for whitespace purposes (`is_expr` is false).
enum CleanNode<'arena> {
    Text(String),
    Expr(&'arena ExpressionTag<'arena>),
    Html(&'arena HtmlTag<'arena>),
    Element(&'arena Element<'arena>),
    /// A `<svelte:element this={…}>` — emitted as a statement-level
    /// `$.element($$renderer, TAG, attrsFn?, childrenFn?)` call (like a component /
    /// control-flow block, it interrupts the template push stream). A non-text,
    /// non-expr node for whitespace normalization. Carries the `this` tag alongside
    /// the element so emission never re-destructures the (guaranteed) kind.
    SvelteElement(&'arena SpecialElement<'arena>, &'arena SpecialThis<'arena>),
    /// A `<svelte:boundary>` — emitted as isolated `<!--[-->` / `<!--]-->` anchor
    /// pushes around a bare block statement, optionally wrapped in a
    /// `$$renderer.boundary({ failed }, ($$renderer) => …)` call
    /// ([`crate::blocks::emit_boundary`]). Like a block, it interrupts the template
    /// push stream, so it is a non-text, non-expr node for whitespace purposes.
    Boundary(&'arena SpecialElement<'arena>),
    If(&'arena IfBlock<'arena>),
    Each(&'arena EachBlock<'arena>),
    Await(&'arena AwaitBlock<'arena>),
    Key(&'arena KeyBlock<'arena>),
    Render(&'arena RenderTag<'arena>),
}

impl CleanNode<'_> {
    fn is_expr(&self) -> bool {
        // Only `{expr}` tags count as text for whitespace purposes — the
        // oracle's `prev?.type !== 'ExpressionTag'` checks; `{@html}` is a
        // regular non-text node there.
        matches!(self, CleanNode::Expr(_))
    }
}

/// Per-fragment emission context.
#[allow(clippy::struct_excessive_bools)] // independent per-fragment flags, not a state machine
pub(crate) struct FragmentCtx<'p> {
    /// Whether a text-first fragment gets the leading `<!---->` anchor. True for
    /// the component root and `{#each}` bodies (the oracle's `is_text_first`:
    /// parent ∈ {Fragment, SnippetBlock, EachBlock, Component, …}); false for
    /// element children and `{#if}`/`{#key}`/`{#await}` bodies.
    pub(crate) mark_text_first: bool,
    /// Whether this is the component's root fragment (a `{@const}` here refuses —
    /// grammatically block-only, and its component-scope placement is unprobed).
    pub(crate) is_component_root: bool,
    /// Whether this fragment is a **block scope** (component root, a block body,
    /// or a `<svelte:head>` closure) that owns snippet hoisting. The oracle
    /// hoists a `{#snippet}` to its nearest enclosing block scope, bubbling
    /// *through* elements (which share the block's `init`), so a block-scope
    /// fragment collects snippets from its whole element subtree and emits their
    /// `function` declarations at the front; an element-child fragment
    /// (`hoist_snippets = false`) leaves its snippets to the enclosing block.
    pub(crate) hoist_snippets: bool,
    /// The enclosing scope's `is_standalone` (the oracle's `clean_nodes`
    /// `is_standalone`, inherited by element children). A block scope recomputes
    /// it from its own trimmed list; an element child inherits it. When true, a
    /// sole `{@render}` reuses the parent block's anchor and emits no trailing
    /// `<!---->`. An element wrapping the render makes the enclosing block's sole
    /// child the element (not a render), so the inherited value is false.
    pub(crate) is_standalone: bool,
    /// Inside `<pre>`/`<textarea>`: no whitespace normalization.
    pub(crate) preserve_whitespace: bool,
    /// The enclosing element's name (`None` at the root, and `None` through a
    /// block/component/`<svelte:element>` body — matching the oracle's `parent`,
    /// which is the block node, not an element, for the select/table and svg
    /// `<text>` per-immediate-parent checks).
    pub(crate) parent_name: Option<&'p str>,
    /// This fragment's SSR namespace (Svelte's re-inferred `state.namespace`),
    /// which governs the svg whitespace-removal rule in [`normalize_whitespace`].
    pub(crate) namespace: Namespace,
    /// Whether the immediate parent or any ancestor is an svg `<text>` element —
    /// svg whitespace is kept inside `<text>` (the oracle's `parent.name !==
    /// 'text' && !path.some(text)` guard, collapsed to one ancestor-aware flag).
    pub(crate) in_svg_text: bool,
}

/// Walk a fragment: normalize whitespace per the oracle's `clean_nodes` rules,
/// then append static HTML / interpolations to the template.
pub(crate) fn emit_fragment<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let nodes: &'arena [FragmentNode<'arena>] = fragment.nodes;
    let source = env.source;

    // Decode and filter into the working list. Comments are dropped (the oracle
    // compiles with preserveComments off); `{@const}` tags are hoisted out of the
    // whitespace list and emitted first (the oracle's `clean_nodes` hoisting).
    let mut list: Vec<CleanNode<'arena>> = Vec::with_capacity(nodes.len());
    let mut const_tags: Vec<&'arena ConstTag<'arena>> = Vec::new();
    let mut head_nodes: Vec<&'arena SpecialElement<'arena>> = Vec::new();
    // `<title>` elements hoist to the front of their fragment exactly like
    // `<svelte:head>` — the oracle's `clean_nodes` lists `TitleElement` in the
    // hoisted set and its visitor pushes to `state.init` (emitted before all
    // template content), so a `<title>` always precedes its head siblings
    // regardless of source order, and never participates in surrounding
    // whitespace normalization.
    let mut title_nodes: Vec<&'arena SpecialElement<'arena>> = Vec::new();
    for node in nodes {
        match node {
            // Every special-element kind is dispatched here. The kinds the transform
            // does not emit refuse FIRST, keyed per variant by the shared
            // `special_element_refusal_kind` mapping the census reads too — so the
            // refusal set has one home. The dispatch below then stays an exhaustive
            // `match` on `se.kind` (a new variant fails compilation both there and in
            // the mapping, rather than silently falling through to a drop).
            FragmentNode::SpecialElement(se) => {
                if let Some(kind) = special_element_refusal_kind(&se.kind) {
                    return Err(unsupported(Refusal::TemplateNode { kind }));
                }
                match &se.kind {
                    SpecialElementKind::SvelteHead => head_nodes.push(se),
                    // `<title>` (only classified as a `TitleElement` inside
                    // `<svelte:head>`) hoists like a head node, emitting a
                    // `$$renderer.title(($$renderer) => …)` statement.
                    SpecialElementKind::TitleElement => title_nodes.push(se),
                    // The SSR-inert special elements: `<svelte:window>`/`<svelte:body>`/
                    // `<svelte:document>` compile to NOTHING (their events/binds are
                    // client-only, so the oracle emits no template output for them).
                    // They are still validated: the oracle runs its phase-2 analysis
                    // over placement, children, and every attribute — tsv's parser is
                    // permissive where the oracle rejects, so those checks live in
                    // `guard_inert_special_element` (children, illegal attributes,
                    // invalid binds, legacy directives) plus the placement/duplicate
                    // guards here. A NESTED one (legal only at the component root —
                    // `svelte_meta_invalid_placement`) and a DUPLICATE of the same kind
                    // (`svelte_meta_duplicate`) refuse.
                    kind @ (SpecialElementKind::SvelteWindow
                    | SpecialElementKind::SvelteBody
                    | SpecialElementKind::SvelteDocument) => {
                        let tag = se.kind.tag_name();
                        // Placement (`svelte_meta_invalid_placement`) and duplicate
                        // (`svelte_meta_duplicate`) are NOT checked here: both fire in
                        // a region SSR drops, which this emitter never visits, so they
                        // live in the upfront whole-document pass (`validate.rs`). Only
                        // the emission-state checks remain.
                        guard_inert_special_element(env, se, kind, tag)?;
                    }
                    // `<svelte:element this={…}>` compiles to a statement-level
                    // `$.element(…)` call — routed to the emit list like a component. The
                    // `this` tag is captured here (the one place the kind is
                    // destructured) so emission needs no impossible fallback.
                    SpecialElementKind::SvelteElement { tag } => {
                        list.push(CleanNode::SvelteElement(se, tag));
                    }
                    // `<svelte:boundary>` emits anchor pushes around a block
                    // statement (and, with a `failed` snippet, a
                    // `$$renderer.boundary(…)` call) — routed to the emit list like a
                    // block.
                    SpecialElementKind::SvelteBoundary => {
                        list.push(CleanNode::Boundary(se));
                    }
                    // Every other special element (`<svelte:component>`, `<svelte:self>`,
                    // `<slot>`, `<svelte:fragment>`, `<svelte:boundary>`) already refused
                    // above via the shared mapping. They are listed rather than folded
                    // into a catch-all so this dispatch stays exhaustive too — and the
                    // arm PANICS rather than falling through: if the mapping ever
                    // returned `None` for one of them, a silent `{}` would DROP the node
                    // from the output, which is content loss. Better a loud bug.
                    //
                    // KNOWN STATIC-SAFETY DEBT, consciously taken: routing the refusal
                    // through the shared mapping split ONE exhaustive match into two
                    // that must agree on which kinds are handled, so a disagreement the
                    // old single match made unrepresentable is now merely a runtime
                    // panic. The trade bought a single home for the refusal set (the
                    // census read a hand-mirrored copy before, and the two had already
                    // drifted); panicking rather than dropping keeps the worst case
                    // loud. Closing it statically would mean generating this dispatch's
                    // handled arms from the same table, which their per-kind bodies do
                    // not fit — not attempted.
                    SpecialElementKind::SvelteComponent { .. }
                    | SpecialElementKind::SvelteSelf
                    | SpecialElementKind::SlotElement
                    | SpecialElementKind::SvelteFragment => {
                        // A silent `{}` here would DROP the node from the output.
                        #[allow(clippy::unreachable)] // the mapping refused these above
                        {
                            unreachable!("refused above via special_element_refusal_kind")
                        }
                    }
                }
            }
            FragmentNode::Text(text) => {
                list.push(CleanNode::Text(text.data(source).into_owned()));
            }
            FragmentNode::Element(element) => list.push(CleanNode::Element(element)),
            FragmentNode::ExpressionTag(tag) => list.push(CleanNode::Expr(tag)),
            FragmentNode::HtmlTag(tag) => list.push(CleanNode::Html(tag)),
            FragmentNode::IfBlock(block) => list.push(CleanNode::If(block)),
            FragmentNode::EachBlock(block) => list.push(CleanNode::Each(block)),
            FragmentNode::AwaitBlock(block) => list.push(CleanNode::Await(block)),
            FragmentNode::KeyBlock(block) => list.push(CleanNode::Key(block)),
            FragmentNode::RenderTag(tag) => list.push(CleanNode::Render(tag)),
            // Snippets are hoisted to the enclosing block scope (below), not
            // emitted inline — skip them in the template list.
            FragmentNode::SnippetBlock(_) => {}
            FragmentNode::ConstTag(tag) => {
                if ctx.is_component_root {
                    return Err(unsupported(Refusal::ConstTagAtRoot));
                }
                const_tags.push(tag);
            }
            FragmentNode::Comment(_) => {}
            other => {
                return Err(unsupported(Refusal::TemplateNode {
                    kind: fragment_node_kind(other),
                }));
            }
        }
    }

    // Snippets hoist to their nearest enclosing block scope, bubbling through
    // elements (which share the block's `init`): a block-scope fragment collects
    // its whole element subtree's snippets and emits their `function`
    // declarations first (hoistable top-level ones to module scope, the rest to
    // this block's init); an element-child fragment leaves them to the block. The
    // collection order is recursive-direct-first — a fragment's own snippets
    // before its descendant elements' — mirroring the oracle's push-order timing
    // (Fragment.js:35-44 + RegularElement.js:229-231; see
    // `collect_hoisted_snippets`).
    let hoisted_snippets = if ctx.hoist_snippets {
        let mut collected: Vec<&'arena SnippetBlock<'arena>> = Vec::new();
        collect_hoisted_snippets(fragment, &mut collected);
        collected
    } else {
        Vec::new()
    };

    // Emit hoisted `{@const}` declarations first — they precede the anchor's
    // following content and enter the evaluator's innermost overlay so later
    // reads in this fragment fold. `<svelte:head>` is hoisted the same way; a
    // fragment carrying both can't fix their relative order (the oracle keeps
    // source order across all hoisted kinds), so refuse that combination.
    if !head_nodes.is_empty() && !const_tags.is_empty() {
        return Err(unsupported(Refusal::SvelteHeadWithConstTag));
    }
    // A snippet sharing a block with a `{@const}`/`<svelte:head>`/`<title>` can't
    // fix the relative hoist order across kinds — refuse the mix.
    if !hoisted_snippets.is_empty()
        && (!const_tags.is_empty() || !head_nodes.is_empty() || !title_nodes.is_empty())
    {
        return Err(unsupported(Refusal::SnippetHoistOrder));
    }
    for tag in &const_tags {
        emit_const_tag(env, tag, out)?;
    }
    for head in &head_nodes {
        emit_svelte_head(env, head, out)?;
    }
    for title in &title_nodes {
        emit_title_element(env, title, out)?;
    }
    for snippet in &hoisted_snippets {
        emit_snippet(env, snippet, out)?;
    }
    // Everything above is the oracle's `init` list; everything below is its
    // template stream. Only a block scope owns that split — an element-child
    // fragment shares the enclosing block's builder and leaves the mark alone.
    if ctx.hoist_snippets {
        out.mark_init_end();
    }

    if !ctx.preserve_whitespace {
        normalize_whitespace(&mut list, ctx.parent_name, ctx.namespace, ctx.in_svg_text);
    }

    // A lone leading newline text in <pre> is dropped (the browser would drop
    // it too, which would otherwise break hydration).
    if ctx.parent_name == Some("pre")
        && let Some(CleanNode::Text(data)) = list.first()
        && (data == "\n" || data == "\r\n")
    {
        list.remove(0);
    }

    // A text-first fragment gets a leading `<!---->` so its text doesn't glue to
    // the surrounding SSR fragment (component root and `{#each}` bodies only).
    if ctx.mark_text_first && matches!(list.first(), Some(CleanNode::Text(_) | CleanNode::Expr(_)))
    {
        out.push_text("<!---->");
    }

    // `is_standalone` (the oracle's `clean_nodes` flag) is recomputed at a block
    // scope from its own trimmed list — a sole non-dynamic `{@render}` reuses the
    // parent block's anchor — and *inherited* by element children (an element
    // wrapping the render already made the block's sole child the element, so the
    // inherited value is false).
    let is_standalone = if ctx.hoist_snippets {
        match list.as_slice() {
            // `render_callee_name` is a SHAPE predicate (it wants a plain-identifier
            // callee), so it must read the ERASED expression — the oracle strips
            // TypeScript before `clean_nodes` runs, and to it `{@render (s as T)(x)}`
            // is a plain `s(x)`. Reading the raw node would call it dynamic and emit
            // an anchor the oracle elides. `emit_render_tag` erases again for its own
            // borrow; a second erase of a type-free node allocates nothing.
            [CleanNode::Render(tag)] => {
                let expression = env.erase(&tag.expression)?;
                render_callee_name(expression, source)
                    .is_some_and(|name| !render_callee_dynamic(env, name))
            }
            [CleanNode::Element(element)] => component_is_standalone_eligible(env, element),
            _ => false,
        }
    } else {
        ctx.is_standalone
    };

    for node in &list {
        match node {
            CleanNode::Text(data) => {
                out.push_text(&escape_template_text(&escape_html_text(data)));
            }
            CleanNode::Element(element) => {
                emit_element(env, element, out, &ctx, is_standalone)?;
            }
            CleanNode::SvelteElement(se, tag) => {
                emit_svelte_element(env, se, tag, out, &ctx)?;
            }
            CleanNode::Boundary(se) => emit_boundary(env, se, out, &ctx)?,
            CleanNode::Expr(tag) => {
                emit_expression_tag(env, &tag.expression, out, true)?;
            }
            CleanNode::Html(tag) => {
                emit_expression_tag(env, &tag.expression, out, false)?;
            }
            CleanNode::If(block) => emit_if_block(env, block, out, &ctx)?,
            CleanNode::Each(block) => emit_each_block(env, block, out, &ctx)?,
            CleanNode::Await(block) => emit_await_block(env, block, out, &ctx)?,
            CleanNode::Key(block) => emit_key_block(env, block, out, &ctx)?,
            CleanNode::Render(tag) => emit_render_tag(env, tag, out, is_standalone)?,
        }
    }
    Ok(())
}

/// Emit a block body fragment into a fresh child body builder, prepending `pre`
/// statements (block anchor pushes, an `{#each}` binding) and pushing a
/// block-scope `overlay` (empty for `{#if}`/`{#key}`, seeded with masked locals
/// for `{#each}`/`{#await}`), and return the finished statement slice. The
/// overlay gives any `{@const}` in the body a scope to enter.
pub(crate) fn emit_child_body<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
    pre: &[Statement<'arena>],
    mark_text_first: bool,
    preserve_whitespace: bool,
    ns: ChildNamespace,
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
    // Re-infer this fragment's namespace from its own nodes (Svelte's `Fragment`
    // visitor), falling back to the inherited namespace.
    let namespace = infer_namespace(env, ns.inherited, ns.parent, fragment.nodes);
    env.push_overlay(overlay)?;
    let mut child = BodyBuilder::new_in(arena);
    for stmt in pre {
        child.stmts.push(stmt.clone());
    }
    let result = emit_fragment(
        env,
        fragment,
        &mut child,
        FragmentCtx {
            mark_text_first,
            is_component_root: false,
            hoist_snippets: true,
            is_standalone: false,
            preserve_whitespace,
            parent_name: None,
            namespace,
            in_svg_text: ns.in_svg_text,
        },
    );
    env.pop_overlay();
    result?;
    Ok(child.finish(&mut env.b, arena))
}

/// Collect the snippets that hoist to this block scope: the fragment's own
/// snippets plus those inside descendant **HTML elements** (which share the
/// block's `init`). Stops at nested blocks, special elements, and **components** —
/// those are separate scopes that collect their own (a component's snippet
/// children become its snippet props, handled by `plan_component_children`).
///
/// Emit ORDER is recursive-direct-first, not document pre-order: at each level
/// the fragment's OWN direct-child snippets come first in source order, THEN each
/// non-Component element subtree is recursed with the same rule. This mirrors the
/// oracle's push-order timing — `server/visitors/Fragment.js:35-44` drains a
/// fragment's own hoisted `SnippetBlock`s into `state.init` before it processes
/// the rest of its children, and the **transparent** branch of
/// `server/visitors/RegularElement.js:229-231` flattens each element's own `init`
/// onto the ancestor's shared array only as that element is reached during that
/// later walk — so a transparent element's snippets always follow the ancestor's
/// own. A **non-transparent** element takes the other branch (`:221-224`),
/// wrapping its `init` + `template` in a nested `b.block` pushed to the parent
/// template, so a snippet inside it stays sealed in that block rather than
/// hoisting to the block scope.
///
/// This collector recurses into EVERY non-Component element unconditionally, yet
/// can't diverge — because no oracle-accepted program reaches a non-transparent
/// element here. The only constructs that make a fragment non-transparent are all
/// unreachable at emission: a fragment-level declaration tag (`{ let … }`) is
/// refused (`Refusal::TemplateNode`, "declaration tag"), a `{@const}` directly in
/// a regular element is oracle-rejected (`const_tag_invalid_placement`), and an
/// async element is `experimental.async`-fenced.
fn collect_hoisted_snippets<'arena>(
    fragment: &Fragment<'arena>,
    out: &mut Vec<&'arena SnippetBlock<'arena>>,
) {
    // Phase 1: this fragment's own direct-child snippets, source order.
    for node in fragment.nodes {
        if let FragmentNode::SnippetBlock(snippet) = node {
            out.push(snippet);
        }
    }
    // Phase 2: each transparent (non-Component) element subtree, source order,
    // recursed with the same two-phase rule.
    for node in fragment.nodes {
        if let FragmentNode::Element(element) = node
            && element.kind != ElementKind::Component
        {
            collect_hoisted_snippets(&element.fragment, out);
        }
    }
}

/// Emit `{expr}` (escaped) or `{@html expr}` (raw) — the oracle's text-sequence
/// interpolation with its fold gate.
fn emit_expression_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
    out: &mut BodyBuilder<'arena>,
    escape: bool,
) -> Result<(), CompileError> {
    // The template borrow point: erase TypeScript ONCE, then feed the erased node
    // to the guard AND the fold gate below (see `EmitEnv::erase`).
    let expr = env.erase(expr)?;
    // Rewrite + guard FIRST (a derived read → `d()`, `$state.snapshot` →
    // `$.snapshot`; stray runes, a template mutation, and top-level await refuse) —
    // the evaluator must never fold an oracle-invalid expression.
    let wrapped = wrap_value_expr(env, expr)?;

    // The fold gate: a known evaluation folds into the static text.
    let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
        .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
    if let Some(value) = evaluated.known_value() {
        if !escape {
            // A statically-known `{@html}` would fold through the oracle's html
            // path — not probed/ported, refuse rather than guess.
            return Err(unsupported(Refusal::HtmlTagStaticValue));
        }
        let text =
            stringify_value(value).map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
        out.push_text(&escape_template_text(&escape_html_text(&text)));
        return Ok(());
    }

    let call = if escape {
        env.b.member_call("$", "escape", wrapped)
    } else {
        env.b.member_call("$", "html", wrapped)
    };
    out.push_expr(call);
    Ok(())
}

/// Emit a `<title>` (a `TitleElement` inside `<svelte:head>`) as
/// `$$renderer.title(($$renderer) => { $$renderer.push(`<title>…</title>`) })`.
///
/// The oracle hoists a `TitleElement` into `state.init` (why the caller emits it
/// with the other hoisted nodes) and processes its children with
/// `process_children` directly — **no** `clean_nodes`, so title children are not
/// whitespace-normalized the way an ordinary fragment's are. Its children are
/// guaranteed `Text`/`ExpressionTag` only; anything else is `title_invalid_content`
/// in the oracle's analysis phase, and any attribute is `title_illegal_attribute`.
/// tsv's parser is permissive about both, so each refuses here. A `{expr}` child
/// emits exactly like a regular element's text content: a statically-known value
/// folds, otherwise it emits `$.escape(expr)`.
fn emit_title_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    title: &'arena SpecialElement<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Any attribute on <title> is `title_illegal_attribute` in the oracle.
    if !title.attributes.is_empty() {
        return Err(unsupported(Refusal::TitleAttributes));
    }
    let arena = env.b.arena;
    let source = env.source;

    // Build `<title>` + children + `</title>` into a fresh body. Title children are
    // only text/expression, so this flushes to a single `$$renderer.push(…)`.
    let mut body = BodyBuilder::new_in(arena);
    body.push_text("<title>");
    for node in title.fragment.nodes {
        match node {
            FragmentNode::Text(text) => {
                let decoded = text.data(source);
                body.push_text(&escape_template_text(&escape_html_text(&decoded)));
            }
            FragmentNode::ExpressionTag(tag) => {
                emit_expression_tag(env, &tag.expression, &mut body, true)?;
            }
            // A non-text/expression child is `title_invalid_content` in the oracle.
            _ => return Err(unsupported(Refusal::TitleInvalidContent)),
        }
    }
    body.push_text("</title>");
    let body_stmts = body.finish(&mut env.b, arena);

    // Wrap in `$$renderer.title(($$renderer) => { … })` — the closure receives a
    // `$$renderer` parameter (like `$.head`'s closure), not the enclosing one.
    let here = env.b.here();
    let renderer_param = Expression::Identifier(env.b.ident("$$renderer"));
    let params = std::slice::from_ref(arena.alloc(renderer_param));
    let arrow = env.b.arrow_block(params, body_stmts, here);
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(arrow);
    let call = env
        .b
        .member_call("$$renderer", "title", args.into_bump_slice());
    let span = call.span();
    out.push_statement(
        &mut env.b,
        arena,
        Statement::ExpressionStatement(ExpressionStatement {
            expression: call,
            span,
            is_directive: false,
        }),
    );
    Ok(())
}

/// Parents whose whitespace-only children are removed entirely instead of
/// collapsing to a single space (Svelte's `can_remove_entirely` list).
const REMOVE_WS_ENTIRELY_PARENTS: &[&str] = &[
    "select", "tr", "table", "tbody", "thead", "tfoot", "colgroup", "datalist",
];

/// The oracle's whitespace normalization (Svelte `clean_nodes`, whitespace
/// pass): boundary whitespace-only nodes dropped and edge-text runs trimmed,
/// then each text node's edge runs abutting a non-text node collapse to one
/// space (or nothing after a whitespace-ending text) — runs abutting `{expr}`
/// tags stay, interior whitespace stays. An all-collapsed `" "` text is dropped
/// entirely under the `select`/`table`-family parents, and — the svg case — under
/// the `svg` namespace anywhere except inside a `<text>` element.
fn normalize_whitespace(
    list: &mut Vec<CleanNode<'_>>,
    parent_name: Option<&str>,
    namespace: Namespace,
    in_svg_text: bool,
) {
    // Boundary: drop whitespace-only text nodes, then trim the edge runs of a
    // surviving edge text node.
    while matches!(list.first(), Some(CleanNode::Text(t)) if is_ws_only(t)) {
        list.remove(0);
    }
    if let Some(CleanNode::Text(t)) = list.first_mut() {
        *t = replace_leading_ws(t, "");
    }
    while matches!(list.last(), Some(CleanNode::Text(t)) if is_ws_only(t)) {
        list.pop();
    }
    if let Some(CleanNode::Text(t)) = list.last_mut() {
        *t = replace_trailing_ws(t, "");
    }

    // The oracle's `can_remove_entirely`: under the svg namespace, collapsed
    // whitespace is dropped rather than kept as a space — EXCEPT inside a `<text>`
    // element (svg `<text>` preserves whitespace) — plus the `select`/`table`-family
    // parents.
    let can_remove_entirely = (namespace == Namespace::Svg && !in_svg_text)
        || parent_name.is_some_and(|name| REMOVE_WS_ENTIRELY_PARENTS.contains(&name));

    // Inner pass: mutate in place reading the (already-mutated) previous
    // neighbor, mirroring the oracle's in-place iteration; drops applied after
    // so neighbors keep indexing the pre-drop list.
    let mut drop_flags = vec![false; list.len()];
    for i in 0..list.len() {
        let prev_is_expr = i > 0 && list[i - 1].is_expr();
        let prev_text_ends_ws = i > 0
            && matches!(&list[i - 1], CleanNode::Text(t) if t.chars().next_back().is_some_and(is_template_ws));
        let next_is_expr = list.get(i + 1).is_some_and(CleanNode::is_expr);
        let has_next = i + 1 < list.len();

        let CleanNode::Text(data) = &mut list[i] else {
            continue;
        };
        if i > 0 && !prev_is_expr {
            *data = replace_leading_ws(data, if prev_text_ends_ws { "" } else { " " });
        }
        if has_next && !next_is_expr {
            *data = replace_trailing_ws(data, " ");
        }
        if data.is_empty() || (data == " " && can_remove_entirely) {
            drop_flags[i] = true;
        }
    }
    let mut keep = drop_flags.iter();
    list.retain(|_| !*keep.next().unwrap_or(&false));
}

pub(crate) fn fragment_node_kind(node: &FragmentNode<'_>) -> &'static str {
    match node {
        FragmentNode::Element(_) => "element",
        FragmentNode::SpecialElement(_) => "special element",
        FragmentNode::ExpressionTag(_) => "expression tag",
        FragmentNode::Text(_) => "text",
        FragmentNode::Comment(_) => "html comment",
        FragmentNode::IfBlock(_) => "{#if} block",
        FragmentNode::EachBlock(_) => "{#each} block",
        FragmentNode::AwaitBlock(_) => "{#await} block",
        FragmentNode::KeyBlock(_) => "{#key} block",
        FragmentNode::SnippetBlock(_) => "{#snippet} block",
        FragmentNode::HtmlTag(_) => "{@html} tag",
        FragmentNode::ConstTag(_) => "{@const} tag",
        FragmentNode::DeclarationTag(_) => "declaration tag",
        FragmentNode::DebugTag(_) => "{@debug} tag",
        FragmentNode::RenderTag(_) => "{@render} tag",
    }
}
