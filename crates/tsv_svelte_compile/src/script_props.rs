//! The `$props()` binding-pattern rewrite: `$bindable(fallback?)` defaults and
//! the `$$slots, $$events` injection.
//!
//! Oracle phase 3, **server**: every shape here is the server module's own —
//! `$$props`, the trailing `$.bind_props($$props, { … })`, the sanitize_slots
//! deconfliction. Called from [`crate::script_rewrite`]'s `$props()` arm.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_ts::ast::internal::{
    AssignmentPattern, Expression, ObjectPattern, ObjectPatternProperty, Property, RestElement,
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
enum BindableDefault<'arena> {
    /// `$bindable()` — argument-less; the default becomes `void 0`.
    ArgLess,
    /// `$bindable(fallback)` — the default becomes `fallback`.
    Arg(&'arena Expression<'arena>),
}

/// Classify a destructure default's right side as a rewritable `$bindable(...)`
/// call (a plain `$bindable` callee with zero or one argument). `None` for
/// anything else — a non-call, a member callee, or `$bindable(a, b)` (the oracle
/// rejects that arity, `rune_invalid_arguments_length`) — so the `$bindable` call
/// survives the rewrite and the guard walk refuses it.
fn bindable_default<'arena>(
    right: &'arena Expression<'arena>,
    source: &str,
) -> Option<BindableDefault<'arena>> {
    let Expression::CallExpression(call) = right else {
        return None;
    };
    let Expression::Identifier(callee) = call.callee else {
        return None;
    };
    if plain_identifier_name(callee, source).as_deref() != Some("$bindable") {
        return None;
    }
    match call.arguments {
        [] => Some(BindableDefault::ArgLess),
        [arg] => Some(BindableDefault::Arg(arg)),
        _ => None,
    }
}

/// If this object-pattern property is a top-level `key = $bindable(fallback?)`
/// default the transform can rewrite — a plain-identifier key, a plain-identifier
/// destructure value, and a rewritable `$bindable(...)` right — return the entry,
/// the `Property`, the `AssignmentPattern`, and the fallback argument. `None`
/// otherwise: the property is emitted unchanged, and a `$bindable` in any
/// unrecognized shape (a non-identifier key — string/numeric/computed —, a
/// nested-pattern value, the wrong arity) survives the rewrite for the guard to
/// refuse — a safe over-refusal, even for a non-identifier-keyed prop the oracle
/// would compile.
#[allow(clippy::type_complexity)]
fn bindable_property<'arena>(
    prop: &'arena ObjectPatternProperty<'arena>,
    source: &str,
) -> Option<(
    BindableEntry,
    &'arena Property<'arena>,
    &'arena AssignmentPattern<'arena>,
    BindableDefault<'arena>,
)> {
    let ObjectPatternProperty::Property(p) = prop else {
        return None;
    };
    if p.computed {
        return None;
    }
    let Expression::AssignmentPattern(assign) = &p.value else {
        return None;
    };
    let default = bindable_default(assign.right, source)?;
    let Expression::Identifier(key_id) = &p.key else {
        return None;
    };
    let key = plain_identifier_name(key_id, source)?;
    let Expression::Identifier(left_id) = assign.left else {
        return None;
    };
    let local = plain_identifier_name(left_id, source)?;
    Some((BindableEntry { key, local }, p, assign, default))
}

/// Rebuild an object-pattern property, replacing its `$bindable(fallback?)`
/// default with the fallback (`void 0` when argument-less). A shallow re-slot:
/// the key, the `AssignmentPattern.left`, and every flag stay borrowed; only the
/// default's `right` changes.
fn rewrite_bindable_default<'arena>(
    b: &mut Builder<'arena>,
    p: &'arena Property<'arena>,
    assign: &'arena AssignmentPattern<'arena>,
    default: BindableDefault<'arena>,
) -> ObjectPatternProperty<'arena> {
    let new_right: &'arena Expression<'arena> = match default {
        BindableDefault::Arg(arg) => arg,
        BindableDefault::ArgLess => b.arena.alloc(b.void_zero()),
    };
    let new_value = Expression::AssignmentPattern(AssignmentPattern {
        left: assign.left,
        right: new_right,
        decorators: assign.decorators,
        span: assign.span,
    });
    ObjectPatternProperty::Property(Property {
        key: p.key.clone(),
        value: new_value,
        kind: p.kind,
        shorthand: p.shorthand,
        computed: p.computed,
        method: p.method,
        span: p.span,
    })
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
/// when nothing changed, so the original borrowed pattern is kept. Refuses a
/// non-identifier/non-object `$props()` pattern (the oracle rejects those —
/// props_invalid_identifier) and both rewrites alongside carried comments (the
/// minted appendix spans between host-span siblings would sweep host comments — a
/// safe over-refusal).
///
/// When the component references `$$slots` (`uses_slots`), the injected
/// sanitize_slots const owns that name, so the destructured prop deconflicts by
/// renaming: `$$slots: $$slots_` (the oracle's `VariableDeclaration.js:56-73`
/// rule — always the `_` suffix, unconditional; `$$events` never renames, and a
/// user `$$slots_`/`$$events` reference or declaration is oracle-rejected input,
/// so no second-order collision exists).
pub(crate) fn rewrite_props_pattern<'arena>(
    b: &mut Builder<'arena>,
    id: &'arena Expression<'arena>,
    source: &str,
    has_comments: bool,
    uses_slots: bool,
) -> Result<(Option<Expression<'arena>>, Vec<BindableEntry>), CompileError> {
    let arena = b.arena;
    match id {
        Expression::ObjectPattern(obj) => {
            // The oracle's `props_illegal_name` (VariableDeclarator.js:94-103): a
            // `$props()` destructure property whose non-computed Identifier key starts
            // with `$$` is reserved for Svelte internals and rejected. Checked before
            // the rest/bindable short-circuit below so a plain `{ $$slots: a }` (no
            // rest, no bindable) still reaches it. A `$$`-prefixed *binding* — a
            // shorthand `{ $$foo }` or default `{ $$foo = 1 }` — is refused upstream as
            // `DollarPrefixedBinding` (`script_rewrite.rs:278`, before this), so only
            // the `{ $$key: value }` form reaches here; a computed `$$` key is the
            // oracle's separate `props_invalid_pattern`; an escaped key falls through
            // (the crate's standing escaped-identifier residual).
            for prop in obj.properties {
                if let ObjectPatternProperty::Property(p) = prop
                    && !p.computed
                    && let Expression::Identifier(key_id) = &p.key
                    && plain_identifier_name(key_id, source).is_some_and(|k| k.starts_with("$$"))
                {
                    return Err(unsupported(Refusal::PropsIllegalName));
                }
            }
            let has_rest = obj
                .properties
                .iter()
                .any(|p| matches!(p, ObjectPatternProperty::RestElement(_)));
            let has_bindable = obj
                .properties
                .iter()
                .any(|p| bindable_property(p, source).is_some());
            if !has_rest && !has_bindable {
                return Ok((None, Vec::new()));
            }
            if has_comments {
                return Err(unsupported(if has_bindable {
                    Refusal::CommentsWithBindable
                } else {
                    Refusal::CommentsWithRestProps
                }));
            }
            let mut entries = Vec::new();
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(arena);
            for prop in obj.properties {
                if matches!(prop, ObjectPatternProperty::RestElement(_)) {
                    properties.push(slots_pattern_prop(b, uses_slots));
                    properties.push(shorthand_pattern_prop(b, "$$events"));
                    properties.push(prop.clone());
                } else if let Some((entry, p, assign, default)) = bindable_property(prop, source) {
                    entries.push(entry);
                    properties.push(rewrite_bindable_default(b, p, assign, default));
                } else {
                    properties.push(prop.clone());
                }
            }
            Ok((
                Some(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    optional: obj.optional,
                    type_annotation: obj.type_annotation.clone(),
                    decorators: obj.decorators,
                    span: obj.span,
                })),
                entries,
            ))
        }
        Expression::Identifier(_) => {
            if has_comments {
                return Err(unsupported(Refusal::CommentsWithNonDestructuredProps));
            }
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(arena);
            properties.push(slots_pattern_prop(b, uses_slots));
            properties.push(shorthand_pattern_prop(b, "$$events"));
            properties.push(ObjectPatternProperty::RestElement(RestElement {
                argument: arena.alloc(id.clone()),
                optional: false,
                type_annotation: None,
                span: id.span(),
            }));
            Ok((
                Some(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    optional: false,
                    type_annotation: None,
                    decorators: None,
                    span: id.span(),
                })),
                Vec::new(),
            ))
        }
        _ => Err(unsupported(Refusal::PropsBindingPattern)),
    }
}

/// The injected `$$slots` pattern property: shorthand `{ $$slots }` normally,
/// renamed `{ $$slots: $$slots_ }` when the sanitize_slots const owns the name
/// (see [`rewrite_props_pattern`]).
fn slots_pattern_prop<'arena>(
    b: &mut Builder<'arena>,
    uses_slots: bool,
) -> ObjectPatternProperty<'arena> {
    if !uses_slots {
        return shorthand_pattern_prop(b, "$$slots");
    }
    let key = b.ident("$$slots");
    b.mint(": ");
    let value = b.ident("$$slots_");
    let span = Span::new(key.span.start, value.span.end);
    ObjectPatternProperty::Property(init_property(
        Expression::Identifier(key),
        Expression::Identifier(value),
        false,
        span,
    ))
}

/// A shorthand `{ name }` pattern property over a synthetic identifier
/// (interned name; the span is the minted appendix text).
fn shorthand_pattern_prop<'arena>(
    b: &mut Builder<'arena>,
    name: &str,
) -> ObjectPatternProperty<'arena> {
    let ident = b.ident(name);
    let span = ident.span;
    ObjectPatternProperty::Property(init_property(
        Expression::Identifier(ident.clone()),
        Expression::Identifier(ident),
        true,
        span,
    ))
}
