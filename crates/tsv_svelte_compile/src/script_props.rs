//! The `$props()` binding-pattern rewrite: `$bindable(fallback?)` defaults and
//! the `$$slots, $$events` injection.
//!
//! Oracle phase 3, **server**: every shape here is the server module's own —
//! `$$props`, the trailing `$.bind_props($$props, { … })`, the sanitize_slots
//! deconfliction. Called from [`crate::script_rewrite`]'s `$props()` arm.

use tsv_lang::Span;
use tsv_ts::ast::{
    AssignmentPattern, AstView, ExprId, Expression, ExpressionKind, ObjectPattern,
    ObjectPatternProperty, Property, RestElement,
};

use crate::build::{Builder, init_property};
use crate::script_decls::plain_identifier_name;
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// A bindable prop the transform must list in the trailing
/// `$.bind_props($$props, { … })` (source order). `key` is the prop name as
/// declared in the `$props()` object pattern; `local` is the destructure value —
/// they differ for a renamed prop (`{ value: v = $bindable() }` → `value`/`v`).
pub(crate) struct BindableEntry {
    /// The prop key as declared in the `$props()` object pattern.
    pub key: String,
    /// The local binding name (the destructured value).
    pub local: String,
}

/// The fallback of a rewritable `$bindable(...)` destructure default.
enum BindableDefault {
    /// `$bindable()` — argument-less; the default becomes `void 0`.
    ArgLess,
    /// `$bindable(fallback)` — the default becomes `fallback`.
    Arg(ExprId),
}

/// Classify a destructure default's right side as a rewritable `$bindable(...)`
/// call (a plain `$bindable` callee with zero or one argument). `None` for
/// anything else — a non-call, a member callee, or `$bindable(a, b)` (the oracle
/// rejects that arity, `rune_invalid_arguments_length`) — so the `$bindable` call
/// survives the rewrite and the guard walk refuses it.
fn bindable_default(
    view: AstView<'_>,
    right: &Expression,
    source: &str,
) -> Option<BindableDefault> {
    let ExpressionKind::CallExpression(call) = &right.kind else {
        return None;
    };
    let ExpressionKind::Identifier(callee) = &view.expr(call.callee).kind else {
        return None;
    };
    if plain_identifier_name(callee, source).as_deref() != Some("$bindable") {
        return None;
    }
    match view.expr_list(call.arguments) {
        [] => Some(BindableDefault::ArgLess),
        [arg] => Some(BindableDefault::Arg(*arg)),
        _ => None,
    }
}

/// If this object-pattern property is a top-level `key = $bindable(fallback?)`
/// default the transform can rewrite — a plain-identifier key, a plain-identifier
/// destructure value, and a rewritable `$bindable(...)` right — return the entry,
/// the `Property`, the `AssignmentPattern`, and the fallback argument. `None`
/// otherwise: the property is emitted unchanged, and a `$bindable` in a shape the
/// per-property validation accepts but this cannot rewrite — a non-identifier key
/// (string/numeric) or the wrong arity `$bindable(a, b)` — survives the rewrite for
/// the guard to refuse (a safe over-refusal, even for a non-identifier-keyed prop
/// the oracle would compile). A computed key or a nested-pattern value never reaches
/// here — both already refused as [`Refusal::PropsInvalidPattern`].
fn bindable_property(
    view: AstView<'_>,
    prop: &ObjectPatternProperty,
    source: &str,
) -> Option<(BindableEntry, Property, AssignmentPattern, BindableDefault)> {
    let ObjectPatternProperty::Property(p) = prop else {
        return None;
    };
    if p.computed {
        return None;
    }
    let ExpressionKind::AssignmentPattern(assign) = view.expr(p.value).kind else {
        return None;
    };
    let default = bindable_default(view, view.expr(assign.right), source)?;
    let ExpressionKind::Identifier(key_id) = &view.expr(p.key).kind else {
        return None;
    };
    let key = plain_identifier_name(key_id, source)?;
    let ExpressionKind::Identifier(left_id) = &view.expr(assign.left).kind else {
        return None;
    };
    let local = plain_identifier_name(left_id, source)?;
    Some((BindableEntry { key, local }, *p, assign, default))
}

/// Rebuild an object-pattern property, replacing its `$bindable(fallback?)`
/// default with the fallback (`void 0` when argument-less). A shallow re-slot:
/// the key and the `AssignmentPattern.left` keep their host ids and every flag
/// is copied; only the default's `right` changes, in a new `AssignmentPattern` row.
fn rewrite_bindable_default(
    b: &mut Builder<'_>,
    p: Property,
    assign: AssignmentPattern,
    default: BindableDefault,
) -> ObjectPatternProperty {
    let assign_span = b.view().expr(p.value).span;
    let new_right = match default {
        BindableDefault::Arg(arg) => arg,
        BindableDefault::ArgLess => {
            let at = b.view().expr(assign.right).span.end;
            let void_zero = b.void_zero_at(at);
            b.push(void_zero)
        }
    };
    let value = b.push(Expression {
        span: assign_span,
        kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
            left: assign.left,
            right: new_right,
            decorators: assign.decorators,
        }),
    });
    ObjectPatternProperty::Property(Property { value, ..p })
}

/// Rewrite a `$props()` binding pattern for the server module: replace each
/// recognized top-level `$bindable(fallback?)` default with its fallback
/// (collecting the bindable props in source order), and inject `$$slots,
/// $$events` wherever a rest element captures the remaining props (probe-verified):
///
/// - `let {a, ...rest} = $props()` →
///   `let { a, $$slots, $$events, ...rest } = $$props;` — injected immediately
///   BEFORE the rest element;
/// - `let props = $props()` (non-destructured) →
///   `let { $$slots, $$events, ...props } = $$props;`;
/// - `let { value = $bindable(42) } = $props()` → `let { value = 42 } = $$props;`
///   plus a `value` entry;
/// - a plain destructure with neither a rest nor a bindable default gets NO
///   rewrite.
///
/// Returns `(replacement pattern, bindable entries)`. The replacement is `None`
/// when nothing changed, so the host pattern `id` is kept. Refuses a
/// non-identifier/non-object `$props()` pattern (the oracle rejects those —
/// props_invalid_identifier). Every node either rewrite injects is zero-width at a
/// host position (the start of the property it precedes, the end of a replaced
/// `$bindable()` call), so a carried comment's windows stay the authored gaps. An
/// ObjectPattern is first validated per-property in the
/// oracle's source order (computed key or nested value → [`Refusal::PropsInvalidPattern`],
/// a `$$` key → [`Refusal::PropsIllegalName`]) before any rewrite.
///
/// When the component references `$$slots` (`uses_slots`), the injected
/// sanitize_slots const owns that name, so the destructured prop deconflicts by
/// renaming: `$$slots: $$slots_` (the `$props` arm of the oracle's server
/// `VariableDeclaration` visitor — always the `_` suffix, unconditional;
/// `$$events` never renames, and a user `$$slots_`/`$$events` reference or
/// declaration is oracle-rejected input, so no second-order collision exists).
pub(crate) fn rewrite_props_pattern(
    b: &mut Builder<'_>,
    id: ExprId,
    source: &str,
    uses_slots: bool,
) -> Result<(Option<ExprId>, Vec<BindableEntry>), CompileError> {
    let pattern = *b.view().expr(id);
    match pattern.kind {
        ExpressionKind::ObjectPattern(obj) => {
            let view = b.view();
            // The properties, copied out: the rewrite below pushes as it walks them.
            let props = view.pat_props(obj.properties).to_vec();
            // The oracle's per-property validation of a `$props()` destructure
            // (`2-analyze/visitors/VariableDeclarator.js:97-110`): three checks in
            // source order, first-wins (the oracle's `e.*` throws). Runs BEFORE the
            // rest/bindable short-circuit below so a plain destructure with neither
            // (`{ [x]: a }`, `{ $$slots: a }`, `{ a: { b } }`) still reaches it. A
            // `RestElement` is skipped (the oracle's `property.type !== 'Property'`
            // continue).
            for prop in &props {
                let ObjectPatternProperty::Property(p) = prop else {
                    continue;
                };
                // 1. A computed key (`{ [x]: a }`, `{ [$$x]: a }`) — `props_invalid_pattern`.
                if p.computed {
                    return Err(unsupported(Refusal::PropsInvalidPattern));
                }
                // 2. A non-computed Identifier key starting with `$$` — `props_illegal_name`
                //    (reserved for Svelte internals). The name is DECODED via
                //    `Identifier::name`, so an ESCAPED `$$` key (`{ $$x: a }` written
                //    `$$x`) refuses too — the oracle reads the DECODED `node.name`.
                //    A `$$`-prefixed *binding* — a shorthand `{ $$foo }` or default
                //    `{ $$foo = 1 }` — is refused upstream as `DollarPrefixedBinding`
                //    (`script_rewrite.rs`, the oracle's `dollar_prefix_invalid`, which fires
                //    first), so only `{ $$key: value }` reaches here.
                if let ExpressionKind::Identifier(key_id) = &view.expr(p.key).kind
                    && key_id.name(source, view).starts_with("$$")
                {
                    return Err(unsupported(Refusal::PropsIllegalName));
                }
                // 3. A value that — after stripping an `= default` (`AssignmentPattern`) —
                //    is not a plain Identifier: a nested pattern (`{ a: { b } }`,
                //    `{ a: [b] }`, `{ a: { b } = {} }`) — `props_invalid_pattern`. A
                //    `$bindable()` default's value is still checked here, so a nested
                //    `{ a: { b } = $bindable() }` refuses BEFORE the guard sees `$bindable`.
                let value = match view.expr(p.value).kind {
                    ExpressionKind::AssignmentPattern(assign) => view.expr(assign.left),
                    _ => view.expr(p.value),
                };
                if !matches!(value.kind, ExpressionKind::Identifier(_)) {
                    return Err(unsupported(Refusal::PropsInvalidPattern));
                }
            }
            let has_rest = props
                .iter()
                .any(|p| matches!(p, ObjectPatternProperty::RestElement(_)));
            let has_bindable = props
                .iter()
                .any(|p| bindable_property(view, p, source).is_some());
            if !has_rest && !has_bindable {
                return Ok((None, Vec::new()));
            }
            let mut entries = Vec::new();
            let mut properties: Vec<ObjectPatternProperty> = Vec::with_capacity(props.len() + 2);
            for prop in props {
                if matches!(prop, ObjectPatternProperty::RestElement(_)) {
                    let at = prop.span().start;
                    properties.push(slots_pattern_prop(b, uses_slots, at));
                    properties.push(shorthand_pattern_prop(b, "$$events", at));
                    properties.push(prop);
                } else if let Some((entry, p, assign, default)) =
                    bindable_property(b.view(), &prop, source)
                {
                    entries.push(entry);
                    properties.push(rewrite_bindable_default(b, p, assign, default));
                } else {
                    properties.push(prop);
                }
            }
            let properties = b.push_list(&properties);
            let rewritten = b.push(Expression {
                span: pattern.span,
                kind: ExpressionKind::ObjectPattern(ObjectPattern {
                    properties,
                    optional: obj.optional,
                    extra: obj.extra,
                }),
            });
            Ok((Some(rewritten), entries))
        }
        ExpressionKind::Identifier(_) => {
            let at = pattern.span.start;
            let slots = slots_pattern_prop(b, uses_slots, at);
            let events = shorthand_pattern_prop(b, "$$events", at);
            // The host identifier becomes the rest element's argument, by its own id.
            let rest = ObjectPatternProperty::RestElement(RestElement {
                argument: id,
                optional: false,
                extra: None,
                span: pattern.span,
            });
            let properties = b.push_list(&[slots, events, rest]);
            let rewritten = b.push(Expression {
                span: pattern.span,
                kind: ExpressionKind::ObjectPattern(ObjectPattern {
                    properties,
                    optional: false,
                    extra: None,
                }),
            });
            Ok((Some(rewritten), Vec::new()))
        }
        _ => Err(unsupported(Refusal::PropsBindingPattern)),
    }
}

/// The injected `$$slots` pattern property: shorthand `{ $$slots }` normally,
/// renamed `{ $$slots: $$slots_ }` when the sanitize_slots const owns the name
/// (see [`rewrite_props_pattern`]). Zero-width at `at`, like
/// [`shorthand_pattern_prop`].
fn slots_pattern_prop(b: &mut Builder<'_>, uses_slots: bool, at: u32) -> ObjectPatternProperty {
    if !uses_slots {
        return shorthand_pattern_prop(b, "$$slots", at);
    }
    let span = Span::new(at, at);
    let key = b.ident_expr_at("$$slots", span);
    let value = b.ident_expr_at("$$slots_", span);
    ObjectPatternProperty::Property(init_property(key, value, false, span))
}

/// A shorthand `{ name }` pattern property over a synthetic identifier, zero-width
/// at host position `at` — the start of the property it is injected before. An
/// appendix span there would open the previous property's trailing window from its
/// host end into the appendix, sweeping every later carried comment into the
/// pattern; at `at`, the windows around it are the authored gaps.
fn shorthand_pattern_prop(b: &mut Builder<'_>, name: &str, at: u32) -> ObjectPatternProperty {
    let span = Span::new(at, at);
    let ident = b.ident_at(name, span);
    let key = b.push(Expression::from_identifier(ident));
    let value = b.push(Expression::from_identifier(ident));
    ObjectPatternProperty::Property(init_property(key, value, true, span))
}
