//! The guards for regions the SSR output does **not** emit: dropped expressions,
//! emitted-verbatim patterns, whole dropped fragments, and the SSR-inert special
//! elements.
//!
//! A **shared primitive** of the emission layer, and the one whose job is the
//! inverse of every emitter's: an emitter never visits a dropped region, so
//! nothing it does can refuse what sits there. The oracle decides TypeScript at
//! parse time and rune placement at analysis time — both before it chooses what to
//! emit — so a dropped region still needs refusal-equivalent walking. These
//! functions are that walk, called by [`crate::fragment`] (the inert special
//! elements), [`crate::blocks`] (the `{#each}` key, the `{#key}` expression, the
//! `{:catch}` branch), [`crate::element`] and [`crate::attribute`] (event
//! handlers, the no-op drop family). Nothing here calls back into an emitter.
//!
//! **Single source of truth** for the *scoping* rule — "refuse where the construct
//! can affect the result", which is deliberately narrower than "a fence refuses
//! everywhere". Both directions are dangerous, which is why the judgment lives in
//! one exhaustive match ([`dropped_presence_refusal`]) rather than at each caller:
//! refusing too little is an over-acceptance the corpus cannot see (a
//! whole-component validation firing from a dropped position), while refusing too
//! much turns correct output into refusals for nothing.
//!
//! See [`crate::transform_server`] for the orchestration, and
//! [`crate::attr_refs`] for the shared traversals these ride.

use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, Fragment, FragmentNode, SpecialElement,
    SpecialElementKind, StyleDirectiveValue,
};
use tsv_ts::ast::internal::Expression;

use crate::analyze::NameSet;
use crate::attr_refs::{TemplateItem, each_child_fragment, each_template_item};
use crate::rune_guard::{WalkCtx, walk_expression_guarded};
use crate::special_element_kind::SPECIAL_ELEMENT_SLOT;
use crate::transform_server::{EmitEnv, unsupported};
use crate::{CompileError, Refusal};

/// Validate and guard-drop the attributes of an SSR-inert special element
/// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`). The element emits
/// NOTHING, but the oracle still runs its full phase-2 analysis over it, so every
/// shape the oracle rejects at analysis must refuse here (tsv's parser is
/// permissive where the oracle is strict). Mirrors the oracle's
/// `SvelteWindow`/`SvelteBody`/`SvelteDocument` visitors + `disallow_children` +
/// `BindDirective`:
///
/// - **children** (`disallow_children`): these cannot have children
///   (`svelte_meta_invalid_content`). tsv's parser *does* parse children into the
///   fragment, so a non-empty fragment refuses.
/// - **illegal attribute** (`illegal_element_attribute` /
///   `svelte_body_illegal_attribute`): only a **modern event attribute**
///   (`on*={expr}`, a single-expression value) is legal; a spread and every other
///   plain attribute refuse.
/// - **invalid bind** (`BindDirective` + `binding_properties`): a `bind:` is valid
///   iff its name is in the per-kind whitelist ([`inert_bind_is_valid`]) **and** its
///   target is a reassignable lvalue (`attribute::validate_inert_bind_target` — the
///   same `$state`-rooted fork regular elements use); otherwise refuse.
/// - **legacy directives**: a legacy `on:` event directive and `let:` are
///   runes-only-fence refusals (`NonPlainAttribute`), matching the regular-element
///   path — even though the oracle happens to accept `on:` here, tsv declines it (a
///   safe over-refusal).
/// - the **no-op drop family** (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/
///   `animate:`/`{@attach}`): oracle-accepted, so guard-and-drop each expression
///   (SSR runs no client lifecycle, but a stray rune / top-level `await` refuses).
pub(crate) fn guard_inert_special_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    se: &'arena SpecialElement<'arena>,
    kind: &SpecialElementKind<'arena>,
    tag: &'static str,
) -> Result<(), CompileError> {
    if !se.fragment.nodes.is_empty() {
        return Err(unsupported(Refusal::SpecialElementChildren {
            name: tag.to_string(),
        }));
    }
    // Exhaustive over `AttributeNode` so a new variant fails compilation here
    // rather than silently being dropped (or refused).
    for attr in se.attributes {
        match attr {
            AttributeNode::Attribute(a) => {
                if is_event_attribute(a, env.source) {
                    // A modern event handler drops from SSR output, but its
                    // expression is still guarded (a misplaced rune / top-level
                    // `await` is an oracle analysis-phase error).
                    if let Some([AttributeValue::ExpressionTag(t)]) = a.value {
                        guard_dropped(env, &t.expression)?;
                    }
                } else {
                    return Err(unsupported(Refusal::SpecialElementIllegalAttribute {
                        name: tag.to_string(),
                    }));
                }
            }
            AttributeNode::SpreadAttribute(_) => {
                return Err(unsupported(Refusal::SpecialElementIllegalAttribute {
                    name: tag.to_string(),
                }));
            }
            AttributeNode::BindDirective(d) => {
                let name = d.name_span.extract(env.source).to_string();
                // (1) the bind NAME must be in the per-kind whitelist, and (2) its
                // TARGET must be a reassignable lvalue — the SAME two-part rule
                // regular elements enforce (`attribute::validate_inert_bind_target`
                // reuses the shared `$state`-rooted lvalue fork), so a non-lvalue /
                // const / undefined / plain-`let` / prop target refuses just as the
                // oracle rejects it (`bind_invalid_expression` / `constant_binding` /
                // `bind_invalid_value`).
                if !inert_bind_is_valid(kind, &name) {
                    return Err(unsupported(Refusal::BindDirective { name }));
                }
                crate::attribute_bind::validate_inert_bind_target(env, d)?;
                // The bind is dropped from SSR output but still guarded (a stray rune
                // / top-level `await`); its reassignment is collected in
                // `needs_context` so a later read of a `$state` target stays dynamic
                // (an unreassigned `$state` read otherwise folds to its init value).
                guard_dropped(env, &d.expression)?;
            }
            // Legacy `on:` event directive and `let:` — runes-only fence: refuse,
            // matching the regular-element path (`element.rs`).
            AttributeNode::OnDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "on:" }));
            }
            AttributeNode::LetDirective(_) => {
                return Err(unsupported(Refusal::RunesOnlyFence { directive: "let:" }));
            }
            // The no-op drop family: guard-and-drop each expression, like a regular
            // element (the oracle accepts them on these elements and drops them).
            AttributeNode::ClassDirective(d) => guard_dropped(env, &d.expression)?,
            AttributeNode::UseDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::TransitionDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::AnimateDirective(d) => {
                if let Some(e) = &d.expression {
                    guard_dropped(env, e)?;
                }
            }
            AttributeNode::AttachTag(t) => guard_dropped(env, &t.expression)?,
            AttributeNode::StyleDirective(d) => match &d.value {
                StyleDirectiveValue::ExpressionTag(t) => guard_dropped(env, &t.expression)?,
                StyleDirectiveValue::Parts(parts) => {
                    for v in *parts {
                        if let AttributeValue::ExpressionTag(t) = v {
                            guard_dropped(env, &t.expression)?;
                        }
                    }
                }
                StyleDirectiveValue::True => {}
            },
        }
    }
    Ok(())
}

/// The oracle's `is_event_attribute` (`utils/ast.js`): a plain attribute whose
/// value is a single expression (`{expr}`) and whose RAW authored name starts with
/// `on`. tsv always wraps an attribute value in an array, so the oracle's
/// `is_expression_attribute` is exactly the single-`ExpressionTag` case here.
fn is_event_attribute(attr: &Attribute<'_>, source: &str) -> bool {
    attr.name_span.extract(source).starts_with("on")
        && matches!(attr.value, Some([AttributeValue::ExpressionTag(_)]))
}

/// Whether the `bind:<name>` NAME is valid on an SSR-inert special element — a
/// faithful SUBSET of the oracle's `binding_properties` rule (`BindDirective` +
/// `bindings.js`): `this`/`focused` are unrestricted, otherwise the name must be
/// in the element's `valid_elements` list. Deliberately over-refuses the extra
/// names the oracle also accepts on `<svelte:body>` (the dimension family —
/// `clientWidth`/`offsetWidth`/…) as a safe "not yet", never an over-acceptance.
/// The bind's TARGET (its lvalue/reassignability) is validated separately by
/// `attribute::validate_inert_bind_target`, which the caller runs next.
fn inert_bind_is_valid(kind: &SpecialElementKind<'_>, name: &str) -> bool {
    if name == "this" || name == "focused" {
        return true;
    }
    match kind {
        SpecialElementKind::SvelteWindow => matches!(
            name,
            "innerWidth"
                | "innerHeight"
                | "outerWidth"
                | "outerHeight"
                | "scrollX"
                | "scrollY"
                | "online"
                | "devicePixelRatio"
        ),
        SpecialElementKind::SvelteDocument => matches!(
            name,
            "activeElement" | "fullscreenElement" | "pointerLockElement" | "visibilityState"
        ),
        // `<svelte:body>` has no element-specific window/document binding; only the
        // unrestricted `this`/`focused` above. (The caller only passes an inert
        // kind, so the other arms are unreachable.)
        _ => false,
    }
}

/// The guard for an expression the SSR output **drops** — the `{#each}` key, the
/// `{#key}` expression, an event-handler attribute, and everything inside a
/// `{:catch}` branch.
///
/// A misplaced rune must still refuse: the oracle rejects one in its *analysis*
/// phase, before it decides what to emit, so dropping the region cannot make the
/// component valid (`{:catch e}{$state(1)}{/await}` is a `state_invalid_placement`
/// error there).
///
/// But the derived-read rule is switched **off** — the empty `derived_names` set.
/// It is an emission *rewrite* (a derived read, bare or nested, becomes `d()`;
/// [`crate::template_value::wrap_value_expr`] applies it before the guard runs, so
/// `walk_expression_guarded` refuses every derived read that reaches it), not a
/// validity rule: the oracle happily accepts a derived read it never emits.
/// Enforcing it in a dropped region would refuse `{#key d}` and
/// `{:catch e}<p>{d}</p>`, which the oracle compiles.
pub(crate) fn guard_dropped<'arena>(
    env: &EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    // The component-wide reassignment/shadow collection is `needs_context`'s job
    // (it walks the dropped regions too), so these two are throwaways.
    let no_derived = NameSet::default();
    // A valid `$name` store reference in a dropped region (an event handler,
    // `{:catch}`, an each/key expression) is allowed — the region isn't emitted,
    // so no rewrite is needed, and the `var $$store_subs` injection is already
    // decided by `needs_context`. A shadowed base still refuses here (the oracle's
    // `store_invalid_scoped_subscription`), so we pass the full shadow set.
    let mut ctx = WalkCtx::new(
        env.source,
        &mut updated,
        &mut nested,
        &no_derived,
        std::rc::Rc::clone(&env.b.interner),
    )
    .allow_store_reads(&env.store_names, Some(&env.store_shadowed));
    walk_expression_guarded(expr, &mut ctx)
}

/// The guard for a binding **pattern** the SSR output EMITS verbatim — the
/// `{#each}` context (`let CTX = each_array[i]`) and the `{:then}` value (the
/// then-arrow's parameter).
///
/// A pattern is not a dropped region: its *default values* are real emitted
/// expressions (`{#each xs as { a = d }}`), and this emitter borrows the pattern
/// through untouched — it never routes a pattern position through the value walk,
/// so it never rewrites a derived read inside one to `d()`. The derived rule stays
/// ON here (unlike [`guard_dropped`]), and the two pattern positions want it for
/// opposite reasons, both satisfied by one uniform "keep it on and refuse":
///
/// - the `{:then}` value default (`{#await p then {x = d}}`): the oracle emits
///   `({ x = d() })` — `d()`. Borrowing the pattern verbatim would emit a bare
///   `d` → a MISMATCH, so refusing is **mandatory** here.
/// - the `{#each}` context default (`{#each xs as {v = d}}`): the oracle emits
///   `let { v = d }` — a bare `d`. tsv *could* match by borrowing verbatim, but
///   keeps the rule ON as a **deferred safe over-refusal** (patterns are not
///   rewritten this slice; refusing is never a MISMATCH).
pub(crate) fn guard_pattern<'arena>(
    env: &EmitEnv<'arena, '_>,
    pattern: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(
        env.source,
        &mut updated,
        &mut nested,
        &env.derived_names,
        std::rc::Rc::clone(&env.b.interner),
    );
    walk_expression_guarded(pattern, &mut ctx)
}

/// Guard a whole fragment the SSR output drops (the `{:catch}` branch) — the
/// dropped-fragment analog of [`guard_dropped`], over `attr_refs`' shared
/// traversals. Without it a rune anywhere inside a `{:catch}` compiles, which the
/// oracle rejects: an emission-dropped fragment still needs refusal-equivalent
/// walking — the property `dropped_fragments_are_walked` pins.
///
/// Two walks, because a dropped region carries two kinds of fact:
///
/// - what it **references** — every borrowed expression, through
///   [`each_template_item`]'s dropped-fragment view;
/// - what it **is** — the node kinds whose mere presence the oracle's analysis
///   reads into the output, through [`guard_dropped_presence`].
///
/// Nothing else covers the second: the emission refusals never fire in a region
/// the emitter does not visit, so without this walk a presence-read node reaches
/// no guard at all. The test `dropped_fragment_refuses_presence_read_nodes` pins
/// it, together with the set that must keep compiling beside it.
pub(crate) fn guard_dropped_fragment<'arena>(
    env: &EmitEnv<'arena, '_>,
    fragment: &'arena Fragment<'arena>,
) -> Result<(), CompileError> {
    guard_dropped_presence(fragment)?;
    each_template_item(fragment, &mut |item| match item {
        TemplateItem::Expression(expr) => guard_dropped(env, expr),
        // A `<T>` clause holds no value reference. TypeScript in a document with
        // no `lang="ts"` is refused up front by `refuse_template_typescript`.
        TemplateItem::SnippetTypeParameters => Ok(()),
    })
}

/// Refuse a node — or an attribute on one — in a dropped fragment whose
/// **presence alone** the oracle reads into its compile result.
///
/// The scoping rule, and why it is narrower than "a fenced construct refuses
/// everywhere": SSR dropping a region suppresses a construct's *emission*, so a
/// construct that only ever affected the output by being emitted is genuinely
/// inert there — both compilers drop it and the outputs agree. What dropping does
/// **not** suppress is a phase-2 fact keyed on a node's presence, which the oracle
/// records before it chooses what to emit. Those facts run on two axes — what gets
/// *emitted*, and whether the component *validates* at all — and a construct on
/// either axis has to refuse here. Refusing more would turn correct output into
/// refusals for nothing; the per-construct membership argument, both axes, and the
/// known open holes live on [`dropped_presence_refusal`].
///
/// Recursion rides [`each_child_fragment`], the shared structural seam, so a new
/// child fragment on an existing node kind cannot hide a node from this walk.
fn guard_dropped_presence(fragment: &Fragment<'_>) -> Result<(), CompileError> {
    for node in fragment.nodes {
        if let Some(reason) = dropped_presence_refusal(node) {
            return Err(unsupported(reason));
        }
        let mut result = Ok(());
        each_child_fragment(node, &mut |child| {
            if result.is_ok() {
                result = guard_dropped_presence(child);
            }
        });
        result?;
    }
    Ok(())
}

/// The presence-read classification of one dropped-fragment node.
///
/// Exhaustive on purpose, three times over — a new [`FragmentNode`],
/// [`SpecialElementKind`], or [`AttributeNode`] variant fails compilation here
/// rather than silently joining the inert majority. ⚠️ The exhaustiveness forces
/// **two** questions on whoever adds a variant, not one:
///
/// 1. does the oracle read this node's presence into the **emitted output** (a
///    signature fact, a hoist decision, a scoping fact)?
/// 2. does its presence feed a whole-component **validation** — a phase-2 check
///    keyed on this node existing *anywhere*, which can turn a component the
///    oracle would otherwise accept into a compile error?
///
/// Either one makes the node presence-read. Answering only (1) is how the second
/// axis stayed invisible; see the open holes below.
///
/// Two constructs are presence-read today:
///
/// - **`<slot>`** (axis 1). Svelte's phase-2 `SlotElement` visitor records every
///   slot in `analysis.slot_names`, and phase 3 folds `slot_names.size > 0` into
///   `should_inject_props` — so a `<slot>` in a `{:catch}` widens the component
///   signature to `($$renderer, $$props)` while SSR emits nothing from the branch.
/// - **a legacy `on:` directive** (axis 2). `OnDirective`'s visitor sets
///   `event_directive_node` and an `onclick`-style attribute sets
///   `uses_event_attributes`; a component carrying both is
///   `mixed_event_handler_syntaxes`. The check is whole-component, so an `on:` in
///   a `{:catch}` flips a component with an emitted `onclick` from valid JS to a
///   compile error — the fenced construct demonstrably affecting the result from a
///   dropped position. `let:` refuses alongside it on the fence argument rather
///   than this one; see [`dropped_presence_attribute_refusal`].
///
/// Both refuse under the **emitted path's own bucket label** (`<slot>`'s
/// `TemplateNode`, the directives' [`Refusal::RunesOnlyFence`]), keeping one census
/// bucket per construct: the fence is firing in a second position, not for a new
/// reason.
///
/// Every other node is probe-verified inert **on the emission axis** — the oracle's
/// output is byte-identical with and without it, *in isolation*. That is a claim
/// about axis 1 only, and it is the whole claim: a construct inert on the emission
/// axis can still feed axis 2, in which case a sibling construct elsewhere in the
/// component makes it not inert at all. Inert-in-isolation is measured per
/// construct; the validations are whole-component, so they are only visible in
/// combination.
///
/// ⚠️ **One known open hole on axis 2**, pre-existing, not corpus-reachable, an
/// over-acceptance (tsv compiles what the oracle rejects): `{$$slots.x}` in a
/// dropped region + an emitted `{@render}` → `slot_snippet_conflict`.
///
/// Closing it means porting the oracle's whole-component validation rather than
/// widening this match, because `$$slots` is not fenced — separate work, tracked in
/// `docs/checklist_svelte_compiler.md`.
///
/// Its former sibling — a dropped `{#snippet}` plus `export { … }` of it from a
/// module script — is closed by `validate.rs`'s `validate_module_exports`. ⚠️ Note
/// the rule is narrower than that phrasing suggested: an exported snippet is an
/// error only when the oracle cannot HOIST it, which a dropped one never can be,
/// while a plain top-level `{#snippet s()}` beside `export { s }` compiles on both
/// sides.
///
/// The rest keep compiling in a dropped region: `<svelte:component>`,
/// `<svelte:self>` (in a nesting the oracle allows, e.g. under an `{#if}`),
/// `<svelte:fragment>` and a `slot="…"` component child (both as a component's
/// children), plus the unfenced `<svelte:element>` and `<svelte:boundary>`. The
/// placement-restricted metas (`<svelte:head>` / `<svelte:window>` /
/// `<svelte:body>` / `<svelte:document>`) the oracle rejects inside any block, so
/// no tsv verdict there is reachable at all.
///
/// Keeping `<svelte:boundary>` out of the refused set is deliberate and not
/// merely a parity accounting choice: it is a first-class Svelte 5 feature and the
/// next implementation target, so refusing it here would obstruct that work.
fn dropped_presence_refusal(node: &FragmentNode<'_>) -> Option<Refusal> {
    match node {
        FragmentNode::SpecialElement(special) => match &special.kind {
            SpecialElementKind::SlotElement => Some(Refusal::TemplateNode {
                kind: SPECIAL_ELEMENT_SLOT,
            }),
            SpecialElementKind::SvelteComponent { .. }
            | SpecialElementKind::SvelteSelf
            | SpecialElementKind::SvelteFragment
            | SpecialElementKind::SvelteBoundary
            | SpecialElementKind::SvelteElement { .. }
            | SpecialElementKind::SvelteHead
            | SpecialElementKind::TitleElement
            | SpecialElementKind::SvelteWindow
            | SpecialElementKind::SvelteBody
            | SpecialElementKind::SvelteDocument => {
                dropped_presence_attribute_refusal(special.attributes)
            }
        },
        // An element (regular or component) is itself inert, but its ATTRIBUTES are
        // not: a directive hangs off the attribute list rather than off the node
        // kind, so the presence walk has to reach into it.
        FragmentNode::Element(element) => dropped_presence_attribute_refusal(element.attributes),
        // Every remaining node kind is inert in a dropped region: what the oracle
        // reads out of one is its *expressions* (references, runes, TypeScript),
        // which the sibling expression walks already cover. Listed rather than
        // collapsed into a `_` so a new variant lands here as a compile error and
        // gets the presence-read judgment made for it explicitly.
        FragmentNode::Text(_)
        | FragmentNode::Comment(_)
        | FragmentNode::ExpressionTag(_)
        | FragmentNode::HtmlTag(_)
        | FragmentNode::RenderTag(_)
        | FragmentNode::DebugTag(_)
        | FragmentNode::ConstTag(_)
        | FragmentNode::DeclarationTag(_)
        | FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_)
        | FragmentNode::SnippetBlock(_) => None,
    }
}

/// The presence-read classification of a dropped-fragment node's attribute list.
///
/// The legacy `on:` and `let:` directives refuse here, under the same
/// [`Refusal::RunesOnlyFence`] bucket the emitted paths use (`element.rs`, and the
/// special-element path above). Their memberships rest on different arguments, and
/// the difference is worth keeping straight:
///
/// - **`on:`** is *forced* — it feeds a whole-component validation from a dropped
///   position (see [`dropped_presence_refusal`]'s axis 2). Not refusing it is an
///   over-acceptance.
/// - **`let:`** is *chosen*. Its only oracle check is the **local**
///   `let_directive_invalid_placement` (`2-analyze/visitors/LetDirective.js`), which
///   reads its parent and nothing else — it writes no whole-component analysis
///   field, so it is genuinely inert in a dropped region on both axes. It refuses
///   anyway because it is a permanent runes-only fence sharing `on:`'s bucket:
///   splitting the pair by position would cost a census bucket to buy parity on a
///   construct tsv will never support.
///
/// That asymmetry is the reason this is a *fence* list rather than a second
/// presence-read list. A construct that is neither forced nor fenced does not
/// belong here — closing an axis-2 hole for one (`$$slots`, `{#snippet}`) means
/// porting the validation, not adding an arm.
///
/// Everything else is inert on both axes. A `bind:` is the closest call and stays
/// out: the oracle's bind validations are all *local* to the binding (its target,
/// its element), so a dropped one cannot invalidate an emitted sibling — and the
/// expression walk beside this one already guards what it references.
fn dropped_presence_attribute_refusal(attributes: &[AttributeNode<'_>]) -> Option<Refusal> {
    attributes.iter().find_map(|attribute| match attribute {
        AttributeNode::OnDirective(_) => Some(Refusal::RunesOnlyFence { directive: "on:" }),
        AttributeNode::LetDirective(_) => Some(Refusal::RunesOnlyFence { directive: "let:" }),
        AttributeNode::Attribute(_)
        | AttributeNode::SpreadAttribute(_)
        | AttributeNode::AttachTag(_)
        | AttributeNode::BindDirective(_)
        | AttributeNode::ClassDirective(_)
        | AttributeNode::StyleDirective(_)
        | AttributeNode::UseDirective(_)
        | AttributeNode::TransitionDirective(_)
        | AttributeNode::AnimateDirective(_) => None,
    })
}
