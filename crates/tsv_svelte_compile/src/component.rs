//! Component invocation: the `Foo($$renderer, {…props})` call, its props object,
//! and its snippet/slot children.
//!
//! A **per-node emitter** in the emission pipeline, and a leaf: it has **zero**
//! outgoing edges to element or `<svelte:element>` emission. The only connection
//! is one dispatch from [`crate::element::emit_element`] on
//! `ElementKind::Component`, plus [`component_is_standalone_eligible`], which
//! [`crate::fragment`]'s `is_standalone` recomputation reads.
//!
//! **Single source of truth** for the component props shape — the plain object
//! literal vs `$.spread_props([…])` fork, the implicit `children` snippet prop for
//! default-slot content, and named `{#snippet}` children as named snippet props
//! with the `$$slots: { key: true, … }` marker beside them. The prop *order* is
//! the oracle's source order across all three kinds, so a second builder that
//! assembled any one of them separately would reorder the object.
//!
//! See [`crate::transform_server`] for the orchestration.

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, Element, ElementKind, Fragment, FragmentNode,
    SpecialElement, SpecialElementKind,
};
use tsv_ts::ast::internal::{
    ArrayExpression, BlockStatement, Expression, ExpressionStatement, ObjectExpression,
    ObjectProperty, Property, Statement,
};

use crate::analyze::{BindingKind, evaluate, stringify_value};
use crate::body_builder::BodyBuilder;
use crate::build::{escape_template_text, init_property};
use crate::fragment::emit_child_body;
use crate::namespace::{ChildNamespace, FragmentParent, Namespace};
use crate::script_decls::plain_identifier_name;
use crate::snippet_emit::build_snippet_function;
use crate::template_value::wrap_value_expr;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Whether a component is *dynamic* — the oracle's `metadata.dynamic`
/// (`2-analyze/visitors/Component.js:14`): `binding !== null && (binding.kind !==
/// 'normal' || name.includes('.'))`. A dynamic component compiles to an
/// `if (expr) {…}` truthiness guard with hydration anchors, not a plain call —
/// refused in this slice.
///
/// - A member component (`<Foo.Bar>`) is always dynamic.
/// - A block-local name (each item/index, `{:then}` value, `{@const}`) resolves
///   through an overlay to a non-`normal` binding → dynamic.
/// - A top-level `prop`/`$derived`/`$state` binding → dynamic. A plain
///   declaration/import (`normal`) or an unresolved global (`binding === null`)
///   is **not** dynamic.
fn component_dynamic(env: &EmitEnv<'_, '_>, name: &str) -> bool {
    if name.contains('.') {
        return true;
    }
    if env
        .overlays
        .iter()
        .any(|overlay| overlay.contains_key(name))
    {
        return true;
    }
    match env.bindings.get(name) {
        None => false,
        Some(binding) => match binding.kind {
            BindingKind::Prop | BindingKind::Derived => true,
            BindingKind::Normal | BindingKind::Opaque => env.state_names.contains(name),
        },
    }
}

/// Whether a sole fragment child is a standalone-eligible component (the oracle's
/// `clean_nodes` `is_standalone`: a non-dynamic `Component` with no
/// `--custom-property` attribute — `hmr` is always off here). When true its call
/// reuses the enclosing block's anchor and emits no trailing `<!---->`.
pub(crate) fn component_is_standalone_eligible(
    env: &EmitEnv<'_, '_>,
    element: &Element<'_>,
) -> bool {
    if element.kind != ElementKind::Component {
        return false;
    }
    let name = {
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(element.name).to_string()
    };
    if component_dynamic(env, &name) {
        return false;
    }
    !element.attributes.iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name).starts_with("--")
    })
}

/// The component-children analysis product: the synthetic props to append after
/// the attribute props, plus the snippet-prop function declarations that go into
/// the component's wrapping block.
struct ChildrenPlan<'arena> {
    /// `function name($$renderer, …) { … }` declarations (source order), placed in
    /// the component's wrapping block so the snippet props can reference them.
    snippet_functions: Vec<Statement<'arena>>,
    /// Snippet prop names (source order) — each emits a `{ name }` shorthand prop.
    snippet_props: Vec<String>,
    /// `$$slots` entries (slot keys, source order): snippet slots, then `default`.
    slot_keys: Vec<String>,
    /// The default-slot children fragment (direct `{#snippet}` children filtered
    /// out) → the implicit `children: ($$renderer) => { … }` arrow, if any.
    default_children: Option<Fragment<'arena>>,
}

impl ChildrenPlan<'_> {
    /// Whether the plan contributes any synthetic props (a snippet prop, the
    /// `children` arrow, or `$$slots`) — `slot_keys` is non-empty exactly then.
    fn has_content(&self) -> bool {
        !self.slot_keys.is_empty()
    }
}

/// Plan a component's children: build the `{#snippet}` prop functions (in source
/// order) and the synthetic prop shape, refusing two cases — a `slot="…"` child
/// (a named slot: the permanent runes-only fence, not a deferral) and a `children`
/// prop alongside default children (the oracle's `$$slots.default` divergence, a
/// genuine deferred gap). A `{#snippet}` child named `children` keeps the
/// `children` prop name but a `default` slot key (the oracle's rename).
fn plan_component_children<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    name: &str,
) -> Result<ChildrenPlan<'arena>, CompileError> {
    let arena = env.b.arena;
    let mut snippet_functions = Vec::new();
    let mut snippet_props = Vec::new();
    let mut slot_keys = Vec::new();
    let mut has_default = false;
    for node in element.fragment.nodes {
        match node {
            FragmentNode::SnippetBlock(snippet) => {
                let (func, snippet_name) = build_snippet_function(env, snippet)?;
                snippet_functions.push(func);
                // The oracle serializes a snippet named `children` under the
                // `default` slot key, but the prop keeps the `children` name.
                let slot_key = if snippet_name == "children" {
                    "default".to_string()
                } else {
                    snippet_name.clone()
                };
                snippet_props.push(snippet_name);
                slot_keys.push(slot_key);
            }
            FragmentNode::Comment(_) => {}
            FragmentNode::Text(text) if text.is_ascii_ws_only => {}
            FragmentNode::Element(child) if child_slot_attribute(env, child) => {
                return Err(unsupported(Refusal::ComponentNamedSlot {
                    name: name.to_string(),
                }));
            }
            // A `<svelte:element slot="x">` as a component child routes to a NAMED
            // slot in the oracle, but tsv's named-slot detection above is
            // `FragmentNode::Element`-only — without this arm the node would fall
            // through to `has_default` and MISROUTE into the default `children`.
            // Named slots are a permanent runes-only fence, never a deferral
            // (`Refusal::is_deliberate_fence`), so this arm IS the fence's
            // special-element half rather than a placeholder for future support. A
            // slot-less `<svelte:element>` is ordinary default content (`_` below).
            FragmentNode::SpecialElement(child)
                if matches!(child.kind, SpecialElementKind::SvelteElement { .. })
                    && special_element_slot_attribute(env, child) =>
            {
                return Err(unsupported(Refusal::ComponentNamedSlot {
                    name: name.to_string(),
                }));
            }
            _ => has_default = true,
        }
    }
    // A `children` prop AND default children route through `$$slots.default` with
    // a `children` error in the oracle — a divergent shape; refuse.
    if has_default && component_has_named_attribute(env, element, "children") {
        return Err(unsupported(Refusal::ComponentChildrenPropConflict {
            name: name.to_string(),
        }));
    }
    let default_children = if has_default {
        // The `children` arrow sees only the default-slot children — direct
        // `{#snippet}` children live in the wrapping block, not the arrow body.
        let mut nodes: BumpVec<'arena, FragmentNode<'arena>> = BumpVec::new_in(arena);
        for node in element.fragment.nodes {
            if !matches!(node, FragmentNode::SnippetBlock(_)) {
                nodes.push(node.clone());
            }
        }
        slot_keys.push("default".to_string());
        Some(Fragment {
            nodes: nodes.into_bump_slice(),
        })
    } else {
        None
    };
    Ok(ChildrenPlan {
        snippet_functions,
        snippet_props,
        slot_keys,
        default_children,
    })
}

/// Whether a component child element carries a `slot="…"` attribute (a named
/// slot).
///
/// Case-**sensitive**, mirroring its oracle site: the server component visitor
/// matches `node.name === 'slot'` (`server/visitors/shared/component.js:50`).
/// ⚠️ Do NOT unify this with `element_census.rs`'s `has_slot_attribute`, which is
/// case-**insensitive** because CSS pruning lowercases
/// (`css-prune.js:1015`) — the oracle is split, so the two predicates are
/// deliberately different, and merging them would break CSS-prune parity.
fn child_slot_attribute(env: &EmitEnv<'_, '_>, element: &Element<'_>) -> bool {
    component_has_named_attribute(env, element, "slot")
}

/// Whether a special element carries a plain `slot="…"` attribute (case-sensitive) —
/// the named-slot marker on a `<svelte:element>` component child.
///
/// The case-sensitivity matches `child_slot_attribute` and the same oracle site
/// (`server/visitors/shared/component.js:50`); see that function for why it must
/// NOT be unified with the census's case-insensitive `has_slot_attribute`.
fn special_element_slot_attribute(env: &EmitEnv<'_, '_>, se: &SpecialElement<'_>) -> bool {
    se.attributes.iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name) == "slot"
    })
}

/// Whether an element carries a plain attribute with the given (case-sensitive)
/// name.
fn component_has_named_attribute(env: &EmitEnv<'_, '_>, element: &Element<'_>, name: &str) -> bool {
    element.attributes.iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name) == name
    })
}

/// Whether `name` matches the ECMAScript identifier grammar Svelte's `b.key` uses
/// (`regex_is_valid_identifier`, `/^[a-zA-Z_$][a-zA-Z_$0-9]*$/`) — an identifier
/// key, otherwise a string-literal key.
fn is_valid_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Emit `Name($$renderer, props)` for a component invocation (`<Foo … />`),
/// followed by a trailing `<!---->` anchor unless the enclosing fragment is
/// standalone (the oracle's `empty_comment` push). Named-snippet children wrap
/// the call in a bare `{ function …; Name(…); }` block. Dynamic components, named
/// slots, `--custom-property` attributes, and directives are refused — see the
/// individual refusal sites.
pub(crate) fn emit_component<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    out: &mut BodyBuilder<'arena>,
    name: &str,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    // A dynamic component (member / reactive binding) compiles to the truthiness
    // guard — a later slice.
    if component_dynamic(env, name) {
        return Err(unsupported(Refusal::DynamicComponent {
            name: name.to_string(),
        }));
    }

    // Children plan: the `{#snippet}` prop functions (for the wrapping block) and
    // the synthetic props (snippet props, the `children` arrow, `$$slots`). Built
    // before the props so the snippet functions mint before the props reference
    // them.
    let plan = plan_component_children(env, element, name)?;

    // Build the props/spreads expression (a plain object, or `$.spread_props`),
    // appending the synthetic children props from the plan.
    let props_expr = build_component_props(env, element, name, &plan)?;

    // `Name($$renderer, props)`. The callee is the component reference (a plain
    // identifier — member components refuse above).
    let callee = env.b.ident_expr(name);
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(props_expr);
    let call = env.b.call_of(callee, args.into_bump_slice(), false);
    let span = call.span();
    let call_stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });

    // Named-snippet children hoist their `function` declarations into a bare block
    // wrapping the call, so the snippet props resolve (the oracle's
    // `b.block([...snippet_declarations, statement])`).
    let stmt = if plan.snippet_functions.is_empty() {
        call_stmt
    } else {
        let mut block_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        for func in plan.snippet_functions {
            block_body.push(func);
        }
        block_body.push(call_stmt);
        let block_span = env.b.here();
        Statement::BlockStatement(BlockStatement {
            body: block_body.into_bump_slice(),
            span: block_span,
        })
    };
    out.push_statement(&mut env.b, arena, stmt);

    // A non-standalone component keeps the `<!---->` anchor so its output doesn't
    // fuse with the surrounding fragment.
    if !is_standalone {
        out.push_text("<!---->");
    }
    Ok(())
}

/// A group of component attributes: consecutive plain attributes accumulate into
/// one object literal; a `{...spread}` starts a new group (the oracle's
/// `props_and_spreads` grouping in `shared/component.js`).
enum PropGroup<'a, 'arena> {
    Props(Vec<&'a Attribute<'arena>>),
    Spread(&'a Expression<'arena>),
}

/// Build the component call's props argument: a plain object `{ … }` when there
/// are no spreads (or a single leading props group), otherwise
/// `$.spread_props([ … ])` interleaving objects and spread expressions in source
/// order. Refuses `--custom-property` attributes, `bind:`, and other directives.
fn build_component_props<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    name: &str,
    plan: &ChildrenPlan<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    // The synthetic children props (snippet props, the `children` arrow,
    // `$$slots`) go into the last props group, or a new one after a trailing
    // spread.
    let synthetic = plan.has_content().then_some(plan);
    let mut groups: Vec<PropGroup<'_, 'arena>> = Vec::new();
    for attr_node in element.attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                let attr_name = {
                    let interner = env.b.interner.borrow();
                    interner.resolve_infallible(attr.name).to_string()
                };
                // A `--custom-property` attribute takes the oracle's `$.css_props`
                // path — a later slice.
                if attr_name.starts_with("--") {
                    return Err(unsupported(Refusal::ComponentCustomProperty {
                        name: name.to_string(),
                    }));
                }
                match groups.last_mut() {
                    Some(PropGroup::Props(props)) => props.push(attr),
                    _ => groups.push(PropGroup::Props(vec![attr])),
                }
            }
            AttributeNode::SpreadAttribute(spread) => {
                groups.push(PropGroup::Spread(&spread.expression));
            }
            AttributeNode::BindDirective(_) => {
                return Err(unsupported(Refusal::ComponentBindDirective {
                    name: name.to_string(),
                }));
            }
            _ => {
                return Err(unsupported(Refusal::ComponentDirective {
                    name: name.to_string(),
                }));
            }
        }
    }

    // The oracle emits a plain object when there are no spreads (no groups, or a
    // single props group); otherwise `$.spread_props([...])`. The synthetic props
    // append to the last props group, or a new one when the last group is a spread
    // (the oracle's `push_prop`).
    let single_object =
        groups.is_empty() || (groups.len() == 1 && matches!(groups[0], PropGroup::Props(_)));
    if single_object {
        let attrs: &[&Attribute<'arena>] = match groups.first() {
            Some(PropGroup::Props(props)) => props,
            _ => &[],
        };
        return build_props_object(env, attrs, synthetic);
    }

    // `$.spread_props([ obj_or_spread, … ])`. Mint the brackets around the
    // element construction so the array span encloses the minted object spans.
    let last_is_props = matches!(groups.last(), Some(PropGroup::Props(_)));
    let lbracket = env.b.mint("[").start;
    let mut elements: BumpVec<'arena, Option<Expression<'arena>>> = BumpVec::new_in(arena);
    let group_count = groups.len();
    for (i, group) in groups.iter().enumerate() {
        let element_expr = match group {
            PropGroup::Props(props) => {
                // The synthetic props join the last props group.
                let syn = if i + 1 == group_count {
                    synthetic
                } else {
                    None
                };
                build_props_object(env, props, syn)?
            }
            PropGroup::Spread(expr) => {
                // The template borrow point (`<Foo {...(o as any)} />`).
                let expr = env.erase(expr)?;
                wrap_value_expr(env, expr)?[0].clone()
            }
        };
        elements.push(Some(element_expr));
    }
    // A trailing spread with synthetic props needs its own props object appended.
    if !last_is_props && let Some(plan) = synthetic {
        elements.push(Some(build_props_object(env, &[], Some(plan))?));
    }
    let rbracket = env.b.mint("]").end;
    let array = Expression::ArrayExpression(ArrayExpression {
        elements: elements.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(lbracket, rbracket),
    });
    let array_alloc = arena.alloc(array);
    Ok(env
        .b
        .member_call("$", "spread_props", std::slice::from_ref(array_alloc)))
}

/// Build a plain object literal `{ … }` from a run of component attributes. The
/// braces are minted around the property construction so the object span encloses
/// the (appendix) key spans (the object printer reads its own span region for the
/// expansion decision — all appendix, no newlines, so it collapses when it fits).
fn build_props_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attrs: &[&'arena Attribute<'arena>],
    synthetic: Option<&ChildrenPlan<'arena>>,
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for attr in attrs {
        properties.push(ObjectProperty::Property(build_component_property(
            env, attr,
        )?));
    }
    // The synthetic children props, in the oracle's order: snippet props (source
    // order), then the implicit `children` arrow, then `$$slots`.
    if let Some(plan) = synthetic {
        for snippet_name in &plan.snippet_props {
            properties.push(ObjectProperty::Property(build_snippet_prop(
                env,
                snippet_name,
            )));
        }
        if let Some(fragment) = &plan.default_children {
            properties.push(ObjectProperty::Property(build_children_prop(
                env, fragment,
            )?));
        }
        properties.push(ObjectProperty::Property(build_slots_prop(
            env,
            &plan.slot_keys,
        )));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// A `{ name }` shorthand prop for a named-snippet child — the value references
/// the `function name(…)` declaration in the component's wrapping block.
fn build_snippet_prop<'arena>(env: &mut EmitEnv<'arena, '_>, name: &str) -> Property<'arena> {
    let key = env.b.ident(name);
    let key_span = key.span;
    let value = env.b.ident(name);
    init_property(
        Expression::Identifier(key),
        Expression::Identifier(value),
        true,
        key_span,
    )
}

/// The implicit `children` prop for a component's default-slot children:
/// `children: ($$renderer) => { …body… }`. The body reuses the fragment
/// machinery (text-first eligible, per the oracle's `is_text_first` Component
/// parent). The key is minted first so the (key-only) property span stays forward.
fn build_children_prop<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let arena = env.b.arena;
    let key = env.b.ident("children");
    let key_span = key.span;
    // Component children (the default-slot `children` snippet) re-infer their
    // namespace from their own nodes (Svelte's `Component` parent — in the
    // deep-walk special list); the inherited context is not threaded here, so the
    // html document default is the fallback when the slot holds no elements.
    let body = emit_child_body(
        env,
        fragment,
        &[],
        true,
        false,
        ChildNamespace {
            inherited: Namespace::Html,
            parent: FragmentParent::Special,
            in_svg_text: false,
        },
        HashMap::new(),
    )?;
    let renderer_param = Expression::Identifier(env.b.ident("$$renderer"));
    let params = std::slice::from_ref(arena.alloc(renderer_param));
    let block_span = env.b.here();
    let arrow = env.b.arrow_block(params, body, block_span);
    Ok(init_property(
        Expression::Identifier(key),
        arrow,
        false,
        key_span,
    ))
}

/// The `$$slots: { key1: true, … }` prop that accompanies component children —
/// one `true` entry per named-snippet slot plus `default` for default children
/// (slot names are always valid identifiers). Named-slot arrow values would live
/// here too, but named slots are refused.
fn build_slots_prop<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    slot_keys: &[String],
) -> Property<'arena> {
    let arena = env.b.arena;
    let key = env.b.ident("$$slots");
    let key_span = key.span;
    let obrace = env.b.mint("{").start;
    let mut inner_props: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for slot_key in slot_keys {
        let entry_key = env.b.ident(slot_key);
        let entry_key_span = entry_key.span;
        let entry_val = env.b.true_literal();
        inner_props.push(ObjectProperty::Property(init_property(
            Expression::Identifier(entry_key),
            entry_val,
            false,
            entry_key_span,
        )));
    }
    let cbrace = env.b.mint("}").end;
    let inner = Expression::ObjectExpression(ObjectExpression {
        properties: inner_props.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    });
    init_property(Expression::Identifier(key), inner, false, key_span)
}

/// Build one `key: value` object property from a component attribute. The key is
/// an identifier when it matches the identifier grammar (else a string literal);
/// `shorthand` is set when the key is an identifier and the value is the plain
/// identifier of the same name (`{ n: n }` prints as `{ n }`). The key is minted
/// before the value, so the (key-only) property span stays forward and in the
/// appendix; the value prints from its own span.
fn build_component_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let name = {
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name).to_string()
    };
    let key_is_ident = is_valid_js_identifier(&name);
    let key = if key_is_ident {
        Expression::Identifier(env.b.ident(&name))
    } else {
        env.b.string_literal_expr(&name)
    };
    let key_span = key.span();
    let value = build_prop_value(env, attr)?;
    let shorthand = key_is_ident
        && matches!(&value, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(name.as_str()));
    Ok(init_property(key, value, shorthand, key_span))
}

/// Build a component attribute's prop value:
///
/// - a boolean attribute → `true`;
/// - a single static text value → the *decoded* data as a string literal (no
///   HTML escaping, no trim — the oracle's `is_component` branch of
///   `build_attribute_value`);
/// - a single expression value → guarded, bare-derived → `d()`, passed through
///   with **no fold** (the single-chunk component path doesn't evaluate);
/// - a mixed text+expression value → a template literal (or a folded string
///   literal when every part is statically known).
fn build_prop_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = attr.value else {
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            Ok(env.b.string_literal_expr(&decoded))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // The template borrow point. The erased node also decides the caller's
            // `{ n }` shorthand test — `<Foo n={n as T} />` is `{ n }` to the oracle.
            let expr = env.erase(&tag.expression)?;
            let wrapped = wrap_value_expr(env, expr)?;
            Ok(wrapped[0].clone())
        }
        _ => build_component_mixed_value(env, values),
    }
}

/// Build a mixed text+expression component attribute value. Unlike the element
/// mixed-attribute path there is no whitespace trim, no HTML escaping, and no
/// `$.attr*` wrapper — the oracle's component `build_attribute_value` returns the
/// bare value: a folded string literal when every part is statically known, else
/// a template literal with `$.stringify(expr)` interpolations (omitted when the
/// evaluator proves a defined string).
fn build_component_mixed_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    values: &'arena [AttributeValue<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value in parallel — consumed only when every part folds.
    let mut raw = String::new();
    let mut exprs: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    for value in values {
        match value {
            AttributeValue::Text(text) => {
                let decoded = text.data(env.source);
                raw.push_str(&decoded);
                #[allow(clippy::unwrap_used)]
                texts
                    .last_mut()
                    .unwrap()
                    .push_str(&escape_template_text(&decoded));
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
        return Ok(env.b.string_literal_expr(&raw));
    }
    Ok(env.b.template_literal(&texts, exprs.into_bump_slice()))
}
