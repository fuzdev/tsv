//! Whole-document validation — the oracle's rules that reject a component
//! *before* it decides what to emit.
//!
//! tsv's compiler implements the oracle's **emission**, not its **analysis**, so a
//! component Svelte rejects can still compile here — an over-acceptance, which the
//! refusal contract forbids. This module is the home for the rules that close that
//! gap and that are **independent of emission state**.
//!
//! # Why one upfront walk, and not a check at each emitter
//!
//! Every rule here fires wherever its construct sits, including a region SSR
//! **drops** — a `{:catch}` branch, an event handler, a `<svelte:boundary>`'s
//! discarded `pending` children. All three rules below are literally *parse-time*
//! in the oracle (`phases/1-parse/state/element.js`), so it raises them before it
//! has any notion of emission at all.
//!
//! Checking them at the emitters would therefore need the rule in **two** places —
//! the emitted path and `guard_dropped_presence` — which is exactly how
//! [`attr_refs`](crate::attr_refs)'s traversals drifted before. Instead this is a
//! single pass over the whole document, run at the top of `analyze()`, riding the
//! shared structural seam [`each_child_fragment`] so a new `FragmentNode` variant
//! reaches it by construction. Because the rules read only a node, its attribute
//! list, and its depth, one walk serves all of them.
//!
//! Rules whose inputs *are* emission state stay at their emitter, where that state
//! lives — e.g. the SSR-inert special elements' children / illegal-attribute /
//! invalid-bind guards in `fragment.rs`. Their *placement* and *duplicate* rules
//! used to live there too and were moved here for exactly the reason above.

use crate::CompileError;
use crate::attr_refs::each_child_fragment;
use crate::refusal::Refusal;
use crate::transform_server::unsupported;
use tsv_svelte::ast::internal::{AttributeNode, Fragment, FragmentNode, Root, SpecialElementKind};

/// The oracle's `root_only_meta_tags` (`phases/1-parse/state/element.js:45`) —
/// the meta tags legal only as a direct child of the component root, and legal at
/// most once per component.
///
/// ⚠️ Both rules were previously enforced for the SSR-inert three
/// (`svelte:window`/`svelte:body`/`svelte:document`) at their **emitter** in
/// `fragment.rs`. That site never runs on a region SSR drops, so one of these in a
/// `{:catch}` compiled — a live over-acceptance the fuzzer found. The rule lives
/// here now, and only here; `fragment.rs` keeps the checks whose inputs really are
/// emission state (children, illegal attributes, invalid binds).
///
/// `svelte:options` is in the oracle's map too and is covered upstream by
/// `analyze()`'s unconditional [`Refusal::SvelteOptions`].
fn root_only_meta_tag(kind: &SpecialElementKind<'_>) -> Option<&'static str> {
    match kind {
        SpecialElementKind::SvelteHead => Some("svelte:head"),
        SpecialElementKind::SvelteWindow => Some("svelte:window"),
        SpecialElementKind::SvelteBody => Some("svelte:body"),
        SpecialElementKind::SvelteDocument => Some("svelte:document"),
        _ => None,
    }
}

/// Run every emission-independent validation rule over the whole document.
///
/// Errors with the first refusal found, in document order.
pub(crate) fn validate_document(root: &Root<'_>, source: &str) -> Result<(), CompileError> {
    let mut seen_meta: Vec<&'static str> = Vec::new();
    walk_fragment(&root.fragment, source, true, &mut seen_meta)
}

fn walk_fragment(
    fragment: &Fragment<'_>,
    source: &str,
    at_root: bool,
    seen_meta: &mut Vec<&'static str>,
) -> Result<(), CompileError> {
    for node in fragment.nodes {
        walk_node(node, source, at_root, seen_meta)?;
    }
    Ok(())
}

fn walk_node(
    node: &FragmentNode<'_>,
    source: &str,
    at_root: bool,
    seen_meta: &mut Vec<&'static str>,
) -> Result<(), CompileError> {
    match node {
        FragmentNode::Element(element) => refuse_duplicate_attributes(element.attributes, source)?,
        FragmentNode::SpecialElement(special) => {
            if let Some(tag) = root_only_meta_tag(&special.kind) {
                // The oracle raises placement BEFORE duplicate at the same site,
                // and does not record a mis-placed tag in its `meta_tags` dict
                // (`element.js:155-164`) — so a nested one refuses on placement and
                // never contributes to the duplicate set.
                if !at_root {
                    return Err(unsupported(Refusal::SpecialElementInvalidPlacement {
                        name: tag.to_string(),
                    }));
                }
                if seen_meta.contains(&tag) {
                    return Err(unsupported(Refusal::DuplicateSpecialElement {
                        name: tag.to_string(),
                    }));
                }
                seen_meta.push(tag);
            }
            refuse_duplicate_attributes(special.attributes, source)?;
        }
        _ => {}
    }
    // Every child fragment is below the root, so `at_root` is false from here down
    // — matching the oracle's `parent.type !== 'Root'`, which is a *direct*-child
    // test: a block or an element between the root and the tag makes it invalid.
    let mut result = Ok(());
    each_child_fragment(node, &mut |child| {
        if result.is_ok() {
            result = walk_fragment(child, source, false, seen_meta);
        }
    });
    result
}

/// The oracle's `attribute_duplicate` (`phases/1-parse/state/element.js:238-256`).
///
/// A parse-time rule over ONE element's attribute list, so it is local and needs no
/// context beyond the list itself. Three details are load-bearing and each is the
/// oracle's, not a simplification:
///
/// - only `Attribute` / `BindDirective` / `StyleDirective` / `ClassDirective`
///   participate. `use:` / `transition:` / `in:` / `out:` / `animate:` / `on:` /
///   `let:` / a spread / `{@attach}` are all deliberately exempt (the oracle's own
///   comment explains why: they either cannot repeat or have their own error);
/// - the key is the attribute KIND joined to the name, with `BindDirective`
///   normalized to `Attribute` — so `bind:value` collides with `value`, while
///   `class:x` and `x` are different keys and legally co-exist;
/// - the name `this` is never *recorded* (though a second one still collides with a
///   recorded key), which is what makes `<svelte:element bind:this this={…}>` legal.
fn refuse_duplicate_attributes(
    attributes: &[AttributeNode<'_>],
    source: &str,
) -> Result<(), CompileError> {
    // Elements carry a handful of attributes, so a linear scan beats a set.
    let mut unique_names: Vec<(u8, &str)> = Vec::new();
    for attribute in attributes {
        // The discriminant the oracle concatenates, with `BindDirective`
        // normalized onto `Attribute`.
        let (kind, name) = match attribute {
            AttributeNode::Attribute(a) => (0u8, a.name_span.extract(source)),
            AttributeNode::BindDirective(d) => (0u8, d.name_span.extract(source)),
            AttributeNode::StyleDirective(d) => (1u8, d.name_span.extract(source)),
            AttributeNode::ClassDirective(d) => (2u8, d.name_span.extract(source)),
            _ => continue,
        };
        if unique_names.contains(&(kind, name)) {
            return Err(unsupported(Refusal::DuplicateAttribute {
                name: name.to_string(),
            }));
        } else if name != "this" {
            unique_names.push((kind, name));
        }
    }
    Ok(())
}
