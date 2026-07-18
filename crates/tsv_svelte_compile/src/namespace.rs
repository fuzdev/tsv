//! Namespace inference for SSR whitespace normalization.
//!
//! Port of Svelte's `infer_namespace` / `check_nodes_for_namespace` /
//! `determine_namespace_for_children` (`3-transform/utils.js`). A fragment's
//! namespace decides whether collapsed inter-node whitespace is *removed*
//! entirely (`svg`) or kept as a single space (`html`/`mathml`) — the svg case
//! of the oracle's `clean_nodes` `can_remove_entirely`, alongside the
//! `select`/`table`-family parents. See [`crate::fragment::normalize_whitespace`].
//!
//! `metadata.svg`/`metadata.mathml` are approximated by NAME
//! ([`element_is_svg`] / [`element_is_mathml`]). The oracle's ancestor-inheriting
//! `<a>`/`<title>` rule IS ported ([`element_is_svg`], keyed on the threaded
//! inherited namespace); the one unported corner is a `<a>`/`<title>` directly
//! inside `<foreignObject>` (Svelte reads the ancestor's `metadata.svg`, svg,
//! where `inherited` is html). A `<svelte:element>`'s xmlns/ancestor namespace
//! resolution is likewise approximated (its runtime tag is unknown).

use tsv_lang::InfallibleResolve;
use tsv_svelte::ast::internal::{
    AwaitBlock, Element, ElementKind, FragmentNode, IfBlock, SpecialElementKind,
};

use crate::transform_server::EmitEnv;

/// The SSR namespace of a fragment (Svelte's `Namespace`). Only [`Namespace::Svg`]
/// changes whitespace handling (collapsed inter-node runs are removed, not
/// collapsed to a space); [`Namespace::Mathml`] behaves like [`Namespace::Html`]
/// for whitespace but is tracked for faithfulness with the oracle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Namespace {
    Html,
    Svg,
    Mathml,
}

/// Which kind of parent a fragment hangs off, selecting the namespace-inference
/// path (Svelte keys this on `parent.type` inside `infer_namespace`).
#[derive(Clone, Copy)]
pub(crate) enum FragmentParent {
    /// Root / Component / `{#snippet}` / `<svelte:fragment>` — Svelte's special
    /// list: re-infer with the deep [`check_nodes_for_namespace`] walk, then the
    /// shallow direct-child loop.
    Special,
    /// A control-flow block (`{#if}`/`{#each}`/`{#await}`/`{#key}`) or a
    /// non-element hoisted parent (`<svelte:head>`): the parent is not in Svelte's
    /// special list, so only the shallow direct-child loop runs.
    Block,
    /// A `<svelte:element>`: Svelte reads its `metadata.svg`, which for a dynamic
    /// tag inherits from the ancestor namespace — approximated here by keeping the
    /// inherited namespace, since the runtime tag name is unknown at compile time.
    DynamicElement,
}

/// The whitespace/namespace context a caller hands [`crate::fragment::emit_child_body`]
/// so it can compute the child fragment's namespace via [`infer_namespace`].
#[derive(Clone, Copy)]
pub(crate) struct ChildNamespace {
    /// The parent (enclosing) fragment's namespace — the inference fallback.
    pub(crate) inherited: Namespace,
    /// Which inference path to take (see [`FragmentParent`]).
    pub(crate) parent: FragmentParent,
    /// Whether the immediate parent or any ancestor is an svg `<text>` element —
    /// the exception that keeps whitespace even under [`Namespace::Svg`].
    pub(crate) in_svg_text: bool,
}

/// Port of Svelte's `is_svg_element` (`2-analyze/visitors/RegularElement.js`):
/// base-list membership, OR `<a>`/`<title>` when the nearest RegularElement
/// ancestor is svg — approximated by `inherited` (the namespace the element sits
/// in, i.e. its parent context's namespace), which the compiler already threads.
/// Intercepting `a`/`title` before the base list also sidesteps `tsv_html`'s stale
/// `"title"` `SVG_ELEMENTS` entry (Svelte's base list omits it, so a bare `<title>`
/// at html scope must NOT be svg).
///
/// Non-circular: `<a>`/`<title>`'s svg-ness keys on the INHERITED namespace (the
/// fragment's parent), never on the fragment being classified. A `<title>`/`<a>`
/// directly inside `<foreignObject>` is the one unported corner — Svelte reads the
/// ancestor's `metadata.svg` (svg for foreignObject) while `inherited` is html
/// there; rare enough to leave as a caveat.
pub(crate) fn element_is_svg(name: &str, inherited: Namespace) -> bool {
    if name == "a" || name == "title" {
        inherited == Namespace::Svg
    } else {
        tsv_html::is_svg_element(name)
    }
}

/// Port of Svelte's `is_mathml`: pure base-list membership (no ancestor case — the
/// mathml set matches the oracle). A thin wrapper so every classification site
/// routes through this module and the two can never drift.
pub(crate) fn element_is_mathml(name: &str) -> bool {
    tsv_html::is_mathml_element(name)
}

/// `(is_svg, is_mathml)` for a regular element in a fragment whose inherited
/// namespace is `inherited` (the ancestor signal for the `<a>`/`<title>` rule).
fn element_flags(env: &EmitEnv<'_, '_>, el: &Element<'_>, inherited: Namespace) -> (bool, bool) {
    let interner = env.b.interner.borrow();
    let name = interner.resolve_infallible(el.name);
    (element_is_svg(name, inherited), element_is_mathml(name))
}

/// Port of Svelte's `determine_namespace_for_children`: the namespace an element's
/// own children fragment is in, from the element's tag name (`foreignObject`
/// resets to html even though it is an svg element). `inherited` is the namespace
/// the element itself sits in — the ancestor signal for a `<a>`/`<title>` element.
pub(crate) fn determine_namespace_for_children(name: &str, inherited: Namespace) -> Namespace {
    if name == "foreignObject" {
        Namespace::Html
    } else if element_is_svg(name, inherited) {
        Namespace::Svg
    } else if element_is_mathml(name) {
        Namespace::Mathml
    } else {
        Namespace::Html
    }
}

/// Port of Svelte's `infer_namespace`: the namespace of a fragment whose parent is
/// a root/block/component (an element's children use
/// [`determine_namespace_for_children`] directly).
pub(crate) fn infer_namespace(
    env: &EmitEnv<'_, '_>,
    inherited: Namespace,
    parent: FragmentParent,
    nodes: &[FragmentNode<'_>],
) -> Namespace {
    match parent {
        FragmentParent::DynamicElement => inherited,
        FragmentParent::Special => {
            // The deep walk. A definite verdict (svg/mathml/html) wins; `keep`
            // (no elements/text) and `maybe_html` (only text) fall through to the
            // shallow direct-child loop — matching the oracle's re-evaluation for
            // the fragment/root/component/snippet parents.
            let mut acc = NsAccum::Keep;
            check_nodes_for_namespace(env, nodes, &mut acc, inherited);
            match acc {
                NsAccum::Svg => Namespace::Svg,
                NsAccum::Mathml => Namespace::Mathml,
                NsAccum::Html => Namespace::Html,
                NsAccum::Keep | NsAccum::MaybeHtml => {
                    shallow_child_namespace(env, nodes, inherited)
                }
            }
        }
        // A block parent is not in Svelte's special list, so only the shallow loop
        // runs (the deep `check_nodes_for_namespace` walk is skipped).
        FragmentParent::Block => shallow_child_namespace(env, nodes, inherited),
    }
}

/// The accumulator state of the deep [`check_nodes_for_namespace`] walk (Svelte's
/// `'keep' | 'maybe_html' | Namespace`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum NsAccum {
    Keep,
    MaybeHtml,
    Svg,
    Mathml,
    Html,
}

/// Port of Svelte's `check_nodes_for_namespace`: descend through blocks/fragments
/// (but NOT into an element's own children, and NOT into components/snippets),
/// classifying the first elements found. Returns `true` when the walk should stop
/// (a non-foreign element forced `html`). `inherited` is the classified fragment's
/// inherited namespace — the `<a>`/`<title>` ancestor signal, constant across the
/// walk because every element found here has that fragment's parent as its nearest
/// RegularElement ancestor (blocks don't count).
fn check_nodes_for_namespace(
    env: &EmitEnv<'_, '_>,
    nodes: &[FragmentNode<'_>],
    acc: &mut NsAccum,
    inherited: Namespace,
) -> bool {
    for node in nodes {
        if check_node(env, node, acc, inherited) {
            return true;
        }
    }
    false
}

/// One node of the deep walk. See [`check_nodes_for_namespace`]. `true` = stop.
fn check_node(
    env: &EmitEnv<'_, '_>,
    node: &FragmentNode<'_>,
    acc: &mut NsAccum,
    inherited: Namespace,
) -> bool {
    match node {
        // A RegularElement is a LEAF here — classified by name, not recursed into
        // (the oracle's zimmerframe `RegularElement` visitor never calls `next`).
        // A Component (uppercase/dotted name) is skipped entirely.
        FragmentNode::Element(el) => {
            if el.kind == ElementKind::Component {
                return false;
            }
            let (svg, mathml) = element_flags(env, el, inherited);
            if !svg && !mathml {
                *acc = NsAccum::Html;
                return true;
            } else if *acc == NsAccum::Keep {
                *acc = if svg { NsAccum::Svg } else { NsAccum::Mathml };
            }
            false
        }
        // A `<svelte:element>` shares the oracle's `RegularElement` visitor. Its
        // name (`svelte:element`) is neither svg nor mathml, so it forces `html`
        // and stops; every other special element is skipped.
        FragmentNode::SpecialElement(se) => {
            if matches!(se.kind, SpecialElementKind::SvelteElement { .. }) {
                *acc = NsAccum::Html;
                return true;
            }
            false
        }
        FragmentNode::Text(t) => {
            if !t.data(env.source).trim().is_empty() {
                *acc = NsAccum::MaybeHtml;
            }
            false
        }
        FragmentNode::IfBlock(b) => check_if_block(env, b, acc, inherited),
        FragmentNode::EachBlock(b) => {
            check_nodes_for_namespace(env, b.body.nodes, acc, inherited)
                || b.fallback
                    .as_ref()
                    .is_some_and(|f| check_nodes_for_namespace(env, f.nodes, acc, inherited))
        }
        FragmentNode::AwaitBlock(b) => check_await_block(env, b, acc, inherited),
        FragmentNode::KeyBlock(b) => {
            check_nodes_for_namespace(env, b.fragment.nodes, acc, inherited)
        }
        // ExpressionTag / HtmlTag / ConstTag / Comment / RenderTag / SnippetBlock /
        // DebugTag / DeclarationTag: not descended (no elements to find).
        _ => false,
    }
}

/// The `{#if}` else-if chain is `alternate` fragments nesting further `IfBlock`s,
/// so descending consequent + alternate recursively covers every branch.
fn check_if_block(
    env: &EmitEnv<'_, '_>,
    b: &IfBlock<'_>,
    acc: &mut NsAccum,
    inherited: Namespace,
) -> bool {
    if check_nodes_for_namespace(env, b.consequent.nodes, acc, inherited) {
        return true;
    }
    if let Some(alt) = &b.alternate
        && check_nodes_for_namespace(env, alt.nodes, acc, inherited)
    {
        return true;
    }
    false
}

/// Descend every `{#await}` phase fragment (pending/then/catch) — the oracle's
/// zimmerframe walk visits all three, even though SSR later drops `{:catch}`.
fn check_await_block(
    env: &EmitEnv<'_, '_>,
    b: &AwaitBlock<'_>,
    acc: &mut NsAccum,
    inherited: Namespace,
) -> bool {
    for f in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
        if check_nodes_for_namespace(env, f.nodes, acc, inherited) {
            return true;
        }
    }
    false
}

/// Port of Svelte's `infer_namespace` shallow tail: iterate the DIRECT
/// RegularElement children only (not components, special elements, or blocks). All
/// svg → svg, all mathml → mathml, any plain-html or a mix → html; no elements →
/// the inherited namespace.
fn shallow_child_namespace(
    env: &EmitEnv<'_, '_>,
    nodes: &[FragmentNode<'_>],
    inherited: Namespace,
) -> Namespace {
    let mut new_ns: Option<Namespace> = None;
    for node in nodes {
        let FragmentNode::Element(el) = node else {
            continue;
        };
        if el.kind != ElementKind::Html {
            continue;
        }
        let (svg, mathml) = element_flags(env, el, inherited);
        if mathml {
            new_ns = Some(if matches!(new_ns, None | Some(Namespace::Mathml)) {
                Namespace::Mathml
            } else {
                Namespace::Html
            });
        } else if svg {
            new_ns = Some(if matches!(new_ns, None | Some(Namespace::Svg)) {
                Namespace::Svg
            } else {
                Namespace::Html
            });
        } else {
            return Namespace::Html;
        }
    }
    new_ns.unwrap_or(inherited)
}
