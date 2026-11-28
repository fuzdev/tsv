//! Rune refusal walk over the script statements — plus the collection
//! passes that ride the same traversal.
//!
//! The server transform rewrites a fixed set of rune shapes elsewhere (the
//! declarator inits `$props()` / `$state(…)` / `$state.snapshot(x)` / `$derived(…)`
//! / `$props.id()`, a `$props()`-default `$bindable(…)`, statement-position
//! `$effect(…)` / `$inspect(…)`, and a template-position `$state.snapshot(x)` — see
//! `analyze.rs::classify_rune_init`, `script_rewrite.rs`, and `fragment.rs`). Every
//! other `$`-prefixed identifier in a walked value position must REFUSE rather than
//! pass through into runtime-broken JS — rune calls in nested functions, member-form
//! calls in a non-sanctioned position (`$effect.tracking()`, `$state.foo()`), and
//! bare *references* (`let x = $state;`, a future `$store` subscription) alike. Calls report
//! their callee root as a rune; name-only positions (non-computed member
//! properties / object keys) are not walked, so `obj.$foo` stays allowed.
//!
//! The same walk collects what the static evaluator (`analyze.rs`) needs:
//!
//! - **assignment/update target roots** (`updated` — an updated binding is
//!   never folded by the oracle), and
//! - **names declared in nested/block scopes** (`nested_declared` — a shadowed
//!   top-level name can't be trusted by this shadow-naive mutation collection,
//!   so the binding goes `Opaque` and refuses if it reaches an evaluated spine),
//!
//! and refuses **reads of derived bindings** (`derived_names`) that no rewrite
//! turns into `d()`. A template value position (bare or nested, via the
//! value-walk in `fragment.rs`) and a **script position** (when the caller opts
//! in via `allow_derived_reads`, the script-body guards — the read is rewritten
//! by the [store rewrite](crate::store_rewrite)) are exempt; a read the rewrites
//! do not reach — a pattern default, an unsupported-wrapper or escaped-identifier
//! read — refuses here, as does a **write** to a derived binding (`d = v` /
//! `d++`, out of scope).
//!
//! The matches are exhaustive on purpose — a new `Statement`/`Expression`
//! variant fails compilation here instead of silently skipping the guard.
//! TS *type* positions are not walked (nothing in type position evaluates).

use std::collections::HashSet;

use tsv_lang::store::Run;
use tsv_ts::ast::{
    ArrowFunctionBody, AstView, ClassBody, ClassMember, ExportDefaultValue, ExprId, Expression,
    ExpressionKind, ForInOfLeft, ForInit, ForLeftId, FunctionExpression, Identifier,
    ImportSpecifier, ObjectPatternProperty, ObjectProperty, Statement, StatementKind, StmtId,
    VariableDeclaration,
};

use crate::analyze::{NameSet, expression_kind, pattern_binding_names};
use crate::script_decls::plain_identifier_str;
use crate::{CompileError, Refusal};

/// The walk's shared state: the source names resolve against, the collection
/// sinks, and the refusal set.
///
/// It holds no view: every walk takes the TypeScript view as its own first
/// argument, so a caller that pushes rows between walks keeps one context across
/// them.
pub(crate) struct WalkCtx<'a> {
    pub source: &'a str,
    /// Assignment/update target root names (fed back as `updated` bindings).
    pub updated: &'a mut HashSet<String>,
    /// Names declared in nested function/block scopes (shadow candidates).
    pub nested_declared: &'a mut HashSet<String>,
    /// Derived binding names — reading one anywhere in walked code refuses.
    pub derived_names: &'a HashSet<String>,
    /// Top-level component binding names, for recognizing a `$name` store
    /// auto-subscription. `Some` **exempts** a valid store read from the
    /// `$`-prefixed-identifier refusal (it is rewritten elsewhere — the script
    /// [store rewrite](crate::store_rewrite) or a dropped-region drop). `None`
    /// keeps the refuse-every-`$name` behavior (the template value guard and the
    /// pattern guard, where a store read reaching them is in an unsupported
    /// position — a safe over-refusal).
    store_names: Option<&'a HashSet<String>>,
    /// Store bases bound in a nested scope — the oracle's
    /// `store_invalid_scoped_subscription`. Consulted only when `store_names` is
    /// `Some`: a shadowed base **refuses** here (the dropped guard, which owns the
    /// refusal for its region); the script guard passes `None` and defers the
    /// shadow refusal to the store rewrite, which has the full shadow set.
    store_shadowed: Option<&'a HashSet<String>>,
    /// Exempt a plain-name read of a `$derived` binding from the refusal: the
    /// script-position rewrite ([store rewrite](crate::store_rewrite)) turns it
    /// into `d()`. `true` on the SCRIPT-body guards; `false` on the pattern and
    /// template-value guards, where a derived read reaching the guard is an
    /// unsupported position (a safe over-refusal). A **write** to the derived
    /// (`d = v` / `d++`) refuses regardless — it is out of scope on every path.
    /// The escaped-identifier read also refuses regardless (classification not
    /// ported).
    allow_derived_reads: bool,
    /// Current function-nesting depth (0 = the statement being walked).
    fn_depth: usize,
}

impl<'a> WalkCtx<'a> {
    pub(crate) fn new(
        source: &'a str,
        updated: &'a mut HashSet<String>,
        nested_declared: &'a mut HashSet<String>,
        derived_names: &'a HashSet<String>,
    ) -> Self {
        Self {
            source,
            updated,
            nested_declared,
            derived_names,
            store_names: None,
            store_shadowed: None,
            allow_derived_reads: false,
            fn_depth: 0,
        }
    }

    /// Exempt valid `$name` store reads from the `$`-prefixed refusal (they are
    /// rewritten elsewhere). `store_shadowed`, when `Some`, refuses a base bound
    /// in a nested scope (the dropped guard); the script guard passes `None` and
    /// leaves that refusal to the store rewrite.
    pub(crate) fn allow_store_reads(
        mut self,
        store_names: &'a HashSet<String>,
        store_shadowed: Option<&'a HashSet<String>>,
    ) -> Self {
        self.store_names = Some(store_names);
        self.store_shadowed = store_shadowed;
        self
    }

    /// Exempt a plain-name read of a `$derived` binding from the refusal (the
    /// script-position [store rewrite](crate::store_rewrite) turns it into `d()`).
    /// Set on the SCRIPT-body guards; a **write** to a derived binding still
    /// refuses, as does an escaped-identifier read.
    pub(crate) fn allow_derived_reads(mut self) -> Self {
        self.allow_derived_reads = true;
        self
    }
}

/// Walk one statement: refuse stray runes and derived reads, collect
/// mutations and nested declarations. `depth` is the statement nesting depth
/// (0 = a top-level script statement, whose declarations are the top bindings).
pub(crate) fn walk_statement_guarded(
    view: AstView<'_>,
    stmt: &Statement,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    walk_statement(view, stmt, ctx, depth)
}

/// Walk one expression (template expressions, rewritten declarator pieces).
pub(crate) fn walk_expression_guarded(
    view: AstView<'_>,
    expr: &Expression,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    walk_expression(view, expr, ctx)
}

/// Walk one class member with the normal refusing rules — a method body, a
/// property init, a static block. Exposed so the class-field `$state` rewrite
/// ([`crate::script_rewrite`]) can guard EVERY member it does not unwrap through
/// the exact same path a class in any other position takes, keeping the
/// guard-exempt set (the unwrapped `$state` fields) equal to the transform set —
/// reach-matched by construction, so no member can be exempted without a matching
/// unwrap (which would emit an undefined `$state` reference — a MISMATCH).
pub(crate) fn walk_class_member_guarded(
    view: AstView<'_>,
    member: &ClassMember,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    walk_class_member(view, member, ctx)
}

/// The `$`-prefixed name of a plain identifier, or `None`. Parsed identifiers
/// are span-identity (`escaped_name: None`); a pooled name is a user escape or a
/// synthetic name (`$$renderer`, `$$props`, …) and is never refused here.
fn dollar_identifier_name<'s>(id: &Identifier, source: &'s str) -> Option<&'s str> {
    let name = identifier_name(id, source)?;
    name.starts_with('$').then_some(name)
}

fn identifier_name<'s>(id: &Identifier, source: &'s str) -> Option<&'s str> {
    plain_identifier_str(id, source)
}

/// The `$`-prefixed root-identifier name of a callee, peeled through member
/// accesses (`$state.raw`), non-null assertions, instantiations, and preserved
/// parens — `None` when the root is not a plain `$`-identifier.
fn dollar_callee_root<'s>(
    view: AstView<'_>,
    callee: &Expression,
    source: &'s str,
) -> Option<&'s str> {
    match &callee.kind {
        ExpressionKind::Identifier(id) => dollar_identifier_name(id, source),
        ExpressionKind::MemberExpression(member) => {
            dollar_callee_root(view, view.expr(member.object), source)
        }
        ExpressionKind::TSNonNullExpression(non_null) => {
            dollar_callee_root(view, view.expr(non_null.expression), source)
        }
        ExpressionKind::TSInstantiationExpression(inst) => {
            dollar_callee_root(view, view.expr(inst.expression), source)
        }
        _ => None,
    }
}

fn rune_error(name: &str) -> CompileError {
    CompileError::Unsupported(Refusal::Rune {
        name: name.to_string(),
    })
}

/// Whether a `$`-prefixed `name` (an identifier reference OR a call/new callee
/// root) is a store access the current context exempts from refusal:
///
/// - `Some(Ok(()))` — an exempt store read (the store rewrite / a dropped-region
///   drop handles it): the caller must NOT refuse, and (for a call/new) should
///   fall through to recurse so the callee's own `$name` is rewritten;
/// - `Some(Err(…))` — a shadowed base, the oracle's `store_invalid_scoped_subscription`;
/// - `None` — NOT an exempt store (stores not allowed in this context, a genuine
///   rune whose base is a `RUNE_BASES` keyword, or a `$name` whose base is not a
///   binding): the caller refuses exactly as before.
///
/// Shared by the identifier, call, and new arms so a callee-position store read
/// (`$fn()`, `$obj.m()`, `new $C()`) is exempted identically to a bare read.
fn store_read_exemption(ctx: &WalkCtx<'_>, name: &str) -> Option<Result<(), CompileError>> {
    let store_names = ctx.store_names?;
    let base = crate::analyze::store_read_base(name)?;
    if !store_names.contains(base) {
        return None;
    }
    if ctx
        .store_shadowed
        .is_some_and(|shadowed| shadowed.contains(base))
    {
        return Some(Err(CompileError::Unsupported(
            Refusal::StoreScopedSubscription,
        )));
    }
    Some(Ok(()))
}

/// Record the root identifier(s) of an assignment target (through member
/// chains and destructuring patterns) into `out` as reassigned/updated names.
///
/// Shared by the guard walk and the whole-component reassignment collection in
/// `needs_context` (which must see mutations inside dropped event handlers so a
/// reassigned binding is never statically folded).
pub(crate) fn assign_target_roots(
    view: AstView<'_>,
    target: &Expression,
    source: &str,
    out: &mut NameSet,
) {
    match &target.kind {
        ExpressionKind::Identifier(id) => {
            if let Some(name) = identifier_name(id, source) {
                out.insert(name.to_string());
            }
        }
        ExpressionKind::MemberExpression(m) => {
            assign_target_roots(view, view.expr(m.object), source, out);
        }
        // All four TypeScript assignment-target wrappers the parser accepts
        // (`expression_assignable.rs`). LOAD-BEARING, not defense in depth: the
        // script's statements are erased before this walk, but the TEMPLATE's are
        // not — the Svelte AST is never rebuilt, so erasure happens per-expression
        // at the emitter's borrow points, and `needs_context`'s whole-component
        // reassignment collection walks the raw fragment. A missing arm silently
        // loses a reassignment root (`(x as any).y = 1` in a handler) and then
        // statically folds a mutated binding.
        ExpressionKind::TSNonNullExpression(t) => {
            assign_target_roots(view, view.expr(t.expression), source, out);
        }
        ExpressionKind::TSAsExpression(t) => {
            assign_target_roots(view, view.expr(t.expression), source, out);
        }
        ExpressionKind::TSSatisfiesExpression(t) => {
            assign_target_roots(view, view.expr(t.expression), source, out);
        }
        ExpressionKind::TSTypeAssertion(t) => {
            assign_target_roots(view, view.expr(t.expression), source, out);
        }
        ExpressionKind::ObjectPattern(obj) => {
            for prop in view.pat_props(obj.properties) {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        assign_target_roots(view, view.expr(p.value), source, out);
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        assign_target_roots(view, view.expr(rest.argument), source, out);
                    }
                }
            }
        }
        ExpressionKind::ArrayPattern(arr) => {
            for &element in view.expr_opt_list(arr.elements).iter().flatten() {
                assign_target_roots(view, view.expr(element), source, out);
            }
        }
        ExpressionKind::AssignmentPattern(a) => {
            assign_target_roots(view, view.expr(a.left), source, out);
        }
        ExpressionKind::RestElement(r) => {
            assign_target_roots(view, view.expr(r.argument), source, out);
        }
        _ => {}
    }
}

/// Refuse a write to a `$derived` binding ITSELF — a **binding-leaf** target: a
/// bare identifier (`d = v`, `d++`) or a destructuring-pattern leaf
/// (`[d] = …`, `({ d } = …)`, `[z, d] = …`). The oracle lowers each to `d(v)` /
/// `$.update_derived(d)` / an `$.to_array` IIFE, none of which this slice emits.
///
/// The recursion is deliberately NARROWER than the store path's
/// [`assign_target_roots`]/`pattern_targets_store`: it **stops at a member/index**
/// target (`d.x = v`, `x[d] = v`), which merely READS the derived (its object /
/// computed index) and is left for the read rewrite to lower (`d().x = v` /
/// `x[d()] = v`). Only a bare-name binding leaf is a write; a default's value
/// (`[d = 1] = …` → the `1`) is a read, so only the `left` of an
/// [`AssignmentPattern`](ExpressionKind::AssignmentPattern) is a binding.
///
/// Keyed on `derived_names`, so in a dropped region (an empty set) it never fires
/// — a dropped-handler derived write compiles, unchanged.
fn refuse_derived_write_target(
    view: AstView<'_>,
    target: &Expression,
    ctx: &WalkCtx<'_>,
) -> Result<(), CompileError> {
    match &target.kind {
        // A bare-identifier binding target — a write to the derived itself.
        ExpressionKind::Identifier(id) => {
            if let Some(name) = identifier_name(id, ctx.source)
                && ctx.derived_names.contains(name)
            {
                return Err(CompileError::Unsupported(Refusal::DerivedBindingRead {
                    name: name.to_string(),
                }));
            }
        }
        // A member/index target READS the derived, never binds it — STOP here so
        // `d.x = v` / `x[d] = v` compile via the read rewrite.
        ExpressionKind::MemberExpression(_) => {}
        // Destructuring-pattern targets: every slot is itself an assignment target,
        // so recurse into each binding-leaf position.
        ExpressionKind::ArrayPattern(pattern) => {
            for &element in view.expr_opt_list(pattern.elements).iter().flatten() {
                refuse_derived_write_target(view, view.expr(element), ctx)?;
            }
        }
        ExpressionKind::ObjectPattern(pattern) => {
            for prop in view.pat_props(pattern.properties) {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        refuse_derived_write_target(view, view.expr(p.value), ctx)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        refuse_derived_write_target(view, view.expr(rest.argument), ctx)?;
                    }
                }
            }
        }
        // A default (`[d = 1] = …`): `left` is the binding, `right` the default
        // (a read the read rewrite / guard handles). Only the binding refuses.
        ExpressionKind::AssignmentPattern(pattern) => {
            refuse_derived_write_target(view, view.expr(pattern.left), ctx)?;
        }
        ExpressionKind::RestElement(rest) => {
            refuse_derived_write_target(view, view.expr(rest.argument), ctx)?;
        }
        _ => {}
    }
    Ok(())
}

/// Record the names a declaration pattern declares into `nested_declared`
/// (best-effort — unusual pattern shapes just record nothing extra).
fn collect_nested_declared(view: AstView<'_>, pattern: &Expression, ctx: &mut WalkCtx<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(view, pattern, ctx.source, &mut names).is_ok() {
        for name in names {
            ctx.nested_declared.insert(name);
        }
    }
}

/// Refuse a `$`-prefixed **binding name** — the oracle's `dollar_prefix_invalid`
/// (`validate_identifier_name` in `phases/scope.js`, literally
/// `node.name.startsWith('$')` on a `Binding`).
///
/// This is the rule that separates the two `$$slots` positions, and it is the
/// only thing that makes the reference carve-out in `walk_expression`'s
/// `ExpressionKind::Identifier` arm sound: a `$$slots` *reference* is the real
/// runtime value the transform injects, while a `$$slots` *declaration* is a
/// compile error. The rule is Svelte-domain, not a JS early error — `let $$slots
/// = 1` is valid JS.
///
/// **The oracle reaches `validate_identifier_name` from FOUR sites, and only one
/// of them (`declare` in `scope.js`) passes `function_depth`** — so
/// "oracle-rejected" is not one answer for the six guarded positions. Checked against every call path:
///
/// | call path | passes `function_depth`? | positions |
/// | --- | --- | --- |
/// | `VariableDeclarator.js`, `FunctionDeclaration.js`, `ClassDeclaration.js` | **no** — so `!function_depth` short-circuits and the gate never applies | a declarator's binding leaves (any depth, any declaration kind, destructured or not), a function-declaration id, a class-declaration id |
/// | `scope.js` (`Scope::declare`) | **yes** — the `function_depth <= 1` gate DOES apply | a function-expression id, a catch-clause parameter, an import specifier's local |
///
/// **Unconditionally oracle-rejected** (probe-verified against the pinned
/// compiler): the three no-depth positions above, plus an **import specifier's
/// local** — which goes through the depth-passing path but is safe by a
/// different mechanism, `declare` in `scope.js` re-delegating `declaration_kind ===
/// 'import'` to the parent scope, so an import binding always lands at
/// `function_depth` 0 however deeply it is written.
///
/// ⚠️ **Two positions are oracle-rejected only at the top level, and tsv refuses
/// them unconditionally — a deliberate over-refusal.** A **function-expression
/// id** and a **catch-clause parameter** both declare through `declare` in `scope.js`,
/// and the instance script's top-level scope sits at `function_depth` 1, so the
/// oracle rejects them there and *accepts* them inside any function body
/// (probe-verified both directions: `const g = function $$slots() {}` and `catch
/// ($$slots)` reject at top level, accept inside a `function f() { … }`; `catch
/// ($x)` with `x` imported likewise). tsv refuses both regardless of depth.
///
/// Narrowing them would need the oracle's depth, which tsv does not have:
/// [`WalkCtx::fn_depth`] counts function *nodes*, while the oracle's
/// non-porous increment happens at a function's **`BlockStatement`**
/// (`scope.js`) — a `FunctionExpression` / `ArrowFunctionExpression`
/// scope is itself `child(true)`, porous. So an expression-bodied arrow does not
/// increment the oracle's depth and does increment tsv's: `const h = () =>
/// function $$slots() {}` is oracle-**rejected**, and a `fn_depth == 0` gate here
/// would compile it — turning a contract-safe over-refusal into an
/// OVER-ACCEPTANCE, which is a refusal-contract bug. Closing the gap faithfully
/// means a second, oracle-shaped depth counter; the shapes it would buy back (a
/// `$`-prefixed catch param or named function expression, inside a function) do
/// not occur in real components, so the over-refusal stands.
///
/// ⚠️ A declarator's leaves are guarded on **two** paths, and both are needed:
/// [`walk_variable_declaration`] here, for a declaration the guard walk owns,
/// and `script_rewrite::rewrite_script_statement`'s per-declarator loop, for a
/// top-level instance-script declaration the transform rewrites instead of
/// walking. The transform path does not reach this rule through the walk at
/// all — a rune declarator's id is never walked, and a non-rune one is walked
/// as an *expression* under a store-read exemption. Each path calls this at its
/// own chokepoint, ahead of any rune dispatch, the way the oracle's
/// `VariableDeclarator` visitor does.
///
/// **Deliberately NOT guarded**, because the oracle *accepts* these: a
/// function / arrow / snippet parameter (`declaration_kind` `param` /
/// `rest_param` is exempt), a template binding (`{@const}`, `{#each … as}`, an
/// `{#each}` index, an `{#await}` `then`/`catch` value — declared in scopes
/// past the `function_depth <= 1` gate).
///
/// A class-EXPRESSION id is the one position the oracle accepts that is routed
/// here anyway — a deliberate over-refusal for reasons unrelated to this rule,
/// stated at that call site.
///
/// Scope: **both** plain and escaped binding names. An escaped leaf
/// (`let $x = 1` binds `$x`, its decoded name in the `escaped_name` channel) is
/// decoded via [`collect_decoded_binding_names`], so the six guarded positions — a declarator
/// leaf, a function-declaration id, a class-declaration id, a function-expression
/// id, an import specifier's local, and a catch-clause parameter — refuse the
/// escaped spelling exactly as the oracle does (it validates the DECODED
/// `node.name`). The paired [`refuse_dollar_binding_name`] likewise decodes. The
/// one `$`-prefixed binding position that deliberately still accepts its escaped
/// spelling is a class-EXPRESSION id — the oracle accepts a class-expression id qua
/// binding rule, so tsv keeps that site span-identity (its own `dollar_identifier_name`
/// check at the `ClassExpression` arm); see `docs/conformance_svelte_compiler.md`.
pub(crate) fn refuse_dollar_binding_pattern(
    view: AstView<'_>,
    pattern: &Expression,
    source: &str,
) -> Result<(), CompileError> {
    // One walk: `collect_decoded_binding_names` both enforces the shape gate
    // (erroring on a pattern the binding table can't enumerate, exactly as
    // `analyze::pattern_binding_names` does) and collects the DECODED leaf names —
    // decoding an escaped leaf `pattern_binding_names` would skip. The `$`-prefix
    // test then reads those decoded names.
    let mut names = Vec::new();
    collect_decoded_binding_names(view, pattern, source, &mut names)?;
    for name in names {
        if name.starts_with('$') {
            return Err(CompileError::Unsupported(Refusal::DollarPrefixedBinding {
                name,
            }));
        }
    }
    Ok(())
}

/// Collect every binding-leaf name a declarator-id / catch-param pattern declares,
/// DECODING escaped identifiers via `Identifier::name`, and refuse a shape the binding
/// table can't enumerate. The decode-aware companion to
/// [`crate::analyze::pattern_binding_names`], which reads the raw source slice and therefore
/// SKIPS an escaped leaf (`let $x = 1` binds `$x`, whose name lives in the
/// `escaped_name` channel, not the source slice); it carries the SAME shape gate (erroring with
/// `BindingPatternShape` on an unrecognized shape) so its sole caller needs only
/// this one walk. Kept in lockstep with that walk's pattern shapes — Identifier,
/// ObjectPattern/ObjectExpression property-values + rest, ArrayPattern elements +
/// rest, an AssignmentPattern's `left`, a RestElement's argument.
fn collect_decoded_binding_names(
    view: AstView<'_>,
    pattern: &Expression,
    source: &str,
    out: &mut Vec<String>,
) -> Result<(), CompileError> {
    match &pattern.kind {
        ExpressionKind::Identifier(id) => {
            out.push(id.name(source, view).to_string());
        }
        ExpressionKind::ObjectPattern(obj) => {
            for prop in view.pat_props(obj.properties) {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        collect_decoded_binding_names(view, view.expr(p.value), source, out)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        collect_decoded_binding_names(view, view.expr(rest.argument), source, out)?;
                    }
                }
            }
        }
        ExpressionKind::ObjectExpression(obj) => {
            for prop in view.obj_props(obj.properties) {
                match prop {
                    ObjectProperty::Property(p) => {
                        collect_decoded_binding_names(view, view.expr(p.value), source, out)?;
                    }
                    ObjectProperty::SpreadElement(s) => {
                        collect_decoded_binding_names(view, view.expr(s.argument), source, out)?;
                    }
                }
            }
        }
        ExpressionKind::ArrayPattern(arr) => {
            for &element in view.expr_opt_list(arr.elements).iter().flatten() {
                collect_decoded_binding_names(view, view.expr(element), source, out)?;
            }
        }
        ExpressionKind::AssignmentPattern(assign) => {
            collect_decoded_binding_names(view, view.expr(assign.left), source, out)?;
        }
        ExpressionKind::RestElement(rest) => {
            collect_decoded_binding_names(view, view.expr(rest.argument), source, out)?;
        }
        _ => {
            return Err(CompileError::Unsupported(Refusal::BindingPatternShape {
                kind: expression_kind(pattern),
            }));
        }
    }
    Ok(())
}

/// [`refuse_dollar_binding_pattern`] for a binding position that is always a
/// bare identifier (a function / class id, an import specifier's local).
///
/// Takes the source rather than a [`WalkCtx`], because two binding positions are
/// validated OUTSIDE the guard walk that owns one: a top-level class
/// declaration's id (`script_rewrite::rewrite_class_state_fields` intercepts the
/// statement before `walk_statement` sees it) and an import specifier's local
/// (`script_bindings::refuse_runes_invalid_import`, the oracle's own import
/// visitor's analog). A new interception point must call this too.
pub(crate) fn refuse_dollar_binding_name(
    view: AstView<'_>,
    id: &Identifier,
    source: &str,
) -> Result<(), CompileError> {
    // Decode via `Identifier::name` — the raw slice for a plain id, the escaped
    // name's pool text for an escaped one — so an escaped `$f` refuses exactly as
    // the oracle rejects the decoded `$f`.
    let name = id.name(source, view);
    if name.starts_with('$') {
        return Err(CompileError::Unsupported(Refusal::DollarPrefixedBinding {
            name: name.to_string(),
        }));
    }
    Ok(())
}

/// [`refuse_dollar_binding_name`] over every local of an import declaration.
///
/// An import's local IS a binding (`declaration_kind: 'import'`), and the
/// oracle's message names it too ("cannot be used for variables and imports").
/// Unlike the other `declare` in `scope.js` positions it is depth-INDEPENDENT:
/// `declare` in `scope.js` re-delegates an `import` declaration to the parent scope, so
/// the binding always lands at `function_depth` 0 and the `function_depth <= 1`
/// gate always passes.
///
/// ⚠️ The oracle EXEMPTS a type-only import from the rule
/// (`validate_identifier_name` in `scope.js`: an `importKind === 'type'` binding
/// is skipped), and this does not — it refuses every local unconditionally. tsv
/// still reaches parity on `import type { $T }` and `import { type $T }`
/// (probe-verified) for a reason this rule does not state: TYPE ERASURE runs
/// first and strips both forms before any import arrives here, so the exempt
/// case is unreachable rather than handled. That is a pass-ordering fact, not a
/// property of this check — if erasure ever stopped removing a type-only import
/// (or a non-`lang="ts"` path grew one), this would over-refuse it and the
/// exemption would have to be implemented for real.
///
/// Two callers, because imports are validated on two paths: [`walk_statement`]
/// here, and `script_bindings::refuse_runes_invalid_import`, for the imports the
/// transform hoists out of the statement stream before the guard walk runs.
pub(crate) fn refuse_dollar_import_locals(
    view: AstView<'_>,
    specifiers: &[ImportSpecifier],
    source: &str,
) -> Result<(), CompileError> {
    for specifier in specifiers {
        let local = match specifier {
            ImportSpecifier::Default(d) => &d.local,
            ImportSpecifier::Named(n) => &n.local,
            ImportSpecifier::Namespace(n) => &n.local,
        };
        refuse_dollar_binding_name(view, local, source)?;
    }
    Ok(())
}

/// Enter a function: one level deeper, its parameters (a run of rows held by
/// value) declared in the nested scope and walked.
fn enter_function(
    view: AstView<'_>,
    params: Run<Expression>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    ctx.fn_depth += 1;
    for param in view.exprs(params) {
        collect_nested_declared(view, param, ctx);
        walk_expression(view, param, ctx)?;
    }
    Ok(())
}

fn walk_statements(
    view: AstView<'_>,
    stmts: Run<StmtId>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for &stmt in view.stmt_list(stmts) {
        walk_stmt(view, stmt, ctx, depth)?;
    }
    Ok(())
}

/// [`walk_statement`] over the statement `stmt` names.
fn walk_stmt(
    view: AstView<'_>,
    stmt: StmtId,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    walk_statement(view, view.stmt(stmt), ctx, depth)
}

fn walk_statement(
    view: AstView<'_>,
    stmt: &Statement,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match &stmt.kind {
        StatementKind::ExpressionStatement(s) => walk(view, s.expression, ctx),
        StatementKind::VariableDeclaration(s) => walk_variable_declaration(view, s, ctx, depth),
        StatementKind::ReturnStatement(s) => walk_opt(view, s.argument, ctx),
        StatementKind::BlockStatement(s) => walk_statements(view, s.body, ctx, depth + 1),
        StatementKind::FunctionDeclaration(s) => {
            let s = view.fn_decl(*s);
            if let Some(id) = &s.id {
                refuse_dollar_binding_name(view, id, ctx.source)?;
            }
            if (depth > 0 || ctx.fn_depth > 0)
                && let Some(id) = &s.id
                && let Some(name) = identifier_name(id, ctx.source)
            {
                ctx.nested_declared.insert(name.to_string());
            }
            enter_function(view, s.params, ctx)?;
            let result = walk_statements(view, s.body.body, ctx, depth + 1);
            ctx.fn_depth -= 1;
            result
        }
        // Nothing else here is walkable — the source is a literal and the
        // imported name is a name-only position.
        StatementKind::ImportDeclaration(s) => refuse_dollar_import_locals(
            view,
            view.import_specs(view.import_decl(*s).specifiers),
            ctx.source,
        ),
        StatementKind::ClassDeclaration(s) => {
            let s = view.class_decl(*s);
            if let Some(id) = &s.id {
                refuse_dollar_binding_name(view, id, ctx.source)?;
            }
            walk_class_body(view, &s.body, ctx)
        }
        StatementKind::ExportNamedDeclaration(s) => match view.export_named(*s).declaration {
            Some(decl) => walk_stmt(view, decl, ctx, depth),
            None => Ok(()),
        },
        StatementKind::ExportDefaultDeclaration(s) => match &view.export_default(*s).declaration {
            ExportDefaultValue::Expression(e) => walk(view, *e, ctx),
            ExportDefaultValue::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    refuse_dollar_binding_name(view, id, ctx.source)?;
                }
                enter_function(view, f.params, ctx)?;
                let result = walk_statements(view, f.body.body, ctx, depth + 1);
                ctx.fn_depth -= 1;
                result
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    refuse_dollar_binding_name(view, id, ctx.source)?;
                }
                walk_class_body(view, &c.body, ctx)
            }
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => Ok(()),
        },
        StatementKind::IfStatement(s) => {
            walk(view, s.test, ctx)?;
            walk_stmt(view, s.consequent, ctx, depth + 1)?;
            match s.alternate {
                Some(alt) => walk_stmt(view, alt, ctx, depth + 1),
                None => Ok(()),
            }
        }
        StatementKind::ForStatement(s) => {
            match view.opt_for_head(s.init) {
                Some(ForInit::VariableDeclaration(decl)) => {
                    // For-scope declarations are block-scoped — always shadow
                    // candidates regardless of depth.
                    for declarator in view.declarators(decl.declarations) {
                        collect_nested_declared(view, view.expr(declarator.id), ctx);
                    }
                    walk_variable_declaration(view, decl, ctx, depth + 1)?;
                }
                Some(ForInit::Expression(e)) => walk(view, *e, ctx)?,
                None => {}
            }
            walk_opt(view, s.test, ctx)?;
            walk_opt(view, s.update, ctx)?;
            walk_stmt(view, s.body, ctx, depth + 1)
        }
        StatementKind::ForInStatement(s) => {
            walk_for_left(view, s.left, ctx, depth)?;
            walk(view, s.right, ctx)?;
            walk_stmt(view, s.body, ctx, depth + 1)
        }
        StatementKind::ForOfStatement(s) => {
            walk_for_left(view, s.left, ctx, depth)?;
            walk(view, s.right, ctx)?;
            walk_stmt(view, s.body, ctx, depth + 1)
        }
        StatementKind::WhileStatement(s) => {
            walk(view, s.test, ctx)?;
            walk_stmt(view, s.body, ctx, depth + 1)
        }
        StatementKind::DoWhileStatement(s) => {
            walk_stmt(view, s.body, ctx, depth + 1)?;
            walk(view, s.test, ctx)
        }
        // Unreachable in practice: a Svelte `<script>` is Module code, so it is strict
        // and the parser refuses `with` there.
        StatementKind::WithStatement(s) => {
            walk(view, s.object, ctx)?;
            walk_stmt(view, s.body, ctx, depth + 1)
        }
        StatementKind::SwitchStatement(s) => {
            walk(view, s.discriminant, ctx)?;
            for case in view.cases(s.cases) {
                walk_opt(view, case.test, ctx)?;
                walk_statements(view, case.consequent, ctx, depth + 1)?;
            }
            Ok(())
        }
        StatementKind::TryStatement(s) => {
            let s = view.try_stmt(*s);
            walk_statements(view, s.block.body, ctx, depth + 1)?;
            if let Some(handler) = view.opt_catch(s.handler) {
                if let Some(param) = &handler.param {
                    collect_nested_declared(view, param, ctx);
                    // A catch parameter IS a binding (`declaration_kind: 'let'`
                    // in a porous scope), so the `$$slots` reference carve-out
                    // must not reach it.
                    refuse_dollar_binding_pattern(view, param, ctx.source)?;
                    walk_expression(view, param, ctx)?;
                }
                walk_statements(view, handler.body.body, ctx, depth + 1)?;
            }
            if let Some(finalizer) = &s.finalizer {
                walk_statements(view, finalizer.body, ctx, depth + 1)?;
            }
            Ok(())
        }
        StatementKind::ThrowStatement(s) => walk(view, s.argument, ctx),
        StatementKind::LabeledStatement(s) => {
            walk_stmt(view, view.labeled_stmt(*s).body, ctx, depth + 1)
        }
        // No expression-bearing children.
        StatementKind::BreakStatement(_)
        | StatementKind::ContinueStatement(_)
        | StatementKind::EmptyStatement(_)
        | StatementKind::DebuggerStatement(_)
        | StatementKind::ExportAllDeclaration(_)
        | StatementKind::TSNamespaceExportDeclaration(_)
        | StatementKind::TSImportEqualsDeclaration(_)
        | StatementKind::TSTypeAliasDeclaration(_)
        | StatementKind::TSInterfaceDeclaration(_)
        | StatementKind::TSDeclareFunction(_) => Ok(()),
        // Unreachable in practice — type erasure runs first and either drops
        // these (a type-only namespace) or refuses them. Kept as defense in
        // depth: their bodies can carry initializer expressions the guard walk
        // isn't wired for, so refuse rather than under-guard.
        StatementKind::TSEnumDeclaration(_) => Err(CompileError::Unsupported(Refusal::TsEnum)),
        StatementKind::TSModuleDeclaration(_) => {
            Err(CompileError::Unsupported(Refusal::TsNamespaceWithValue))
        }
        StatementKind::TSExportAssignment(s) => walk(view, view.export_assign(*s).expression, ctx),
    }
}

fn walk_variable_declaration(
    view: AstView<'_>,
    decl: &VariableDeclaration,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for declarator in view.declarators(decl.declarations) {
        if depth > 0 || ctx.fn_depth > 0 {
            collect_nested_declared(view, view.expr(declarator.id), ctx);
        }
        refuse_dollar_binding_pattern(view, view.expr(declarator.id), ctx.source)?;
        walk(view, declarator.id, ctx)?;
        walk_opt(view, declarator.init, ctx)?;
    }
    Ok(())
}

fn walk_for_left(
    view: AstView<'_>,
    left: ForLeftId,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match view.for_left(left) {
        ForInOfLeft::VariableDeclaration(decl) => {
            for declarator in view.declarators(decl.declarations) {
                collect_nested_declared(view, view.expr(declarator.id), ctx);
            }
            walk_variable_declaration(view, decl, ctx, depth + 1)
        }
        ForInOfLeft::Pattern(pattern) => {
            let pattern = *pattern;
            assign_target_roots(view, view.expr(pattern), ctx.source, ctx.updated);
            // A `$derived` binding-leaf loop target (`for ([d] of …)`) is a derived
            // write the oracle lowers to an unimplemented shape — refuse. (A bare
            // `for (d of …)` target refuses here too; the oracle itself emits
            // invalid JS there, so a clean refusal only improves on it. A member
            // target `for (d.x of …)` reads the derived and compiles.)
            refuse_derived_write_target(view, view.expr(pattern), ctx)?;
            walk(view, pattern, ctx)
        }
    }
}

/// [`walk_expression`] over the expression `id` names.
fn walk(view: AstView<'_>, id: ExprId, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    walk_expression(view, view.expr(id), ctx)
}

fn walk_opt(
    view: AstView<'_>,
    expr: Option<ExprId>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    match expr {
        Some(e) => walk(view, e, ctx),
        None => Ok(()),
    }
}

fn walk_expressions(
    view: AstView<'_>,
    exprs: Run<ExprId>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    for &expr in view.expr_list(exprs) {
        walk(view, expr, ctx)?;
    }
    Ok(())
}

fn walk_function_expression(
    view: AstView<'_>,
    f: &FunctionExpression,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    if let Some(id) = &f.id {
        refuse_dollar_binding_name(view, id, ctx.source)?;
        if let Some(name) = identifier_name(id, ctx.source) {
            ctx.nested_declared.insert(name.to_string());
        }
    }
    enter_function(view, f.params, ctx)?;
    let result = walk_statements(view, f.body.body, ctx, 1);
    ctx.fn_depth -= 1;
    result
}

fn walk_class_body(
    view: AstView<'_>,
    body: &ClassBody,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    for member in view.class_members(body.body) {
        walk_class_member(view, member, ctx)?;
    }
    Ok(())
}

fn walk_class_member(
    view: AstView<'_>,
    member: &ClassMember,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    match member {
        ClassMember::MethodDefinition(m) => {
            if m.computed {
                walk(view, m.key, ctx)?;
            }
            walk_function_expression(view, &m.value, ctx)?;
        }
        ClassMember::PropertyDefinition(p) => {
            if p.computed {
                walk(view, p.key, ctx)?;
            }
            walk_opt(view, p.value, ctx)?;
        }
        ClassMember::StaticBlock(b) => {
            ctx.fn_depth += 1;
            let result = walk_statements(view, b.body, ctx, 1);
            ctx.fn_depth -= 1;
            result?;
        }
        ClassMember::IndexSignature(_) => {}
    }
    Ok(())
}

fn walk_expression(
    view: AstView<'_>,
    expr: &Expression,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    match &expr.kind {
        // The rune guard: any call/new whose callee roots in a `$`-identifier. A
        // store-base callee root (`$fn()`, `$obj.m()`, `new $C()`) is a store read
        // in callee position — EXEMPT it (the store rewrite descends into the
        // callee and rewrites it, exactly as for a bare read; the template path
        // already compiles `{$fn()}`), then recurse. A genuine rune callee stays
        // refused (`store_read_base` excludes `RUNE_BASES`, so `$state()` etc. are
        // never store bases), as does a shadowed base
        // (`store_invalid_scoped_subscription`).
        ExpressionKind::CallExpression(call) => {
            if let Some(name) = dollar_callee_root(view, view.expr(call.callee), ctx.source) {
                match store_read_exemption(ctx, name) {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None => return Err(rune_error(name)),
                }
            }
            walk(view, call.callee, ctx)?;
            walk_expressions(view, call.arguments, ctx)
        }
        ExpressionKind::NewExpression(new_expr) => {
            if let Some(name) = dollar_callee_root(view, view.expr(new_expr.callee), ctx.source) {
                match store_read_exemption(ctx, name) {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None => return Err(rune_error(name)),
                }
            }
            walk(view, new_expr.callee, ctx)?;
            walk_expressions(view, new_expr.arguments, ctx)
        }

        // A bare `$`-prefixed identifier reference (`let x = $state;`, a
        // `$store` subscription) is oracle-rejected input — refuse. A derived
        // binding read the template value-walk does not rewrite to `d()` (a
        // pattern default, a script position, an unsupported wrapper, or an
        // escaped-identifier read) refuses here. Name-only positions
        // (non-computed member properties / object keys) are never walked.
        ExpressionKind::Identifier(id) => {
            if let Some(name) = dollar_identifier_name(id, ctx.source) {
                // `$$slots` READ in a value position is a real runtime reference
                // (the transform injects `const $$slots = $.sanitize_slots(
                // $$props)`), not a rune — so it is exempt HERE and only here.
                // ⚠️ This test reads the NAME, not the position — the arm cannot
                // tell a reference from a binding, so it exempts both. Binding
                // positions DO reach it (a declarator id, a catch param, a
                // template `{#each … as}` pattern, a function parameter), and
                // this arm is not what makes them safe. The oracle's
                // `dollar_prefix_invalid` rejects a `$$slots` *declaration*
                // while accepting the reference, so a binding position the
                // oracle rejects must be refused UPSTREAM, before its pattern is
                // walked as an expression: `refuse_dollar_binding_pattern` /
                // `refuse_dollar_binding_name` at each such site (see the former
                // for the two declarator paths, which is where this was missed
                // twice). The positions the oracle genuinely EXEMPTS — a
                // function / arrow / snippet parameter, a template binding, and
                // (inside any function body only) a function-expression id or a
                // catch-clause parameter — deliberately have no upstream refusal
                // and land here. ⚠️ The dichotomy is not clean for those last
                // two: the oracle's exemption is depth-conditional, and tsv
                // refuses them upstream at EVERY depth, a deliberate
                // over-refusal. See `refuse_dollar_binding_pattern` for the
                // four oracle call paths and why the depth is not portable.
                //
                // So the standing obligation is on the caller, not on this arm:
                // a new site that walks a binding pattern through
                // `walk_expression_guarded` inherits the exemption and must
                // refuse first if the oracle rejects that position. Both upstream
                // refusals now DECODE via `Identifier::name`
                // (`refuse_dollar_binding_pattern` / `refuse_dollar_binding_name`),
                // so an ESCAPED `$` leaf (`let $x = 1`) refuses upstream exactly
                // as the plain spelling does — the class-EXPRESSION id being the
                // one deliberate exception (see the `ClassExpression` arm).
                if name != "$$slots" {
                    // A valid `$name` store read is exempt when the caller opts in
                    // (the script guard / the dropped-region guard): it is rewritten
                    // to `$.store_get(...)` by the store rewrite, or dropped with its
                    // region. A shadowed base is the oracle's
                    // `store_invalid_scoped_subscription` (refused here for the
                    // dropped guard; the script guard defers it to the store rewrite).
                    return match store_read_exemption(ctx, name) {
                        Some(result) => result,
                        None => Err(CompileError::Unsupported(
                            Refusal::DollarPrefixedIdentifier {
                                name: name.to_string(),
                            },
                        )),
                    };
                }
            }
            // A plain-name read of a `$derived` binding refuses UNLESS the caller
            // opts in (`allow_derived_reads`, the script-body guards): when
            // allowed, the read passes and the script-position rewrite
            // (`store_rewrite`) turns it into `d()`, exactly as the template value
            // walk does. A WRITE to the derived is handled at the assignment/update
            // arms (out of scope on every path); this arm only sees reads.
            if !ctx.allow_derived_reads
                && let Some(name) = identifier_name(id, ctx.source)
                && ctx.derived_names.contains(name)
            {
                return Err(CompileError::Unsupported(Refusal::DerivedBindingRead {
                    name: name.to_string(),
                }));
            }
            // An ESCAPED identifier (`d` → `d`) that decodes to a `$derived`
            // name is a derived read the oracle emits as `d()`; the template
            // value-walk can't rewrite an escaped read (classification not ported,
            // like needs_context/snippet), so refuse rather than emit a bare `d` —
            // a MISMATCH. A plain escaped local (not a derived name) stays legal.
            if let Some(escaped) = id.escaped_name {
                let name = view.escaped_text(escaped);
                if ctx.derived_names.contains(name) {
                    return Err(CompileError::Unsupported(Refusal::DerivedBindingRead {
                        name: name.to_string(),
                    }));
                }
            }
            Ok(())
        }

        // Leaves.
        ExpressionKind::Literal(_)
        | ExpressionKind::PrivateIdentifier(_)
        | ExpressionKind::RegexLiteral(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Super(_)
        | ExpressionKind::MetaProperty(_) => Ok(()),

        ExpressionKind::ObjectExpression(obj) => {
            for prop in view.obj_props(obj.properties) {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            walk(view, p.key, ctx)?;
                        }
                        walk(view, p.value, ctx)?;
                    }
                    ObjectProperty::SpreadElement(s) => walk(view, s.argument, ctx)?,
                }
            }
            Ok(())
        }
        ExpressionKind::ArrayExpression(arr) => {
            for &element in view.expr_opt_list(arr.elements) {
                walk_opt(view, element, ctx)?;
            }
            Ok(())
        }
        ExpressionKind::UnaryExpression(u) => walk(view, u.argument, ctx),
        ExpressionKind::UpdateExpression(u) => {
            assign_target_roots(view, view.expr(u.argument), ctx.source, ctx.updated);
            refuse_derived_write_target(view, view.expr(u.argument), ctx)?;
            walk(view, u.argument, ctx)
        }
        ExpressionKind::BinaryExpression(b) => {
            walk(view, b.left, ctx)?;
            walk(view, b.right, ctx)
        }
        ExpressionKind::MemberExpression(m) => {
            walk(view, m.object, ctx)?;
            if m.computed {
                walk(view, m.property, ctx)?;
            }
            Ok(())
        }
        ExpressionKind::ConditionalExpression(c) => {
            walk(view, c.test, ctx)?;
            walk(view, c.consequent, ctx)?;
            walk(view, c.alternate, ctx)
        }
        ExpressionKind::ArrowFunctionExpression(a) => {
            let a = view.arrow_fn(*a);
            enter_function(view, a.params, ctx)?;
            let result = match &a.body {
                ArrowFunctionBody::Expression(e) => walk(view, *e, ctx),
                ArrowFunctionBody::BlockStatement(b) => walk_statements(view, b.body, ctx, 1),
            };
            ctx.fn_depth -= 1;
            result
        }
        ExpressionKind::FunctionExpression(f) => {
            walk_function_expression(view, view.fn_expr(*f), ctx)
        }
        // A class EXPRESSION id is the one `$`-prefixed binding name the oracle
        // ACCEPTS (it declares no binding for it, so `dollar_prefix_invalid`
        // never fires) — this refusal is a deliberate over-refusal, not the
        // binding rule. Two reasons, both verified against the pinned compiler:
        // the oracle's reference analysis is name-based and counts the id as a
        // READ, so `class $$slots {}` injects `$.sanitize_slots` (a MISMATCH tsv
        // would otherwise produce), and `class $Foo {}` makes its store rewrite
        // emit `class $.store_get(…) {}` — invalid JS. Reproducing either is
        // worse than declining a shape no real component writes.
        //
        // ⚠️ Span-identity on purpose — NOT `refuse_dollar_binding_name` (which
        // decodes). The oracle accepts a class-expression id qua binding rule, so
        // the ESCAPED spelling (`class $Foo {}`) is DELIBERATELY left
        // compiling — it lands on the oracle's own mis-compile rather than a tsv
        // over-refusal, and is not one of the escaped over-acceptances the
        // `Identifier::name` decode closes. See `docs/conformance_svelte_compiler.md`.
        ExpressionKind::ClassExpression(c) => {
            let c = view.class_expr(*c);
            if let Some(id) = &c.id
                && let Some(name) = dollar_identifier_name(id, ctx.source)
            {
                return Err(CompileError::Unsupported(Refusal::DollarPrefixedBinding {
                    name: name.to_string(),
                }));
            }
            walk_class_body(view, &c.body, ctx)
        }
        ExpressionKind::SpreadElement(s) => walk(view, s.argument, ctx),
        ExpressionKind::TemplateLiteral(t) => walk_expressions(view, t.expressions, ctx),
        ExpressionKind::TaggedTemplateExpression(t) => {
            let t = view.tagged_tpl(*t);
            walk(view, t.tag, ctx)?;
            walk_expressions(view, t.quasi.expressions, ctx)
        }
        // Top-level/template `await` forces the oracle's async-component
        // shapes (blockers, thunked pushes) — not implemented, refuse. Inside
        // a nested function it is ordinary code and passes through.
        ExpressionKind::AwaitExpression(a) => {
            if ctx.fn_depth == 0 {
                return Err(CompileError::Unsupported(Refusal::TopLevelAwait));
            }
            walk(view, a.argument, ctx)
        }
        ExpressionKind::YieldExpression(y) => match y.argument {
            Some(argument) => walk(view, argument, ctx),
            None => Ok(()),
        },
        ExpressionKind::SequenceExpression(s) => walk_expressions(view, s.expressions, ctx),
        ExpressionKind::AssignmentExpression(a) => {
            assign_target_roots(view, view.expr(a.left), ctx.source, ctx.updated);
            refuse_derived_write_target(view, view.expr(a.left), ctx)?;
            walk(view, a.left, ctx)?;
            walk(view, a.right, ctx)
        }
        ExpressionKind::ObjectPattern(p) => {
            for prop in view.pat_props(p.properties) {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk(view, prop.key, ctx)?;
                        }
                        walk(view, prop.value, ctx)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        walk(view, rest.argument, ctx)?;
                    }
                }
            }
            Ok(())
        }
        ExpressionKind::ArrayPattern(p) => {
            for &element in view.expr_opt_list(p.elements) {
                walk_opt(view, element, ctx)?;
            }
            Ok(())
        }
        ExpressionKind::AssignmentPattern(p) => {
            walk(view, p.left, ctx)?;
            walk(view, p.right, ctx)
        }
        ExpressionKind::RestElement(r) => walk(view, r.argument, ctx),
        ExpressionKind::TSTypeAssertion(t) => walk(view, t.expression, ctx),
        ExpressionKind::TSAsExpression(t) => walk(view, t.expression, ctx),
        ExpressionKind::TSSatisfiesExpression(t) => walk(view, t.expression, ctx),
        ExpressionKind::TSInstantiationExpression(t) => walk(view, t.expression, ctx),
        ExpressionKind::TSNonNullExpression(t) => walk(view, t.expression, ctx),
        ExpressionKind::TSParameterProperty(t) => walk(view, t.parameter, ctx),
        ExpressionKind::ImportExpression(i) => {
            walk(view, i.source, ctx)?;
            match i.options {
                Some(options) => walk(view, options, ctx),
                None => Ok(()),
            }
        }
        ExpressionKind::CommentBoundPair(j) => walk(view, view.comment_pair(*j).inner, ctx),
        ExpressionKind::ParenthesizedExpression(p) => {
            walk_expression(view, p.unreachable_contents(view), ctx)
        }
    }
}
