//! Control-flow block emission: `{#if}`, `{#each}`, `{#await}`, `{#key}`,
//! `{@const}`, and `<svelte:head>`.
//!
//! Each block splits the single accumulating template into multiple
//! `$$renderer.push(…)` statements, emitting its own statements between
//! flushes and merging its opener/closer anchors into the adjacent template —
//! the oracle's multi-push output shape.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_svelte::ast::internal::{
    AwaitBlock, ConstTag, EachBlock, Element, Fragment, FragmentNode, IfBlock, KeyBlock,
    SpecialElement,
};
use tsv_ts::ast::internal::{
    BinaryOperator, BlockStatement, Expression, ExpressionStatement, ForInit, ForStatement,
    IfStatement, Statement, UpdateOperator, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};

use crate::analyze::{Binding, BindingKind, Initial, ScopeEntry, pattern_binding_names};
use crate::build::Builder;
use crate::fragment::{
    BodyBuilder, FragmentCtx, emit_child_body, guard_dropped, guard_dropped_fragment,
    guard_pattern, wrap_single,
};
use crate::namespace::{ChildNamespace, FragmentParent, Namespace};
use crate::script_rewrite::plain_identifier_name;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Refuse if a generated block name (`each_array`, `$$index`, `$$length`) would
/// collide with a user binding — the oracle's component-scope name generation
/// would then pick a different suffix, which this port doesn't replicate.
fn check_name_free(env: &EmitEnv<'_, '_>, name: &str) -> Result<(), CompileError> {
    if env.bindings.contains(name) || env.snippets.names.contains(name) {
        return Err(unsupported(Refusal::GeneratedNameCollision {
            name: name.to_string(),
        }));
    }
    Ok(())
}

/// The namespace context a control-flow block body inherits: the enclosing
/// fragment's namespace as the fallback, re-inferred from the body's own DIRECT
/// children (a block is not in the oracle's deep-walk special list, so only the
/// shallow loop runs), carrying the svg-`<text>` flag through.
fn block_child_ns(ctx: &FragmentCtx<'_>) -> ChildNamespace {
    ChildNamespace {
        inherited: ctx.namespace,
        parent: FragmentParent::Block,
        in_svg_text: ctx.in_svg_text,
    }
}

/// A single-declarator `let`/`const` declaration statement.
/// The filename the deterministic oracle compiles under. `$.head`'s dedup hash
/// is `hash(filename)` (`SvelteHead.js`), so a fixed filename makes it constant.
const COMPILE_FILENAME: &str = "input.svelte";

/// Port of Svelte's `hash` (`utils.js`): strip carriage returns, then a djb2-xor
/// fold over the code units in reverse, rendered base-36. Used for `$.head`.
///
/// Folds over `chars()` (code points) where the oracle uses `charCodeAt` (UTF-16
/// code units) — identical for BMP input, divergent for astral characters. Safe
/// at the only call site (the ASCII [`COMPILE_FILENAME`] constant); revisit if a
/// real filename ever feeds this.
fn svelte_hash(s: &str) -> String {
    let mut hash: u32 = 5381;
    for ch in s.chars().rev() {
        if ch == '\r' {
            continue;
        }
        hash = hash.wrapping_shl(5).wrapping_sub(hash) ^ (ch as u32);
    }
    if hash == 0 {
        return "0".to_string();
    }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = String::new();
    while hash > 0 {
        out.push(DIGITS[(hash % 36) as usize] as char);
        hash /= 36;
    }
    out.chars().rev().collect()
}

/// Emit `$.head(hash, $$renderer, ($$renderer) => { … })` for `<svelte:head>`.
///
/// The head fragment is a normal fragment emitted into the closure body, so a
/// `<title>` (a `TitleElement`) hoists there and emits its own
/// `$$renderer.title(($$renderer) => …)` statement (`fragment.rs`), while any other
/// unsupported special child refuses through the usual `emit_fragment` path.
/// Attributes on the head element are refused (the oracle carries none in this
/// subset).
pub(crate) fn emit_svelte_head<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    head: &'arena SpecialElement<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    if !head.attributes.is_empty() {
        return Err(unsupported(Refusal::SvelteHeadAttributes));
    }
    // The head body: a normal fragment (not text-first-marked) in the closure.
    // `<svelte:head>` is not in the oracle's deep-walk special list, and its
    // content is html at the component root.
    let body = emit_child_body(
        env,
        &head.fragment,
        &[],
        false,
        false,
        ChildNamespace {
            inherited: Namespace::Html,
            parent: FragmentParent::Block,
            in_svg_text: false,
        },
        HashMap::new(),
    )?;
    let here = env.b.here();
    let renderer_param = Expression::Identifier(env.b.ident("$$renderer"));
    let params = std::slice::from_ref(arena.alloc(renderer_param));
    let arrow = env.b.arrow_block(params, body, here);

    // `$.head('<hash>', $$renderer, arrow)`.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(env.b.string_literal_expr(&svelte_hash(COMPILE_FILENAME)));
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(arrow);
    let call = env.b.member_call("$", "head", args.into_bump_slice());
    let span = call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    Ok(())
}

pub(crate) fn declaration_stmt<'arena>(
    b: &Builder<'arena>,
    kind: VariableDeclarationKind,
    id: Expression<'arena>,
    init: Expression<'arena>,
) -> Statement<'arena> {
    let span = Span::new(id.span().start, init.span().end);
    let declarator = VariableDeclarator {
        id,
        init: Some(init),
        definite: false,
        span,
    };
    let decls = std::slice::from_ref(b.arena.alloc(declarator));
    Statement::VariableDeclaration(VariableDeclaration {
        kind,
        declarations: decls,
        declare: false,
        span,
    })
}

/// Wrap a finished statement slice in a `Statement::BlockStatement` (`{ … }`).
fn block_stmt<'arena>(
    b: &Builder<'arena>,
    body: &'arena [Statement<'arena>],
) -> &'arena Statement<'arena> {
    let span = b.here();
    b.arena
        .alloc(Statement::BlockStatement(BlockStatement { body, span }))
}

/// Emit `{@const name = init}` into the current fragment: a hoisted `const`
/// declaration plus an evaluator overlay entry so later reads fold.
pub(crate) fn emit_const_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    tag: &'arena ConstTag<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Two template borrow points: the binding pattern (`{@const a: T = v}`) and
    // the initializer. The erased init feeds BOTH the emitted declaration and the
    // evaluator overlay below, so a later `{a}` read folds through the same node
    // the oracle folds.
    let id = env.erase(&tag.id)?;
    let init = env.erase(&tag.init)?;
    // Only a plain-identifier binding is modeled: a destructured `{@const}`
    // whose init folds would have the oracle fold each read, which this port
    // can't reproduce per-binding — refuse rather than risk a silent mismatch.
    let Expression::Identifier(ident) = id else {
        return Err(unsupported(Refusal::DestructuredConstTag));
    };
    let Some(name) = plain_identifier_name(ident, env.source) else {
        return Err(unsupported(Refusal::ConstTagNonPlainName));
    };
    // A `{@const}` that shadows a top-level `$derived` binding is refused, the
    // same as an each/await binding (see `push_overlay`): the derived-read
    // rewrite is name-based and would wrongly turn a later `{name}` read into a
    // `name()` call. `emit_const_tag` inserts into the overlay directly, so it
    // must repeat that check here.
    if env.derived_names.contains(&name) {
        return Err(unsupported(Refusal::BlockScopeShadowsDerived { name }));
    }
    // Guard + wrap the init (bare derived → d(), refuse runes/mutations).
    let wrapped_init = wrap_single(env, init)?;
    let id_expr = id.clone();
    let arena = env.b.arena;
    let stmt = declaration_stmt(
        &env.b,
        VariableDeclarationKind::Const,
        id_expr,
        wrapped_init,
    );
    out.push_statement(&mut env.b, arena, stmt);

    // Enter the innermost overlay so `{name}` reads fold through its init.
    let binding = Binding {
        kind: BindingKind::Normal,
        initial: Initial::Expr(init),
        updated: false,
    };
    match env.overlays.last_mut() {
        Some(overlay) => {
            overlay.insert(name, ScopeEntry::Const(binding));
        }
        None => {
            // Unreachable: root-level `{@const}` already refused in emit_fragment.
            return Err(unsupported(Refusal::ConstTagOutsideBlock));
        }
    }
    Ok(())
}

/// Emit `{#if}` / `{:else if}` / `{:else}`: a flat `if … else if … else` chain
/// with per-branch hydration anchors (`<!--[N-->`, terminal `<!--[-1-->`) and a
/// merge-forward `<!--]-->` closer. A missing `{:else}` synthesizes the
/// anchor-only terminal branch.
pub(crate) fn emit_if_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    if_block: &'arena IfBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    // Flatten the else-if chain into (test, consequent) branches + terminal else
    // (an `{:else if}` nests as an alternate fragment of one `IfBlock{elseif}`).
    let mut branches: Vec<(&'arena Expression<'arena>, &'arena Fragment<'arena>)> = Vec::new();
    let mut current = if_block;
    let final_else: Option<&'arena Fragment<'arena>>;
    loop {
        branches.push((&current.test, &current.consequent));
        match &current.alternate {
            Some(alt) => {
                if let [FragmentNode::IfBlock(inner)] = alt.nodes
                    && inner.elseif
                {
                    current = inner;
                    continue;
                }
                final_else = Some(alt);
                break;
            }
            None => {
                final_else = None;
                break;
            }
        }
    }

    // Build branch bodies in document order (anchors 0,1,2,…) so nested-block
    // name counters advance in the oracle's order.
    let mut cons_blocks: Vec<(Expression<'arena>, &'arena Statement<'arena>)> = Vec::new();
    for (i, &(test, frag)) in branches.iter().enumerate() {
        let test = env.erase(test)?;
        let test_expr = wrap_single(env, test)?;
        let anchor = env.b.push_string_stmt(&format!("<!--[{i}-->"));
        let body = emit_child_body(
            env,
            frag,
            std::slice::from_ref(&anchor),
            false,
            preserve,
            block_child_ns(ctx),
            HashMap::new(),
        )?;
        let block = block_stmt(&env.b, body);
        cons_blocks.push((test_expr, block));
    }

    // Terminal else (document order: after every consequent).
    let else_anchor = env.b.push_string_stmt("<!--[-1-->");
    let else_body = match final_else {
        Some(frag) => emit_child_body(
            env,
            frag,
            std::slice::from_ref(&else_anchor),
            false,
            preserve,
            block_child_ns(ctx),
            HashMap::new(),
        )?,
        None => {
            let mut v: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
            v.push(else_anchor);
            v.into_bump_slice()
        }
    };
    let mut alternate: &'arena Statement<'arena> = block_stmt(&env.b, else_body);

    // Assemble the chain inner-to-outer.
    for (test, cons) in cons_blocks.into_iter().rev() {
        let here = env.b.here();
        let if_stmt = IfStatement {
            test,
            consequent: cons,
            alternate: Some(alternate),
            span: here,
        };
        alternate = arena.alloc(Statement::IfStatement(if_stmt));
    }

    out.push_statement(&mut env.b, arena, (*alternate).clone());
    out.push_text("<!--]-->");
    Ok(())
}

/// The one element position a single `animate:` directive is legal: the sole
/// non-trivial child of a keyed `{#each}` body. Mirrors the oracle's phase-2
/// placement check (`2-analyze/visitors/shared/element.js:93-108`): a keyed each
/// whose body has at most one child once `Comment`/`ConstTag`/`DeclarationTag`/
/// whitespace-only `Text` are filtered out, and that child is a regular element.
/// Returns that element (whose span the emitter matches) or `None`.
fn animate_host_element<'arena>(
    each: &'arena EachBlock<'arena>,
) -> Option<&'arena Element<'arena>> {
    each.key.as_ref()?;
    let mut count = 0usize;
    let mut sole_element: Option<&'arena Element<'arena>> = None;
    for node in each.body.nodes {
        match node {
            FragmentNode::Comment(_)
            | FragmentNode::ConstTag(_)
            | FragmentNode::DeclarationTag(_) => {}
            FragmentNode::Text(t) if t.is_ascii_ws_only => {}
            FragmentNode::Element(el) => {
                count += 1;
                sole_element = Some(el);
            }
            _ => count += 1,
        }
    }
    if count <= 1 { sole_element } else { None }
}

/// Emit `{#each}`: `const each_array = $.ensure_array_like(expr)` + a `for` loop
/// binding `let CTX = each_array[IDX]`. Without `{:else}` the opener `<!--[-->`
/// merges into the preceding template; with it, `each_array` hoists before an
/// `if (each_array.length !== 0) { … } else { … }` whose openers are string
/// pushes. Nested `{#each}` refuses (unique-name order not reproducible).
pub(crate) fn emit_each_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    each: &'arena EachBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    if env.in_each {
        return Err(unsupported(Refusal::NestedEach));
    }
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    // The key `(key)` and the context pattern are guard-walked (a rune could hide
    // in either); the key is then dropped — SSR ignores it, so its TypeScript
    // (`{#each xs as x (k as string)}`) never reaches output and needs no erasure.
    // The guard walk carries its own TypeScript-unwrap arms for exactly this.
    if let Some(key) = &each.key {
        guard_dropped(env, key)?;
    }
    // The context pattern IS borrowed into the emitted `let CTX = each_array[IDX]`
    // — a template borrow point (`{#each xs as x: T}`).
    let context = match &each.context {
        Some(context) => {
            let context = env.erase(context)?;
            guard_pattern(env, context)?;
            Some(context)
        }
        None => None,
    };

    // Collection (guard + bare-derived rewrite).
    let collection_expr = env.erase(&each.expression)?;
    let collection = wrap_single(env, collection_expr)?;

    // Unique names: both counters advance once per each block (lockstep with the
    // oracle's per-each `scope.generate`, so `$$index` advances even when the
    // index is authored). `$$length` is a fixed block-scoped name.
    let array_name = env.next_each_array_name();
    let generated_index = env.next_index_name();
    let index_name = match each.index {
        Some(i) => i.to_string(),
        None => generated_index,
    };
    check_name_free(env, &array_name)?;
    check_name_free(env, &index_name)?;
    check_name_free(env, "$$length")?;

    // const each_array = $.ensure_array_like(collection);
    // Mint the `each_array` id BEFORE the call so the declaration span runs
    // forward (id.start < init.end) — the printer's call-head width math
    // subtracts the two and would underflow on an inverted span.
    let array_id = Expression::Identifier(env.b.ident(&array_name));
    let coll_alloc = arena.alloc(collection);
    let ensure = env
        .b
        .member_call("$", "ensure_array_like", std::slice::from_ref(coll_alloc));
    let const_each = declaration_stmt(&env.b, VariableDeclarationKind::Const, array_id, ensure);

    // for-loop init: `let IDX = 0, $$length = each_array.length`.
    let index_id = Expression::Identifier(env.b.ident(&index_name));
    let zero = env.b.number(0.0);
    let index_span = Span::new(index_id.span().start, zero.span().end);
    let index_declarator = VariableDeclarator {
        id: index_id,
        init: Some(zero),
        definite: false,
        span: index_span,
    };
    let length_id = Expression::Identifier(env.b.ident("$$length"));
    let arr_for_length = env.b.ident_expr(&array_name);
    let length_member = env.b.member_prop(arr_for_length, "length");
    let length_span = Span::new(length_id.span().start, length_member.span().end);
    let length_declarator = VariableDeclarator {
        id: length_id,
        init: Some(length_member),
        definite: false,
        span: length_span,
    };
    let mut init_decls: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(arena);
    init_decls.push(index_declarator);
    init_decls.push(length_declarator);
    let init_here = env.b.here();
    let init_decl = VariableDeclaration {
        kind: VariableDeclarationKind::Let,
        declarations: init_decls.into_bump_slice(),
        declare: false,
        span: init_here,
    };

    // test: `IDX < $$length`; update: `IDX++`.
    let idx_test = env.b.ident_expr(&index_name);
    let len_test = env.b.ident_expr("$$length");
    let test = env.b.binary(idx_test, BinaryOperator::LessThan, len_test);
    let idx_update = env.b.ident_expr(&index_name);
    let update = env.b.update(idx_update, UpdateOperator::Increment);

    // Body: `let CTX = each_array[IDX]` + the body fragment (text-first marker),
    // with context/index names masked to UNKNOWN in the evaluator.
    let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
    if let Some(context) = context {
        let mut names = Vec::new();
        pattern_binding_names(context, env.source, &mut names)?;
        for name in names {
            overlay.insert(name, ScopeEntry::Masked);
        }
    }
    overlay.insert(index_name.clone(), ScopeEntry::Masked);
    // The sanctioned `animate:` position (the sole non-trivial child of this keyed
    // each) is recognized by span, so `emit_element` can accept exactly it and
    // refuse every other `animate:`. Save/restore around the body like `in_each`.
    let saved_animate_host = env.animate_host_span;
    env.animate_host_span = animate_host_element(each).map(|el| el.span);
    env.in_each = true;
    let body_result = emit_each_body(
        env,
        each,
        context,
        &array_name,
        &index_name,
        preserve,
        block_child_ns(ctx),
        overlay,
    );
    env.in_each = false;
    env.animate_host_span = saved_animate_host;
    let body_stmts = body_result?;

    let for_body = block_stmt(&env.b, body_stmts);
    let for_here = env.b.here();
    let for_loop = Statement::ForStatement(ForStatement {
        init: Some(ForInit::VariableDeclaration(init_decl)),
        test: Some(test),
        update: Some(update),
        body: for_body,
        span: for_here,
    });

    if let Some(fallback) = &each.fallback {
        out.push_statement(&mut env.b, arena, const_each);
        // if branch: `{ $$renderer.push('<!--[-->'); for_loop }`.
        let open_anchor = env.b.push_string_stmt("<!--[-->");
        let mut if_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        if_body.push(open_anchor);
        if_body.push(for_loop);
        let if_branch = block_stmt(&env.b, if_body.into_bump_slice());
        // else branch: `{ $$renderer.push('<!--[!-->'); …fallback… }`. The
        // fallback's parent is the each block, so it is text-first-eligible.
        let else_anchor = env.b.push_string_stmt("<!--[!-->");
        let fallback_stmts = emit_child_body(
            env,
            fallback,
            std::slice::from_ref(&else_anchor),
            true,
            preserve,
            block_child_ns(ctx),
            HashMap::new(),
        )?;
        let else_branch = block_stmt(&env.b, fallback_stmts);
        // condition: `each_array.length !== 0`.
        let arr_cond = env.b.ident_expr(&array_name);
        let len_cond = arena.alloc(env.b.member_prop(arr_cond, "length"));
        let zero_cond = arena.alloc(env.b.number(0.0));
        let cond = env
            .b
            .binary(len_cond, BinaryOperator::BangEqualsEquals, zero_cond);
        let if_here = env.b.here();
        let if_stmt = Statement::IfStatement(IfStatement {
            test: cond,
            consequent: if_branch,
            alternate: Some(else_branch),
            span: if_here,
        });
        out.push_statement(&mut env.b, arena, if_stmt);
    } else {
        // Opener merges into the preceding template; then const + for loop.
        out.push_text("<!--[-->");
        out.push_statement(&mut env.b, arena, const_each);
        out.push_statement(&mut env.b, arena, for_loop);
    }
    out.push_text("<!--]-->");
    Ok(())
}

/// The `{#each}` body: `let CTX = each_array[IDX]` (when `as` is present) then
/// the body fragment (which gets the text-first `<!---->` marker). `context` is
/// the **erased** binding pattern.
#[allow(clippy::too_many_arguments)] // one cohesive each-body emit; splitting would just re-thread the same state
fn emit_each_body<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    each: &'arena EachBlock<'arena>,
    context: Option<&'arena Expression<'arena>>,
    array_name: &str,
    index_name: &str,
    preserve: bool,
    ns: ChildNamespace,
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
    let mut pre: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    if let Some(context) = context {
        let arr = env.b.ident_expr(array_name);
        let idx = env.b.ident_expr(index_name);
        let member = env.b.member_computed(arr, idx);
        let let_stmt = declaration_stmt(
            &env.b,
            VariableDeclarationKind::Let,
            context.clone(),
            member,
        );
        pre.push(let_stmt);
    }
    emit_child_body(env, &each.body, &pre, true, preserve, ns, overlay)
}

/// Emit `{#await}`: `$.await($$renderer, expr, () => {pending}, (value?) => {then})`
/// followed by a merge-forward `<!--]-->` closer. The `{:catch}` branch is
/// dropped (the oracle omits it from SSR); empty callbacks are `() => {}`.
pub(crate) fn emit_await_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    await_block: &'arena AwaitBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    let promise = env.erase(&await_block.expression)?;
    let expr = wrap_single(env, promise)?;
    // The `{:then value}` binding is borrowed into the then-arrow's parameter list
    // — a template borrow point (`{#await p then v: T}`). The `{:catch error}`
    // binding is NOT: the oracle drops the catch branch from SSR entirely, so
    // `await_block.error` never reaches output and needs no erasure.
    let value = match &await_block.value {
        Some(value) => Some(env.erase(value)?),
        None => None,
    };

    // Pending arrow: `() => { pending }` (empty when there is no pending content).
    let empty: &'arena [Statement<'arena>] = &[];
    let pending_stmts = match &await_block.pending {
        Some(frag) => emit_child_body(
            env,
            frag,
            &[],
            false,
            preserve,
            block_child_ns(ctx),
            HashMap::new(),
        )?,
        None => empty,
    };
    let pending_here = env.b.here();
    let no_params: &'arena [Expression<'arena>] = &[];
    let pending_arrow = env.b.arrow_block(no_params, pending_stmts, pending_here);

    // Then arrow: `(value?) => { then }`. The `{:then value}` pattern binds one
    // param; its names mask to UNKNOWN in the then body.
    let then_params: &'arena [Expression<'arena>] = match value {
        Some(value) => {
            guard_pattern(env, value)?;
            std::slice::from_ref(value)
        }
        None => &[],
    };
    let then_stmts = match &await_block.then {
        Some(frag) => {
            let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
            if let Some(value) = value {
                let mut names = Vec::new();
                pattern_binding_names(value, env.source, &mut names)?;
                for name in names {
                    overlay.insert(name, ScopeEntry::Masked);
                }
            }
            emit_child_body(
                env,
                frag,
                &[],
                false,
                preserve,
                block_child_ns(ctx),
                overlay,
            )?
        }
        None => empty,
    };
    let then_here = env.b.here();
    let then_arrow = env.b.arrow_block(then_params, then_stmts, then_here);

    // The `{:catch}` branch is dropped from SSR — the emitter never visits it. It
    // still gets the dropped-fragment guard, which covers both what the branch
    // REFERENCES (a misplaced rune inside it would otherwise compile where the
    // oracle's analysis phase rejects) and what it IS — a construct whose mere
    // presence the oracle's phase 2 reads, either into the emitted code (`<slot>`
    // widens the component signature from a dropped branch) or into a
    // whole-component validation (a legacy `on:` plus an emitted `onclick` is
    // `mixed_event_handler_syntaxes`, so the dropped branch decides whether the
    // component compiles at all).
    //
    // `{:catch}` is the only dropped FRAGMENT. The other dropped regions — an
    // `{#each}` key, a `{#key}` expression, an event handler's value — are single
    // expressions, so they carry no node kind and route through `guard_dropped`.
    if let Some(catch) = &await_block.catch {
        guard_dropped_fragment(env, catch)?;
    }

    // `$.await($$renderer, expr, pending, then)`.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(expr);
    args.push(pending_arrow);
    args.push(then_arrow);
    let call = env.b.member_call("$", "await", args.into_bump_slice());
    let span = call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    out.push_text("<!--]-->");
    Ok(())
}

/// Emit `{#key}`: a `<!---->` marker, a bare `{ … }` block wrapping the body,
/// and a closing `<!---->`. The key expression is SSR-ignored (guard-walked,
/// then dropped).
pub(crate) fn emit_key_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    key: &'arena KeyBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;
    guard_dropped(env, &key.expression)?;
    out.push_text("<!---->");
    let body = emit_child_body(
        env,
        &key.fragment,
        &[],
        false,
        preserve,
        block_child_ns(ctx),
        HashMap::new(),
    )?;
    let block = block_stmt(&env.b, body);
    out.push_statement(&mut env.b, arena, (*block).clone());
    out.push_text("<!---->");
    Ok(())
}
