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

use tsv_lang::store::Run;
use tsv_ts::ast::{
    ArrayExpression, AstView, BinaryExpression, CallExpression, ConditionalExpression, ExprId,
    Expression, ExpressionKind, MemberExpression, NewExpression, SequenceExpression, SpreadElement,
    TemplateLiteral, UnaryExpression,
};

use crate::analyze::NameSet;
use crate::rune_guard::{WalkCtx, walk_expression_guarded};
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Prepare a borrowed value expression for a synthetic call argument slot — the
/// emitter's **item-6 template-value walk**, the single home every template value
/// position routes through ([`crate::fragment::emit_fragment`]'s expression tags,
/// the block heads (`{#if}` test, `{#each}` collection, `{#await}` promise), the
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
/// stays on the guarded fast path and is returned as its own id (nothing pushed).
pub(crate) fn wrap_value_expr(env: &mut EmitEnv<'_>, expr: ExprId) -> Result<ExprId, CompileError> {
    rewrite_template_value(env, expr)
}

/// Whether `expr` is a bare read of a `$derived` binding — a plain (non-escaped)
/// identifier whose name is in `derived_names`. Such a read rewrites to the
/// derived-thunk call `d()` at every value position, at any depth.
pub(crate) fn is_bare_derived_read(
    source: &str,
    derived_names: &NameSet,
    expr: &Expression,
) -> bool {
    if let ExpressionKind::Identifier(id) = &expr.kind
        && id.escaped_name.is_none()
    {
        let start = id.span.start as usize;
        let name = &source[start..start + id.name_len as usize];
        return derived_names.contains(name);
    }
    false
}

/// Whether `expr` is a read of a store binding — a `$`-prefixed identifier whose
/// `$`-stripped base is a top-level binding (`store_names`). Such a read rewrites
/// to `$.store_get(($$store_subs ??= {}), '$name', name)` at every value position,
/// at any depth. Returns the base store name (the `$`-stripped variable).
///
/// The identifier name is DECODED: a plain identifier is its span slice (the fast
/// path — no pooled string touched), while a unicode-escaped `$`-identifier
/// (`$count` written `$count`) reads its decoded name through `view`, so it is
/// read as the store the oracle sees (the oracle decodes `node.name`, treating the
/// escaped spelling exactly as the plain one). A `$name` whose base is NOT a
/// binding is the oracle's `global_reference_invalid` error; the PLAIN form is left
/// for the guard to refuse (a safe over-refusal), while an escaped non-binding
/// `$name` — which the guard's span-identity `dollar_identifier_name` can't read —
/// is emitted verbatim, a pre-existing over-acceptance in the escaped-reference
/// residual.
fn bare_store_read(
    view: AstView<'_>,
    source: &str,
    store_names: &NameSet,
    expr: &Expression,
) -> Option<String> {
    let ExpressionKind::Identifier(id) = &expr.kind else {
        return None;
    };
    if id.escaped_name.is_some() {
        // An escaped `$`-identifier decodes to a store name the oracle treats
        // exactly as its plain spelling — resolve it and check the store base.
        let name = id.name(source, view);
        let base = crate::analyze::store_read_base(name)?;
        return store_names.contains(base).then(|| base.to_string());
    }
    let start = id.span.start as usize;
    let name = &source[start..start + id.name_len as usize];
    let base = crate::analyze::store_read_base(name)?;
    store_names.contains(base).then(|| base.to_string())
}

/// The recursive core of [`wrap_value_expr`]: rewrite one value expression,
/// returning the borrowed input's own id when nothing needs rewriting (after
/// guarding it).
fn rewrite_template_value(env: &mut EmitEnv<'_>, expr: ExprId) -> Result<ExprId, CompileError> {
    let view = env.b.view();
    let row = view.expr(expr);
    // A bare read of a derived binding becomes `d()`.
    if is_bare_derived_read(env.source, &env.derived_names, row) {
        let call = env.b.call_expr(expr, &[]);
        return Ok(env.b.push(call));
    }
    // A bare read of a store binding becomes
    // `$.store_get(($$store_subs ??= {}), '$name', name)`. The `var $$store_subs`
    // / `$.unsubscribe_stores` injection is analysis-driven (`EmitEnv::uses_stores`,
    // set upfront by `needs_context`), NOT flagged here. A store base shadowed by a
    // block-local (an `{#each}`/`{#await}`/snippet binding) is NOT the top-level
    // store — the oracle errors `store_invalid_scoped_subscription`, so it is left
    // for the guard to refuse (a safe refusal). A `$derived` base reads `d()` (the
    // store the derived currently holds).
    let span = row.span();
    let store_base = bare_store_read(view, env.source, &env.store_names, row);
    if let Some(base) = store_base
        && !env
            .overlays
            .iter()
            .any(|overlay| overlay.contains_key(&base))
    {
        let call = env
            .b
            .store_get(&base, env.derived_names.contains(&base), span);
        return Ok(env.b.push(call));
    }
    // A `$state.snapshot(x)` call → `$.snapshot(<processed x>)`.
    let view = env.b.view();
    if let Some(arg) = snapshot_call_arg(view, env.source, view.expr(expr)) {
        let processed = rewrite_template_value(env, arg)?;
        let call = env.b.member_call("$", "snapshot", &[processed]);
        return Ok(env.b.push(call));
    }
    // No rewrite target in this subtree: guard it whole and pass through borrowed
    // — the guarded fast path, so every target-free template expression keeps its
    // exact behavior (and pushes nothing).
    if !contains_rewrite_target(
        view,
        env.source,
        &env.derived_names,
        &env.store_names,
        view.expr(expr),
    ) {
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
fn guard_template_value(env: &EmitEnv<'_>, expr: ExprId) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
    let view = env.b.view();
    walk_expression_guarded(view, view.expr(expr), &mut ctx)?;
    if !updated.is_empty() {
        return Err(unsupported(Refusal::MutationInTemplateExpr));
    }
    Ok(())
}

/// If `expr` is a `$state.snapshot(x)` call (the `$state.snapshot` keypath with
/// exactly one argument), the argument `x`. Shares [`crate::analyze::callee_keypath`]
/// with the declarator classifier, so template and script recognize it identically.
fn snapshot_call_arg(view: AstView<'_>, source: &str, expr: &Expression) -> Option<ExprId> {
    let ExpressionKind::CallExpression(call) = &expr.kind else {
        return None;
    };
    let arguments = view.expr_list(call.arguments);
    if arguments.len() != 1
        || crate::analyze::callee_keypath(view, view.expr(call.callee), source).as_deref()
            != Some("$state.snapshot")
    {
        return None;
    }
    arguments.first().copied()
}

/// Whether `expr` contains a rewrite target — a bare `$derived` read (→ `d()`),
/// a bare store read (→ `$.store_get(...)`), or a `$state.snapshot(...)` call
/// (→ `$.snapshot(...)`) — anywhere this walk descends. A false negative is safe:
/// the target then reaches the rune guard, which refuses it (a safe
/// over-refusal). Descends exactly the wrapper node kinds [`rebuild_value`]
/// rebuilds — the two stay in lockstep on one node set.
fn contains_rewrite_target(
    view: AstView<'_>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    expr: &Expression,
) -> bool {
    if is_bare_derived_read(source, derived_names, expr) {
        return true;
    }
    if bare_store_read(view, source, store_names, expr).is_some() {
        return true;
    }
    if snapshot_call_arg(view, source, expr).is_some() {
        return true;
    }
    let contains = |&e: &ExprId| {
        contains_rewrite_target(view, source, derived_names, store_names, view.expr(e))
    };
    let list = |run: Run<ExprId>| view.expr_list(run).iter().any(contains);
    match &expr.kind {
        ExpressionKind::CallExpression(c) => contains(&c.callee) || list(c.arguments),
        ExpressionKind::NewExpression(n) => contains(&n.callee) || list(n.arguments),
        ExpressionKind::BinaryExpression(b) => contains(&b.left) || contains(&b.right),
        ExpressionKind::MemberExpression(m) => {
            contains(&m.object) || (m.computed && contains(&m.property))
        }
        ExpressionKind::ConditionalExpression(c) => {
            contains(&c.test) || contains(&c.consequent) || contains(&c.alternate)
        }
        ExpressionKind::UnaryExpression(u) => contains(&u.argument),
        ExpressionKind::SequenceExpression(s) => list(s.expressions),
        ExpressionKind::SpreadElement(s) => contains(&s.argument),
        ExpressionKind::ArrayExpression(a) => view
            .expr_opt_list(a.elements)
            .iter()
            .any(|e| e.as_ref().is_some_and(contains)),
        ExpressionKind::TemplateLiteral(t) => list(t.expressions),
        _ => false,
    }
}

/// Rebuild a value expression along the spine down to each nested rewrite target
/// (a `$state.snapshot(...)` call or a bare `$derived` read), recursing
/// [`rewrite_template_value`] on every value-position child (a target-free child
/// re-enters the guarded fast path). A node kind this match does not cover falls
/// through to the guard, which refuses the target it carries — a safe
/// over-refusal. The rebuilt spine row is pushed and its id returned.
fn rebuild_value(env: &mut EmitEnv<'_>, expr: ExprId) -> Result<ExprId, CompileError> {
    let Expression { span, kind } = *env.b.view().expr(expr);
    let rebuilt = match kind {
        // A `$`-rooted (non-snapshot) callee refuses via the recursive guard on the
        // callee itself, so no explicit rune check is needed here.
        ExpressionKind::CallExpression(call) => {
            let callee = rewrite_template_value(env, call.callee)?;
            let arguments = rewrite_value_run(env, call.arguments)?;
            Expression {
                span,
                kind: ExpressionKind::CallExpression(CallExpression {
                    callee,
                    type_args: None,
                    arguments,
                    ..call
                }),
            }
        }
        ExpressionKind::NewExpression(new) => {
            let callee = rewrite_template_value(env, new.callee)?;
            let arguments = rewrite_value_run(env, new.arguments)?;
            Expression {
                span,
                kind: ExpressionKind::NewExpression(NewExpression {
                    callee,
                    type_args: None,
                    arguments,
                }),
            }
        }
        ExpressionKind::BinaryExpression(b) => {
            let left = rewrite_template_value(env, b.left)?;
            let right = rewrite_template_value(env, b.right)?;
            Expression {
                span,
                kind: ExpressionKind::BinaryExpression(BinaryExpression {
                    left,
                    right,
                    // `relexes_as_type_arguments` rides along: it is a claim about the SOURCE
                    // bytes this rewrite invalidates, and inheriting is the SAFE direction — a
                    // stale `true` costs a redundant paren pair, a stale `false` an output
                    // nothing reparses.
                    ..b
                }),
            }
        }
        ExpressionKind::MemberExpression(m) => {
            let object = rewrite_template_value(env, m.object)?;
            // A non-computed property is a NAME, never a value read — leave it.
            let property = if m.computed {
                rewrite_template_value(env, m.property)?
            } else {
                m.property
            };
            Expression {
                span,
                kind: ExpressionKind::MemberExpression(MemberExpression {
                    object,
                    property,
                    ..m
                }),
            }
        }
        ExpressionKind::ConditionalExpression(c) => {
            let test = rewrite_template_value(env, c.test)?;
            let consequent = rewrite_template_value(env, c.consequent)?;
            let alternate = rewrite_template_value(env, c.alternate)?;
            Expression {
                span,
                kind: ExpressionKind::ConditionalExpression(ConditionalExpression {
                    test,
                    consequent,
                    alternate,
                }),
            }
        }
        ExpressionKind::UnaryExpression(u) => {
            let argument = rewrite_template_value(env, u.argument)?;
            Expression::from_unary_expression(UnaryExpression { argument, ..u })
        }
        ExpressionKind::SequenceExpression(s) => {
            let expressions = rewrite_value_run(env, s.expressions)?;
            Expression {
                span,
                kind: ExpressionKind::SequenceExpression(SequenceExpression { expressions }),
            }
        }
        ExpressionKind::SpreadElement(s) => {
            let argument = rewrite_template_value(env, s.argument)?;
            Expression::from_spread_element(SpreadElement {
                argument,
                span: s.span,
            })
        }
        ExpressionKind::ArrayExpression(a) => {
            let elements = rewrite_opt_run(env, a.elements)?;
            Expression {
                span,
                kind: ExpressionKind::ArrayExpression(ArrayExpression { elements, ..a }),
            }
        }
        ExpressionKind::TemplateLiteral(t) => {
            let expressions = rewrite_value_run(env, t.expressions)?;
            Expression::from_template_literal(span, TemplateLiteral { expressions, ..t })
        }
        _ => {
            guard_template_value(env, expr)?;
            return Ok(expr);
        }
    };
    Ok(env.b.push(rebuilt))
}

/// Rewrite each expression of a run (call arguments, sequence, template
/// expressions), returning a freshly pushed run of the rewritten ids.
fn rewrite_value_run(env: &mut EmitEnv<'_>, run: Run<ExprId>) -> Result<Run<ExprId>, CompileError> {
    let ids = env.b.view().expr_list(run).to_vec();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(rewrite_template_value(env, id)?);
    }
    Ok(env.b.push_list(&out))
}

/// Rewrite each present element of an array-element run (`[a, , b]` holes stay
/// `None`), returning a freshly pushed run.
fn rewrite_opt_run(
    env: &mut EmitEnv<'_>,
    run: Run<Option<ExprId>>,
) -> Result<Run<Option<ExprId>>, CompileError> {
    let elements = env.b.view().expr_opt_list(run).to_vec();
    let mut out = Vec::with_capacity(elements.len());
    for element in elements {
        out.push(match element {
            Some(id) => Some(rewrite_template_value(env, id)?),
            None => None,
        });
    }
    Ok(env.b.push_list(&out))
}
