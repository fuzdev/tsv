//! `bind:` directive resolution and emission — the validity fork, its inline
//! `$.attr(…)` emission, and its spread object property.
//!
//! A **per-node emitter** in the emission pipeline, and the most self-contained
//! one: it calls into neither [`crate::attribute`]'s value shaping nor
//! [`crate::attribute_class_style`]'s directive builders. [`crate::element`]
//! handles a `bind:` inline at its source slot via [`emit_bind_directive`], routes
//! the spread path through [`build_bind_object_property`], and
//! [`crate::dropped`] validates an SSR-inert special element's bind through
//! [`validate_inert_bind_target`].
//!
//! **Single source of truth** for the bind validity fork
//! ([`resolve_bind_directive`]), which the inline and spread paths both read so the
//! two can never drift — a divergence there would emit a `value`/`checked` property
//! on one path and refuse on the other for the same authored bind. The
//! [`OMIT_IN_SSR_BINDS`] list is likewise one place: the oracle skips those with an
//! early `continue` *before* it visits the target, so a copy that drifted would
//! either emit output the oracle omits or refuse a bind it accepts.
//!
//! See [`crate::transform_server`] for the orchestration.

use bumpalo::collections::Vec as BumpVec;
use tsv_svelte::ast::internal::{Attribute, AttributeNode, AttributeValue, BindDirective, Element};
use tsv_ts::ast::internal::{BinaryOperator, Expression, Property};

use crate::attribute::escape_html_attr;
use crate::body_builder::BodyBuilder;
use crate::build::init_property;
use crate::script_decls::plain_identifier_name;
use crate::template_value::wrap_value_expr;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// The `bind:` names Svelte's `binding_properties` marks `omit_in_ssr` — media,
/// dimension, window, and readback bindings (`bindings.js`) that produce **no** SSR
/// output. The oracle's server `BindDirective` handling skips them with an early
/// `continue`, before it visits the target (`shared/element.js`). `this` is
/// `omit_in_ssr` too but is handled separately (it carries `{get, set}`/lvalue
/// validation and skips on both the inline and spread paths), so it is excluded
/// here. Membership is by exact (case-sensitive) name, matching
/// `binding_properties[attribute.name]`.
const OMIT_IN_SSR_BINDS: &[&str] = &[
    "currentTime",
    "duration",
    "paused",
    "buffered",
    "seekable",
    "played",
    "volume",
    "muted",
    "playbackRate",
    "seeking",
    "ended",
    "readyState",
    "videoHeight",
    "videoWidth",
    "naturalWidth",
    "naturalHeight",
    "activeElement",
    "fullscreenElement",
    "pointerLockElement",
    "visibilityState",
    "innerWidth",
    "innerHeight",
    "outerWidth",
    "outerHeight",
    "scrollX",
    "scrollY",
    "online",
    "devicePixelRatio",
    "clientWidth",
    "clientHeight",
    "offsetWidth",
    "offsetHeight",
    "contentRect",
    "contentBoxSize",
    "borderBoxSize",
    "devicePixelContentBoxSize",
    "indeterminate",
    "files",
];

/// Refuse a `bind:` directive with the collapsing `bind: directive {name}` bucket.
fn refuse_bind<T>(name: &str) -> Result<T, CompileError> {
    Err(unsupported(Refusal::BindDirective {
        name: name.to_string(),
    }))
}

/// The `type` attribute's shape on a `bind:`-bearing `<input>` — the split the
/// oracle keys on (`is_text_attribute`): absent, a bare boolean `type`, a static
/// text value (`checkbox`/`file`/…), or a dynamic/mixed expression value.
enum InputType {
    None,
    Bare,
    Static(String),
    Dynamic,
}

/// Classify the `<input>`'s `type` attribute (case-sensitive `type`, mirroring the
/// oracle's `attr.name === 'type'` — a `TYPE` would not be found).
fn classify_input_type(env: &EmitEnv<'_, '_>, element: &Element<'_>) -> InputType {
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let is_type = attr.name(env.source) == "type";
        if !is_type {
            continue;
        }
        return match attr.value {
            None => InputType::Bare,
            Some([AttributeValue::Text(text)]) => {
                InputType::Static(text.data(env.source).into_owned())
            }
            Some(_) => InputType::Dynamic,
        };
    }
    InputType::None
}

/// The root identifier name of a bind target that is an `Identifier` or a member
/// chain rooted at one (`v`, `obj.x`, `a[i].b`); `None` for any other shape (a
/// call, literal, binary — not an lvalue root). Mirrors the oracle's `object()`.
///
/// An **optional** link anywhere in the chain (`o?.el`, `o?.a.b`) yields `None`:
/// acorn wraps a chain containing one in a `ChainExpression`, so the oracle's
/// `bind_invalid_expression` test — which admits only `Identifier` /
/// `MemberExpression` / a `{get, set}` `SequenceExpression` — rejects it. tsv's
/// internal AST has no chain wrapper (the flag rides each member), so the refusal
/// is expressed by refusing any optional link; the recursion propagates it from a
/// deeper link up.
fn bind_target_root(expr: &Expression<'_>, source: &str) -> Option<String> {
    match expr {
        Expression::Identifier(id) => plain_identifier_name(id, source),
        Expression::MemberExpression(member) if !member.optional => {
            bind_target_root(member.object, source)
        }
        _ => None,
    }
}

/// A `{get, set}` bind target: a `SequenceExpression` of exactly two expressions
/// (`() => x, (v) => x = v`), the third valid bind form the oracle accepts beside
/// an Identifier / member chain (`SequenceExpression` + `expressions.length === 2`
/// in its `BindDirective` analysis). Recognized only so `bind:this` can OMIT it at
/// parity — the value/checked/group arms still refuse it (no attr value to emit).
fn is_get_set_pair(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::SequenceExpression(seq) if seq.expressions.len() == 2)
}

/// The bind target's root name, but only when that root is something the oracle
/// will let a `bind:` WRITE to. `None` when the expression is not an
/// `Identifier`/member-chain lvalue at all, **or** when its root is one the oracle
/// rejects with `constant_binding`.
///
/// The oracle keys that rejection on the DECLARATION KEYWORD, not on inferred
/// reassignability (`phases/2-analyze/visitors/shared/utils.js:82-105`): an
/// `import` local, or a `const` binding whose kind is not `each`. So a
/// `const c = $state(0)` is refused as a bind target even though it is reactive —
/// which is exactly why `env.unassignable_names` is its own set rather than a
/// filter on `state_names` (that set also drives component-dynamic classification,
/// where the same binding IS dynamic).
///
/// This is the single seam every `bind:` target check shares — the `$state`-gated
/// ones ([`emitted_bind_target`], [`validate_inert_bind_target`]) and the
/// `bind:this` branches, which take any lvalue with no `$state` gate and so would
/// otherwise accept a `const`/import target unchallenged.
///
/// ⚠️ The set it consults ([`crate::transform_server::EmitEnv::unassignable_names`])
/// is keyed on **top-level script statements only**, so this seam enforces the
/// oracle's `constant_binding` for a top-level `const`/import and is BLIND to a
/// TEMPLATE-scoped one — a `{@const}` name, a `{:then}`/`{:catch}` value
/// (`scope.js:1310`/`:1324`), an `{#each}` index (`:1273`), each equally
/// `declaration_kind: 'const'` (kind `'template'`/`'static'`, not `'each'`).
///
/// That blindness is no longer a gap, because this is not the only enforcement
/// point: `needs_context`'s whole-component walk reaches every `bind:` too
/// (`BindDirective.js:181` is the oracle's own third caller of the same validator)
/// and applies the rule from its scoped `template_consts` set. So a bind to a
/// template-scoped const refuses THERE, not here. Keep the two straight when
/// changing either — a shape this seam accepts is not thereby accepted by tsv.
fn reassignable_bind_target_root(env: &EmitEnv<'_, '_>, expr: &Expression<'_>) -> Option<String> {
    let name = bind_target_root(expr, env.source)?;
    // The rejection applies to a BARE identifier target only. The oracle's
    // `validate_no_const_assignment` recurses through array/object PATTERNS and
    // tests `argument.type === 'Identifier'`; a `MemberExpression` argument matches
    // no branch and falls through with no error. That is not an oversight — writing
    // through a const binding (`const o = $state({v: ''})` + `bind:value={o.v}`)
    // MUTATES the object and never rebinds the name, so only rebinding the name
    // itself is refused. Walking to the member-chain root here instead would
    // over-refuse a common shape.
    if matches!(expr, Expression::Identifier(_)) && env.unassignable_names.contains(&name) {
        return None;
    }
    Some(name)
}

fn emitted_bind_target<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    bind_name: &str,
) -> Result<Expression<'arena>, CompileError> {
    // The template borrow point: erase the bind target once, then gate its erased
    // shape (an `x as T` target is `x`, an assignable lvalue).
    let expr = env.erase(&directive.expression)?;
    match reassignable_bind_target_root(env, expr) {
        Some(name) if env.state_names.contains(&name) => {}
        _ => return refuse_bind(bind_name),
    }
    Ok(wrap_value_expr(env, expr)?[0].clone())
}

/// Validate the TARGET of a `bind:` on an SSR-inert special element
/// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`) — **validation only**
/// (the bind is dropped from SSR output, no `$.attr` is emitted; the bind NAME is
/// already whitelisted by the caller). Reproduces the SAME reassignable-lvalue rule
/// regular elements enforce, reusing the shared primitives so the two never drift:
///
/// - `bind:this` accepts any reassignable `Identifier`/member-chain lvalue or a
///   `{get, set}` pair, no `$state` gate — exactly [`resolve_bind_directive`]'s
///   `this` branch.
/// - every other (whitelisted-name) bind requires a `$state`-rooted
///   `Identifier`/member lvalue — the SAFE gate [`emitted_bind_target`] applies,
///   over-refusing a prop / plain reassignable `let` / each binding (which the
///   oracle accepts) exactly as the regular path does. A non-lvalue (call / literal
///   / logical) and a plain (non-`$state`) `const` or undefined identifier never
///   reach `state_names`, so both refuse — matching the oracle's own
///   `bind_invalid_expression` / `constant_binding` / `bind_invalid_value`.
///
/// The oracle's `constant_binding` rule is enforced on both paths at once by the
/// shared [`reassignable_bind_target_root`], so a **top-level** `const`-declared or
/// imported target refuses here exactly as it does on the regular-element path.
///
/// That covers the top-level half only — `unassignable_names` is built from
/// top-level script statements alone. A TEMPLATE-scoped const target (a `{@const}`
/// name, a `{:then}`/`{:catch}` value, an `{#each}` index) is just as
/// `const`-declared to the oracle and is refused instead by `needs_context`'s
/// `template_consts` scope, which every `bind:` routes through; see
/// [`reassignable_bind_target_root`].
///
/// A failure refuses with the collapsing `Refusal::BindDirective { name }` bucket.
pub(crate) fn validate_inert_bind_target<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
) -> Result<(), CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();
    // Erase a TypeScript wrapper (`bind:this={x as T}`) first, exactly as the
    // regular fork does before gating the target's shape.
    let expr = env.erase(&directive.expression)?;
    if bind_name == "this" {
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(());
        }
        return refuse_bind(&bind_name);
    }
    match reassignable_bind_target_root(env, expr) {
        Some(name) if env.state_names.contains(&name) => Ok(()),
        _ => refuse_bind(&bind_name),
    }
}

/// Validate a `bind:` on a `<svelte:element>`. The dynamic tag carries no static
/// element identity, so the input-centric core-kind logic (`bind:value`/`checked`/
/// `group`) never applies — the oracle rejects those as `bind_invalid_target`. Only
/// `bind:this` is handled: it OMITS from SSR output (nothing to emit), exactly as on
/// a regular element — any lvalue (`Identifier`/member chain) or `{get, set}` pair
/// target, no `$state` gate (a `TS` wrapper is erased first). Every other bind
/// refuses. Deferred as a **safe over-refusal** (the current alternative is refusing
/// the whole `<svelte:element>`): `bind:focused` (which the oracle emits as
/// `$.attr('focused', …)`), the `omit_in_ssr` dimension family (which it drops),
/// and the `bind:innerHTML`-family content-editable binds.
///
/// Returns `Ok(())` for a valid `bind:this` — the caller emits nothing (inline) or
/// contributes no object property (spread).
pub(crate) fn validate_dynamic_bind<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
) -> Result<(), CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();
    if bind_name == "this" {
        let expr = env.erase(&directive.expression)?;
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(());
        }
        return refuse_bind(&bind_name);
    }
    refuse_bind(&bind_name)
}

/// Build and push `$.attr(name, value[, true])` for a synthesized bind attribute.
fn push_bind_attr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    value: Expression<'arena>,
    boolean: bool,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(env.b.string_literal_expr(name));
    args.push(value);
    if boolean {
        args.push(env.b.true_literal());
    }
    let call = env.b.member_call("$", "attr", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// The companion `value` attribute (case-sensitive `value`, the oracle's
/// `attr.name === 'value'`) a `bind:group` reads for its synthesized `checked`.
fn find_value_attribute<'arena>(
    env: &EmitEnv<'_, '_>,
    element: &'arena Element<'arena>,
) -> Option<&'arena Attribute<'arena>> {
    element.attributes.iter().find_map(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return None;
        };
        (attr.name(env.source) == "value").then_some(attr)
    })
}

/// Build the companion `value` attribute's value for a `bind:group` synthesis —
/// the oracle's `build_attribute_value(value_attribute.value, …)` (no trim, not
/// component mode): a static text → an attr-escaped string literal; a single
/// `{expr}` → the erased/wrapped expression; a bare `value` → `true`; a mixed
/// value → refuse this group for now.
fn build_companion_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    value_attr: &'arena Attribute<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = value_attr.value else {
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            Ok(env.b.string_literal_expr(&escape_html_attr(&decoded)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            let expr = env.erase(&tag.expression)?;
            Ok(wrap_value_expr(env, expr)?[0].clone())
        }
        _ => refuse_bind("group"),
    }
}

/// The SSR emission a `bind:` **core kind** reduces to, independent of whether the
/// element carries a `{...spread}` — the fork the inline (`emit_bind_directive`)
/// and spread (`build_bind_object_property`) callers share so the two never drift.
enum BindEmission<'arena> {
    /// The synthesized attribute `name = value` (`bind:value`/`bind:checked`, or
    /// `bind:group`'s synthesized `checked`). `boolean` picks the inline
    /// `$.attr(name, value, true)` boolean form; the spread object drops it (the
    /// object property is just `name: value`).
    Attr {
        name: &'static str,
        value: Expression<'arena>,
        boolean: bool,
    },
    /// Emit nothing on BOTH paths: a valid `bind:this`, or a `bind:group` with no
    /// companion `value` attribute (the oracle silently drops it).
    Omit,
    /// An `omit_in_ssr` bind ([`OMIT_IN_SSR_BINDS`]): the oracle drops it from SSR
    /// entirely (its early `continue`). **Both** paths **refuse** it (a safe
    /// over-refusal — the oracle rejects the ill-formed shapes that reach here, and
    /// tsv declines to reproduce the drop for the well-formed ones rather than
    /// silently emit nothing; well-formed `omit_in_ssr`+spread parity is deferred).
    OmitInSsr,
}

/// Resolve a `bind:` **core kind** on a regular `<input>`/element to its SSR
/// emission (the oracle's server `BindDirective` handling in `shared/element.js`),
/// applying every validity gate. Shared by the inline and spread callers so a bind
/// is validated identically whether or not the element also carries a spread:
///
/// - **`bind:this`** → [`BindEmission::Omit`] (no output). Valid on any variable /
///   any element, no `$state` gate, but the (erased) target must be one of the
///   oracle's three accepted forms — an Identifier, a member chain, or a
///   `{get, set}` pair; any other shape is `bind_invalid_expression` (refuse).
/// - **`omit_in_ssr` binds** → [`BindEmission::OmitInSsr`] (both callers refuse).
/// - **`bind:value`** on `<input>` → `Attr { "value", .. }`. A bare `type` is
///   `attribute_invalid_type` (refuse); a static `type="file"` is the files trap
///   the oracle silently drops (refuse rather than emit a divergent value); a
///   dynamic `type={x}` / static-non-file / no type is fine.
/// - **`bind:checked`** on `<input>` → `Attr { "checked", .., boolean }`; requires
///   a static `type="checkbox"` (else the oracle rejects).
/// - **`bind:group`** on `<input>` with a static `type` → a synthesized
///   `Attr { "checked", <synth>, .. }` where `<synth>` is `group.includes(<value>)`
///   for `type="checkbox"` else `group === <value>`, and `<value>` is the companion
///   `value` attribute's value. No companion `value` → [`BindEmission::Omit`]. The
///   companion `value` still emits normally at its own source slot — it is only
///   READ here.
///
/// Everything else refuses with the collapsing [`Refusal::BindDirective`] bucket: a
/// bind on a non-`<input>` target, `value` on `<textarea>`/`<select>`, the
/// content-editable trio, `open`, `focused`, an invalid target/type, or a bind
/// expression that isn't a `$state`-rooted lvalue.
fn resolve_bind_directive<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
) -> Result<BindEmission<'arena>, CompileError> {
    let bind_name = directive.name_span.extract(env.source).to_string();

    // `bind:this` (itself an `omit_in_ssr` bind, but with the `{get, set}`/lvalue
    // validation and a skip — not a refusal — on the inline path too). Erase a
    // `bind:this={x as T}` TS wrapper first.
    if bind_name == "this" {
        let expr = env.erase(&directive.expression)?;
        if reassignable_bind_target_root(env, expr).is_some() || is_get_set_pair(expr) {
            return Ok(BindEmission::Omit);
        }
        return refuse_bind(&bind_name);
    }

    // Every other `omit_in_ssr` bind: the oracle skips it before visiting the
    // target. `value`/`checked`/`group`/`open`/`focused`/the content-editable trio
    // are NOT `omit_in_ssr`, so they fall through to the dispatch below.
    if OMIT_IN_SSR_BINDS.contains(&bind_name.as_str()) {
        return Ok(BindEmission::OmitInSsr);
    }

    match (bind_name.as_str(), element_name) {
        ("value", "input") => {
            // A BARE `type` rejects (`attribute_invalid_type`); a static
            // `type="file"` is dropped by the oracle (refuse rather than diverge);
            // dynamic / static-non-file / no type is fine for `value`.
            match classify_input_type(env, element) {
                InputType::Bare => return refuse_bind(&bind_name),
                InputType::Static(t) if t == "file" => return refuse_bind(&bind_name),
                _ => {}
            }
            let value = emitted_bind_target(env, directive, &bind_name)?;
            Ok(BindEmission::Attr {
                name: "value",
                value,
                boolean: false,
            })
        }
        ("checked", "input") => {
            // `bind:checked` requires a static `type="checkbox"` (a dynamic / bare /
            // missing / other type is an oracle error).
            match classify_input_type(env, element) {
                InputType::Static(t) if t == "checkbox" => {}
                _ => return refuse_bind(&bind_name),
            }
            let value = emitted_bind_target(env, directive, &bind_name)?;
            Ok(BindEmission::Attr {
                name: "checked",
                value,
                boolean: true,
            })
        }
        ("group", "input") => {
            // A static `type` is required (a dynamic / bare type is
            // `attribute_invalid_type`; tsv also over-refuses no type). checkbox →
            // `group.includes(value)`, any other static type → `group === value`.
            let is_checkbox = match classify_input_type(env, element) {
                InputType::Static(t) => t == "checkbox",
                InputType::None | InputType::Bare | InputType::Dynamic => {
                    return refuse_bind(&bind_name);
                }
            };
            // Validate the bind TARGET (`g`) even when the bind is later dropped —
            // the oracle rejects an invalid group expression in its analysis phase,
            // before it decides whether to emit.
            let group = emitted_bind_target(env, directive, &bind_name)?;
            // No companion `value` → the oracle silently drops the bind.
            let Some(value_attr) = find_value_attribute(env, element) else {
                return Ok(BindEmission::Omit);
            };
            let value = build_companion_value(env, value_attr)?;
            let arena = env.b.arena;
            let synth = if is_checkbox {
                let group_alloc = arena.alloc(group);
                let callee = env.b.member_prop(group_alloc, "includes");
                let callee_alloc = arena.alloc(callee);
                let args = std::slice::from_ref(arena.alloc(value));
                env.b.call_of(callee_alloc, args, false)
            } else {
                let group_alloc = arena.alloc(group);
                let value_alloc = arena.alloc(value);
                env.b
                    .binary(group_alloc, BinaryOperator::EqualsEqualsEquals, value_alloc)
            };
            Ok(BindEmission::Attr {
                name: "checked",
                value: synth,
                boolean: true,
            })
        }
        _ => refuse_bind(&bind_name),
    }
}

/// Emit a `bind:` directive on a regular `<input>`/element **inline** at its source
/// slot (the non-spread path) — a handled core kind ([`resolve_bind_directive`])
/// becomes `$.attr(name, value[, true])`; a `bind:this` / no-companion `bind:group`
/// emits nothing; an `omit_in_ssr` bind refuses (a safe over-refusal — the oracle
/// drops it, but the inline path declines rather than reproduce the drop; the spread
/// path skips it instead).
pub(crate) fn emit_bind_directive<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    match resolve_bind_directive(env, directive, element, element_name)? {
        BindEmission::Attr {
            name,
            value,
            boolean,
        } => push_bind_attr(env, name, value, boolean, out),
        BindEmission::Omit => Ok(()),
        BindEmission::OmitInSsr => refuse_bind(directive.name_span.extract(env.source)),
    }
}

/// Build the object property a `bind:` **core kind** contributes to a spread
/// element's `$.attributes({…})` object, at the bind's source position (the oracle
/// pre-processes each bind into a synthetic attribute inside `build_spread_object`).
/// Reuses [`resolve_bind_directive`] for every validity gate, then adapts:
///
/// - `Attr { name, value, .. }` → the object property `{ name: value }` (the boolean
///   flag is an `$.attr` concern and is dropped), with the object-shorthand collapse
///   (`bind:value={value}` → `{ value }`);
/// - `Omit` (a valid `bind:this`, a no-companion `bind:group`) → `None`;
/// - `OmitInSsr` → refuse, exactly as the inline path does (a safe over-refusal;
///   well-formed `omit_in_ssr`+spread parity is deferred to a future slice).
pub(crate) fn build_bind_object_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena BindDirective<'arena>,
    element: &'arena Element<'arena>,
    element_name: &str,
) -> Result<Option<Property<'arena>>, CompileError> {
    match resolve_bind_directive(env, directive, element, element_name)? {
        BindEmission::Attr {
            name,
            value,
            boolean: _,
        } => {
            // The synthesized bind name (`value`/`checked`) is always a valid
            // identifier key; `shorthand` collapses `{ value: value }` → `{ value }`.
            let key = env.b.ident(name);
            let key_span = key.span;
            let shorthand = matches!(&value, Expression::Identifier(id)
                if plain_identifier_name(id, env.source).as_deref() == Some(name));
            Ok(Some(init_property(
                Expression::Identifier(key),
                value,
                shorthand,
                key_span,
            )))
        }
        // A valid `bind:this` / no-companion `bind:group` drops with no entry.
        BindEmission::Omit => Ok(None),
        // An `omit_in_ssr` bind refuses here too — consistent with the inline
        // path's `refuse_bind`, and the SAFE side (the oracle rejects these
        // shapes; tsv declines rather than silently drop them).
        BindEmission::OmitInSsr => refuse_bind(directive.name_span.extract(env.source)),
    }
}
