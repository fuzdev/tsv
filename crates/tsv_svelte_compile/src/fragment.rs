//! Fragment walking, whitespace normalization, and template-value guards.
//!
//! [`emit_fragment`] is the core per-fragment loop: decode nodes into
//! [`CleanNode`], normalize whitespace per the oracle's `clean_nodes` rules,
//! then dispatch each surviving node to its emitter. [`BodyBuilder`] accumulates
//! the alternating static-text/interpolation template pending a
//! `$$renderer.push(…)` flush. The `guard_*`/`wrap_*` family prepares a borrowed
//! template expression for a synthetic call argument slot, guarding stray runes
//! and rewriting a bare derived read to `d()`.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_svelte::ast::internal::{
    AwaitBlock, ConstTag, EachBlock, Element, ElementKind, ExpressionTag, Fragment, FragmentNode,
    HtmlTag, IfBlock, KeyBlock, RenderTag, SnippetBlock, SpecialElement, SpecialElementKind,
};
use tsv_ts::ast::internal::{Expression, ExpressionStatement, Statement};

use crate::analyze::{NameSet, ScopeEntry, evaluate, stringify_value};
use crate::attr_refs::{
    TemplateItem, each_reference_bearing_attribute_expression, each_template_item, fragment_any,
};
use crate::blocks::{
    emit_await_block, emit_const_tag, emit_each_block, emit_if_block, emit_key_block,
    emit_svelte_head,
};
use crate::build::{Builder, escape_template_text};
use crate::element::{component_is_standalone_eligible, emit_element};
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
    /// The enclosing element's name (`None` at the root).
    pub(crate) parent_name: Option<&'p str>,
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
    // The SSR-inert special-element tags (`svelte:window`/`svelte:body`/
    // `svelte:document`) already seen among this fragment's direct children — the
    // oracle allows at most one of each (`svelte_meta_duplicate`).
    let mut seen_inert: Vec<&'static str> = Vec::new();
    for node in nodes {
        match node {
            FragmentNode::SpecialElement(se)
                if matches!(se.kind, SpecialElementKind::SvelteHead) =>
            {
                head_nodes.push(se);
            }
            // The SSR-inert special elements: `<svelte:window>`/`<svelte:body>`/
            // `<svelte:document>` compile to NOTHING (their events/binds are
            // client-only, so the oracle emits no template output for them) and are
            // parser-guaranteed childless. Emit nothing, but still guard-and-drop
            // each attribute expression so a stray rune / top-level `await` refuses,
            // exactly as a dropped event handler on a regular element does. Two
            // invalid-input shapes the oracle rejects at analysis (tsv's parser is
            // permissive about both, so the guard lives here): a NESTED one (legal
            // only at the component root — `svelte_meta_invalid_placement`) and a
            // DUPLICATE of the same kind (`svelte_meta_duplicate`).
            FragmentNode::SpecialElement(se)
                if matches!(
                    se.kind,
                    SpecialElementKind::SvelteWindow
                        | SpecialElementKind::SvelteBody
                        | SpecialElementKind::SvelteDocument
                ) =>
            {
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
                let mut attr_exprs: Vec<&'arena Expression<'arena>> = Vec::new();
                each_reference_bearing_attribute_expression(se.attributes, &mut |e| {
                    attr_exprs.push(e);
                });
                for expr in attr_exprs {
                    guard_dropped(env, expr)?;
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
    // A snippet sharing a block with a `{@const}`/`<svelte:head>` can't fix the
    // relative hoist order across kinds — refuse the mix.
    if !hoisted_snippets.is_empty() && (!const_tags.is_empty() || !head_nodes.is_empty()) {
        return Err(unsupported(Refusal::SnippetHoistOrder));
    }
    for tag in &const_tags {
        emit_const_tag(env, tag, out)?;
    }
    for head in &head_nodes {
        emit_svelte_head(env, head, out)?;
    }
    for snippet in &hoisted_snippets {
        emit_snippet(env, snippet, out)?;
    }

    if !ctx.preserve_whitespace {
        normalize_whitespace(&mut list, ctx.parent_name);
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

/// Recursively test whether a fragment contains any control-flow block or
/// `{@const}` tag (the comments+blocks refusal gate). Rides the shared
/// child-fragment seam ([`fragment_any`]), so it descends every sub-fragment —
/// `SpecialElement` included — like its `fragment_has_*` siblings.
pub(crate) fn fragment_contains_block(fragment: &Fragment<'_>) -> bool {
    fragment_any(fragment, &|node| {
        matches!(
            node,
            FragmentNode::IfBlock(_)
                | FragmentNode::EachBlock(_)
                | FragmentNode::AwaitBlock(_)
                | FragmentNode::KeyBlock(_)
                | FragmentNode::ConstTag(_)
        )
    })
}

/// Recursively test whether a fragment contains a `{#snippet}` or `{@render}`
/// (the comments+snippet/render refusal gate — a hoisted snippet function or a
/// per-render flush reshapes the body in ways whose comment windows aren't
/// probed). Rides the shared child-fragment seam ([`fragment_any`]).
pub(crate) fn fragment_has_snippet_or_render(fragment: &Fragment<'_>) -> bool {
    fragment_any(fragment, &|node| {
        matches!(
            node,
            FragmentNode::SnippetBlock(_) | FragmentNode::RenderTag(_)
        )
    })
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
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
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
        },
    );
    env.pop_overlay();
    result?;
    Ok(child.finish(&mut env.b, arena))
}

/// Prepare a single borrowed value expression for a read position (`{#if}` test,
/// `{#each}` collection, `{#await}` promise): a bare derived read becomes `d()`,
/// everything else is guarded and passed through borrowed.
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
/// It is an emission *rewrite* (a bare read becomes `d()`; `wrap_value_expr`
/// applies it before the guard runs, so `walk_expression_guarded` refuses every
/// derived read that reaches it), not a validity rule: the oracle happily accepts
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
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &no_derived);
    walk_expression_guarded(expr, &mut ctx)
}

/// The guard for a binding **pattern** the SSR output EMITS verbatim — the
/// `{#each}` context (`let CTX = each_array[i]`) and the `{:then}` value (the
/// then-arrow's parameter).
///
/// A pattern is not a dropped region: its *default values* are real emitted
/// expressions (`{#each xs as { a = d }}`), and this emitter borrows the pattern
/// through untouched — it never rewrites a derived read inside one to `d()` the
/// way `wrap_value_expr` does for a value position. So the derived rule stays ON
/// here (unlike [`guard_dropped`]): a derived read in a pattern default would
/// otherwise emit a bare `d` where the oracle emits `d()`. A MISMATCH, so refuse.
pub(crate) fn guard_pattern<'arena>(
    env: &EmitEnv<'arena, '_>,
    pattern: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
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
    // Guard FIRST (stray runes, non-bare derived reads, template mutations
    // refuse) — the evaluator must never fold an oracle-invalid expression.
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

/// Prepare a borrowed value expression for a synthetic call argument slot:
/// a bare read of a derived binding becomes `d()`; anything else is guarded
/// (stray runes, non-bare derived reads, and template mutations refuse) and
/// passed through borrowed.
pub(crate) fn wrap_value_expr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<&'arena [Expression<'arena>], CompileError> {
    if let Expression::Identifier(id) = expr
        && id.escaped_name.is_none()
    {
        let start = id.span.start as usize;
        let name = &env.source[start..start + id.name_len as usize];
        if env.derived_names.contains(name) {
            let call = env.b.call_expr(expr, &[]);
            let call_alloc = env.b.arena.alloc(call);
            return Ok(std::slice::from_ref(call_alloc));
        }
    }
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
    walk_expression_guarded(expr, &mut ctx)?;
    if !updated.is_empty() {
        // A mutation here would postdate the binding analysis the fold already
        // consulted — refuse rather than fold stale.
        return Err(unsupported(Refusal::MutationInTemplateExpr));
    }
    Ok(std::slice::from_ref(expr))
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
/// entirely under the `select`/`table`-family parents.
fn normalize_whitespace(list: &mut Vec<CleanNode<'_>>, parent_name: Option<&str>) {
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

    let can_remove_entirely =
        parent_name.is_some_and(|name| REMOVE_WS_ENTIRELY_PARENTS.contains(&name));

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
