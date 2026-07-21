//! The per-statement rune rewrites for the server transform.
//!
//! Oracle phase 3, **server**: `$props()` → `$$props`, `$state`/`$derived`
//! unwrap, the class-field `$state` unwrap, and the dropped `$effect`/`$inspect`
//! statements. Every shape here mints server-module syntax, so a client
//! transform would need its own; the target-independent halves that used to
//! share this file live beside it:
//!
//! - [`crate::script_ts_gate`] — the document TypeScript flag, gate, self-check
//! - [`crate::script_decls`] — the shared "what does this script declare" seam
//! - [`crate::script_bindings`] — the binding table + module-script analysis
//! - [`crate::script_collision`] — the rune/store collision pre-pass
//! - [`crate::script_comments`] — which host comments carry (server printer policy)
//! - [`crate::script_props`] — the `$props()` pattern rewrite this dispatches to
//!
//! See [`crate::transform_server`] for the orchestration that calls these in
//! sequence.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{SharedInterner, Span};
use tsv_ts::ast::internal::{
    ClassBody, ClassDeclaration, ClassMember, Expression, PropertyDefinition, Statement,
    VariableDeclaration, VariableDeclarator,
};

use crate::analyze::{NameSet, RuneInit, classify_rune_init, is_effect_call, is_inspect_call};
use crate::build::Builder;
use crate::rune_guard::{
    WalkCtx, refuse_dollar_binding_name, refuse_dollar_binding_pattern, walk_class_member_guarded,
    walk_expression_guarded, walk_statement_guarded,
};
use crate::script_decls::{identifier_binding_name, plain_identifier_name};
use crate::script_props::{BindableEntry, rewrite_props_pattern};
use crate::template_value::is_bare_derived_read;
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// The guard context every script-body walk on this path uses.
///
/// Exempt valid `$name` store reads from the guard's `$`-prefixed refusal and
/// `$derived` reads from the derived-read refusal: the store rewrite
/// ([`crate::store_rewrite`]) turns both into `$.store_get(…)` / `d()` after the
/// rewrite loop. Both shadow refusals are deferred — the store's needs the full
/// nested-scope set, so `store_shadowed` is `None`; the derived's is a
/// whole-compile check in `compile_server`.
fn script_walk_ctx<'a>(
    source: &'a str,
    updated: &'a mut NameSet,
    nested_declared: &'a mut NameSet,
    derived_names: &'a NameSet,
    store_names: &'a NameSet,
    interner: SharedInterner,
) -> WalkCtx<'a> {
    WalkCtx::new(source, updated, nested_declared, derived_names, interner)
        .allow_store_reads(store_names, None)
        .allow_derived_reads()
}

/// The oracle's `unthunk` peephole, at the only arity a thunk can have.
///
/// `b.thunk(value)` builds `arrow([], value)` and immediately runs it through
/// `unthunk`, which returns the call's **callee** when the arrow is non-async,
/// its body is a `CallExpression` with an `Identifier` callee, and its
/// parameters match the call's arguments one-for-one by name
/// (`utils/builders.js`). A thunk's parameter list is always empty, so the
/// name-matching clause reduces to "the call takes no arguments":
///
/// - `$derived(get_library())` → `$.derived(get_library)`
/// - `$derived(f(a))` → `$.derived(() => f(a))` (an argument survives)
/// - `$derived(o.m())` → `$.derived(() => o.m())` (the callee is not an identifier)
///
/// An optional call (`f?.()`) is a `ChainExpression` in the oracle's AST, never a
/// bare `CallExpression`, so it never collapses either.
fn unthunk_callee<'arena>(expr: &Expression<'arena>) -> Option<&'arena Expression<'arena>> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    if !call.arguments.is_empty() || call.optional {
        return None;
    }
    matches!(call.callee, Expression::Identifier(_)).then_some(call.callee)
}

/// Refuse a `$derived(…)` whose WHOLE argument is a bare `$derived` read
/// (`$derived(d)` where `d` is another derived). The oracle unthunk-collapses it
/// to `$.derived(d)` (the derived function passed straight through, not read), a
/// form the script rewrite can't reproduce — the store rewrite would turn the
/// argument into `d()`, giving `$.derived(() => d())`. A safe over-refusal (the
/// read refused before this slice too). NOT applied to `$derived.by(d)`, whose
/// oracle output IS `$.derived(d())` — reproduced by the rewrite (`.by` runs no
/// `unthunk`), so it compiles.
fn refuse_bare_derived_arg(
    expr: &Expression<'_>,
    source: &str,
    derived_names: &NameSet,
) -> Result<(), CompileError> {
    if let Expression::Identifier(id) = expr
        && is_bare_derived_read(source, derived_names, expr)
        && let Some(name) = plain_identifier_name(id, source)
    {
        return Err(unsupported(Refusal::DerivedBindingRead { name }));
    }
    Ok(())
}

/// Rewrite one instance-script statement for the server module:
///
/// - a top-level `$props()` declarator init becomes `$$props` (and the
///   component gains the `$$props` param); a `$bindable(fallback?)` default in
///   the destructure pattern is rewritten to its fallback (`void 0` when
///   argument-less) and the prop is collected into `bindable` so the transform
///   appends the trailing `$.bind_props($$props, { … })`;
/// - `$state(v)` / `$state.raw(v)` inits drop the wrapper (`void 0` when
///   argument-less);
/// - `$derived(e)` → `$.derived(() => e)`; `$derived.by(f)` → `$.derived(f)`;
/// - statement-position `$effect(…)` / `$effect.pre(…)` are dropped
///   (returning `None`) and force the component wrapper;
/// - everything else passes through borrowed after the guard walk (which also
///   collects mutations and shadow names for the evaluator).
///
/// Passthrough/rebuild is a *shallow* re-slot: `Statement`/`VariableDeclarator`
/// hold children inline by value, so placing a borrowed statement into the
/// synthetic body clones the wrapper only — children remain shared `&'arena`
/// refs into the parsed AST, and the original wrapper never enters the printed
/// tree (no duplicate spans in what the printer walks). See `build.rs` for the
/// address-keyed side-table caveat.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rewrite_script_statement<'arena>(
    b: &mut Builder<'arena>,
    stmt: &'arena Statement<'arena>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    updated: &mut NameSet,
    nested_declared: &mut NameSet,
    uses_props: &mut bool,
    has_effects: &mut bool,
    has_comments: bool,
    uses_slots: bool,
    dropped_regions: &mut Vec<Span>,
    bindable: &mut Vec<BindableEntry>,
    props_id: &mut Option<String>,
) -> Result<Option<Statement<'arena>>, CompileError> {
    // A top-level `$:` label is a legacy reactive statement — invalid in
    // runes mode (the oracle rejects it with legacy_reactive_statement_invalid),
    // so cloning it through would emit a dead label with no reactivity, a
    // silent mis-compile. Only the top level refuses: the oracle accepts a
    // `$` label inside a function (an ordinary JS label) and clones it
    // through, as does the fallback below. An escaped label name can't be
    // classified from its raw span, so it refuses conservatively.
    if let Statement::LabeledStatement(labeled) = stmt {
        let label = &labeled.label;
        let is_dollar = label.escaped_name.is_some() || {
            let start = label.span.start as usize;
            &source[start..start + label.name_len as usize] == "$"
        };
        if is_dollar {
            return Err(unsupported(Refusal::LegacyReactiveStatement));
        }
    }

    // Statement-position effects are dropped (and force the wrapper); their
    // callback is still guard-walked so stray runes inside refuse.
    if let Statement::ExpressionStatement(expr_stmt) = stmt
        && let Some(callback) = is_effect_call(&expr_stmt.expression, source)
    {
        *has_effects = true;
        dropped_regions.push(stmt.span());
        let mut ctx = script_walk_ctx(
            source,
            updated,
            nested_declared,
            derived_names,
            store_names,
            std::rc::Rc::clone(&b.interner),
        );
        walk_expression_guarded(callback, &mut ctx)?;
        return Ok(None);
    }

    // Statement-position `$inspect(…)` (bare or `.with(cb)`) is dropped on the
    // server, like `$effect` — but it does NOT force the wrapper on its own.
    // The `.with` / prop-rooted-argument cases that DO wrap are already covered
    // by `needs_context` (which walks the raw instance body — `$inspect`
    // statements included — before this drop). The arguments and `.with`
    // callback are still guard-walked so a stray rune (`$inspect($state(x))`,
    // which the oracle rejects) or a derived read refuses; the `$inspect` callee
    // itself is exempt at this recognized position.
    if let Statement::ExpressionStatement(expr_stmt) = stmt
        && let Some(guarded) = is_inspect_call(&expr_stmt.expression, source)
    {
        dropped_regions.push(stmt.span());
        let mut ctx = script_walk_ctx(
            source,
            updated,
            nested_declared,
            derived_names,
            store_names,
            std::rc::Rc::clone(&b.interner),
        );
        for expr in guarded {
            walk_expression_guarded(expr, &mut ctx)?;
        }
        return Ok(None);
    }

    // A top-level class declaration may carry `$state`/`$state.raw` fields, which
    // the server unwraps exactly like a top-level `$state` declarator. Every other
    // member — a `$derived` field, a static/computed rune field, a method body, a
    // nested class — takes the normal refusing guard walk, so the guard-exempt set
    // equals the unwrap set: reach-matched by construction (see
    // `rewrite_class_state_fields`).
    if let Statement::ClassDeclaration(class) = stmt {
        return rewrite_class_state_fields(
            b,
            class,
            source,
            derived_names,
            store_names,
            updated,
            nested_declared,
            dropped_regions,
        )
        .map(Some);
    }

    let Statement::VariableDeclaration(decl) = stmt else {
        let mut ctx = script_walk_ctx(
            source,
            updated,
            nested_declared,
            derived_names,
            store_names,
            std::rc::Rc::clone(&b.interner),
        );
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    };

    let has_rune_init = decl.declarations.iter().any(|d| {
        d.init
            .as_ref()
            .is_some_and(|i| classify_rune_init(i, source).is_some())
    });
    if !has_rune_init {
        let mut ctx = script_walk_ctx(
            source,
            updated,
            nested_declared,
            derived_names,
            store_names,
            std::rc::Rc::clone(&b.interner),
        );
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    }

    let mut declarations: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(b.arena);
    for declarator in decl.declarations {
        let mut ctx = script_walk_ctx(
            source,
            updated,
            nested_declared,
            derived_names,
            store_names,
            std::rc::Rc::clone(&b.interner),
        );
        // The `$`-prefixed BINDING rule, at the one point every declarator on
        // this path passes — before the rune dispatch below, mirroring the
        // oracle's own `VariableDeclarator` visitor, which runs
        // `validate_identifier_name` over every `extract_paths` leaf ahead of
        // its rune branch (`2-analyze/visitors/VariableDeclarator.js:24-26`).
        // It cannot ride the guard walk: none of the three arms below reaches
        // the binding leaves with the rule applied — a rune declarator's id is
        // not walked at all, and the two that are walked go through
        // `walk_expression_guarded`, which sees a pattern as an expression and
        // takes the store-read exemption this `WalkCtx` enables.
        refuse_dollar_binding_pattern(&declarator.id, source, &b.interner)?;
        let rune = declarator
            .init
            .as_ref()
            .and_then(|init| classify_rune_init(init, source));

        // `$props.id()` — skip the declarator entirely: the transform hoists
        // `const <name> = $.props_id($$renderer)` to the top of the component body
        // (the oracle's `component_block.body.unshift`, for hydration). At most one
        // per component (`props_duplicate`), and a plain-identifier target only
        // (`props_id_invalid_placement` rejects a destructure). The whole declarator
        // is a dropped region, so a comment inside refuses.
        if matches!(rune, Some(RuneInit::PropsId)) {
            if props_id.is_some() {
                return Err(unsupported(Refusal::DuplicatePropsId));
            }
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::PropsIdBindingPattern))?;
            *props_id = Some(name);
            dropped_regions.push(declarator.span);
            continue;
        }

        // Guard the binding pattern (a rune or derived read can hide in a
        // pattern default) — except for state/derived declarators, whose id is
        // an enforced plain identifier and is a *declaration* of the (possibly
        // derived) name, not a read. A `$props()` pattern is guard-walked AFTER
        // its bindable rewrite (in the arm below), so an exempt `$bindable(...)`
        // default — rewritten to its fallback — isn't seen as a stray rune, while
        // a `$bindable` left in any UNrecognized position survives the rewrite and
        // still refuses.
        if rune.is_none() {
            walk_expression_guarded(&declarator.id, &mut ctx)?;
        }
        // A rune init rewrite drops the call's own syntax around the kept
        // argument — record the dropped region(s) so comments inside refuse.
        if let (Some(init), Some(_)) = (&declarator.init, &rune) {
            let init_span = init.span();
            match rune {
                // `$state(v)` / `$state.snapshot(x)` unwrap to the bare argument (no
                // synthesized syntax around it), so the borrowed argument carries its
                // own interior comments and only the call syntax around it is dropped.
                Some(RuneInit::State(Some(arg))) | Some(RuneInit::StateSnapshot(arg)) => {
                    let arg_span = arg.span();
                    dropped_regions.push(Span::new(init_span.start, arg_span.start));
                    dropped_regions.push(Span::new(arg_span.end, init_span.end));
                }
                // `$derived(e)` / `$derived.by(f)` wrap the argument in a synthesized
                // `() => …` arrow whose param-list span sweeps a comment INTERIOR to the
                // argument into a double-print (and the oracle relocates it). Drop the
                // WHOLE init span so a comment anywhere inside refuses — the argument's
                // borrowed expression must not carry a comment through the arrow synthesis.
                _ => dropped_regions.push(init_span),
            }
        }

        let mut new_id = declarator.id.clone();
        // `RuneInit::PropsId` is skipped via `continue` above, so the arm below is
        // genuinely dead — it documents that invariant rather than a live branch.
        #[allow(clippy::unreachable)]
        let new_init = match rune {
            Some(RuneInit::Props) => {
                *uses_props = true;
                let (rewritten, entries) =
                    rewrite_props_pattern(b, &declarator.id, source, has_comments, uses_slots)?;
                if let Some(rewritten) = rewritten {
                    new_id = rewritten;
                }
                bindable.extend(entries);
                // Guard-walk the REWRITTEN pattern: the recognized top-level
                // `$bindable(...)` defaults are now their fallback expressions, so
                // a stray rune / derived read inside a fallback still refuses,
                // while a `$bindable` in any unrecognized position (nested, wrong
                // arity, non-identifier key/local) survived the rewrite and
                // refuses here.
                walk_expression_guarded(&new_id, &mut ctx)?;
                // Span-steal: the synthetic `$$props` takes the replaced
                // `$props()` call's host span, so the declarator's `=`-gap
                // comment windows stay exactly the authored ones.
                let init_span = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                let props_ident = b.ident_at("$$props", init_span);
                Some(Expression::Identifier(props_ident))
            }
            // Handled above by `continue` — the declarator is skipped, never
            // rebuilt, so this arm is unreachable. Kept for match exhaustiveness.
            Some(RuneInit::PropsId) => unreachable!("$props.id() is skipped above"),
            Some(RuneInit::State(arg)) => match arg {
                Some(arg) => {
                    walk_expression_guarded(arg, &mut ctx)?;
                    Some(arg.clone())
                }
                None => {
                    if has_comments {
                        // `void 0` mints an appendix literal; the declarator's
                        // init windows would then sweep host comments.
                        return Err(unsupported(Refusal::CommentsWithArglessState));
                    }
                    Some(b.void_zero())
                }
            },
            // `$state.snapshot(x)` unwraps to `x` (like `$state`), guarding `x`.
            Some(RuneInit::StateSnapshot(arg)) => {
                walk_expression_guarded(arg, &mut ctx)?;
                Some(arg.clone())
            }
            Some(RuneInit::Derived(expr)) => {
                // `$derived(d)` whose WHOLE body is a bare `$derived` read: the
                // oracle rewrites the read to `d()`, then `unthunk` collapses
                // `() => d()` to `d`, emitting `$.derived(d)` — the derived
                // function passed directly, never read. The script rewrite can't
                // reproduce that collapse (its store-rewrite pass would turn the
                // argument into `d()`, giving `$.derived(() => d())`), so refuse —
                // a safe over-refusal (the read refused before this slice too).
                refuse_bare_derived_arg(expr, source, derived_names)?;
                walk_expression_guarded(expr, &mut ctx)?;
                // The oracle wraps the value with `b.thunk`, which is
                // `unthunk(arrow([], value))` — and `unthunk` COLLAPSES the arrow
                // when its body is a plain call whose callee is a bare identifier
                // and whose arguments match the parameter list one-for-one by
                // name (`utils/builders.js`; call site
                // `3-transform/server/visitors/VariableDeclaration.js`). With the
                // empty parameter list a thunk always has, that reduces to an
                // argument-less, non-optional call on an identifier — so
                // `$derived(get_library())` emits `$.derived(get_library)`, not
                // `$.derived(() => get_library())`.
                //
                // The synthetic `$.derived(...)` and its arrow steal the replaced
                // `$derived(...)` init's host span so a carried script comment's
                // declarator/call windows stay empty (`derived_call`), the
                // call-structure analog of the `$$props` span-steal above.
                let anchor = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                let argument = match unthunk_callee(expr) {
                    Some(callee) => callee,
                    None => &*b.arena.alloc(b.arrow_expr_at(anchor, expr)),
                };
                Some(b.derived_call(anchor, argument))
            }
            Some(RuneInit::DerivedBy(f)) => {
                // `$derived.by(d)` passes `d` straight through as the compute
                // function → `$.derived(d)` (`.by` runs no `unthunk`), and the
                // store-rewrite pass then lowers the bare `d` read to `d()` →
                // `$.derived(d())`, exactly the oracle's output — so no refusal is
                // needed (unlike the `$derived(d)` arm, whose `() => d()` the oracle
                // collapses to `$.derived(d)`, a form the rewrite can't reproduce).
                walk_expression_guarded(f, &mut ctx)?;
                let anchor = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                Some(b.derived_call(anchor, f))
            }
            None => {
                if let Some(init) = &declarator.init {
                    walk_expression_guarded(init, &mut ctx)?;
                }
                declarator.init.clone()
            }
        };
        declarations.push(VariableDeclarator {
            id: new_id,
            init: new_init,
            definite: declarator.definite,
            span: declarator.span,
        });
    }
    // Every declarator was a skipped `$props.id()` — drop the whole statement
    // (its `id` binding lives on as the hoisted `const id = $.props_id($$renderer)`).
    if declarations.is_empty() {
        return Ok(None);
    }
    Ok(Some(Statement::VariableDeclaration(VariableDeclaration {
        kind: decl.kind,
        declarations: declarations.into_bump_slice(),
        declare: decl.declare,
        span: decl.span,
    })))
}

/// Rewrite a top-level class declaration for the server module: unwrap each
/// **direct** `$state(v)` / `$state.raw(v)` class field to its argument (exactly
/// like a top-level `$state` declarator init), and guard-walk every other member
/// through the normal refusing path.
///
/// The unwrap set is deliberately narrow — a non-static, non-computed field whose
/// init [`classify_rune_init`] recognizes as [`RuneInit::State`] — and it EXACTLY
/// equals the set the guard exempts, because every member that is not that shape
/// (a `$derived` field, a `static`/computed rune field, a method body, a nested
/// class or class expression inside one) flows through
/// [`walk_class_member_guarded`], the same refusing walk a class in any other
/// position takes. So a member is exempted from refusal iff it is unwrapped here:
/// there is no reach gap where the guard would pass a `$state` field the transform
/// leaves referencing an undefined `$state` (a MISMATCH). The reach is structural
/// — only a top-level `Statement::ClassDeclaration` reaches this function.
///
/// Oracle shape: `field = $state(v)` → `field = v`; a no-arg `field = $state()` →
/// a BARE field `field;` (the value dropped, NOT `void 0` — the divergence from the
/// top-level no-arg declarator, which mints `void 0`); a `static`/computed field is
/// oracle-rejected placement and refuses here. Non-rune members clone through in
/// source order (the class member order is preserved). Only the call syntax around
/// the kept argument is dropped, recorded in `dropped_regions` so a comment inside
/// refuses.
///
/// The member list is rebuilt **lazily** (the `erase.rs::class_body`
/// structural-sharing idiom): `out` stays `None` — allocating nothing — until the
/// first `$state` field is unwrapped, at which point the untouched prefix is
/// backfilled once; a rune-free top-level class (the common case) returns
/// `class.clone()` having allocated no member `Vec`. The per-member side effects —
/// the guard walk, the refusal checks, the `dropped_regions` pushes — run for every
/// member regardless of whether `out` ever materializes.
#[allow(clippy::too_many_arguments)]
fn rewrite_class_state_fields<'arena>(
    b: &Builder<'arena>,
    class: &'arena ClassDeclaration<'arena>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    updated: &mut NameSet,
    nested_declared: &mut NameSet,
    dropped_regions: &mut Vec<Span>,
) -> Result<Statement<'arena>, CompileError> {
    let arena = b.arena;
    // This path intercepts the statement before the guard walk's
    // `ClassDeclaration` arm, so it owns the class id's binding check.
    if let Some(id) = &class.id {
        refuse_dollar_binding_name(id, source, &b.interner)?;
    }
    // Same exemptions as the surrounding script guard: a `$name` store read / a
    // `$derived` read inside a method body is rewritten later, not refused here.
    let mut ctx = script_walk_ctx(
        source,
        updated,
        nested_declared,
        derived_names,
        store_names,
        std::rc::Rc::clone(&b.interner),
    );

    let members = class.body.body;
    let mut out: Option<BumpVec<'arena, ClassMember<'arena>>> = None;
    for (i, member) in members.iter().enumerate() {
        // The per-member step produces `Some(replacement)` for an unwrapped
        // `$state` field and `None` for an unchanged member — AFTER running the
        // member's side effects. The lazy `out` then decides allocation only.
        let replacement = if let ClassMember::PropertyDefinition(p) = member
            // The one exempt shape — a DIRECT top-level `$state`/`$state.raw`
            // field. `!is_static && !computed` keeps static/computed rune fields
            // (which the oracle rejects as `state_invalid_placement`) on the
            // refusing path.
            && !p.is_static
            && !p.computed
            && let Some(value) = &p.value
            && let Some(RuneInit::State(arg)) = classify_rune_init(value, source)
        {
            let init_span = value.span();
            let new_value = match arg {
                // `field = $state(v)` → `field = v`: guard-walk the borrowed
                // argument, drop the call syntax around it.
                Some(arg) => {
                    // A LONE reactive-binding argument (`$state($count)` /
                    // `$state(d)`) refuses: the oracle keeps such a lone store /
                    // `$derived` read BARE in the unwrapped field, but tsv's store
                    // rewrite descends into class bodies and would rewrite the kept
                    // argument to `$.store_get(…)` / `d()` — a MISMATCH. A compound
                    // (`$state($count + 1)`) or a plain-variable argument is fine —
                    // the inner read there IS rewritten at parity.
                    if is_lone_reactive_binding(arg, source, derived_names, store_names) {
                        return Err(unsupported(Refusal::ClassFieldStateReactiveArg));
                    }
                    walk_expression_guarded(arg, &mut ctx)?;
                    let arg_span = arg.span();
                    dropped_regions.push(Span::new(init_span.start, arg_span.start));
                    dropped_regions.push(Span::new(arg_span.end, init_span.end));
                    Some(arg.clone())
                }
                // `field = $state()` → a bare field `field;` (value dropped, no
                // `void 0`). The whole call is a dropped region.
                None => {
                    dropped_regions.push(init_span);
                    None
                }
            };
            Some(ClassMember::PropertyDefinition(PropertyDefinition {
                value: new_value,
                ..p.clone()
            }))
        } else {
            // Every other member — the normal refusing guard walk.
            walk_class_member_guarded(member, &mut ctx)?;
            None
        };

        match replacement {
            // Unchanged — only clone into `out` once it has been materialized.
            None => {
                if let Some(vec) = out.as_mut() {
                    vec.push(member.clone());
                }
            }
            // Changed — materialize `out` (backfilling the untouched prefix
            // `members[..i]` on the first change) and push the replacement.
            Some(new) => out
                .get_or_insert_with(|| {
                    let mut vec = BumpVec::with_capacity_in(members.len(), arena);
                    vec.extend_from_slice(&members[..i]);
                    vec
                })
                .push(new),
        }
    }

    match out {
        // No `$state` field — allocated nothing; clone the whole class through.
        None => Ok(Statement::ClassDeclaration(class.clone())),
        Some(members) => Ok(Statement::ClassDeclaration(ClassDeclaration {
            body: ClassBody {
                body: members.into_bump_slice(),
                span: class.body.span,
            },
            ..class.clone()
        })),
    }
}

/// Whether `arg` — the WHOLE argument of a class-field `$state(…)` /
/// `$state.raw(…)` — is a lone reactive-binding identifier the store rewrite
/// would otherwise rewrite: a **store read** (a plain `$name` whose `$`-stripped
/// base is a store binding and not a rune) or a **`$derived` binding** read.
///
/// Mirrors `store_rewrite`'s `store_base` / `derived_read` decision (both skip
/// escaped identifiers via `plain_identifier_name` — so an escaped lone argument
/// is not caught here, matching the store rewrite, which would not rewrite it
/// either; an escaped derived read is separately refused by the guard). The
/// discriminant is exactly "would the store rewrite touch this lone identifier?",
/// so the refusal covers precisely the shapes the oracle keeps bare and nothing
/// wider — a compound argument (`$state($count + 1)` → `$.store_get(…) + 1`) or a
/// plain-variable argument stays compiling.
fn is_lone_reactive_binding(
    arg: &Expression<'_>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
) -> bool {
    let Expression::Identifier(id) = arg else {
        return false;
    };
    let Some(name) = plain_identifier_name(id, source) else {
        return false;
    };
    if derived_names.contains(&name) {
        return true;
    }
    crate::analyze::store_read_base(&name).is_some_and(|base| store_names.contains(base))
}
