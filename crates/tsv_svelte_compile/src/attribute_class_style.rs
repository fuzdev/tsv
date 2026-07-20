//! The `class:` and `style:` directive builders — the fused `$.attr_class` /
//! `$.attr_style` calls, on both the inline and spread paths.
//!
//! A **per-node emitter** in the emission pipeline, downstream of
//! [`crate::attribute`]: it borrows that module's value-shaping helpers
//! (`collapse_attr_whitespace`, `preceded_by_quote`, `class_needs_clsx`,
//! `is_js_identifier`) and nothing there depends back on it. [`crate::element`]'s
//! attribute loop pre-scans an element's `class:` / `style:` directives and calls
//! [`emit_class_directives`] / [`emit_style_directives`] at the authored
//! `class`/`style` slot (or after all plain attributes when synthetic); the spread
//! path calls [`build_spread_class_object`] / [`build_spread_style_object`] for the
//! fused `$.attributes(…)` call's `classes` and `styles` arguments.
//!
//! **Single source of truth** for the class-vs-style asymmetry, which is easy to
//! collapse by mistake and wrong in both directions: `class` carries the CSS scope
//! hash and `style` never does (style is not scoped), `class` wraps a dynamic base
//! in `$.clsx` and `style` takes the bare expression, and `|important` partitions
//! the INLINE `$.attr_style` argument into a 2-element array while the SPREAD
//! `styles` object stays FLAT — an oracle divergence between the two paths, not an
//! oversight.
//!
//! See [`crate::transform_server`] for the orchestration.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_svelte::ast::internal::{
    Attribute, AttributeValue, ClassDirective, StyleDirective, StyleDirectiveValue,
};
use tsv_ts::ast::internal::{
    ArrayExpression, Expression, LiteralValue, ObjectExpression, ObjectProperty, Property,
};

use crate::attribute::{
    class_needs_clsx, collapse_attr_whitespace, escape_html_attr, is_js_identifier,
    preceded_by_quote,
};
use crate::body_builder::BodyBuilder;
use crate::build::init_property;
use crate::css_scope::SCOPE_HASH_CLASS;
use crate::script_decls::plain_identifier_name;
use crate::template_value::wrap_value_expr;
use crate::text_class::js_trim;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// The base (1st) argument of a `$.attr_class(base, hash, directives)` call — the
/// authored `class` attribute value, or the synthetic empty string. The two arms
/// govern CSS-scope handling: a string-literal base folds the hash into its text
/// and contributes its tokens to selector matching; any other base carries the
/// hash in the 2nd argument and contributes no static tokens.
enum ClassBase<'arena> {
    /// A statically-known string base (`class="foo"`, or the synthetic `''`).
    StringLiteral(String),
    /// A non-string-literal base expression (`$.clsx(w)`, a bare identifier, a
    /// template literal, `true`).
    Expr(Expression<'arena>),
}

/// Build the base expression for an element's `$.attr_class(...)` call from its
/// authored `class` attribute (`None` = no authored class → the oracle's synthetic
/// empty `''`). Mirrors the oracle's `build_attribute_value` + `needs_clsx`
/// handling for the `class` attribute.
fn build_class_base<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_attr: Option<&'arena Attribute<'arena>>,
) -> Result<ClassBase<'arena>, CompileError> {
    let Some(attr) = class_attr else {
        // No authored `class` — the oracle injects a synthetic `class=""`
        // (2-analyze/index.js), so the base is the empty string literal.
        return Ok(ClassBase::StringLiteral(String::new()));
    };
    let Some(values) = attr.value else {
        // A bare boolean `class` — the oracle's `build_attribute_value(true)` → `true`.
        return Ok(ClassBase::Expr(env.b.true_literal()));
    };
    match values {
        [AttributeValue::Text(text)] => {
            // Static class value: whitespace-insensitive collapse + trim.
            let decoded = text.data(env.source);
            Ok(ClassBase::StringLiteral(collapse_attr_whitespace(&decoded)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // Dynamic `class={expr}` / `class="{expr}"`. The oracle wraps in
            // `$.clsx` per the `needs_clsx` rule (`class_needs_clsx`). A
            // string-literal expression takes the oracle's inline-literal path we
            // don't reproduce — refuse, matching the standalone dynamic-attribute
            // path (`emit_dynamic_attribute`).
            let quoted = preceded_by_quote(env.source, tag.span.start);
            let expr = env.erase(&tag.expression)?;
            if matches!(expr, Expression::Literal(lit)
                if matches!(lit.value, LiteralValue::String(_)))
            {
                return Err(unsupported(Refusal::StringLiteralExprAttribute));
            }
            let wrapped = wrap_value_expr(env, expr)?;
            let base = if class_needs_clsx(expr, quoted) {
                env.b.member_call("$", "clsx", wrapped)
            } else {
                wrapped[0].clone()
            };
            Ok(ClassBase::Expr(base))
        }
        // A mixed-value `class="a {b}"` base — deferred (rare).
        _ => Err(unsupported(Refusal::ClassDirectiveWithMixedClass)),
    }
}

/// Build the directives object `{ 'name': expr, … }` (source order) for
/// `$.attr_class`. The key is always a string literal (the oracle's
/// `b.literal(directive.name)`); `format_canonical` drops the quotes for an
/// identifier-safe name, matching the oracle's own canonicalized output. Each
/// value is the directive expression, erased + guarded + derived-rewritten.
fn build_class_directives_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_directives: &[&'arena ClassDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in class_directives {
        let name = directive.name_span.extract(env.source);
        let key = env.b.string_literal_expr(name);
        let key_span = key.span();
        // The template borrow point: erase once, then guard + rewrite a bare
        // derived read to `d()`.
        let expr = env.erase(&directive.expression)?;
        let value = wrap_value_expr(env, expr)?[0].clone();
        properties.push(ObjectProperty::Property(init_property(
            key, value, false, key_span,
        )));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// Build the `classes` (3rd) argument of a spread element's `$.attributes(…)` call
/// from its `class:` directives (the oracle's `prepare_element_spread`, which uses
/// `b.init(directive.name, …)`). Unlike the non-spread `$.attr_class` object
/// ([`build_class_directives_object`], string-literal keys), this uses an
/// **identifier key** (a quoted string literal only when the name isn't
/// identifier-safe, e.g. `class:foo-bar` → `{ 'foo-bar': x }`) with the
/// **object-shorthand** collapse the oracle's `b.init` applies: `class:active`
/// (shorthand) and `class:active={active}` both → `{ active }`, `class:active={x}` →
/// `{ active: x }`. Class names are **case-sensitive** (never lowercased). The
/// same-named-identifier fork reads the **raw** directive expression, mirroring the
/// oracle's `directive.expression.type === 'Identifier' && … === directive.name`
/// (a `class:active={active as T}` is not shorthand — the raw node is a
/// `TSAsExpression` — and its value is the erased/guarded expression).
pub(crate) fn build_spread_class_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_directives: &[&'arena ClassDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in class_directives {
        let name = directive.name_span.extract(env.source).to_string();
        // The oracle's same-named-identifier shorthand fork, read on the RAW node:
        // the value is the bare `b.id(directive.name)` (no transform / derived
        // rewrite), exactly like a `style:` shorthand.
        let same_named = matches!(&directive.expression, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(name.as_str()));
        let (value, is_shorthand_id) = if same_named {
            (Expression::Identifier(env.b.ident(&name)), true)
        } else {
            // The template borrow point: erase once, then guard + rewrite a bare
            // derived read to `d()`.
            let expr = env.erase(&directive.expression)?;
            (wrap_value_expr(env, expr)?[0].clone(), false)
        };
        let key_is_ident = is_js_identifier(&name);
        let shorthand = is_shorthand_id && key_is_ident;
        let (key, key_span) = if key_is_ident {
            let id = env.b.ident(&name);
            let span = id.span;
            (Expression::Identifier(id), span)
        } else {
            let key = env.b.string_literal_expr(&name);
            let span = key.span();
            (key, span)
        };
        properties.push(ObjectProperty::Property(init_property(
            key, value, shorthand, key_span,
        )));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// Build the `styles` (4th) argument of a spread element's `$.attributes(…)` call
/// from its `style:` directives (the oracle's `prepare_element_spread`). A **flat**
/// object `{ prop: value, … }` in source order — **no** `|important` partitioning
/// (the CRITICAL divergence from the non-spread `$.attr_style` path, which builds
/// the `[ {normal}, {important} ]` array): in spread mode the oracle's
/// `build_attribute_value(directive.value, …, true)` folds every directive into one
/// object regardless of modifier. `|important` is still **validated** (only a single
/// `|important` is legal, else refuse) but does not partition. Reuses
/// [`build_style_property`] for the per-property build (key lowercasing / quoting,
/// shorthand).
pub(crate) fn build_spread_style_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_directives: &[&'arena StyleDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in style_directives {
        validate_style_modifiers(directive)?;
        properties.push(ObjectProperty::Property(build_style_property(
            env, directive,
        )?));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// Emit the fused `$.attr_class(base, css_hash, { name: expr, … })` call for a
/// regular element carrying `class:` directives (the oracle's `build_attr_class`,
/// `shared/element.js`). `class_attr` is the authored `class` attribute when
/// present (its value is the base), else `None` (the oracle's synthetic empty
/// `''`).
///
/// CSS scoping: the element is scoped when any of its candidate classes — the
/// static tokens of a string-literal base ∪ the `class:` directive names — is a
/// scoped selector. Each matched class is recorded (so the no-match post-check
/// passes). A scoped hash folds into a string-literal base (`(value + ' ' +
/// hash).trim()`) or, for any other base, rides the 2nd argument; otherwise the
/// 2nd argument is `void 0` (the directives object is always the 3rd argument, so
/// the middle argument is never elided).
pub(crate) fn emit_class_directives<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    class_attr: Option<&'arena Attribute<'arena>>,
    class_directives: &[&'arena ClassDirective<'arena>],
    out: &mut BodyBuilder<'arena>,
    element_scoped: bool,
) -> Result<(), CompileError> {
    let base = build_class_base(env, class_attr)?;
    let base_is_string = matches!(base, ClassBase::StringLiteral(_));

    // CSS scope: whether any scoped compound matches this element (decided up front
    // by `element_scope`, including type/id/attribute selectors — not only a class
    // token or `class:` name). A string base folds the hash into its text; any
    // other base carries it in the 2nd argument.
    let scoped = element_scoped;

    // The base expression, folding the scope hash into a string-literal base.
    let base_expr = match base {
        ClassBase::StringLiteral(s) => {
            // HTML-escape the static base (`escape_html(data, true)`), after token
            // matching, matching the oracle.
            let escaped = escape_html_attr(&s);
            let text = if scoped {
                js_trim(&format!("{escaped} {SCOPE_HASH_CLASS}")).to_string()
            } else {
                escaped
            };
            env.b.string_literal_expr(&text)
        }
        ClassBase::Expr(e) => e,
    };

    // The css-hash 2nd argument: `void 0` unless scoped with a non-string base,
    // where it carries the hash literal (a string base folded it in above).
    let css_hash = if scoped && !base_is_string {
        env.b.string_literal_expr(SCOPE_HASH_CLASS)
    } else {
        env.b.void_zero()
    };

    let directives = build_class_directives_object(env, class_directives)?;

    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(base_expr);
    args.push(css_hash);
    args.push(directives);
    let call = env.b.member_call("$", "attr_class", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}

/// The `style:` allowed-modifier gate: the oracle accepts only a single
/// `|important` (or none). Any other modifier, or two or more, is
/// `style_directive_invalid_modifier` — an oracle *error*, so tsv refuses rather
/// than compile it.
fn validate_style_modifiers(directive: &StyleDirective<'_>) -> Result<(), CompileError> {
    match directive.modifiers {
        [] | ["important"] => Ok(()),
        _ => Err(unsupported(Refusal::StyleDirectiveInvalidModifier)),
    }
}

/// Build the base (1st) argument of a `$.attr_style(base, directives)` call from
/// the authored `style` attribute (`None` = no authored `style` → the oracle's
/// phase-2 synthetic empty `''`). Mirrors `build_class_base` MINUS `$.clsx` and
/// MINUS any CSS scoping (style is never scoped): a dynamic `style={expr}` is the
/// bare expression, not `$.clsx(expr)`.
fn build_style_base<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_attr: Option<&'arena Attribute<'arena>>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(attr) = style_attr else {
        // No authored `style` — the oracle injects a synthetic `style=""`
        // (2-analyze/index.js), so the base is the empty string literal.
        return Ok(env.b.string_literal_expr(""));
    };
    let Some(values) = attr.value else {
        // A bare boolean `style` — the oracle's `build_attribute_value(true)` → `true`.
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            // Static style value: whitespace-insensitive collapse + trim, then the
            // attribute HTML-escape the oracle applies to a text base literal
            // (`build_attribute_value`, non-component → `escape_html(data, true)`).
            let decoded = text.data(env.source);
            let collapsed = collapse_attr_whitespace(&decoded);
            Ok(env.b.string_literal_expr(&escape_html_attr(&collapsed)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // Dynamic `style={expr}` / `style="{expr}"` — the bare expression (NO
            // `$.clsx`, unlike `class`). A string-literal expression takes the
            // oracle's inline-literal path we don't reproduce — refuse, matching the
            // standalone dynamic-attribute path (`emit_dynamic_attribute`).
            let expr = env.erase(&tag.expression)?;
            if matches!(expr, Expression::Literal(lit)
                if matches!(lit.value, LiteralValue::String(_)))
            {
                return Err(unsupported(Refusal::StringLiteralExprAttribute));
            }
            Ok(wrap_value_expr(env, expr)?[0].clone())
        }
        // A mixed-value `style="a {b}"` base — deferred (rare).
        _ => Err(unsupported(Refusal::StyleDirectiveWithMixedStyle)),
    }
}

/// Build one `{ name: value }` property for a `style:` directive (the oracle's
/// `build_attr_style` per-directive `b.init`). The key is the property name,
/// lowercased unless it starts with `--` (custom properties keep case); a
/// shorthand `style:color` prints as object-shorthand `{ color }` when the
/// (lowercased) key coincides with the raw same-name identifier value.
fn build_style_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    directive: &'arena StyleDirective<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let raw_name = directive.name_span.extract(env.source).to_string();
    // The oracle lowercases the property name unless it is a `--custom-property`.
    let key_name = if raw_name.starts_with("--") {
        raw_name.clone()
    } else {
        raw_name.to_ascii_lowercase()
    };

    // The value, and whether it is the same-name shorthand identifier.
    let (value, is_shorthand_id) = match &directive.value {
        StyleDirectiveValue::True => {
            // Shorthand `style:color` → `b.id(directive.name)` (RAW name).
            (Expression::Identifier(env.b.ident(&raw_name)), true)
        }
        StyleDirectiveValue::ExpressionTag(tag) => {
            // The template borrow point: erase once, then guard + rewrite a bare
            // derived read to `d()`. `|important` does NOT wrap the value.
            let expr = env.erase(&tag.expression)?;
            (wrap_value_expr(env, expr)?[0].clone(), false)
        }
        StyleDirectiveValue::Parts(parts) => match parts {
            // A static `style:color="red"` → the string literal, collapsed +
            // trimmed + attribute-escaped like the base text value.
            [AttributeValue::Text(text)] => {
                let decoded = text.data(env.source);
                let collapsed = collapse_attr_whitespace(&decoded);
                (
                    env.b.string_literal_expr(&escape_html_attr(&collapsed)),
                    false,
                )
            }
            // A mixed `style:color="a {b}"` value — deferred (rare).
            _ => return Err(unsupported(Refusal::StyleDirectiveWithMixedValue)),
        },
    };

    // Object-shorthand `{ color }` requires an identifier key equal to the value
    // identifier's name — i.e. a shorthand whose lowercased name is unchanged and
    // identifier-safe. Otherwise a string-literal key (whose quotes
    // `format_canonical` drops when the name is identifier-safe) with an explicit
    // value.
    let shorthand = is_shorthand_id && key_name == raw_name && is_js_identifier(&key_name);
    let (key, key_span) = if shorthand {
        let id = env.b.ident(&key_name);
        let span = id.span;
        (Expression::Identifier(id), span)
    } else {
        let key = env.b.string_literal_expr(&key_name);
        let span = key.span();
        (key, span)
    };

    Ok(init_property(key, value, shorthand, key_span))
}

/// Build the directives (2nd) argument of `$.attr_style`: a plain object
/// `{ normal… }`, or — when any directive carries `|important` — a 2-element
/// array `[ { normal… }, { important… } ]` (the normal object is `{}` when all are
/// important). Source order is preserved within each group.
fn build_style_directives_arg<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_directives: &[&'arena StyleDirective<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut normal: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    let mut important: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for directive in style_directives {
        validate_style_modifiers(directive)?;
        let property = ObjectProperty::Property(build_style_property(env, directive)?);
        if directive.modifiers.contains(&"important") {
            important.push(property);
        } else {
            normal.push(property);
        }
    }
    // An object span is only the printer's newline-scan region for the expansion
    // heuristic — both minted `{…}` regions are appendix-only and newline-free, so
    // each collapses when it fits (same rationale as `build_props_object`). The
    // properties render from their own key/value spans, so the normal span
    // enclosing the (source-order-interleaved) important property text is harmless.
    let cbrace = env.b.mint("}").end;
    let normal_obj = Expression::ObjectExpression(ObjectExpression {
        properties: normal.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    });
    if important.is_empty() {
        return Ok(normal_obj);
    }
    // Any `|important` → the partitioned `[ {normal}, {important} ]` array.
    let normal_alloc = arena.alloc(normal_obj);
    let iobrace = env.b.mint("{").start;
    let icbrace = env.b.mint("}").end;
    let important_obj = Expression::ObjectExpression(ObjectExpression {
        properties: important.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(iobrace, icbrace),
    });
    let important_alloc = arena.alloc(important_obj);
    let lbracket = env.b.mint("[").start;
    let rbracket = env.b.mint("]").end;
    let mut elements: BumpVec<'arena, Option<Expression<'arena>>> = BumpVec::new_in(arena);
    elements.push(Some(normal_alloc.clone()));
    elements.push(Some(important_alloc.clone()));
    Ok(Expression::ArrayExpression(ArrayExpression {
        elements: elements.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(lbracket, rbracket),
    }))
}

/// Emit the fused `$.attr_style(base, { name: value, … })` call for a regular
/// element carrying `style:` directives (the oracle's `build_attr_style`,
/// `shared/element.js`). `style_attr` is the authored `style` attribute when
/// present (its value is the base), else `None` (the oracle's synthetic empty
/// `''`). Unlike `$.attr_class`, there is no css-hash argument (style is never
/// scoped) and the directives argument may be an important-partitioned array.
pub(crate) fn emit_style_directives<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    style_attr: Option<&'arena Attribute<'arena>>,
    style_directives: &[&'arena StyleDirective<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let base = build_style_base(env, style_attr)?;
    let directives = build_style_directives_arg(env, style_directives)?;

    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    args.push(base);
    args.push(directives);
    let call = env.b.member_call("$", "attr_style", args.into_bump_slice());
    out.push_expr(call);
    Ok(())
}
