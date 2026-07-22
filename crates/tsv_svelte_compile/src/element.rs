//! Element emission: plain elements, the `AttrHost`-parametrized attribute
//! machinery, and `<svelte:element>`.
//!
//! A **per-node emitter** in the emission pipeline, reached from
//! [`crate::fragment`]'s dispatch. [`emit_element`] prints static HTML for a plain
//! element and routes a component invocation (`<Foo … />`) to
//! [`crate::component::emit_component`] — that one dispatch is the only edge
//! between the two modules; component emission has no outgoing edge back here.
//!
//! **The attribute machinery is deliberately shared** between regular elements and
//! `<svelte:element>` via [`AttrHost`], threaded through
//! [`emit_plain_attributes`] / [`emit_spread_attributes`] /
//! [`build_element_spread_object`] "so the two never drift". Those functions must
//! NOT be forked per host: the shared machinery lives physically with both its
//! hosts precisely because splitting `<svelte:element>` into its own module would
//! *read* as licensing exactly that fork. The two legitimate forks are named at
//! their sites — the `bind:` handling and the spread `flags` argument.
//!
//! See [`crate::transform_server`] for the orchestration, and
//! [`crate::component`] for the component-invocation half.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, ClassDirective, Element, ElementKind, FragmentNode, SpecialElement,
    SpecialThis, StyleDirective,
};
use tsv_ts::ast::internal::{
    Expression, ExpressionStatement, ObjectExpression, ObjectProperty, SpreadElement, Statement,
};

use crate::attribute::{build_spread_object_property, emit_attribute, is_load_error_element};
use crate::attribute_bind::{
    build_bind_object_property, emit_bind_directive, validate_dynamic_bind,
};
use crate::attribute_class_style::{
    build_spread_class_object, build_spread_style_object, emit_class_directives,
    emit_style_directives,
};
use crate::body_builder::BodyBuilder;
use crate::component::emit_component;
use crate::css_scope::SCOPE_HASH_CLASS;
use crate::dropped::guard_dropped;
use crate::fragment::{FragmentCtx, emit_child_body, emit_fragment};
use crate::namespace::{
    ChildNamespace, FragmentParent, Namespace, determine_namespace_for_children,
};
use crate::template_value::wrap_value_expr;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// The oracle's phase-2 directive-validity checks (`2-analyze/visitors/shared/
/// element.js:92-132`), which run before it discards the SSR visit — so a
/// combination it rejects must refuse, not compile.
///
/// - **Transitions**: a `transition:` claims both intro and outro, `in:` intro,
///   `out:` outro; a channel claimed twice is `transition_duplicate`/
///   `transition_conflict`. Equivalent rule: refuse iff two or more directives
///   claim intro, or two or more claim outro (modifiers are irrelevant).
/// - **Animate**: legal only as the sole non-trivial child of a keyed `{#each}`
///   (`animate_host`, decided in `blocks.rs`) and only one per element;
///   everything else is `animation_invalid_placement`/`animation_missing_key`/
///   `animation_duplicate`.
///
/// Runs on the HTML-element path only (components early-return above). Valid
/// combinations fall through to the per-attribute drop loop unchanged.
fn validate_directive_combinations(
    attributes: &[AttributeNode<'_>],
    animate_host: bool,
) -> Result<(), CompileError> {
    let mut intro_seen = false;
    let mut outro_seen = false;
    let mut animate_count = 0usize;
    for attr in attributes {
        match attr {
            AttributeNode::TransitionDirective(d) => {
                if d.direction.has_intro() {
                    if intro_seen {
                        return Err(unsupported(Refusal::TransitionDirectiveConflict));
                    }
                    intro_seen = true;
                }
                if d.direction.has_outro() {
                    if outro_seen {
                        return Err(unsupported(Refusal::TransitionDirectiveConflict));
                    }
                    outro_seen = true;
                }
            }
            AttributeNode::AnimateDirective(_) => animate_count += 1,
            _ => {}
        }
    }
    if animate_count > 1 || (animate_count == 1 && !animate_host) {
        return Err(unsupported(Refusal::AnimateDirectiveInvalid));
    }
    Ok(())
}

/// Whether `attr` is the element's `class` attribute (case-insensitive, matching
/// the oracle's lowercasing of attribute names on non-foreign elements).
fn attribute_is_class(env: &EmitEnv<'_, '_>, attr: &Attribute<'_>) -> bool {
    attribute_name_eq(env, attr, "class")
}

/// Whether `attr` is the element's `style` attribute (see [`attribute_is_class`]).
fn attribute_is_style(env: &EmitEnv<'_, '_>, attr: &Attribute<'_>) -> bool {
    attribute_name_eq(env, attr, "style")
}

/// Case-insensitive attribute-name test (the oracle lowercases attribute names on
/// non-foreign elements).
fn attribute_name_eq(env: &EmitEnv<'_, '_>, attr: &Attribute<'_>, name: &str) -> bool {
    attr.name(env.source).eq_ignore_ascii_case(name)
}

/// The two element kinds that share the attribute-emission machinery: a regular
/// HTML element and a `<svelte:element this={…}>`. Both project the same shape onto
/// [`emit_plain_attributes`] / [`emit_spread_attributes`] — a name, an attribute
/// list, an optional CSS scope, and the same `class:`/`style:`/spread emission — and
/// differ only in two localized places:
///
/// - the **`bind:` fork**: a regular element routes to the input-centric
///   [`emit_bind_directive`] / [`build_bind_object_property`]; a `<svelte:element>`
///   validates a `bind:this` (omit) and refuses every other bind
///   ([`validate_dynamic_bind`]) — the dynamic tag has no static `<input>` identity;
/// - the **spread `flags`**: a `<svelte:element>`'s name is always the literal
///   `svelte:element`, so it is never `<input>`/custom → the 5th `$.attributes`
///   argument is always absent (`Dynamic` reports flags `0`).
///
/// Passing `name = "svelte:element"` makes the other name-keyed logic naturally
/// correct: it is not void, not `<select>`/`<option>`, not a load-error element, and
/// not a custom element, so those guards fall through exactly as intended.
#[derive(Clone, Copy)]
enum AttrHost<'arena> {
    Regular(&'arena Element<'arena>),
    Dynamic(&'arena SpecialElement<'arena>),
}

impl<'arena> AttrHost<'arena> {
    fn attributes(self) -> &'arena [AttributeNode<'arena>] {
        match self {
            AttrHost::Regular(element) => element.attributes,
            AttrHost::Dynamic(special) => special.attributes,
        }
    }

    /// The element's span — the key for the `animate:` host lookup (and, for a
    /// regular element, the CSS-scope lookup the caller performs). A
    /// `<svelte:element>` is never an `animate:` host (that is a keyed-`{#each}`
    /// sole-child role decided in `blocks.rs` over regular elements), so its
    /// `animate_host` reads `false` and a stray `animate:` refuses.
    fn span(self) -> Span {
        match self {
            AttrHost::Regular(element) => element.span,
            AttrHost::Dynamic(special) => special.span,
        }
    }
}

/// Emit an element's open-tag attributes: the per-attribute drop/emit loop, or —
/// when a `{...spread}` is present — the one fused `$.attributes(…)` call. The
/// single entry both the regular-element (`<name`-prefixed) and `<svelte:element>`
/// (attributes-closure) paths route through, so the two never drift.
fn emit_host_attributes<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    host: AttrHost<'arena>,
    name: &str,
    out: &mut BodyBuilder<'arena>,
    scoped: bool,
    namespace: Namespace,
) -> Result<(), CompileError> {
    let has_spread = host
        .attributes()
        .iter()
        .any(|attr_node| matches!(attr_node, AttributeNode::SpreadAttribute(_)));
    if has_spread {
        emit_spread_attributes(env, host, name, out, scoped, namespace)
    } else {
        emit_plain_attributes(env, host, name, out, scoped, namespace)
    }
}

/// Emit one element's open tag, children, and close tag into the template.
pub(crate) fn emit_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    out: &mut BodyBuilder<'arena>,
    parent_ctx: &FragmentCtx<'_>,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let name = element.name(env.source).to_string();
    // A component (`<Foo>`, `<Foo.Bar>`) compiles to a call
    // (`Foo($$renderer, {…props})`), not static markup — route it to the
    // component emitter. Exhaustive over `ElementKind` so a new kind can't
    // silently fall through to the static-HTML path below.
    match element.kind {
        ElementKind::Html => {}
        ElementKind::Component => return emit_component(env, element, out, &name, is_standalone),
    }
    // The oracle's `RegularElement` visitor lowercases the tag name once, at the
    // top, whenever the element sits in the html namespace
    // (`3-transform/server/visitors/RegularElement.js:18`: `const name =
    // context.state.namespace === 'html' ? node.name.toLowerCase() : node.name`),
    // and then reuses that ONE lowered name for every downstream decision in the
    // visitor: `is_void(name)`, the `script`/`style`/`select`/`option` special
    // cases, and both the open- and close-tag literals. So `<bR>` lowers to `br`,
    // is therefore VOID, and self-closes. Everything keyed on the RAW name in the
    // oracle stays keyed on `name` here: `preserve_whitespace` (`node.name ===
    // 'pre'`), the ancestor svg-`<text>` guard, `determine_namespace_for_children`
    // (which reads `node.metadata.svg`, itself derived from the raw name), and the
    // whole attribute machinery (`shared/element.js` tests `node.name`
    // throughout, and `get_attribute_name` keys on `element.metadata.svg`).
    let emit_name = if parent_ctx.namespace == Namespace::Html {
        name.to_lowercase()
    } else {
        name.clone()
    };
    match emit_name.as_str() {
        // Template-level <script>/<style> have special semantics in the oracle.
        "script" | "style" => {
            return Err(unsupported(Refusal::TemplateLevelElement {
                name: name.clone(),
            }));
        }
        // The oracle compiles every <option> into `$$renderer.option(…)`
        // closure calls — static markup would be a divergent compile.
        "option" => {
            return Err(unsupported(Refusal::OptionElement));
        }
        // A populated <select>/<optgroup> gets a `<!>` anchor after its
        // children in the oracle's output (probe-verified; empty ones emit
        // statically and match, so only the populated shape refuses).
        "select" | "optgroup"
            if element
                .fragment
                .nodes
                .iter()
                .any(|n| !matches!(n, FragmentNode::Text(t) if t.is_ascii_ws_only)) =>
        {
            return Err(unsupported(Refusal::ElementWithChildren {
                name: name.clone(),
            }));
        }
        _ => {}
    }

    // The open tag's attributes: the per-attribute drop/emit loop, or — for an
    // element carrying a `{...spread}` — one fused `$.attributes(…)` call (routed
    // inside `emit_host_attributes`).
    out.push_text(&format!("<{emit_name}"));
    emit_host_attributes(
        env,
        AttrHost::Regular(element),
        &name,
        out,
        env.element_scope(element),
        // The element sits in the enclosing fragment's namespace — the ancestor
        // signal for its own `<a>`/`<title>` svg-ness (attribute-name casing,
        // spread flags).
        parent_ctx.namespace,
    )?;

    if tsv_html::is_void_element(&emit_name) {
        // XHTML-compliant self-close, matching the oracle.
        out.push_text("/>");
        if !element.fragment.nodes.is_empty() {
            return Err(unsupported(Refusal::VoidElementChildren {
                name: name.clone(),
            }));
        }
        return Ok(());
    }
    out.push_text(">");
    emit_fragment(
        env,
        &element.fragment,
        out,
        FragmentCtx {
            mark_text_first: false,
            is_component_root: false,
            hoist_snippets: false,
            is_standalone,
            preserve_whitespace: parent_ctx.preserve_whitespace
                || name == "pre"
                || name == "textarea",
            parent_name: Some(&name),
            // An element's children fragment is in the element's own namespace
            // (Svelte's `determine_namespace_for_children`) — computed from the
            // element's name and the namespace IT sits in (`parent_ctx.namespace`,
            // the ancestor signal for a `<a>`/`<title>` element).
            namespace: determine_namespace_for_children(&name, parent_ctx.namespace),
            // Entering an svg `<text>` turns whitespace preservation on for its
            // whole subtree (the oracle's ancestor `<text>` guard).
            in_svg_text: parent_ctx.in_svg_text || name == "text",
        },
    )?;
    out.push_text(&format!("</{emit_name}>"));
    Ok(())
}

/// Emit an element's plain (non-spread) attributes into the open tag: the
/// per-attribute drop/emit loop plus the fused `class:`/`style:` calls. The
/// `<{name}` prefix is already pushed by the caller; the void/children/close
/// suffix follows it.
fn emit_plain_attributes<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    host: AttrHost<'arena>,
    name: &str,
    out: &mut BodyBuilder<'arena>,
    scoped: bool,
    namespace: Namespace,
) -> Result<(), CompileError> {
    let attributes = host.attributes();
    // The oracle's phase-2 directive-validity checks run before it discards the
    // SSR visit, so a rejected combination must refuse here — not fall through to
    // the drop loop and compile. `animate_host` is whether this element is the
    // sanctioned `animate:` position (decided in `blocks.rs`).
    let animate_host = env.animate_host_span == Some(host.span());
    validate_directive_combinations(attributes, animate_host)?;

    // CSS scope: did the upfront census match give this element the
    // `svelte-tsvhash` class? A scoped element folds the hash into its authored
    // `class` / `class:` markup below, or synthesizes it after all plain attributes
    // when it has neither. Both a regular element and a `<svelte:element>` route here
    // — the caller passes its census-match result (`element_scope` /
    // `special_element_scope`).
    let element_scoped = scoped;
    let has_class_attr = attributes.iter().any(
        |attr_node| matches!(attr_node, AttributeNode::Attribute(a) if attribute_is_class(env, a)),
    );

    // Pre-scan the `class:` and `style:` directives (source order). When present,
    // the authored `class`/`style` attribute (if any) and the same-family directives
    // fuse into one `$.attr_class(base, hash, { name: expr, … })` /
    // `$.attr_style(base, { name: value, … })` call (the oracle's `build_attr_class`
    // / `build_attr_style`), emitted at the authored `class`/`style` slot — or, with
    // no authored attribute, after all plain attributes (the oracle's phase-2
    // synthetic empty-`class`/`style` injection appends to the attribute list, class
    // before style).
    let class_directives: Vec<&'arena ClassDirective<'arena>> = attributes
        .iter()
        .filter_map(|attr_node| match attr_node {
            AttributeNode::ClassDirective(d) => Some(d),
            _ => None,
        })
        .collect();
    let style_directives: Vec<&'arena StyleDirective<'arena>> = attributes
        .iter()
        .filter_map(|attr_node| match attr_node {
            AttributeNode::StyleDirective(d) => Some(d),
            _ => None,
        })
        .collect();
    let has_class_directives = !class_directives.is_empty();
    let has_style_directives = !style_directives.is_empty();
    let mut class_call_emitted = false;
    let mut style_call_emitted = false;

    for attr_node in attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                // With `class:`/`style:` directives present, the authored
                // `class`/`style` attribute is not emitted inline — it becomes the
                // base of the fused `$.attr_class(...)`/`$.attr_style(...)` call,
                // emitted here at its source slot.
                if has_class_directives && !class_call_emitted && attribute_is_class(env, attr) {
                    emit_class_directives(env, Some(attr), &class_directives, out, element_scoped)?;
                    class_call_emitted = true;
                } else if has_style_directives
                    && !style_call_emitted
                    && attribute_is_style(env, attr)
                {
                    emit_style_directives(env, Some(attr), &style_directives, out)?;
                    style_call_emitted = true;
                } else {
                    emit_attribute(env, attr, name, out, element_scoped, namespace)?;
                }
            }
            // `class:`/`style:` directives fuse into the single
            // `$.attr_class(...)`/`$.attr_style(...)` call (emitted at the authored
            // slot, or after all plain attributes when synthetic) — never inline
            // here. A directive node implies its `has_*_directives` flag, so these
            // arms only run for the fused case.
            AttributeNode::ClassDirective(_) | AttributeNode::StyleDirective(_) => {}
            // The no-op drop family: `use:`/`transition:`/`in:`/`out:`/`animate:`/
            // `{@attach}` contribute nothing to the tag — SSR runs no client
            // lifecycle, so the oracle discards their output (the discarded
            // `context.visit` in `shared/element.js`). Their expressions are still
            // walked for scope/`needs_context` (up front, via `attr_refs`) and
            // still validated: a misplaced rune or a top-level `await` inside the
            // expression must refuse, exactly as for a dropped event handler, so
            // guard the expression here.
            //
            // The one exception: a `use:` on a load-error element makes the oracle
            // add `onload`/`onerror` capture attributes (its `events_to_capture`
            // set) — not implemented, so refuse. `transition:`/`animate:`/`{@attach}`
            // on such an element still drop cleanly (only `use:` and a spread
            // trigger the capture — `shared/element.js`).
            AttributeNode::UseDirective(directive) => {
                if is_load_error_element(name) {
                    return Err(unsupported(Refusal::UseDirectiveOnLoadErrorElement));
                }
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::TransitionDirective(directive) => {
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::AnimateDirective(directive) => {
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::AttachTag(attach) => guard_dropped(env, &attach.expression)?,
            // `bind:` is handled inline at its source slot. On a regular element a
            // handled core kind (`this` omits; `value`/`checked`/`group` on `<input>`
            // synthesize a `$.attr(...)`) emits, everything else refuses
            // (`emit_bind_directive`); a `bind:group`'s companion `value` attribute
            // still emits normally via the `Attribute` arm above — it is only READ for
            // the synthesis. On a `<svelte:element>` only a `bind:this` is handled (it
            // omits), everything else refuses (`validate_dynamic_bind`) — the dynamic
            // tag has no static `<input>` identity.
            AttributeNode::BindDirective(directive) => match host {
                AttrHost::Regular(element) => {
                    emit_bind_directive(env, directive, element, name, out)?;
                }
                AttrHost::Dynamic(_) => validate_dynamic_bind(env, directive)?,
            },
            // A legacy `on:` directive and `let:` deliberately refuse — a runes-only
            // fence (the oracle compiles `on:` in runes mode, but it's deprecated
            // Svelte-4 syntax; migrate to `onclick={fn}` / the runes event attribute).
            // (`class:`/`style:`/`bind:` alongside one of these still refuses here,
            // via the sibling.)
            AttributeNode::OnDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "on:" }));
            }
            AttributeNode::LetDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "let:" }));
            }
            // Unreachable: an element carrying a `{...spread}` routed to the spread
            // path (`has_spread` in `emit_host_attributes`), so this per-attribute
            // loop (the non-spread case) never sees one.
            AttributeNode::SpreadAttribute(_) => {}
        }
    }
    // No authored `class`/`style` attribute: the fused call emits after all plain
    // attributes (the oracle appends the synthetic empty `class`, then the synthetic
    // empty `style`, to the end of the attribute list — class before style).
    if has_class_directives && !class_call_emitted {
        emit_class_directives(env, None, &class_directives, out, element_scoped)?;
    } else if element_scoped && !has_class_attr {
        // A scoped element with no `class` markup of any kind gets a synthetic
        // `class="svelte-tsvhash"` — appended after all plain attributes, before
        // any synthetic `style` (the oracle's phase-2 class-before-style order).
        out.push_text(&format!(" class=\"{SCOPE_HASH_CLASS}\""));
    }
    if has_style_directives && !style_call_emitted {
        emit_style_directives(env, None, &style_directives, out)?;
    }
    Ok(())
}

/// Whether `element` is a custom element (Svelte's `is_custom_element_node`): a
/// hyphenated tag name, or a plain `is` attribute (case-sensitive). A custom
/// element sets `ELEMENT_PRESERVE_ATTRIBUTE_CASE` in the spread flags.
fn is_custom_element_node(env: &EmitEnv<'_, '_>, element: &Element<'_>, name: &str) -> bool {
    name.contains('-')
        || element.attributes.iter().any(|attr_node| {
            let AttributeNode::Attribute(attr) = attr_node else {
                return false;
            };
            attr.name(env.source) == "is"
        })
}

/// The ELEMENT flag bits the oracle folds into the 5th `$.attributes(…)` argument
/// (`prepare_element_spread`), in its `if`/`else if` precedence: an svg/mathml
/// element is namespaced AND preserves attribute case (`ELEMENT_IS_NAMESPACED |
/// ELEMENT_PRESERVE_ATTRIBUTE_CASE`, `1 | 2 = 3`), else a custom element preserves
/// attribute case (`ELEMENT_PRESERVE_ATTRIBUTE_CASE`, `2`), else an `<input>` gets
/// `ELEMENT_IS_INPUT` (`4`); every other element is `0`. `inherited` is the
/// namespace the element sits in — the ancestor signal for a `<a>`/`<title>`
/// element's svg-ness.
fn spread_element_flags(
    env: &EmitEnv<'_, '_>,
    element: &Element<'_>,
    name: &str,
    inherited: Namespace,
) -> u32 {
    if crate::namespace::element_is_foreign(name, inherited) {
        3
    } else if is_custom_element_node(env, element, name) {
        2
    } else if name == "input" {
        4
    } else {
        0
    }
}

/// Build the `{ … }` object (1st argument of `$.attributes`) from every plain
/// attribute, `bind:` entry, and spread on the element, in SOURCE ORDER (the
/// oracle's `build_spread_object`, over the attribute list its main loop
/// pre-processes each `bind:` into): a plain attribute → a `key: value` property
/// (dropped for an event handler / `defaultValue`), a `bind:` core kind → its
/// synthesized `value`/`checked` property at the bind's position (`bind:this` /
/// `omit_in_ssr` / a no-companion `bind:group` contribute nothing), a `{...spread}`
/// → a `...expr` spread element. `class:`/`style:` and the drop family carry no
/// object property (they ride the other `$.attributes` arguments / drop). The braces
/// are minted around the property construction so the object span (the printer's
/// only newline-scan region for the expansion decision) is appendix-only and
/// collapses when it fits — the same idiom as `build_props_object`.
fn build_element_spread_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    host: AttrHost<'arena>,
    name: &str,
    namespace: Namespace,
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for attr_node in host.attributes() {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                if let Some(property) = build_spread_object_property(env, attr, name, namespace)? {
                    properties.push(ObjectProperty::Property(property));
                }
            }
            // A `bind:` core kind synthesizes its `value`/`checked` object property
            // at the bind's source slot (the oracle inlines the bind into the
            // `attributes` list its `build_spread_object` walks); its validity gates
            // still apply and refuse an invalid target/type/expression. On a
            // `<svelte:element>` only a `bind:this` is valid (it contributes no
            // property), everything else refuses (`validate_dynamic_bind`).
            AttributeNode::BindDirective(bind) => {
                let property = match host {
                    AttrHost::Regular(element) => {
                        build_bind_object_property(env, bind, element, name)?
                    }
                    AttrHost::Dynamic(_) => {
                        validate_dynamic_bind(env, bind)?;
                        None
                    }
                };
                if let Some(property) = property {
                    properties.push(ObjectProperty::Property(property));
                }
            }
            AttributeNode::SpreadAttribute(spread) => {
                // The template borrow point: erase + guard + derived-rewrite.
                let expr = env.erase(&spread.expression)?;
                let argument = wrap_value_expr(env, expr)?[0].clone();
                let argument_alloc = arena.alloc(argument);
                // The span is the borrowed argument's — the printer emits `...`
                // statically and its comment windows over this template-region span
                // are empty (a carried script comment lives before the last surviving
                // statement, so it never falls in a template-region window).
                properties.push(ObjectProperty::SpreadElement(SpreadElement {
                    argument: argument_alloc,
                    span: argument_alloc.span(),
                }));
            }
            // `class:`/`style:` ride the `classes`/`styles` arguments, and the drop
            // family (`use:`/`transition:`/…/`{@attach}`) contributes nothing to the
            // tag — none carry an object property. `on:`/`let:` already refused in
            // `emit_spread_attributes` before this is reached.
            _ => {}
        }
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// Assemble a `$.attributes(…)` argument list applying the oracle's `b.call`
/// elision: scanning from the end, drop trailing absent arguments; once a present
/// one is seen, every earlier absent one becomes an explicit `void 0` (so e.g. an
/// `<input>`'s flags argument forces `void 0` for the elided css_hash/classes/styles).
fn elide_call_args<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    slots: [Option<Expression<'arena>>; 5],
) -> &'arena [Expression<'arena>] {
    let arena = env.b.arena;
    let last_present = slots.iter().rposition(Option::is_some);
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    if let Some(last) = last_present {
        for slot in slots.into_iter().take(last + 1) {
            match slot {
                Some(expr) => args.push(expr),
                None => args.push(env.b.void_zero()),
            }
        }
    }
    args.into_bump_slice()
}

/// Emit the fused `$.attributes(object, css_hash, classes, styles, flags)` call
/// for a regular element carrying a `{...spread}` (the oracle's
/// `build_element_spread_attributes` / `prepare_element_spread`). The whole
/// attribute set routes through this one call, replacing per-attribute emission:
///
/// - **object** (1st): plain attributes + `bind:` entries + spreads, in source
///   order ([`build_element_spread_object`]);
/// - **css_hash** (2nd): `'svelte-tsvhash'` when the element is scoped — a
///   static-class token **or** a `class:` directive name matches a scoped selector.
///   Unlike the non-spread `class` path the hash is NOT folded into the class value;
///   it rides this argument, which the runtime `$.attributes` merges;
/// - **classes** (3rd): the `class:` directives object ([`build_spread_class_object`],
///   identifier keys + shorthand), absent when there are none;
/// - **styles** (4th): the `style:` directives object ([`build_spread_style_object`],
///   a FLAT object — no `|important` partitioning), absent when there are none;
/// - **flags** (5th): `3` (svg/mathml element — namespaced + preserve-case) /
///   `2` (custom element) / `4` (`<input>`), in that precedence, else absent.
///
/// Trailing absent arguments elide; an interior absent one becomes `void 0`. The
/// no-op drop family (`use:`/`transition:`/…/`{@attach}`) contributes nothing but is
/// still guarded (a stray rune / top-level `await` refuses) exactly as on a
/// non-spread element.
///
/// Refuses the deferred/divergent shapes: a `<select>` (the `$$renderer.select`
/// trap) and a load-error element (which gets `onload`/`onerror` capture markup) —
/// plus, a deliberate runes-only fence rather than a deferral, a legacy `on:` / `let:`
/// directive (deprecated Svelte-4 syntax; migrate to `onclick` / the runes event
/// attribute). A `bind:`/`class:`/`style:` validity gate can also refuse (an invalid
/// bind target, a bad `style:` modifier).
fn emit_spread_attributes<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    host: AttrHost<'arena>,
    name: &str,
    out: &mut BodyBuilder<'arena>,
    scoped: bool,
    namespace: Namespace,
) -> Result<(), CompileError> {
    // The `<select>` spread trap: the oracle routes a spread on a select through
    // `$$renderer.select(...)`, a different callee than `$.attributes`. (A
    // `<svelte:element>`'s name is never `select`, so this never fires for one.)
    if name == "select" {
        return Err(unsupported(Refusal::SpreadOnSelect));
    }
    // A spread on a load-error element makes the oracle add `onload`/`onerror`
    // capture attributes (its `events_to_capture` set, like a `use:`).
    if is_load_error_element(name) {
        return Err(unsupported(Refusal::SpreadOnLoadErrorElement));
    }

    // The oracle's phase-2 directive-validity checks (transition/animate placement)
    // run before it discards the SSR visit — a rejected combination must refuse
    // here too, exactly as on a non-spread element.
    let animate_host = env.animate_host_span == Some(host.span());
    validate_directive_combinations(host.attributes(), animate_host)?;

    // Collect the `class:`/`style:` directives (source order) for the 3rd/4th
    // arguments, guard-and-drop the no-op drop family (SSR runs no client lifecycle,
    // but a misplaced rune / top-level `await` inside the expression still refuses),
    // and deliberately refuse `on:`/`let:` (the runes-only fence). Plain attributes /
    // spreads / `bind:` are handled inside the object builder.
    let mut class_directives: Vec<&'arena ClassDirective<'arena>> = Vec::new();
    let mut style_directives: Vec<&'arena StyleDirective<'arena>> = Vec::new();
    for attr_node in host.attributes() {
        match attr_node {
            AttributeNode::Attribute(_)
            | AttributeNode::SpreadAttribute(_)
            | AttributeNode::BindDirective(_) => {}
            AttributeNode::ClassDirective(d) => class_directives.push(d),
            AttributeNode::StyleDirective(d) => style_directives.push(d),
            // The drop family: `use:` on a load-error element already refused above
            // (the whole element), so only the guard remains here.
            AttributeNode::UseDirective(directive) => {
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::TransitionDirective(directive) => {
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::AnimateDirective(directive) => {
                if let Some(expr) = &directive.expression {
                    guard_dropped(env, expr)?;
                }
            }
            AttributeNode::AttachTag(attach) => guard_dropped(env, &attach.expression)?,
            // A legacy `on:` directive and `let:` deliberately refuse — a runes-only
            // fence matching the non-spread path (the oracle drops them in SSR, but
            // tsv declines: deprecated syntax, migrate to `onclick` / the runes event
            // attribute).
            AttributeNode::OnDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "on:" }));
            }
            AttributeNode::LetDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "let:" }));
            }
        }
    }

    // Whether the element is CSS-scoped (the caller supplies the lookup: a regular
    // element passes `env.element_scope`, a `<svelte:element>` passes
    // `env.special_element_scope`). When scoped, the hash rides the `css_hash` (2nd)
    // argument, never concatenated into the class value.
    let object = build_element_spread_object(env, host, name, namespace)?;
    let css_hash = scoped.then(|| env.b.string_literal_expr(SCOPE_HASH_CLASS));
    let classes = (!class_directives.is_empty())
        .then(|| build_spread_class_object(env, &class_directives))
        .transpose()?;
    let styles = (!style_directives.is_empty())
        .then(|| build_spread_style_object(env, &style_directives))
        .transpose()?;
    // A `<svelte:element>`'s name is always the literal `svelte:element`, so it is
    // never `<input>`/custom → no `flags` argument (the oracle never sets
    // `ELEMENT_IS_INPUT`/`ELEMENT_PRESERVE_ATTRIBUTE_CASE` for one, even
    // `this="input"`). The oracle WOULD set `ELEMENT_IS_NAMESPACED` when the
    // dynamic tag resolves to svg/mathml (its ancestor-derived `metadata.svg`), but
    // that rides the same runtime-tag namespace approximation as the rest of the
    // `<svelte:element>` path (see `namespace::FragmentParent::DynamicElement`).
    let flags_value = match host {
        AttrHost::Regular(element) => spread_element_flags(env, element, name, namespace),
        AttrHost::Dynamic(_) => 0,
    };
    let flags = match flags_value {
        0 => None,
        f => Some(env.b.number(f64::from(f))),
    };
    let args = elide_call_args(env, [Some(object), css_hash, classes, styles, flags]);
    let call = env.b.member_call("$", "attributes", args);
    out.push_expr(call);
    Ok(())
}

/// Emit a `<svelte:element this={…}>` as a statement-level
/// `$.element($$renderer, TAG, attrsFn?, childrenFn?)` call (the oracle's
/// `$.element` server helper). Like a component it splits the template push stream
/// into its own statement; unlike one it pushes NO trailing `<!---->` anchor, and
/// its children fragment is neither text-first nor a component root.
///
/// - **TAG**: `this="div"` → the `'div'` string literal (the parser has already
///   collapsed a mixed `this="a{b}"` to its first static chunk, matching the
///   oracle's legacy warn-and-keep-first); `this={expr}` → the erased expression
///   with a bare derived read rewritten to `d()` (the template borrow point). No
///   static fold — the oracle emits the expression as written.
/// - **attrsFn** (`() => { $$renderer.push(…) }`): the exact regular-element
///   attribute machinery ([`emit_host_attributes`]) rendered into a parameterless
///   closure over the enclosing `$$renderer` — a spread becomes `$.attributes({…})`,
///   `class:`/`style:` become `$.attr_class`/`$.attr_style`. Elided when it would
///   push nothing (e.g. a sole `bind:this`, which omits).
/// - **childrenFn** (`() => { … }`): the element's fragment, emitted like any
///   element child. Elided when the fragment renders nothing.
///
/// CSS scope: a type/universal selector matches a `<svelte:element>`
/// unconditionally, so a styled component scopes every one the census reaches — the
/// hash class is synthesized into the attributes closure exactly like a regular
/// element's (via the shared [`emit_host_attributes`], keyed on the census match).
pub(crate) fn emit_svelte_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    se: &'arena SpecialElement<'arena>,
    tag: &'arena SpecialThis<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;

    // CSS scope: the oracle scopes a `<svelte:element>` whenever a type/universal
    // selector reaches it (its type match is unconditional, `css-prune.js:637-647`),
    // synthesizing `class="svelte-…"` in its attributes closure. The upfront census
    // (`element_census`) holds it as a scoping leaf and owner, so this is the same
    // span lookup as a regular element's — passed through `emit_host_attributes`,
    // where a scoped element with no `class` markup synthesizes one.
    let scoped = env.special_element_scope(se);

    // A `{@const}` is NOT valid as a direct child of a `<svelte:element>` — the
    // oracle rejects it (`const_tag_invalid_placement`; its valid-parent list is
    // `{#snippet}`/`{#if}`/`{:else if}`/`{:else}`/`{#each}`/`{:then}`/`{:catch}`/
    // `<svelte:fragment>`/`<svelte:boundary>`/`<Component>`, and a `<svelte:element>`
    // is not among them). Children are emitted through `emit_child_body`, which
    // pushes a block-scope overlay (load-bearing for snippet hoisting in the
    // closure), and `emit_const_tag` treats a non-empty overlay stack as "inside a
    // block" — so without this guard a direct `{@const}` child would wrongly compile.
    // Refuse it here rather than drop the overlay. (A `{@const}` deeper inside a
    // regular child element remains the pre-existing regular-element placement gap —
    // the same class as `{#if}<div>{@const}</div>{/if}` — tracked separately.)
    if se
        .fragment
        .nodes
        .iter()
        .any(|node| matches!(node, FragmentNode::ConstTag(_)))
    {
        return Err(unsupported(Refusal::ConstTagOutsideBlock));
    }

    // TAG — the dynamic tag name.
    let tag_expr = match tag {
        SpecialThis::Plain { content, .. } => env.b.string_literal_expr(content),
        SpecialThis::Braced(et) => {
            // The template borrow point: erase a TS wrapper, rewrite a bare derived
            // read to `d()`, guard a stray rune / top-level `await`.
            let erased = env.erase(&et.expression)?;
            wrap_value_expr(env, erased)?[0].clone()
        }
    };

    // attrsFn — the attribute machinery rendered into a fresh body, then wrapped in
    // a parameterless closure over the enclosing `$$renderer`. Elided when empty.
    let mut attrs_body = BodyBuilder::new_in(arena);
    emit_host_attributes(
        env,
        AttrHost::Dynamic(se),
        "svelte:element",
        &mut attrs_body,
        scoped,
        // The `<svelte:element>` sits in the enclosing fragment's namespace. Its
        // own tag is never svg/mathml by NAME (`svelte:element`), so this only
        // affects a name-keyed classification that never fires for it.
        ctx.namespace,
    )?;
    let attr_stmts = attrs_body.finish(&mut env.b, arena);
    let attrs_fn = (!attr_stmts.is_empty()).then(|| paramless_renderer_arrow(env, attr_stmts));

    // childrenFn — the element's fragment, emitted like any element child (not
    // text-first, not a component root); whitespace is preserved when inside a
    // `<pre>`/`<textarea>` ancestor. Elided when it renders nothing.
    let child_stmts = emit_child_body(
        env,
        &se.fragment,
        &[],
        false,
        ctx.preserve_whitespace,
        // A `<svelte:element>`'s runtime tag is unknown at compile time, so its
        // children keep the inherited namespace (the oracle reads its ancestor-
        // derived `metadata.svg`; see `FragmentParent::DynamicElement`).
        ChildNamespace {
            inherited: ctx.namespace,
            parent: FragmentParent::DynamicElement,
            in_svg_text: ctx.in_svg_text,
        },
        HashMap::new(),
    )?;
    let children_fn = (!child_stmts.is_empty()).then(|| paramless_renderer_arrow(env, child_stmts));

    // `$.element($$renderer, TAG, attrsFn?, childrenFn?)` with the oracle's argument
    // elision: a present childrenFn forces an absent attrsFn to `void 0`; a trailing
    // absent argument drops.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(tag_expr);
    match (attrs_fn, children_fn) {
        (attrs, Some(children)) => {
            args.push(attrs.unwrap_or_else(|| env.b.void_zero()));
            args.push(children);
        }
        (Some(attrs), None) => args.push(attrs),
        (None, None) => {}
    }
    let call = env.b.member_call("$", "element", args.into_bump_slice());
    let span = call.span();
    out.push_statement(
        &mut env.b,
        arena,
        Statement::ExpressionStatement(ExpressionStatement {
            expression: call,
            span,
            is_directive: false,
        }),
    );
    Ok(())
}

/// A parameterless arrow closing over the enclosing `$$renderer`
/// (`() => { <body> }`) — the shape a `<svelte:element>`'s attributes and children
/// closures take. They capture the outer `$$renderer` rather than receiving one, so
/// there is no parameter (unlike a component's `children: ($$renderer) => …` prop,
/// which is passed a fresh renderer).
fn paramless_renderer_arrow<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    body: &'arena [Statement<'arena>],
) -> Expression<'arena> {
    let block_span = env.b.here();
    env.b.arrow_block(&[], body, block_span)
}
