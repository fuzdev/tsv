//! Rune refusal walk over borrowed script statements — plus the collection
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

use tsv_lang::{InfallibleResolve, SharedInterner};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, FunctionExpression, ImportSpecifier, ObjectPatternProperty, ObjectProperty, Statement,
};

use crate::analyze::{NameSet, expression_kind, pattern_binding_names};
use crate::{CompileError, Refusal};

/// The walk's shared state: the source names resolve against, the collection
/// sinks, the refusal set, and the interner (to decode an escaped identifier).
pub(crate) struct WalkCtx<'a> {
    pub source: &'a str,
    /// Assignment/update target root names (fed back as `updated` bindings).
    pub updated: &'a mut HashSet<String>,
    /// Names declared in nested function/block scopes (shadow candidates).
    pub nested_declared: &'a mut HashSet<String>,
    /// Derived binding names — reading one anywhere in walked code refuses.
    pub derived_names: &'a HashSet<String>,
    /// The parse's interner — to decode an escaped identifier's name (an owned
    /// `Rc<RefCell<…>>` clone, a cheap refcount bump, avoids a new lifetime).
    pub interner: SharedInterner,
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
    pub fn new(
        source: &'a str,
        updated: &'a mut HashSet<String>,
        nested_declared: &'a mut HashSet<String>,
        derived_names: &'a HashSet<String>,
        interner: SharedInterner,
    ) -> Self {
        Self {
            source,
            updated,
            nested_declared,
            derived_names,
            interner,
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

/// Walk one borrowed statement: refuse stray runes and derived reads, collect
/// mutations and nested declarations. `depth` is the statement nesting depth
/// (0 = a top-level script statement, whose declarations are the top bindings).
pub(crate) fn walk_statement_guarded(
    stmt: &Statement<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    walk_statement(stmt, ctx, depth)
}

/// Walk one expression (template expressions, rewritten declarator pieces).
pub(crate) fn walk_expression_guarded(
    expr: &Expression<'_>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    walk_expression(expr, ctx)
}

/// Walk one class member with the normal refusing rules — a method body, a
/// property init, a static block. Exposed so the class-field `$state` rewrite
/// ([`crate::script_rewrite`]) can guard EVERY member it does not unwrap through
/// the exact same path a class in any other position takes, keeping the
/// guard-exempt set (the unwrapped `$state` fields) equal to the transform set —
/// reach-matched by construction, so no member can be exempted without a matching
/// unwrap (which would emit an undefined `$state` reference — a MISMATCH).
pub(crate) fn walk_class_member_guarded(
    member: &ClassMember<'_>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    walk_class_member(member, ctx)
}

/// The `$`-prefixed name of a plain identifier, or `None`. Parsed identifiers
/// are span-identity (`escaped: None`); an interned (escaped) name is synthetic
/// (`$$renderer`, `$$props`, …) and never refused.
fn dollar_identifier_name<'s>(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &'s str,
) -> Option<&'s str> {
    let name = identifier_name(id, source)?;
    name.starts_with('$').then_some(name)
}

fn identifier_name<'s>(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &'s str,
) -> Option<&'s str> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(&source[start..start + id.name_len as usize])
}

/// The `$`-prefixed root-identifier name of a callee, peeled through member
/// accesses (`$state.raw`), non-null assertions, instantiations, and preserved
/// parens — `None` when the root is not a plain `$`-identifier.
fn dollar_callee_root<'s>(callee: &Expression<'_>, source: &'s str) -> Option<&'s str> {
    match callee {
        Expression::Identifier(id) => dollar_identifier_name(id, source),
        Expression::MemberExpression(member) => dollar_callee_root(member.object, source),
        Expression::TSNonNullExpression(non_null) => {
            dollar_callee_root(non_null.expression, source)
        }
        Expression::TSInstantiationExpression(inst) => dollar_callee_root(inst.expression, source),
        Expression::ParenthesizedExpression(paren) => dollar_callee_root(paren.expression, source),
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
pub(crate) fn assign_target_roots(target: &Expression<'_>, source: &str, out: &mut NameSet) {
    match target {
        Expression::Identifier(id) => {
            if let Some(name) = identifier_name(id, source) {
                out.insert(name.to_string());
            }
        }
        Expression::MemberExpression(m) => assign_target_roots(m.object, source, out),
        // All four TypeScript assignment-target wrappers the parser accepts
        // (`expression_assignable.rs`). LOAD-BEARING, not defense in depth: the
        // script's statements are erased before this walk, but the TEMPLATE's are
        // not — the Svelte AST is never rebuilt, so erasure happens per-expression
        // at the emitter's borrow points, and `needs_context`'s whole-component
        // reassignment collection walks the raw fragment. A missing arm silently
        // loses a reassignment root (`(x as any).y = 1` in a handler) and then
        // statically folds a mutated binding.
        Expression::TSNonNullExpression(t) => assign_target_roots(t.expression, source, out),
        Expression::TSAsExpression(t) => assign_target_roots(t.expression, source, out),
        Expression::TSSatisfiesExpression(t) => assign_target_roots(t.expression, source, out),
        Expression::TSTypeAssertion(t) => assign_target_roots(t.expression, source, out),
        Expression::ParenthesizedExpression(p) => assign_target_roots(p.expression, source, out),
        Expression::ObjectPattern(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        assign_target_roots(&p.value, source, out);
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        assign_target_roots(rest.argument, source, out);
                    }
                }
            }
        }
        Expression::ArrayPattern(arr) => {
            for element in arr.elements.iter().flatten() {
                assign_target_roots(element, source, out);
            }
        }
        Expression::AssignmentPattern(a) => assign_target_roots(a.left, source, out),
        Expression::RestElement(r) => assign_target_roots(r.argument, source, out),
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
/// [`AssignmentPattern`](Expression::AssignmentPattern) is a binding.
///
/// Keyed on `derived_names`, so in a dropped region (an empty set) it never fires
/// — a dropped-handler derived write compiles, unchanged.
fn refuse_derived_write_target(
    target: &Expression<'_>,
    ctx: &WalkCtx<'_>,
) -> Result<(), CompileError> {
    match target {
        // A bare-identifier binding target — a write to the derived itself.
        Expression::Identifier(id) => {
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
        Expression::MemberExpression(_) => {}
        // Destructuring-pattern targets: every slot is itself an assignment target,
        // so recurse into each binding-leaf position.
        Expression::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                refuse_derived_write_target(element, ctx)?;
            }
        }
        Expression::ObjectPattern(pattern) => {
            for prop in pattern.properties {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        refuse_derived_write_target(&p.value, ctx)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        refuse_derived_write_target(rest.argument, ctx)?;
                    }
                }
            }
        }
        // A default (`[d = 1] = …`): `left` is the binding, `right` the default
        // (a read the read rewrite / guard handles). Only the binding refuses.
        Expression::AssignmentPattern(pattern) => {
            refuse_derived_write_target(pattern.left, ctx)?;
        }
        Expression::RestElement(rest) => {
            refuse_derived_write_target(rest.argument, ctx)?;
        }
        Expression::ParenthesizedExpression(paren) => {
            refuse_derived_write_target(paren.expression, ctx)?;
        }
        _ => {}
    }
    Ok(())
}

/// Record the names a declaration pattern declares into `nested_declared`
/// (best-effort — unusual pattern shapes just record nothing extra).
fn collect_nested_declared(pattern: &Expression<'_>, ctx: &mut WalkCtx<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(pattern, ctx.source, &mut names).is_ok() {
        for name in names {
            ctx.nested_declared.insert(name);
        }
    }
}

/// Refuse a `$`-prefixed **binding name** — the oracle's `dollar_prefix_invalid`
/// (`phases/2-analyze/visitors/shared/utils.js:278`, literally
/// `node.name.startsWith('$')` on a `Binding`).
///
/// This is the rule that separates the two `$$slots` positions, and it is the
/// only thing that makes the reference carve-out in `walk_expression`'s
/// `Expression::Identifier` arm sound: a `$$slots` *reference* is the real
/// runtime value the transform injects, while a `$$slots` *declaration* is a
/// compile error. The rule is Svelte-domain, not a JS early error — `let $$slots
/// = 1` is valid JavaScript.
///
/// **The oracle reaches `validate_identifier_name` from FOUR sites, and only two
/// of them pass `function_depth`** — so "oracle-rejected" is not one answer for
/// the six guarded positions. Checked against every call path:
///
/// | call path | passes `function_depth`? | positions |
/// | --- | --- | --- |
/// | `VariableDeclarator.js:25`, `FunctionDeclaration.js:12`, `ClassDeclaration.js:12` | **no** — so `!function_depth` short-circuits and the gate never applies | a declarator's binding leaves (any depth, any declaration kind, destructured or not), a function-declaration id, a class-declaration id |
/// | `scope.js:695` (`Scope::declare`) | **yes** — the `function_depth <= 1` gate DOES apply | a function-expression id, a catch-clause parameter, an import specifier's local |
///
/// **Unconditionally oracle-rejected** (probe-verified against the pinned
/// compiler): the three no-depth positions above, plus an **import specifier's
/// local** — which goes through the depth-passing path but is safe by a
/// different mechanism, `scope.js:680` re-delegating `declaration_kind ===
/// 'import'` to the parent scope, so an import binding always lands at
/// `function_depth` 0 however deeply it is written.
///
/// ⚠️ **Two positions are oracle-rejected only at the top level, and tsv refuses
/// them unconditionally — a deliberate over-refusal.** A **function-expression
/// id** and a **catch-clause parameter** both declare through `scope.js:695`,
/// and the instance script's top-level scope sits at `function_depth` 1, so the
/// oracle rejects them there and *accepts* them inside any function body
/// (probe-verified both directions: `const g = function $$slots() {}` and `catch
/// ($$slots)` reject at top level, accept inside a `function f() { … }`; `catch
/// ($x)` with `x` imported likewise). tsv refuses both regardless of depth.
///
/// Narrowing them would need the oracle's depth, which tsv does not have:
/// [`WalkCtx::fn_depth`] counts function *nodes*, while the oracle's
/// non-porous increment happens at a function's **`BlockStatement`**
/// (`scope.js:1174-1188`) — a `FunctionExpression` / `ArrowFunctionExpression`
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
/// (`let $x = 1` binds `$x`, its decoded name in the interner) is decoded via
/// [`collect_decoded_binding_names`], so the six guarded positions — a declarator
/// leaf, a function-declaration id, a class-declaration id, a function-expression
/// id, an import specifier's local, and a catch-clause parameter — refuse the
/// escaped spelling exactly as the oracle does (it validates the DECODED
/// `node.name`). The paired [`refuse_dollar_binding_name`] likewise decodes. The
/// one `$`-prefixed binding position that deliberately still accepts its escaped
/// spelling is a class-EXPRESSION id — the oracle accepts a class-expression id qua
/// binding rule, so tsv keeps that site span-identity (its own `dollar_identifier_name`
/// check at the `ClassExpression` arm); see `docs/conformance_svelte_compiler.md`.
pub(crate) fn refuse_dollar_binding_pattern(
    pattern: &Expression<'_>,
    source: &str,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    // One walk: `collect_decoded_binding_names` both enforces the shape gate
    // (erroring on a pattern the binding table can't enumerate, exactly as
    // `analyze::pattern_binding_names` does) and collects the DECODED leaf names —
    // decoding an escaped leaf `pattern_binding_names` would skip. The `$`-prefix
    // test then reads those decoded names.
    let mut names = Vec::new();
    collect_decoded_binding_names(pattern, source, interner, &mut names)?;
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
/// DECODING escaped identifiers via the interner, and refuse a shape the binding
/// table can't enumerate. The decode-aware companion to
/// [`crate::analyze::pattern_binding_names`], which is interner-free and therefore
/// SKIPS an escaped leaf (`let $x = 1` binds `$x`, whose name lives in the
/// interner, not the source slice); it carries the SAME shape gate (erroring with
/// `BindingPatternShape` on an unrecognized shape) so its sole caller needs only
/// this one walk. Kept in lockstep with that walk's pattern shapes — Identifier,
/// ObjectPattern/ObjectExpression property-values + rest, ArrayPattern elements +
/// rest, an AssignmentPattern's `left`, a RestElement's argument.
fn collect_decoded_binding_names(
    pattern: &Expression<'_>,
    source: &str,
    interner: &SharedInterner,
    out: &mut Vec<String>,
) -> Result<(), CompileError> {
    match pattern {
        Expression::Identifier(id) => {
            out.push(id.name(source, &interner.borrow()).to_string());
        }
        Expression::ObjectPattern(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        collect_decoded_binding_names(&p.value, source, interner, out)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        collect_decoded_binding_names(rest.argument, source, interner, out)?;
                    }
                }
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        collect_decoded_binding_names(&p.value, source, interner, out)?;
                    }
                    ObjectProperty::SpreadElement(s) => {
                        collect_decoded_binding_names(s.argument, source, interner, out)?;
                    }
                }
            }
        }
        Expression::ArrayPattern(arr) => {
            for element in arr.elements.iter().flatten() {
                collect_decoded_binding_names(element, source, interner, out)?;
            }
        }
        Expression::AssignmentPattern(assign) => {
            collect_decoded_binding_names(assign.left, source, interner, out)?;
        }
        Expression::RestElement(rest) => {
            collect_decoded_binding_names(rest.argument, source, interner, out)?;
        }
        other => {
            return Err(CompileError::Unsupported(Refusal::BindingPatternShape {
                kind: expression_kind(other),
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
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &str,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    // Decode via the interner (`Identifier::name`) — the raw slice for a plain id,
    // the interned decoded form for an escaped one — so an escaped `$f` refuses
    // exactly as the oracle rejects the decoded `$f`. The `Ref` is held across the
    // `starts_with` + `to_string` so the borrowed `&str` outlives both.
    let borrow = interner.borrow();
    let name = id.name(source, &borrow);
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
/// Unlike the other `scope.js:695` positions it is depth-INDEPENDENT:
/// `scope.js:680` re-delegates an `import` declaration to the parent scope, so
/// the binding always lands at `function_depth` 0 and the `function_depth <= 1`
/// gate always passes.
///
/// ⚠️ The oracle EXEMPTS a type-only import from the rule (`utils.js:270-276`:
/// an `importKind === 'type'` binding is skipped), and this does not — it
/// refuses every local unconditionally. tsv still reaches parity on
/// `import type { $T }` and `import { type $T }` (probe-verified) for a reason
/// this rule does not state: TYPE ERASURE runs first and strips both forms
/// before any import arrives here, so the exempt case is unreachable rather than
/// handled. That is a pass-ordering fact, not a property of this check — if
/// erasure ever stopped removing a type-only import (or a non-`lang="ts"` path
/// grew one), this would over-refuse it and the exemption would have to be
/// implemented for real.
///
/// Two callers, because imports are validated on two paths: [`walk_statement`]
/// here, and `script_bindings::refuse_runes_invalid_import`, for the imports the
/// transform hoists out of the statement stream before the guard walk runs.
pub(crate) fn refuse_dollar_import_locals(
    specifiers: &[ImportSpecifier<'_>],
    source: &str,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    for specifier in specifiers {
        let local = match specifier {
            ImportSpecifier::Default(d) => &d.local,
            ImportSpecifier::Named(n) => &n.local,
            ImportSpecifier::Namespace(n) => &n.local,
        };
        refuse_dollar_binding_name(local, source, interner)?;
    }
    Ok(())
}

fn enter_function(params: &[Expression<'_>], ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    ctx.fn_depth += 1;
    for param in params {
        collect_nested_declared(param, ctx);
        walk_expression(param, ctx)?;
    }
    Ok(())
}

fn walk_statements(
    stmts: &[Statement<'_>],
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for stmt in stmts {
        walk_statement(stmt, ctx, depth)?;
    }
    Ok(())
}

fn walk_statement(
    stmt: &Statement<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match stmt {
        Statement::ExpressionStatement(s) => walk_expression(&s.expression, ctx),
        Statement::VariableDeclaration(s) => walk_variable_declaration(s, ctx, depth),
        Statement::ReturnStatement(s) => walk_opt(s.argument.as_ref(), ctx),
        Statement::BlockStatement(s) => walk_statements(s.body, ctx, depth + 1),
        Statement::FunctionDeclaration(s) => {
            if let Some(id) = &s.id {
                refuse_dollar_binding_name(id, ctx.source, &ctx.interner)?;
            }
            if (depth > 0 || ctx.fn_depth > 0)
                && let Some(id) = &s.id
                && let Some(name) = identifier_name(id, ctx.source)
            {
                ctx.nested_declared.insert(name.to_string());
            }
            enter_function(s.params, ctx)?;
            let result = walk_statements(s.body.body, ctx, depth + 1);
            ctx.fn_depth -= 1;
            result
        }
        // Nothing else here is walkable — the source is a literal and the
        // imported name is a name-only position.
        Statement::ImportDeclaration(s) => {
            refuse_dollar_import_locals(s.specifiers, ctx.source, &ctx.interner)
        }
        Statement::ClassDeclaration(s) => {
            if let Some(id) = &s.id {
                refuse_dollar_binding_name(id, ctx.source, &ctx.interner)?;
            }
            walk_class_body(&s.body, ctx)
        }
        Statement::ExportNamedDeclaration(s) => match &s.declaration {
            Some(decl) => walk_statement(decl, ctx, depth),
            None => Ok(()),
        },
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => walk_expression(e, ctx),
            ExportDefaultValue::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    refuse_dollar_binding_name(id, ctx.source, &ctx.interner)?;
                }
                enter_function(f.params, ctx)?;
                let result = walk_statements(f.body.body, ctx, depth + 1);
                ctx.fn_depth -= 1;
                result
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    refuse_dollar_binding_name(id, ctx.source, &ctx.interner)?;
                }
                walk_class_body(&c.body, ctx)
            }
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => Ok(()),
        },
        Statement::IfStatement(s) => {
            walk_expression(&s.test, ctx)?;
            walk_statement(s.consequent, ctx, depth + 1)?;
            match s.alternate {
                Some(alt) => walk_statement(alt, ctx, depth + 1),
                None => Ok(()),
            }
        }
        Statement::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(decl)) => {
                    // For-scope declarations are block-scoped — always shadow
                    // candidates regardless of depth.
                    for declarator in decl.declarations {
                        collect_nested_declared(&declarator.id, ctx);
                    }
                    walk_variable_declaration(decl, ctx, depth + 1)?;
                }
                Some(ForInit::Expression(e)) => walk_expression(e, ctx)?,
                None => {}
            }
            walk_opt(s.test.as_ref(), ctx)?;
            walk_opt(s.update.as_ref(), ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::ForInStatement(s) => {
            walk_for_left(&s.left, ctx, depth)?;
            walk_expression(&s.right, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::ForOfStatement(s) => {
            walk_for_left(&s.left, ctx, depth)?;
            walk_expression(&s.right, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::WhileStatement(s) => {
            walk_expression(&s.test, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::DoWhileStatement(s) => {
            walk_statement(s.body, ctx, depth + 1)?;
            walk_expression(&s.test, ctx)
        }
        Statement::SwitchStatement(s) => {
            walk_expression(&s.discriminant, ctx)?;
            for case in s.cases {
                walk_opt(case.test.as_ref(), ctx)?;
                walk_statements(case.consequent, ctx, depth + 1)?;
            }
            Ok(())
        }
        Statement::TryStatement(s) => {
            walk_statements(s.block.body, ctx, depth + 1)?;
            if let Some(handler) = &s.handler {
                if let Some(param) = &handler.param {
                    collect_nested_declared(param, ctx);
                    // A catch parameter IS a binding (`declaration_kind: 'let'`
                    // in a porous scope), so the `$$slots` reference carve-out
                    // must not reach it.
                    refuse_dollar_binding_pattern(param, ctx.source, &ctx.interner)?;
                    walk_expression(param, ctx)?;
                }
                walk_statements(handler.body.body, ctx, depth + 1)?;
            }
            if let Some(finalizer) = &s.finalizer {
                walk_statements(finalizer.body, ctx, depth + 1)?;
            }
            Ok(())
        }
        Statement::ThrowStatement(s) => walk_expression(&s.argument, ctx),
        Statement::LabeledStatement(s) => walk_statement(s.body, ctx, depth + 1),
        // No expression-bearing children.
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_) => Ok(()),
        // Unreachable in practice — type erasure runs first and either drops
        // these (a type-only namespace) or refuses them. Kept as defense in
        // depth: their bodies can carry initializer expressions the guard walk
        // isn't wired for, so refuse rather than under-guard.
        Statement::TSEnumDeclaration(_) => Err(CompileError::Unsupported(Refusal::TsEnum)),
        Statement::TSModuleDeclaration(_) => {
            Err(CompileError::Unsupported(Refusal::TsNamespaceWithValue))
        }
        Statement::TSExportAssignment(s) => walk_expression(&s.expression, ctx),
    }
}

fn walk_variable_declaration(
    decl: &tsv_ts::ast::internal::VariableDeclaration<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for declarator in decl.declarations {
        if depth > 0 || ctx.fn_depth > 0 {
            collect_nested_declared(&declarator.id, ctx);
        }
        refuse_dollar_binding_pattern(&declarator.id, ctx.source, &ctx.interner)?;
        walk_expression(&declarator.id, ctx)?;
        walk_opt(declarator.init.as_ref(), ctx)?;
    }
    Ok(())
}

fn walk_for_left(
    left: &ForInOfLeft<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match left {
        ForInOfLeft::VariableDeclaration(decl) => {
            for declarator in decl.declarations {
                collect_nested_declared(&declarator.id, ctx);
            }
            walk_variable_declaration(decl, ctx, depth + 1)
        }
        ForInOfLeft::Pattern(pattern) => {
            assign_target_roots(pattern, ctx.source, ctx.updated);
            // A `$derived` binding-leaf loop target (`for ([d] of …)`) is a derived
            // write the oracle lowers to an unimplemented shape — refuse. (A bare
            // `for (d of …)` target refuses here too; the oracle itself emits
            // invalid JS there, so a clean refusal only improves on it. A member
            // target `for (d.x of …)` reads the derived and compiles.)
            refuse_derived_write_target(pattern, ctx)?;
            walk_expression(pattern, ctx)
        }
    }
}

fn walk_opt(expr: Option<&Expression<'_>>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    match expr {
        Some(e) => walk_expression(e, ctx),
        None => Ok(()),
    }
}

fn walk_expressions(exprs: &[Expression<'_>], ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    for expr in exprs {
        walk_expression(expr, ctx)?;
    }
    Ok(())
}

fn walk_function_expression(
    f: &FunctionExpression<'_>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    if let Some(id) = &f.id {
        refuse_dollar_binding_name(id, ctx.source, &ctx.interner)?;
        if let Some(name) = identifier_name(id, ctx.source) {
            ctx.nested_declared.insert(name.to_string());
        }
    }
    enter_function(f.params, ctx)?;
    let result = walk_statements(f.body.body, ctx, 1);
    ctx.fn_depth -= 1;
    result
}

fn walk_class_body(body: &ClassBody<'_>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    for member in body.body {
        walk_class_member(member, ctx)?;
    }
    Ok(())
}

fn walk_class_member(member: &ClassMember<'_>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    match member {
        ClassMember::MethodDefinition(m) => {
            if m.computed {
                walk_expression(&m.key, ctx)?;
            }
            walk_function_expression(&m.value, ctx)?;
        }
        ClassMember::PropertyDefinition(p) => {
            if p.computed {
                walk_expression(&p.key, ctx)?;
            }
            walk_opt(p.value.as_ref(), ctx)?;
        }
        ClassMember::StaticBlock(b) => {
            ctx.fn_depth += 1;
            let result = walk_statements(b.body, ctx, 1);
            ctx.fn_depth -= 1;
            result?;
        }
        ClassMember::IndexSignature(_) => {}
    }
    Ok(())
}

fn walk_expression(expr: &Expression<'_>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    match expr {
        // The rune guard: any call/new whose callee roots in a `$`-identifier. A
        // store-base callee root (`$fn()`, `$obj.m()`, `new $C()`) is a store read
        // in callee position — EXEMPT it (the store rewrite descends into the
        // callee and rewrites it, exactly as for a bare read; the template path
        // already compiles `{$fn()}`), then recurse. A genuine rune callee stays
        // refused (`store_read_base` excludes `RUNE_BASES`, so `$state()` etc. are
        // never store bases), as does a shadowed base
        // (`store_invalid_scoped_subscription`).
        Expression::CallExpression(call) => {
            if let Some(name) = dollar_callee_root(call.callee, ctx.source) {
                match store_read_exemption(ctx, name) {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None => return Err(rune_error(name)),
                }
            }
            walk_expression(call.callee, ctx)?;
            walk_expressions(call.arguments, ctx)
        }
        Expression::NewExpression(new_expr) => {
            if let Some(name) = dollar_callee_root(new_expr.callee, ctx.source) {
                match store_read_exemption(ctx, name) {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None => return Err(rune_error(name)),
                }
            }
            walk_expression(new_expr.callee, ctx)?;
            walk_expressions(new_expr.arguments, ctx)
        }

        // A bare `$`-prefixed identifier reference (`let x = $state;`, a
        // `$store` subscription) is oracle-rejected input — refuse. A derived
        // binding read the template value-walk does not rewrite to `d()` (a
        // pattern default, a script position, an unsupported wrapper, or an
        // escaped-identifier read) refuses here. Name-only positions
        // (non-computed member properties / object keys) are never walked.
        Expression::Identifier(id) => {
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
                // refusals now DECODE via the interner
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
            if let Some(sym) = id.escaped_name {
                let interner = ctx.interner.borrow();
                let name = interner.resolve_infallible(sym);
                if ctx.derived_names.contains(name) {
                    return Err(CompileError::Unsupported(Refusal::DerivedBindingRead {
                        name: name.to_string(),
                    }));
                }
            }
            Ok(())
        }

        // Leaves.
        Expression::Literal(_)
        | Expression::PrivateIdentifier(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => Ok(()),

        Expression::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            walk_expression(&p.key, ctx)?;
                        }
                        walk_expression(&p.value, ctx)?;
                    }
                    ObjectProperty::SpreadElement(s) => walk_expression(s.argument, ctx)?,
                }
            }
            Ok(())
        }
        Expression::ArrayExpression(arr) => {
            for element in arr.elements {
                walk_opt(element.as_ref(), ctx)?;
            }
            Ok(())
        }
        Expression::UnaryExpression(u) => walk_expression(u.argument, ctx),
        Expression::UpdateExpression(u) => {
            assign_target_roots(u.argument, ctx.source, ctx.updated);
            refuse_derived_write_target(u.argument, ctx)?;
            walk_expression(u.argument, ctx)
        }
        Expression::BinaryExpression(b) => {
            walk_expression(b.left, ctx)?;
            walk_expression(b.right, ctx)
        }
        Expression::MemberExpression(m) => {
            walk_expression(m.object, ctx)?;
            if m.computed {
                walk_expression(m.property, ctx)?;
            }
            Ok(())
        }
        Expression::ConditionalExpression(c) => {
            walk_expression(c.test, ctx)?;
            walk_expression(c.consequent, ctx)?;
            walk_expression(c.alternate, ctx)
        }
        Expression::ArrowFunctionExpression(a) => {
            enter_function(a.params, ctx)?;
            let result = match &a.body {
                ArrowFunctionBody::Expression(e) => walk_expression(e, ctx),
                ArrowFunctionBody::BlockStatement(b) => walk_statements(b.body, ctx, 1),
            };
            ctx.fn_depth -= 1;
            result
        }
        Expression::FunctionExpression(f) => walk_function_expression(f, ctx),
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
        // interner decode closes. See `docs/conformance_svelte_compiler.md`.
        Expression::ClassExpression(c) => {
            if let Some(id) = &c.id
                && let Some(name) = dollar_identifier_name(id, ctx.source)
            {
                return Err(CompileError::Unsupported(Refusal::DollarPrefixedBinding {
                    name: name.to_string(),
                }));
            }
            walk_class_body(&c.body, ctx)
        }
        Expression::SpreadElement(s) => walk_expression(s.argument, ctx),
        Expression::TemplateLiteral(t) => walk_expressions(t.expressions, ctx),
        Expression::TaggedTemplateExpression(t) => {
            walk_expression(t.tag, ctx)?;
            walk_expressions(t.quasi.expressions, ctx)
        }
        // Top-level/template `await` forces the oracle's async-component
        // shapes (blockers, thunked pushes) — not implemented, refuse. Inside
        // a nested function it is ordinary code and passes through.
        Expression::AwaitExpression(a) => {
            if ctx.fn_depth == 0 {
                return Err(CompileError::Unsupported(Refusal::TopLevelAwait));
            }
            walk_expression(a.argument, ctx)
        }
        Expression::YieldExpression(y) => match y.argument {
            Some(argument) => walk_expression(argument, ctx),
            None => Ok(()),
        },
        Expression::SequenceExpression(s) => walk_expressions(s.expressions, ctx),
        Expression::AssignmentExpression(a) => {
            assign_target_roots(a.left, ctx.source, ctx.updated);
            refuse_derived_write_target(a.left, ctx)?;
            walk_expression(a.left, ctx)?;
            walk_expression(a.right, ctx)
        }
        Expression::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk_expression(&prop.key, ctx)?;
                        }
                        walk_expression(&prop.value, ctx)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        walk_expression(rest.argument, ctx)?;
                    }
                }
            }
            Ok(())
        }
        Expression::ArrayPattern(p) => {
            for element in p.elements {
                walk_opt(element.as_ref(), ctx)?;
            }
            Ok(())
        }
        Expression::AssignmentPattern(p) => {
            walk_expression(p.left, ctx)?;
            walk_expression(p.right, ctx)
        }
        Expression::RestElement(r) => walk_expression(r.argument, ctx),
        Expression::TSTypeAssertion(t) => walk_expression(t.expression, ctx),
        Expression::TSAsExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSSatisfiesExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSInstantiationExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSNonNullExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSParameterProperty(t) => walk_expression(t.parameter, ctx),
        Expression::ImportExpression(i) => {
            walk_expression(i.source, ctx)?;
            match i.options {
                Some(options) => walk_expression(options, ctx),
                None => Ok(()),
            }
        }
        Expression::JsdocCast(j) => walk_expression(j.inner, ctx),
        Expression::ParenthesizedExpression(p) => walk_expression(p.expression, ctx),
    }
}
