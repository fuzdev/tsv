//! Fragment walking, whitespace normalization, and template-value guards.
//!
//! [`emit_fragment`] is the core per-fragment loop: decode nodes into
//! [`CleanNode`], normalize whitespace per the oracle's `clean_nodes` rules,
//! then dispatch each surviving node to its emitter. [`BodyBuilder`] accumulates
//! the alternating static-text/interpolation template pending a
//! `$$renderer.push(…)` flush. The `guard_*`/`wrap_*` family prepares a borrowed
//! template expression for a synthetic call argument slot, guarding stray runes
//! and rewriting every derived read — bare or nested — to `d()`.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, AwaitBlock, ConstTag, EachBlock, Element,
    ElementKind, ExpressionTag, Fragment, FragmentNode, HtmlTag, IfBlock, KeyBlock, RenderTag,
    SnippetBlock, SpecialElement, SpecialElementKind, SpecialThis, StyleDirectiveValue,
};
use tsv_ts::ast::internal::{
    ArrayExpression, BinaryExpression, CallExpression, ConditionalExpression, Expression,
    ExpressionStatement, MemberExpression, NewExpression, ParenthesizedExpression,
    SequenceExpression, SpreadElement, Statement, TemplateLiteral, UnaryExpression,
};

use crate::analyze::{NameSet, ScopeEntry, evaluate, stringify_value};
use crate::attr_refs::{TemplateItem, each_template_item};
use crate::blocks::{
    emit_await_block, emit_const_tag, emit_each_block, emit_if_block, emit_key_block,
    emit_svelte_head,
};
use crate::build::{Builder, escape_template_text};
use crate::element::{component_is_standalone_eligible, emit_element, emit_svelte_element};
use crate::namespace::{ChildNamespace, Namespace, infer_namespace};
use crate::rune_guard::{WalkCtx, walk_expression_guarded};
use crate::snippet_emit::{
    emit_render_tag, emit_snippet, render_callee_dynamic, render_callee_name,
};
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// A statement body under construction: the statements emitted so far plus the
/// pending template accumulator (alternating static text and interpolation
/// expressions, `texts.len() == exprs.len() + 1` — the
/// [`Builder::template_literal`] shape). Control-flow blocks `flush` the
/// pending template into a `$$renderer.push(…)` statement, emit their own
/// statements, and let closer-anchor text accumulate into the next template —
/// the oracle's multi-push output shape.
pub(crate) struct BodyBuilder<'arena> {
    pub(crate) stmts: BumpVec<'arena, Statement<'arena>>,
    texts: Vec<String>,
    exprs: BumpVec<'arena, Expression<'arena>>,
}

impl<'arena> BodyBuilder<'arena> {
    pub(crate) fn new_in(arena: &'arena bumpalo::Bump) -> Self {
        Self {
            stmts: BumpVec::new_in(arena),
            texts: vec![String::new()],
            exprs: BumpVec::new_in(arena),
        }
    }

    /// Append an already template-escaped chunk to the current static part.
    ///
    /// **The cross-chunk `${` seam.** Each chunk is template-escaped on its own
    /// (`escape_template_text` rewrites `$` to `\$` only when it sees the `{`
    /// itself), so a literal `$` *ending* one chunk and a literal `{` *starting*
    /// the next slip through as a live interpolation — the emitted
    /// `` $$renderer.push(`… ${NAME} …`) `` would then evaluate `NAME`, or fail to
    /// parse. Real: `ssh ${'{'}DEPLOY_USER}` writes a shell variable by folding a
    /// `'{'` string literal into the text right after a `$`. The oracle escapes it
    /// (it assembles the whole string before escaping); tsv joins the seam here.
    pub(crate) fn push_text(&mut self, chunk: &str) {
        // Every element of `texts` exists by construction (starts with one entry;
        // `push_expr` appends the follower).
        #[allow(clippy::unwrap_used)]
        let current = self.texts.last_mut().unwrap();
        if current.ends_with('$') && chunk.starts_with('{') {
            // The trailing `$` is raw (any preceding backslash was already
            // doubled), so escaping it here is the identity escape `\$` — the
            // rendered text is unchanged, the interpolation is not.
            current.pop();
            current.push_str("\\$");
        }
        current.push_str(chunk);
    }

    pub(crate) fn push_expr(&mut self, expr: Expression<'arena>) {
        self.exprs.push(expr);
        self.texts.push(String::new());
    }

    /// Flush the pending template (if any) into a `$$renderer.push(…)`
    /// statement.
    fn flush(&mut self, b: &mut Builder<'arena>, arena: &'arena bumpalo::Bump) {
        if self.exprs.is_empty() && self.texts.iter().all(String::is_empty) {
            return;
        }
        let texts = std::mem::replace(&mut self.texts, vec![String::new()]);
        let exprs = std::mem::replace(&mut self.exprs, BumpVec::new_in(arena));
        let template = b.template_literal(&texts, exprs.into_bump_slice());
        let template_alloc = arena.alloc(template);
        let push_call = b.member_call("$$renderer", "push", std::slice::from_ref(template_alloc));
        let span = push_call.span();
        self.stmts
            .push(Statement::ExpressionStatement(ExpressionStatement {
                expression: push_call,
                span,
                is_directive: false,
            }));
    }

    /// Flush the pending template, then append a statement.
    pub(crate) fn push_statement(
        &mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
        stmt: Statement<'arena>,
    ) {
        self.flush(b, arena);
        self.stmts.push(stmt);
    }

    /// Finish: flush and return the statement slice.
    pub(crate) fn finish(
        mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
    ) -> &'arena [Statement<'arena>] {
        self.flush(b, arena);
        self.stmts.into_bump_slice()
    }
}

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

/// HTML-escape a static attribute value (`escape_html(value, true)`, `[&"<]`).
pub(crate) fn escape_html_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
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
    // The SSR-inert special-element tags (`svelte:window`/`svelte:body`/
    // `svelte:document`) already seen among this fragment's direct children — the
    // oracle allows at most one of each (`svelte_meta_duplicate`).
    let mut seen_inert: Vec<&'static str> = Vec::new();
    for node in nodes {
        match node {
            // Every special-element kind is dispatched here (exhaustive `match` on
            // `se.kind`, so a new `SpecialElementKind` variant fails compilation
            // rather than silently falling through to a refusal or a drop).
            FragmentNode::SpecialElement(se) => match &se.kind {
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
                    if !ctx.is_component_root {
                        return Err(unsupported(Refusal::SpecialElementInvalidPlacement {
                            name: tag.to_string(),
                        }));
                    }
                    if seen_inert.contains(&tag) {
                        return Err(unsupported(Refusal::DuplicateSpecialElement {
                            name: tag.to_string(),
                        }));
                    }
                    seen_inert.push(tag);
                    guard_inert_special_element(env, se, kind, tag)?;
                }
                // `<svelte:element this={…}>` compiles to a statement-level
                // `$.element(…)` call — routed to the emit list like a component. The
                // `this` tag is captured here (the one place the kind is
                // destructured) so emission needs no impossible fallback.
                SpecialElementKind::SvelteElement { tag } => {
                    list.push(CleanNode::SvelteElement(se, tag));
                }
                // Every other special element refuses (`<svelte:component>`,
                // `<svelte:self>`, `<slot>`, `<svelte:fragment>`,
                // `<svelte:boundary>`) — not emitted yet. The bucket key matches
                // `fragment_node_kind`'s "special element".
                SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteBoundary => {
                    return Err(unsupported(Refusal::TemplateNode {
                        kind: "special element",
                    }));
                }
            },
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
    // this block's init); an element-child fragment leaves them to the block.
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

/// Validate and guard-drop the attributes of an SSR-inert special element
/// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`). The element emits
/// NOTHING, but the oracle still runs its full phase-2 analysis over it, so every
/// shape the oracle rejects at analysis must refuse here (tsv's parser is
/// permissive where the oracle is strict). Mirrors the oracle's
/// `SvelteWindow`/`SvelteBody`/`SvelteDocument` visitors + `disallow_children` +
/// `BindDirective`:
///
/// - **children** (`disallow_children`): these cannot have children
///   (`svelte_meta_invalid_content`). tsv's parser *does* parse children into the
///   fragment, so a non-empty fragment refuses.
/// - **illegal attribute** (`illegal_element_attribute` /
///   `svelte_body_illegal_attribute`): only a **modern event attribute**
///   (`on*={expr}`, a single-expression value) is legal; a spread and every other
///   plain attribute refuse.
/// - **invalid bind** (`BindDirective` + `binding_properties`): a `bind:` is valid
///   iff its name is in the per-kind whitelist ([`inert_bind_is_valid`]) **and** its
///   target is a reassignable lvalue (`attribute::validate_inert_bind_target` — the
///   same `$state`-rooted fork regular elements use); otherwise refuse.
/// - **legacy directives**: a legacy `on:` event directive and `let:` are
///   runes-only-fence refusals (`NonPlainAttribute`), matching the regular-element
///   path — even though the oracle happens to accept `on:` here, tsv declines it (a
///   safe over-refusal).
/// - the **no-op drop family** (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/
///   `animate:`/`{@attach}`): oracle-accepted, so guard-and-drop each expression
///   (SSR runs no client lifecycle, but a stray rune / top-level `await` refuses).
fn guard_inert_special_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    se: &'arena SpecialElement<'arena>,
    kind: &SpecialElementKind<'arena>,
    tag: &'static str,
) -> Result<(), CompileError> {
    if !se.fragment.nodes.is_empty() {
        return Err(unsupported(Refusal::SpecialElementChildren {
            name: tag.to_string(),
        }));
    }
    // Exhaustive over `AttributeNode` so a new variant fails compilation here
    // rather than silently being dropped (or refused).
    for attr in se.attributes {
        match attr {
            AttributeNode::Attribute(a) => {
                if is_event_attribute(a, env.source) {
                    // A modern event handler drops from SSR output, but its
                    // expression is still guarded (a misplaced rune / top-level
                    // `await` is an oracle analysis-phase error).
                    if let Some([AttributeValue::ExpressionTag(t)]) = a.value {
                        guard_dropped(env, &t.expression)?;
                    }
                } else {
                    return Err(unsupported(Refusal::SpecialElementIllegalAttribute {
                        name: tag.to_string(),
                    }));
                }
            }
            AttributeNode::SpreadAttribute(_) => {
                return Err(unsupported(Refusal::SpecialElementIllegalAttribute {
                    name: tag.to_string(),
                }));
            }
            AttributeNode::BindDirective(d) => {
                let name = d.name_span.extract(env.source).to_string();
                // (1) the bind NAME must be in the per-kind whitelist, and (2) its
                // TARGET must be a reassignable lvalue — the SAME two-part rule
                // regular elements enforce (`attribute::validate_inert_bind_target`
                // reuses the shared `$state`-rooted lvalue fork), so a non-lvalue /
                // const / undefined / plain-`let` / prop target refuses just as the
                // oracle rejects it (`bind_invalid_expression` / `constant_binding` /
                // `bind_invalid_value`).
                if !inert_bind_is_valid(kind, &name) {
                    return Err(unsupported(Refusal::BindDirective { name }));
                }
                crate::attribute::validate_inert_bind_target(env, d)?;
                // The bind is dropped from SSR output but still guarded (a stray rune
                // / top-level `await`); its reassignment is collected in
                // `needs_context` so a later read of a `$state` target stays dynamic
                // (an unreassigned `$state` read otherwise folds to its init value).
                guard_dropped(env, &d.expression)?;
            }
            // Legacy `on:` event directive and `let:` — runes-only fence: refuse,
            // matching the regular-element path (`element.rs`).
            AttributeNode::OnDirective(_) | AttributeNode::LetDirective(_) => {
                return Err(unsupported(Refusal::NonPlainAttribute));
            }
            // The no-op drop family: guard-and-drop each expression, like a regular
            // element (the oracle accepts them on these elements and drops them).
            AttributeNode::ClassDirective(d) => guard_dropped(env, &d.expression)?,
            AttributeNode::UseDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::TransitionDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::AnimateDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::AttachTag(t) => guard_dropped(env, &t.expression)?,
            AttributeNode::StyleDirective(d) => match &d.value {
                StyleDirectiveValue::ExpressionTag(t) => guard_dropped(env, &t.expression)?,
                StyleDirectiveValue::Parts(parts) => {
                    for v in *parts {
                        if let AttributeValue::ExpressionTag(t) = v {
                            guard_dropped(env, &t.expression)?;
                        }
                    }
                }
                StyleDirectiveValue::True => {}
            },
        }
    }
    Ok(())
}

/// The oracle's `is_event_attribute` (`utils/ast.js`): a plain attribute whose
/// value is a single expression (`{expr}`) and whose RAW authored name starts with
/// `on`. tsv always wraps an attribute value in an array, so the oracle's
/// `is_expression_attribute` is exactly the single-`ExpressionTag` case here.
fn is_event_attribute(attr: &Attribute<'_>, source: &str) -> bool {
    attr.name_span.extract(source).starts_with("on")
        && matches!(attr.value, Some([AttributeValue::ExpressionTag(_)]))
}

/// Whether the `bind:<name>` NAME is valid on an SSR-inert special element — a
/// faithful SUBSET of the oracle's `binding_properties` rule (`BindDirective` +
/// `bindings.js`): `this`/`focused` are unrestricted, otherwise the name must be
/// in the element's `valid_elements` list. Deliberately over-refuses the extra
/// names the oracle also accepts on `<svelte:body>` (the dimension family —
/// `clientWidth`/`offsetWidth`/…) as a safe "not yet", never an over-acceptance.
/// The bind's TARGET (its lvalue/reassignability) is validated separately by
/// `attribute::validate_inert_bind_target`, which the caller runs next.
fn inert_bind_is_valid(kind: &SpecialElementKind<'_>, name: &str) -> bool {
    if name == "this" || name == "focused" {
        return true;
    }
    match kind {
        SpecialElementKind::SvelteWindow => matches!(
            name,
            "innerWidth"
                | "innerHeight"
                | "outerWidth"
                | "outerHeight"
                | "scrollX"
                | "scrollY"
                | "online"
                | "devicePixelRatio"
        ),
        SpecialElementKind::SvelteDocument => matches!(
            name,
            "activeElement" | "fullscreenElement" | "pointerLockElement" | "visibilityState"
        ),
        // `<svelte:body>` has no element-specific window/document binding; only the
        // unrestricted `this`/`focused` above. (The caller only passes an inert
        // kind, so the other arms are unreachable.)
        _ => false,
    }
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

/// Prepare a single borrowed value expression for a read position (`{#if}` test,
/// `{#each}` collection, `{#await}` promise): a derived read (bare or nested)
/// becomes `d()`, everything else is guarded and passed through borrowed.
pub(crate) fn wrap_single<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let wrapped = wrap_value_expr(env, expr)?;
    Ok(wrapped[0].clone())
}

/// The guard for an expression the SSR output **drops** — the `{#each}` key, the
/// `{#key}` expression, an event-handler attribute, and everything inside a
/// `{:catch}` branch.
///
/// A misplaced rune must still refuse: the oracle rejects one in its *analysis*
/// phase, before it decides what to emit, so dropping the region cannot make the
/// component valid (`{:catch e}{$state(1)}{/await}` is a `state_invalid_placement`
/// error there).
///
/// But the derived-read rule is switched **off** — the empty `derived_names` set.
/// It is an emission *rewrite* (a derived read, bare or nested, becomes `d()`;
/// `wrap_value_expr` applies it before the guard runs, so `walk_expression_guarded`
/// refuses every derived read that reaches it), not a validity rule: the oracle happily accepts
/// a derived read it never emits. Enforcing it in a dropped region would refuse
/// `{#key d}` and `{:catch e}<p>{d}</p>`, which the oracle compiles.
pub(crate) fn guard_dropped<'arena>(
    env: &EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    // The component-wide reassignment/shadow collection is `needs_context`'s job
    // (it walks the dropped regions too), so these two are throwaways.
    let no_derived = NameSet::default();
    // A valid `$name` store reference in a dropped region (an event handler,
    // `{:catch}`, an each/key expression) is allowed — the region isn't emitted,
    // so no rewrite is needed, and the `var $$store_subs` injection is already
    // decided by `needs_context`. A shadowed base still refuses here (the oracle's
    // `store_invalid_scoped_subscription`), so we pass the full shadow set.
    let mut ctx = WalkCtx::new(
        env.source,
        &mut updated,
        &mut nested,
        &no_derived,
        std::rc::Rc::clone(&env.b.interner),
    )
    .allow_store_reads(&env.store_names, Some(&env.store_shadowed));
    walk_expression_guarded(expr, &mut ctx)
}

/// The guard for a binding **pattern** the SSR output EMITS verbatim — the
/// `{#each}` context (`let CTX = each_array[i]`) and the `{:then}` value (the
/// then-arrow's parameter).
///
/// A pattern is not a dropped region: its *default values* are real emitted
/// expressions (`{#each xs as { a = d }}`), and this emitter borrows the pattern
/// through untouched — it never routes a pattern position through the value walk,
/// so it never rewrites a derived read inside one to `d()`. The derived rule stays
/// ON here (unlike [`guard_dropped`]), and the two pattern positions want it for
/// opposite reasons, both satisfied by one uniform "keep it on and refuse":
///
/// - the `{:then}` value default (`{#await p then {x = d}}`): the oracle emits
///   `({ x = d() })` — `d()`. Borrowing the pattern verbatim would emit a bare
///   `d` → a MISMATCH, so refusing is **mandatory** here.
/// - the `{#each}` context default (`{#each xs as {v = d}}`): the oracle emits
///   `let { v = d }` — a bare `d`. tsv *could* match by borrowing verbatim, but
///   keeps the rule ON as a **deferred safe over-refusal** (patterns are not
///   rewritten this slice; refusing is never a MISMATCH).
pub(crate) fn guard_pattern<'arena>(
    env: &EmitEnv<'arena, '_>,
    pattern: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(
        env.source,
        &mut updated,
        &mut nested,
        &env.derived_names,
        std::rc::Rc::clone(&env.b.interner),
    );
    walk_expression_guarded(pattern, &mut ctx)
}

/// Guard a whole fragment the SSR output drops (the `{:catch}` branch) — the
/// dropped-fragment analog of [`guard_dropped`], over `attr_refs`' shared
/// traversal. Without it a rune anywhere inside a `{:catch}` compiles, which the
/// oracle rejects: the M4 lesson (an emission-dropped fragment still needs
/// refusal-equivalent walking), and the property `dropped_fragments_are_walked`
/// pins.
pub(crate) fn guard_dropped_fragment<'arena>(
    env: &EmitEnv<'arena, '_>,
    fragment: &'arena Fragment<'arena>,
) -> Result<(), CompileError> {
    each_template_item(fragment, &mut |item| match item {
        TemplateItem::Expression(expr) => guard_dropped(env, expr),
        // A `<T>` clause holds no value reference. TypeScript in a document with
        // no `lang="ts"` is refused up front by `refuse_template_typescript`.
        TemplateItem::SnippetTypeParameters => Ok(()),
    })
}

/// Collect the snippets that hoist to this block scope: the fragment's own
/// snippets plus those inside descendant **HTML elements** (which share the
/// block's `init`). Stops at nested blocks, special elements, and **components** —
/// those are separate scopes that collect their own (a component's snippet
/// children become its snippet props, handled by `plan_component_children`).
fn collect_hoisted_snippets<'arena>(
    fragment: &Fragment<'arena>,
    out: &mut Vec<&'arena SnippetBlock<'arena>>,
) {
    for node in fragment.nodes {
        match node {
            FragmentNode::SnippetBlock(snippet) => out.push(snippet),
            FragmentNode::Element(element) if element.kind != ElementKind::Component => {
                collect_hoisted_snippets(&element.fragment, out);
            }
            _ => {}
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
pub(crate) fn emit_title_element<'arena>(
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

/// Prepare a borrowed value expression for a synthetic call argument slot — the
/// emitter's **item-6 template-value walk**, the single home every template value
/// position routes through ([`emit_expression_tag`], [`wrap_single`], the
/// attribute/spread/component-prop borrow points). It:
///
/// - rewrites every read of a `$derived` binding — bare (`{d}`) or nested at any
///   depth (`{d + 1}`, `{obj[d]}`, `{f(d)}`, `{d.x}`) — to the derived-thunk call
///   `d()`;
/// - rewrites every `$state.snapshot(x)` sub-node it descends into
///   `$.snapshot(<processed x>)`, processing the argument as a value in turn (a
///   derived arg → `d()`, a nested snapshot → `$.snapshot(...)`); and
/// - guards everything else and passes it through borrowed — stray runes,
///   top-level await, a template mutation, and a derived read or `$state.snapshot`
///   under a node kind this walk does not descend (an `ObjectExpression`, an
///   arrow, a tagged template) all refuse there (a safe over-refusal).
///
/// It rebuilds only the spine down to each rewrite target; a target-free subtree
/// stays on the guarded fast path, byte-identical to before (and does no extra
/// allocation).
pub(crate) fn wrap_value_expr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<&'arena [Expression<'arena>], CompileError> {
    Ok(std::slice::from_ref(rewrite_template_value(env, expr)?))
}

/// Whether `expr` is a bare read of a `$derived` binding — a plain (non-escaped)
/// identifier whose name is in `derived_names`. Such a read rewrites to the
/// derived-thunk call `d()` at every value position, at any depth.
pub(crate) fn is_bare_derived_read(
    source: &str,
    derived_names: &NameSet,
    expr: &Expression<'_>,
) -> bool {
    if let Expression::Identifier(id) = expr
        && id.escaped_name.is_none()
    {
        let start = id.span.start as usize;
        let name = &source[start..start + id.name_len as usize];
        return derived_names.contains(name);
    }
    false
}

/// Whether `expr` is a bare read of a store binding — a plain (non-escaped)
/// `$`-prefixed identifier whose `$`-stripped base is a top-level binding
/// (`store_names`). Such a read rewrites to
/// `$.store_get(($$store_subs ??= {}), '$name', name)` at every value position,
/// at any depth. Returns the base store name (the `$`-stripped variable). A
/// `$name` whose base is NOT a binding is the oracle's `global_reference_invalid`
/// error, so it is left for the guard to refuse (a safe over-refusal); an escaped
/// `$`-identifier is likewise left refused (its decoded base can't be read here).
fn bare_store_read(source: &str, store_names: &NameSet, expr: &Expression<'_>) -> Option<String> {
    let Expression::Identifier(id) = expr else {
        return None;
    };
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    let name = &source[start..start + id.name_len as usize];
    let base = crate::analyze::store_read_base(name)?;
    store_names.contains(base).then(|| base.to_string())
}

/// The recursive core of [`wrap_value_expr`]: rewrite one value expression,
/// returning the borrowed input unchanged when nothing needs rewriting (after
/// guarding it).
fn rewrite_template_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<&'arena Expression<'arena>, CompileError> {
    // A bare read of a derived binding becomes `d()`.
    if is_bare_derived_read(env.source, &env.derived_names, expr) {
        let call = env.b.call_expr(expr, &[]);
        return Ok(env.b.arena.alloc(call));
    }
    // A bare read of a store binding becomes
    // `$.store_get(($$store_subs ??= {}), '$name', name)`. The `var $$store_subs`
    // / `$.unsubscribe_stores` injection is analysis-driven (`EmitEnv::uses_stores`,
    // set upfront by `needs_context`), NOT flagged here. A store base shadowed by a
    // block-local (an `{#each}`/`{#await}`/snippet binding) is NOT the top-level
    // store — the oracle errors `store_invalid_scoped_subscription`, so it is left
    // for the guard to refuse (a safe refusal). A `$derived` base reads `d()` (the
    // store the derived currently holds).
    if let Some(base) = bare_store_read(env.source, &env.store_names, expr)
        && !env
            .overlays
            .iter()
            .any(|overlay| overlay.contains_key(&base))
    {
        let call = env.b.store_get(&base, env.derived_names.contains(&base));
        return Ok(env.b.arena.alloc(call));
    }
    // A `$state.snapshot(x)` call → `$.snapshot(<processed x>)`.
    if let Some(arg) = snapshot_call_arg(env.source, expr) {
        let processed = rewrite_template_value(env, arg)?;
        let call = env
            .b
            .member_call("$", "snapshot", std::slice::from_ref(processed));
        return Ok(env.b.arena.alloc(call));
    }
    // No rewrite target in this subtree: guard it whole and pass through borrowed
    // — the guarded fast path, so every target-free template expression keeps its
    // exact behavior (and does no extra allocation).
    if !contains_rewrite_target(env.source, &env.derived_names, &env.store_names, expr) {
        guard_template_value(env, expr)?;
        return Ok(expr);
    }
    // A rewrite target (a nested derived read or `$state.snapshot`) sits inside a
    // wrapper — rebuild along the spine.
    rebuild_value(env, expr)
}

/// Guard a snapshot-free template value expression (the pre-item-6 behavior):
/// stray runes, non-bare derived reads, and top-level await refuse, and a
/// mutation refuses via [`Refusal::MutationInTemplateExpr`] (a mutation would
/// postdate the binding analysis the fold already consulted).
fn guard_template_value<'arena>(
    env: &EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(
        env.source,
        &mut updated,
        &mut nested,
        &env.derived_names,
        std::rc::Rc::clone(&env.b.interner),
    );
    walk_expression_guarded(expr, &mut ctx)?;
    if !updated.is_empty() {
        return Err(unsupported(Refusal::MutationInTemplateExpr));
    }
    Ok(())
}

/// If `expr` is a `$state.snapshot(x)` call (the `$state.snapshot` keypath with
/// exactly one argument), the argument `x`. Shares [`crate::analyze::callee_keypath`]
/// with the declarator classifier, so template and script recognize it identically.
fn snapshot_call_arg<'arena>(
    source: &str,
    expr: &'arena Expression<'arena>,
) -> Option<&'arena Expression<'arena>> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    if call.arguments.len() != 1
        || crate::analyze::callee_keypath(call.callee, source).as_deref() != Some("$state.snapshot")
    {
        return None;
    }
    call.arguments.first()
}

/// Whether `expr` contains a rewrite target — a bare `$derived` read (→ `d()`),
/// a bare store read (→ `$.store_get(...)`), or a `$state.snapshot(...)` call
/// (→ `$.snapshot(...)`) — anywhere this walk descends. A false negative is safe:
/// the target then reaches the rune guard, which refuses it (a safe
/// over-refusal). Descends exactly the wrapper node kinds [`rebuild_value`]
/// rebuilds — the two stay in lockstep on one node set.
fn contains_rewrite_target(
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    expr: &Expression<'_>,
) -> bool {
    if is_bare_derived_read(source, derived_names, expr) {
        return true;
    }
    if bare_store_read(source, store_names, expr).is_some() {
        return true;
    }
    if snapshot_call_arg(source, expr).is_some() {
        return true;
    }
    let contains =
        |e: &Expression<'_>| contains_rewrite_target(source, derived_names, store_names, e);
    match expr {
        Expression::CallExpression(c) => contains(c.callee) || c.arguments.iter().any(&contains),
        Expression::NewExpression(n) => contains(n.callee) || n.arguments.iter().any(&contains),
        Expression::BinaryExpression(b) => contains(b.left) || contains(b.right),
        Expression::MemberExpression(m) => {
            contains(m.object) || (m.computed && contains(m.property))
        }
        Expression::ConditionalExpression(c) => {
            contains(c.test) || contains(c.consequent) || contains(c.alternate)
        }
        Expression::UnaryExpression(u) => contains(u.argument),
        Expression::ParenthesizedExpression(p) => contains(p.expression),
        Expression::SequenceExpression(s) => s.expressions.iter().any(&contains),
        Expression::SpreadElement(s) => contains(s.argument),
        Expression::ArrayExpression(a) => {
            a.elements.iter().any(|e| e.as_ref().is_some_and(&contains))
        }
        Expression::TemplateLiteral(t) => t.expressions.iter().any(&contains),
        _ => false,
    }
}

/// Rebuild a value expression along the spine down to each nested rewrite target
/// (a `$state.snapshot(...)` call or a bare `$derived` read), recursing
/// [`rewrite_template_value`] on every value-position child (a target-free child
/// re-enters the guarded fast path). A node kind this match does not cover falls
/// through to the guard, which refuses the target it carries — a safe
/// over-refusal.
fn rebuild_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<&'arena Expression<'arena>, CompileError> {
    let rebuilt = match expr {
        // A `$`-rooted (non-snapshot) callee refuses via the recursive guard on the
        // callee itself, so no explicit rune check is needed here.
        Expression::CallExpression(call) => {
            let callee = rewrite_template_value(env, call.callee)?;
            let arguments = rewrite_value_slice(env, call.arguments)?;
            Expression::CallExpression(CallExpression {
                callee,
                type_arguments: None,
                arguments,
                ..call.clone()
            })
        }
        Expression::NewExpression(new) => {
            let callee = rewrite_template_value(env, new.callee)?;
            let arguments = rewrite_value_slice(env, new.arguments)?;
            Expression::NewExpression(NewExpression {
                callee,
                type_arguments: None,
                arguments,
                span: new.span,
            })
        }
        Expression::BinaryExpression(b) => {
            let left = rewrite_template_value(env, b.left)?;
            let right = rewrite_template_value(env, b.right)?;
            Expression::BinaryExpression(BinaryExpression {
                left,
                right,
                ..b.clone()
            })
        }
        Expression::MemberExpression(m) => {
            let object = rewrite_template_value(env, m.object)?;
            // A non-computed property is a NAME, never a value read — leave it.
            let property = if m.computed {
                rewrite_template_value(env, m.property)?
            } else {
                m.property
            };
            Expression::MemberExpression(MemberExpression {
                object,
                property,
                ..m.clone()
            })
        }
        Expression::ConditionalExpression(c) => {
            let test = rewrite_template_value(env, c.test)?;
            let consequent = rewrite_template_value(env, c.consequent)?;
            let alternate = rewrite_template_value(env, c.alternate)?;
            Expression::ConditionalExpression(ConditionalExpression {
                test,
                consequent,
                alternate,
                span: c.span,
            })
        }
        Expression::UnaryExpression(u) => {
            let argument = rewrite_template_value(env, u.argument)?;
            Expression::UnaryExpression(UnaryExpression {
                argument,
                ..u.clone()
            })
        }
        Expression::ParenthesizedExpression(p) => {
            let expression = rewrite_template_value(env, p.expression)?;
            Expression::ParenthesizedExpression(ParenthesizedExpression {
                expression,
                span: p.span,
            })
        }
        Expression::SequenceExpression(s) => {
            let expressions = rewrite_value_slice(env, s.expressions)?;
            Expression::SequenceExpression(SequenceExpression {
                expressions,
                span: s.span,
            })
        }
        Expression::SpreadElement(s) => {
            let argument = rewrite_template_value(env, s.argument)?;
            Expression::SpreadElement(SpreadElement {
                argument,
                span: s.span,
            })
        }
        Expression::ArrayExpression(a) => {
            let elements = rewrite_opt_slice(env, a.elements)?;
            Expression::ArrayExpression(ArrayExpression {
                elements,
                ..a.clone()
            })
        }
        Expression::TemplateLiteral(t) => {
            let expressions = rewrite_value_slice(env, t.expressions)?;
            Expression::TemplateLiteral(TemplateLiteral {
                expressions,
                ..t.clone()
            })
        }
        _ => {
            guard_template_value(env, expr)?;
            return Ok(expr);
        }
    };
    Ok(env.b.arena.alloc(rebuilt))
}

/// Rewrite each expression of a slice (call arguments, sequence, template
/// expressions), returning a fresh arena slice (shallow clones — pointers, never
/// subtrees).
fn rewrite_value_slice<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    exprs: &'arena [Expression<'arena>],
) -> Result<&'arena [Expression<'arena>], CompileError> {
    let arena = env.b.arena;
    let mut out: BumpVec<'arena, Expression<'arena>> =
        BumpVec::with_capacity_in(exprs.len(), arena);
    for expr in exprs {
        out.push(rewrite_template_value(env, expr)?.clone());
    }
    Ok(out.into_bump_slice())
}

/// Rewrite each present element of an array-element slice (`[a, , b]` holes stay
/// `None`), returning a fresh arena slice.
fn rewrite_opt_slice<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    elements: &'arena [Option<Expression<'arena>>],
) -> Result<&'arena [Option<Expression<'arena>>], CompileError> {
    let arena = env.b.arena;
    let mut out: BumpVec<'arena, Option<Expression<'arena>>> =
        BumpVec::with_capacity_in(elements.len(), arena);
    for element in elements {
        match element {
            Some(expr) => out.push(Some(rewrite_template_value(env, expr)?.clone())),
            None => out.push(None),
        }
    }
    Ok(out.into_bump_slice())
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
