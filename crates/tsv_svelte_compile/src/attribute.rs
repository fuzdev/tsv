//! Attribute emission: static inline values, dynamic `$.attr` calls, and mixed
//! text+expression attribute templates.
//!
//! A **per-node emitter** in the emission pipeline, and the base its two siblings
//! build on: the `class:` / `style:` directive builders
//! ([`crate::attribute_class_style`]) borrow this module's value-shaping helpers
//! ([`collapse_attr_whitespace`], [`preceded_by_quote`], [`class_needs_clsx`],
//! [`is_js_identifier`]) and nothing here depends back on them.
//! [`crate::element`] drives all three from its attribute loop.
//!
//! **Single source of truth** for the fold-or-template loop
//! ([`build_mixed_attr_value`]), shared by the object-path value builder
//! ([`build_attribute_value_expr`]) and the inline emitter
//! ([`emit_mixed_attribute`], which alone HTML-escapes and pushes the full-fold
//! static form). The two differ only in that escaping, so a second copy of the loop
//! would drift on the fold decision — which changes whether an attribute emits as
//! static text or as a `$.stringify` template.
//!
//! See [`crate::transform_server`] for the orchestration, and
//! [`crate::attribute_bind`] for the `bind:` half.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::InfallibleResolve;
use tsv_svelte::ast::internal::{Attribute, AttributeValue};
use tsv_ts::ast::internal::{Expression, LiteralValue, Property};

use crate::analyze::{evaluate, stringify_value};
use crate::body_builder::BodyBuilder;
use crate::build::{escape_template_text, init_property};
use crate::css_scope::SCOPE_HASH_CLASS;
use crate::dropped::guard_dropped;
use crate::namespace::{Namespace, element_is_mathml, element_is_svg};
use crate::script_decls::plain_identifier_name;
use crate::template_value::wrap_value_expr;
use crate::text_class::js_trim;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Elements that emit `load`/`error` events (Svelte's `LOAD_ERROR_ELEMENTS`,
/// `utils.js`): an `onload`/`onerror` handler on one of these injects an
/// `on{name}="this.__e=event"` capture attribute instead of being dropped.
const LOAD_ERROR_ELEMENTS: &[&str] = &[
    "body", "embed", "iframe", "img", "link", "object", "script", "style", "track",
];

/// Whether `name` is a load-error element (see [`LOAD_ERROR_ELEMENTS`]).
pub(crate) fn is_load_error_element(name: &str) -> bool {
    LOAD_ERROR_ELEMENTS.contains(&name)
}

/// HTML-escape a static attribute value (`escape_html(value, true)`, `[&"<]`).
///
/// The attribute-position sibling of the fragment walk's text escape, which
/// escapes `[&<]` only — a `"` is content in text and a delimiter here.
pub(crate) fn escape_html_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// The DOM boolean attributes (the oracle's `DOM_BOOLEAN_ATTRIBUTES`): a
/// dynamic value on one of these emits `$.attr(name, value, true)`.
// TODO: consider a home in tsv_html beside the element classification tables.
const DOM_BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "disabled",
    "formnovalidate",
    "indeterminate",
    "inert",
    "ismap",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "seamless",
    "selected",
    "webkitdirectory",
    "defer",
    "disablepictureinpicture",
    "disableremoteplayback",
];

/// Emit one plain attribute. Static text values inline (with entity decoding,
/// attribute escaping, `class`/`style` whitespace collapse, and the scope hash
/// on matched classes); dynamic and mixed values emit the oracle's runtime
/// calls (`$.attr` / `$.attr_class` / `$.attr_style`).
pub(crate) fn emit_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
    element_name: &str,
    out: &mut BodyBuilder<'arena>,
    element_scoped: bool,
    namespace: Namespace,
) -> Result<(), CompileError> {
    // The oracle lowercases attribute names outside foreign namespaces
    // (`get_attribute_name`, server `element.js`) — at EMISSION only. The
    // event-handler decision below tests the RAW authored name, so both are kept;
    // `name` (lowercased) drives the `value`/`class`/`style` special-key checks
    // (those keys are lowercase by definition), while `emit_name` is what actually
    // reaches the output — case-preserved on an svg/mathml element (`viewBox`,
    // `preserveAspectRatio`, …).
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    let name = raw_name.to_ascii_lowercase();
    let emit_name: &str = if element_is_foreign(element_name, namespace) {
        &raw_name
    } else {
        &name
    };

    // `value` on <textarea> becomes child content, on <select> it is omitted
    // with select_value bookkeeping — neither shape is implemented.
    if name == "value" && (element_name == "textarea" || element_name == "select") {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }

    let Some(values) = attr.value else {
        // Boolean attribute: the oracle emits `name=""`.
        out.push_text(&escape_template_text(&format!(" {emit_name}=\"\"")));
        return Ok(());
    };

    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let value = if name == "class" || name == "style" {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            if name == "class" {
                // CSS scope: a scoped element folds the hash into its `class`
                // value (`(escaped + ' ' + hash).trim()`, the oracle's order —
                // escape, then append the safe hash), so `class=""` on a scoped
                // element becomes `class="svelte-tsvhash"`. A `class` that ends up
                // empty (unscoped, and blank/whitespace-only) is dropped entirely
                // (oracle-probed: `class=""`/`class="  "` emit no attribute) — but
                // a bare boolean `class` keeps `class=""` (handled above), and
                // empty `style`/`id` stay.
                let escaped = escape_html_attr(&value);
                let text = if element_scoped {
                    js_trim(&format!("{escaped} {SCOPE_HASH_CLASS}")).to_string()
                } else {
                    escaped
                };
                if text.is_empty() {
                    return Ok(());
                }
                out.push_text(&escape_template_text(&format!(" class=\"{text}\"")));
                return Ok(());
            }
            out.push_text(&escape_template_text(&format!(
                " {emit_name}=\"{}\"",
                escape_html_attr(&value)
            )));
            Ok(())
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // An `on`-prefixed single-expression attribute is an event handler —
            // tested on the RAW authored name, exactly like the oracle
            // (`is_event_attribute`, server `element.js:71`, runs before any
            // lowercasing): `onClick` drops, but `ONCLICK`/`oNclick` are NOT
            // events and emit as regular `$.attr('onclick', …)` attributes
            // (probe-verified). A dropped handler's expression still feeds
            // `needs_context` (walked up front in `needs_context.rs`), so a
            // `new`/prop-rooted member or call inside it still forces the
            // wrapper — only the attribute markup is dropped. Raw `onload`/
            // `onerror` (exact match — `onLoad` on `<img>` is a plain drop,
            // probe-verified) on a load-error element are the exception (the
            // oracle injects an `on{name}="this.__e=event"` capture attribute),
            // refused for now.
            if raw_name.starts_with("on") {
                if (raw_name == "onload" || raw_name == "onerror")
                    && is_load_error_element(element_name)
                {
                    return Err(unsupported(Refusal::EventCaptureAttribute { name }));
                }
                // Dropped, but still guarded: a misplaced rune inside a handler is
                // an oracle analysis-phase error, not an emission one.
                return guard_dropped(env, &tag.expression);
            }
            // Quoted (`class="{a}"`) vs bare (`class={a}`): the oracle's AST
            // represents the quoted form as a one-chunk ARRAY and the bare form
            // as a plain ExpressionTag (the wire writer's `preceded_by_quote`
            // discriminant) — the split the class `$.clsx` rule keys on.
            let quoted = preceded_by_quote(env.source, tag.span.start);
            emit_dynamic_attribute(env, emit_name, &tag.expression, quoted, out)
        }
        _ => {
            // A mixed-value attribute whose RAW name starts with `on` is an
            // event attribute the oracle rejects as input
            // (`attribute_invalid_event_handler`) — refuse rather than guess.
            // `ONCLICK="a {h}"` is NOT an event (raw test) and emits through the
            // normal mixed path (probe-verified).
            if raw_name.starts_with("on") {
                return Err(unsupported(Refusal::EventAttribute { name }));
            }
            emit_mixed_attribute(env, emit_name, values, out)
        }
    }
}

/// The oracle's `get_attribute_name` namespace test: an svg/mathml element
/// preserves its attribute-name case; every other element lowercases. `inherited`
/// is the namespace the element sits in — the ancestor signal for the svg
/// `<a>`/`<title>` rule (`namespace::element_is_svg`).
fn element_is_foreign(element_name: &str, inherited: Namespace) -> bool {
    element_is_svg(element_name, inherited) || element_is_mathml(element_name)
}

/// Whether the byte before `pos` is a quote — the same discriminant the wire
/// writer uses to emit a quoted single-expression attribute value as an array.
pub(crate) fn preceded_by_quote(source: &str, pos: u32) -> bool {
    matches!(
        (pos as usize)
            .checked_sub(1)
            .and_then(|i| source.as_bytes().get(i)),
        Some(b'"' | b'\'')
    )
}

/// The oracle's `needs_clsx` rule (`2-analyze/visitors/Attribute.js`): only a
/// bare `class={expr}` wraps in `$.clsx`, and only when the expression is not a
/// `Literal`, `TemplateLiteral`, or ESTree `BinaryExpression`. tsv's internal
/// AST folds logical operators into `BinaryExpression`, but ESTree types them
/// `LogicalExpression` (`&&`/`||`/`??` DO wrap — oracle-probed), so the
/// exclusion is arithmetic/comparison binaries only. The terminal arm mirrors
/// the oracle's own negative-list rule: everything else wraps.
pub(crate) fn class_needs_clsx(expr: &Expression<'_>, quoted: bool) -> bool {
    if quoted {
        return false;
    }
    match expr {
        Expression::Literal(_) | Expression::TemplateLiteral(_) => false,
        Expression::BinaryExpression(b) => b.operator.is_logical(),
        _ => true,
    }
}

/// The single-`Text`-chunk `class`/`style` value rule — the oracle's
/// `WHITESPACE_INSENSITIVE_ATTRIBUTES` handling, which is literally
/// `chunk.data.replace(regex_whitespaces_strict, ' ').trim()`
/// (`server/visitors/shared/utils.js:208`).
///
/// **Two different whitespace classes, in this order.** The collapse is the
/// NARROW `regex_whitespaces_strict = /[ \t\n\r\f]+/g` (`phases/patterns.js:11`)
/// — deliberately not `\s+`, so an explicit `&nbsp;` inside the value survives.
/// The trim is then JavaScript's WIDE `String.prototype.trim`
/// ([`js_trim`]). Fusing the two into one narrow-class pass — collapsing and
/// dropping the edge runs together — is not the same function: a boundary
/// character that is JS whitespace but not in the narrow class (`U+00A0`,
/// `U+FEFF`, `U+000B`, `U+3000`, …) then survives where the oracle strips it.
///
/// Rust's `str::trim` is not the wide class either — see [`js_trim`]. The
/// discriminator is exactly ECMAScript's whitespace set: `U+0085` (`<NEL>`) and
/// `U+180E` sit outside it and the oracle KEEPS them.
///
/// The multi-chunk (mixed-value) rule is a different function: the oracle
/// collapses each chunk and never trims — [`collapse_runs_no_trim`], which this
/// composes with.
pub(crate) fn collapse_attr_whitespace(decoded: &str) -> String {
    js_trim(&collapse_runs_no_trim(decoded)).to_string()
}

/// A single-expression attribute value: `title={expr}` (or quoted,
/// `title="{expr}"` — `quoted` carries the distinction, which only the class
/// `$.clsx` rule reads).
fn emit_dynamic_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    expr: &'arena Expression<'arena>,
    quoted: bool,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event handlers (`on*` single-expression attributes) are dropped in
    // `emit_attribute` before dispatch, so they never reach here.
    // The template borrow point. Both shape predicates below read the node's
    // variant, so they must see the ERASED one: `class={'a' as string}` is a
    // string literal to the oracle (inline-literal path), and a `TSAsExpression`
    // left in place would take the `$.clsx` branch instead.
    let expr = env.erase(expr)?;
    // A string-literal expression value takes the oracle's inline-literal path
    // (pre-escaped static emission) — refuse rather than guess its edge rules.
    if matches!(expr, Expression::Literal(lit)
        if matches!(lit.value, LiteralValue::String(_)))
    {
        return Err(unsupported(Refusal::StringLiteralExprAttribute));
    }

    let wrapped = wrap_value_expr(env, expr)?;
    let call = match name {
        // Dynamic class/style interact with CSS scoping (hash argument,
        // pruning) — supported only on unstyled components.
        "class" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicClassOnStyled));
            }
            if class_needs_clsx(expr, quoted) {
                let clsx = env.b.member_call("$", "clsx", wrapped);
                let clsx_alloc = env.b.arena.alloc(clsx);
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(clsx_alloc))
            } else {
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(&wrapped[0]))
            }
        }
        "style" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicStyleOnStyled));
            }
            env.b.member_call("$", "attr_style", wrapped)
        }
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(wrapped[0].clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// The value of a mixed text+expression attribute, as the oracle's
/// `build_attribute_value` produces it in its multi-chunk path.
enum MixedAttrValue<'arena> {
    /// Every part folded statically — the raw concatenated value (un-HTML-escaped;
    /// each caller escapes as its own target requires).
    Folded(String),
    /// A template literal `` `t${$.stringify(a)}u` `` (interpolations omitted when
    /// the evaluator proves a defined string).
    Template(Expression<'arena>),
}

/// Build a mixed text+expression attribute value — the fold-or-template logic
/// shared by the element static-attribute path ([`emit_mixed_attribute`]) and the
/// spread object-builder ([`build_attribute_value_expr`]). `trim_whitespace`
/// collapses `class`/`style` runs (no per-chunk trim, matching the oracle's
/// template path). Returns [`MixedAttrValue::Folded`] when every part folds — each
/// caller decides how to escape/emit it — else the assembled template literal.
fn build_mixed_attr_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    trim_whitespace: bool,
    values: &'arena [AttributeValue<'arena>],
) -> Result<MixedAttrValue<'arena>, CompileError> {
    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value, in parallel — consumed only when every part
    // folds statically.
    let mut raw = String::new();
    let mut exprs: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    for value in values {
        match value {
            AttributeValue::Text(text) => {
                let decoded = text.data(env.source);
                let chunk = if trim_whitespace {
                    // Runs collapse but edges are NOT trimmed per-chunk (the
                    // oracle's replace() without trim in the template path).
                    collapse_runs_no_trim(&decoded)
                } else {
                    decoded.into_owned()
                };
                raw.push_str(&chunk);
                // Attribute templates carry no HTML escaping — the runtime
                // escapes; only template metachars are escaped here.
                #[allow(clippy::unwrap_used)]
                texts
                    .last_mut()
                    .unwrap()
                    .push_str(&escape_template_text(&chunk));
            }
            AttributeValue::ExpressionTag(tag) => {
                // The template borrow point: erase once, then guard AND fold the
                // erased node (the fold gate is the silent-divergence trap).
                let expr = env.erase(&tag.expression)?;
                // Guard first — never fold an oracle-invalid expression.
                let wrapped = wrap_value_expr(env, expr)?;
                let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
                    .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
                if let Some(value) = evaluated.known_value() {
                    // Folds into the quasi — plain `(value ?? '') + ''`, no
                    // HTML escaping in the template-value path.
                    let text = stringify_value(value)
                        .map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
                    raw.push_str(&text);
                    #[allow(clippy::unwrap_used)]
                    texts
                        .last_mut()
                        .unwrap()
                        .push_str(&escape_template_text(&text));
                    continue;
                }
                let piece = if evaluated.is_defined_string() {
                    wrapped[0].clone()
                } else {
                    env.b.member_call("$", "stringify", wrapped)
                };
                exprs.push(piece);
                texts.push(String::new());
            }
        }
    }

    if exprs.is_empty() {
        return Ok(MixedAttrValue::Folded(raw));
    }
    Ok(MixedAttrValue::Template(
        env.b.template_literal(&texts, exprs.into_bump_slice()),
    ))
}

/// A mixed text+expression attribute value: `title="t {a} u"` — an attribute
/// template literal with `$.stringify(expr)` interpolations (omitted when the
/// oracle's evaluator proves a defined string), folded where known.
fn emit_mixed_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    values: &'arena [AttributeValue<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event attributes (RAW name starting `on`) are refused by the dispatch in
    // `emit_attribute` before this is reached.
    if (name == "class" || name == "style") && env.scope.is_some() {
        return Err(unsupported(Refusal::InterpolatedAttrOnStyled {
            name: name.to_string(),
        }));
    }
    let trim_whitespace = name == "class" || name == "style";

    let template = match build_mixed_attr_value(env, trim_whitespace, values)? {
        MixedAttrValue::Folded(raw) => {
            // Every part folded statically — the oracle emits a *static* attribute
            // (oracle-probed rules): attr-escape `[&"<]`, folded value verbatim (no
            // trim, no empty-class drop, boolean attributes keep the folded value,
            // null/undefined already stringified to '' by the fold above). Only the
            // chunk-array path folds; a single-expression attribute never does
            // (`emit_dynamic_attribute`).
            out.push_text(&escape_template_text(&format!(
                " {name}=\"{}\"",
                escape_html_attr(&raw)
            )));
            return Ok(());
        }
        MixedAttrValue::Template(template) => template,
    };
    let template_alloc = env.b.arena.alloc(template);
    let call = match name {
        "class" => env
            .b
            .member_call("$", "attr_class", std::slice::from_ref(template_alloc)),
        "style" => env
            .b
            .member_call("$", "attr_style", std::slice::from_ref(template_alloc)),
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(template_alloc.clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// Build an attribute's value expression the way the oracle's
/// `build_attribute_value` does for a **non-component** element (`is_component`
/// false) — the value shape a spread element's `$.attributes({…})` object property
/// carries. Unlike the static-attribute emitters this returns the bare
/// `Expression`, never pushing an `$.attr(…)` call or inlining HTML:
///
/// - a boolean attribute (`value: None`) → `true`;
/// - a single static text value → the collapsed/trimmed (class/style) then
///   **HTML-escaped** (`[&"<]`) string literal;
/// - a single expression value → the erased + guarded + derived-rewritten
///   expression, wrapped in `$.clsx(…)` for a `class` that needs it
///   (`class_needs_clsx`), **no fold** (the single-chunk path doesn't evaluate);
/// - a mixed text+expression value → a fully-folded string literal (the raw
///   concatenation, **not** HTML-escaped — the runtime escapes the object value)
///   or a `$.stringify` template literal.
fn build_attribute_value_expr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    value: Option<&'arena [AttributeValue<'arena>]>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = value else {
        // A boolean attribute: `build_attribute_value(true)` → `true`.
        return Ok(env.b.true_literal());
    };
    let trim_whitespace = name == "class" || name == "style";
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let collapsed = if trim_whitespace {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            Ok(env.b.string_literal_expr(&escape_html_attr(&collapsed)))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // The template borrow point. Both the `class` `$.clsx` gate and the
            // wrap read the ERASED node (an `x as T` value is the assignable `x`).
            let quoted = preceded_by_quote(env.source, tag.span.start);
            let expr = env.erase(&tag.expression)?;
            let wrapped = wrap_value_expr(env, expr)?[0].clone();
            // A `class` single-expression wraps in `$.clsx` per the oracle's
            // `needs_clsx` rule (its `has_spread` branch pre-wraps the expression);
            // every other name (and a non-clsx class) passes the bare value.
            if name == "class" && class_needs_clsx(expr, quoted) {
                let wrapped_alloc = env.b.arena.alloc(wrapped);
                Ok(env
                    .b
                    .member_call("$", "clsx", std::slice::from_ref(wrapped_alloc)))
            } else {
                Ok(wrapped)
            }
        }
        _ => Ok(
            match build_mixed_attr_value(env, trim_whitespace, values)? {
                // The object value is a JS string literal the runtime escapes — no
                // HTML escaping here (unlike the static-attribute full-fold path).
                MixedAttrValue::Folded(raw) => env.b.string_literal_expr(&raw),
                MixedAttrValue::Template(template) => template,
            },
        ),
    }
}

/// Build one `key: value` object property for an element `{...spread}`'s
/// `$.attributes({…})` object from a plain attribute (the oracle's
/// `build_spread_object`). Returns `None` for a **dropped** attribute — a
/// single-expression event handler (still guarded), and `defaultValue`/
/// `defaultChecked` (which don't exist as attributes). The key is lowercased
/// (matching `get_attribute_name`) and emitted as a bare identifier when it is
/// identifier-safe, else a quoted string literal; `shorthand` is set when the key
/// is an identifier and the value is the same-named identifier (`{ hidden }`, the
/// oracle's `b.prop` collapse).
pub(crate) fn build_spread_object_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
    element_name: &str,
    inherited: Namespace,
) -> Result<Option<Property<'arena>>, CompileError> {
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    // `defaultValue`/`defaultChecked` are properties, not attributes — the oracle
    // omits them from the object (case-sensitive raw-name test).
    if raw_name == "defaultValue" || raw_name == "defaultChecked" {
        return Ok(None);
    }
    let name = raw_name.to_ascii_lowercase();
    // A `value` on `<textarea>` becomes child content in the oracle (a divergent
    // shape); refuse. (`<select>` refuses the whole spread at the element level.)
    if name == "value" && element_name == "textarea" {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }
    // Event-handler dispatch, mirroring the oracle. A single-expression `on*`
    // attribute is `is_event_attribute` → dropped from the object but still
    // guarded (a stray rune inside it is an analysis-phase error). Any other `on*`
    // value form with name length > 2 is `attribute_invalid_event_handler` (an
    // oracle analysis error) → refuse.
    if raw_name.starts_with("on") {
        if let Some([AttributeValue::ExpressionTag(tag)]) = attr.value {
            guard_dropped(env, &tag.expression)?;
            return Ok(None);
        }
        if raw_name.len() > 2 {
            return Err(unsupported(Refusal::EventAttribute { name }));
        }
        // A bare `on` (length 2) with a non-expression value is a normal attribute.
    }

    let value = build_attribute_value_expr(env, &name, attr.value)?;
    // The object key is the emitted attribute name — case-preserved on an
    // svg/mathml element (`get_attribute_name`), lowercased otherwise. The value
    // builder above still keys the `class` → `$.clsx` decision on the lowercased
    // `name` (`class` is lowercase by definition).
    let emit_name: &str = if element_is_foreign(element_name, inherited) {
        &raw_name
    } else {
        &name
    };
    let key_is_ident = is_js_identifier(emit_name);
    let key = if key_is_ident {
        Expression::Identifier(env.b.ident(emit_name))
    } else {
        env.b.string_literal_expr(emit_name)
    };
    let key_span = key.span();
    // Object shorthand `{ hidden }`: an identifier key whose value is the plain
    // identifier of the same (emitted) name (`hidden={hidden}`, `viewBox={viewBox}`).
    let shorthand = key_is_ident
        && matches!(&value, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(emit_name));
    Ok(Some(init_property(key, value, shorthand, key_span)))
}

/// Whether `name` is a valid JS identifier (`/^[a-zA-Z_$][a-zA-Z_$0-9]*$/`) — the
/// oracle's `regex_is_valid_identifier` gate (`b.key`), which decides whether a
/// style property key prints as a bare identifier or a quoted string. `format_canonical`
/// applies the same test when dropping quotes off a string-literal key, so a
/// non-shorthand key can always be a string literal; the identifier form matters
/// only for the object-shorthand `{ color }` a `style:color` shorthand builds.
pub(crate) fn is_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Collapse `[ \t\n\r\f]+` runs to one space without trimming (the mixed-value
/// `class`/`style` chunk rule).
fn collapse_runs_no_trim(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}
