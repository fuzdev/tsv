//! The template-value rewrite walk: every borrowed template expression's single
//! path into a synthetic call argument slot.
//!
//! A **shared primitive** of the emission layer — the one home every template
//! value position routes through ([`crate::fragment`]'s expression tags,
//! [`crate::blocks`]' block heads, [`crate::element`]'s and
//! [`crate::attribute`]'s attribute/spread/component-prop borrow points). Nothing
//! here walks a fragment or emits template text; the recursion is closed over
//! expressions alone ([`rewrite_template_value`] ↔ [`rebuild_value`]), so the
//! whole family depends on no emitter and no emitter's shape depends on it.
//!
//! **Single source of truth** for the *item-6 substitution set*: a `$derived`
//! read → `d()`, a store read → `$.store_get(…)`, a `$state.snapshot(x)` →
//! `$.snapshot(…)`. Duplicating any of the three at a borrow point would be
//! dangerous in both directions — a missed rewrite emits a bare `d` where the
//! oracle emits `d()` (a MISMATCH), while a rewrite the guard does not know about
//! turns a safe over-refusal into silently divergent output. The fast-path
//! predicate [`contains_rewrite_target`] and the rebuild it guards are likewise
//! one pair over one node set, and must stay in lockstep.
//!
//! See [`crate::transform_server`] for the orchestration, and
//! [`crate::store_rewrite`] for the *script*-position analog of this walk.

use bumpalo::collections::Vec as BumpVec;
use tsv_ts::ast::internal::{
    ArrayExpression, BinaryExpression, CallExpression, ConditionalExpression, Expression,
    MemberExpression, NewExpression, ParenthesizedExpression, SequenceExpression, SpreadElement,
    TemplateLiteral, UnaryExpression,
};

use crate::analyze::NameSet;
use crate::rune_guard::{WalkCtx, walk_expression_guarded};
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

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

/// Prepare a borrowed value expression for a synthetic call argument slot — the
/// emitter's **item-6 template-value walk**, the single home every template value
/// position routes through ([`crate::fragment::emit_fragment`]'s expression tags,
/// [`wrap_single`], the attribute/spread/component-prop borrow points). It:
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
