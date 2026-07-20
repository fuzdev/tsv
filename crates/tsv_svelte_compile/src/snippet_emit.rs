//! `{#snippet}`/`{@render}` emission.
//!
//! A `{#snippet}` becomes a `function name($$renderer, ...params) { … }`
//! declaration (hoisted to module scope or emitted into its enclosing block's
//! init, per `crate::snippet`'s hoist analysis); a `{@render callee(args)}`
//! becomes `callee($$renderer, ...args)`. Named `snippet.rs` is the hoist
//! *analysis*; this module is the emission that consumes it.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_svelte::ast::internal::{RenderTag, SnippetBlock};
use tsv_ts::ast::internal::{CallExpression, Expression, ExpressionStatement, Statement};

use crate::analyze::{ScopeEntry, pattern_binding_names};
use crate::body_builder::BodyBuilder;
use crate::fragment::emit_child_body;
use crate::namespace::{ChildNamespace, FragmentParent, Namespace};
use crate::snippet::snippet_name;
use crate::template_value::wrap_single;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Emit a `{#snippet name(params)}…{/snippet}` as a
/// `function name($$renderer, ...params) { … }` declaration. A hoistable
/// top-level snippet goes to module scope (`env.hoisted_snippets`); everything
/// else goes to the current fragment's init (via `out`, flushing any pending
/// template first). The snippet body reuses the fragment machinery, with the
/// parameters masked to UNKNOWN so their reads never fold.
pub(crate) fn emit_snippet<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    snippet: &'arena SnippetBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let (fn_decl, name) = build_snippet_function(env, snippet)?;
    // Only a hoistable *top-level* snippet is in the hoistable map; a nested or
    // body-local snippet (`is_hoisted` false) goes to this block's init.
    if env.snippets.is_hoisted(&name) {
        env.hoisted_snippets.push(fn_decl);
    } else {
        out.push_statement(&mut env.b, arena, fn_decl);
    }
    Ok(())
}

/// Build a `{#snippet}` as a `function name($$renderer, ...params) { … }`
/// declaration (with its plain name). Refuses typed/generic snippets and escaped
/// names. Shared by the template-hoist path ([`emit_snippet`]) and the component
/// snippet-prop path (where the function lives in the component's wrapping block).
pub(crate) fn build_snippet_function<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    snippet: &'arena SnippetBlock<'arena>,
) -> Result<(Statement<'arena>, String), CompileError> {
    let arena = env.b.arena;
    // The snippet's signature head (`<T>(params)`) is parsed by wrapping it in a
    // synthetic `function f<T>(params) {}`. When that inner parse FAILS the AST is
    // empty and the raw text is kept instead — nothing to erase or emit, so refuse.
    if snippet.raw_parameters.is_some()
        || (snippet.type_params_raw.is_some() && snippet.type_parameters.is_none())
    {
        return Err(unsupported(Refusal::SnippetSignatureUnparsed));
    }
    // A **top-level** rest parameter is a `snippet_invalid_rest_parameter` error in
    // the oracle's analysis phase (`2-analyze/visitors/SnippetBlock.js`), which
    // scans `node.parameters` itself and never descends — so a rest element NESTED
    // in a destructuring parameter (`{#snippet s({ ...rest })}`, `s(a, [b, ...t])`)
    // is legal and compiles. Match that exactly: top level only.
    if snippet
        .parameters
        .iter()
        .any(|param| matches!(param, Expression::RestElement(_)))
    {
        return Err(unsupported(Refusal::SnippetRestParameter));
    }
    let Some(name) = snippet_name(snippet, env.source) else {
        // An escaped snippet name isn't reproducible by this port.
        return Err(unsupported(Refusal::SnippetEscapedName));
    };
    let name = name.to_string();

    // A generic `{#snippet s<T>(x: T)}` declares a *type*-level parameter only —
    // the oracle emits `function s($$renderer, x)`, the `<T>` simply gone. The
    // clause needs no erasure of its own: the synthetic `function_declaration`
    // never carries type parameters, so not reading it IS the erasure. (Without
    // `lang="ts"` the whole shape is refused up front — see
    // `refuse_template_typescript`.)

    // The parameter list is a template borrow point — the patterns are borrowed
    // verbatim into the emitted function, so `(x: string)` erases to `(x)` here.
    let mut params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    for param in snippet.parameters {
        params.push(env.erase(param)?.clone());
    }
    let params = params.into_bump_slice();

    // Mask the parameters to UNKNOWN in the body (a parameter read never folds),
    // and refuse a parameter that shadows a `$derived` binding (the overlay push
    // enforces this).
    let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
    for param in params {
        let mut names = Vec::new();
        pattern_binding_names(param, env.source, &mut names)?;
        for name in names {
            overlay.insert(name, ScopeEntry::Masked);
        }
    }

    // Snippet bodies are in the oracle's `is_text_first` parent set, so a
    // text-first body gets the leading `<!---->` anchor. A `{#snippet}` is in the
    // deep-walk special list and re-infers its namespace from its own nodes; the
    // defining-scope namespace is not threaded through the hoist, so the html
    // default is the fallback when the body holds no elements.
    let body = emit_child_body(
        env,
        &snippet.body,
        &[],
        true,
        false,
        ChildNamespace {
            inherited: Namespace::Html,
            parent: FragmentParent::Special,
            in_svg_text: false,
        },
        overlay,
    )?;

    // `($$renderer, ...params)` — the synthetic renderer first, then the erased
    // parameter patterns.
    let mut all_params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    all_params.push(Expression::Identifier(env.b.ident("$$renderer")));
    all_params.extend_from_slice(params);
    let block_span = env.b.here();
    let fn_decl = env
        .b
        .function_declaration(&name, all_params.into_bump_slice(), body, block_span);
    Ok((fn_decl, name))
}

/// Unwrap a `{@render}` expression to the call it applies, tolerating a single
/// layer of parentheses around the call (`{@render (foo)(x)}`); `None` for a
/// non-call.
///
/// This is the oracle's **parse-time** shape rule, decided on the RAW (un-erased)
/// node: a TypeScript wrapper AROUND the call (`{@render (s(x) as T)}`,
/// `{@render s(x)!}`) is `render_tag_invalid_expression` there even though erasure
/// would reveal a call underneath, while a wrapper around the *callee*
/// (`{@render (s as T)(x)}`) leaves a call and compiles. One definition, shared by
/// the emitter (its shape gate and callee/argument extraction, on the raw node
/// then again on the erased one) and the `needs_context` render walk.
pub(crate) fn render_call_expression<'a, 'arena>(
    expr: &'a Expression<'arena>,
) -> Option<&'a CallExpression<'arena>> {
    match expr {
        Expression::CallExpression(call) => Some(call),
        Expression::ParenthesizedExpression(paren) => match paren.expression {
            Expression::CallExpression(call) => Some(call),
            _ => None,
        },
        _ => None,
    }
}

/// The plain callee name of a `{@render}` expression: unwrap the (possibly
/// optional) call, requiring a plain-identifier callee. `None` for a member
/// callee, a non-call, or an escaped identifier.
pub(crate) fn render_callee_name<'s>(expr: &Expression<'_>, source: &'s str) -> Option<&'s str> {
    let call = render_call_expression(expr)?;
    match call.callee {
        Expression::Identifier(id) if id.escaped_name.is_none() => {
            let start = id.span.start as usize;
            Some(&source[start..start + id.name_len as usize])
        }
        _ => None,
    }
}

/// Whether a `{@render}` callee is *dynamic* (the oracle's
/// `binding?.kind !== 'normal'`): a local snippet is non-dynamic, a snippet prop
/// is dynamic. A non-standalone (dynamic) callee's fragment keeps the trailing
/// `<!---->` anchor even when the render is the sole child.
pub(crate) fn render_callee_dynamic(env: &EmitEnv<'_, '_>, name: &str) -> bool {
    !env.snippets.names.contains(name)
}

/// Emit `{@render callee(args)}` → `callee($$renderer, ...args)` (or
/// `callee?.($$renderer, …)` when optional), followed by a `<!---->` anchor
/// unless the enclosing fragment is standalone. The callee must resolve to a
/// local snippet or a snippet prop; anything else refuses. Arguments ride the
/// same value machinery as block tests (a bare derived read becomes `d()`).
pub(crate) fn emit_render_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    tag: &'arena RenderTag<'arena>,
    out: &mut BodyBuilder<'arena>,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    // "A `{@render}` holds a call expression" is a **parse-time** rule in the
    // oracle (`render_tag_invalid_expression`, raised while reading the tag) — so
    // it is decided on the RAW node, BEFORE type erasure. The distinction is
    // observable: `{@render (s as T)(x)}` is a call and compiles, while
    // `{@render (s(x) as T)}` and `{@render s(x)!}` are rejected even though
    // erasure would turn both into calls.
    if render_call_expression(&tag.expression).is_none() {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    }
    // The template borrow point: the whole `callee(args)` call is erased once, so
    // the callee classification and the arguments below both read the type-free
    // node (`{@render s<T>(x as U)}` → `s(x)`). Erasing a call yields a call, so
    // the shape settled above survives.
    let expression = env.erase(&tag.expression)?;
    let Some(call) = render_call_expression(expression) else {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    };
    // The callee must be a plain identifier resolving to a local snippet or a
    // snippet prop.
    let Some(name) = render_callee_name(expression, env.source) else {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    };
    let is_snippet = env.snippets.names.contains(name);
    let is_prop = env.bindings.is_prop(name);
    if !is_snippet && !is_prop {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    }

    // `callee($$renderer, ...args)`. Arguments go through the value machinery so
    // a bare derived read becomes `d()` and runes/mutations refuse.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    for arg in call.arguments {
        args.push(wrap_single(env, arg)?);
    }
    let render_call = env
        .b
        .call_of(call.callee, args.into_bump_slice(), call.optional);
    let span = render_call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: render_call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    // A dynamic or non-sole render keeps the anchor so its output doesn't glue to
    // the surrounding fragment (the oracle's `empty_comment` push).
    if !is_standalone {
        out.push_text("<!---->");
    }
    Ok(())
}
