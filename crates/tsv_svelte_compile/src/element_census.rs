//! The upfront element census — the substrate the CSS combinator matcher walks.
//!
//! tsv's Svelte AST has no upward navigability: an [`Element`] has no parent,
//! path, or sibling links. The oracle's CSS matcher
//! (`phases/2-analyze/css/css-prune.js`) rides on `element.metadata.path` — the
//! ancestor-node chain. This module builds that chain.
//!
//! One top-down walk over `root.fragment` produces a [`CensusElement`] per
//! **regular** HTML element (components are excluded, matching the oracle's
//! element list, which only pushes `RegularElement`/`SvelteElement`). Each census
//! element carries its [`path`](CensusElement::path): the ancestor chain snapshot
//! (references, never clones), from which [`get_ancestor_elements`] and
//! [`get_possible_element_siblings`] recover the ancestor/sibling context the
//! combinator matcher needs — direct ports of the oracle's `get_ancestor_elements`
//! / `get_possible_element_siblings` / `get_possible_nested_siblings` / `loop_child`.
//!
//! # What the walk descends into
//!
//! Every fragment that reaches SSR output: element/component subtrees, `{#if}`
//! branches, `{#each}` body + fallback, `{#await}` pending + then, `{#key}`,
//! `{#snippet}` bodies, and `<svelte:head>` content. It deliberately **excludes
//! `{:catch}`** — tsv drops that branch from output (the oracle's SSR `$.await`
//! has no catch arm either), so a catch element is never emitted and never an
//! `element_scope` candidate. Keeping the census leaf set equal to the emitted set
//! makes the single-selector match byte-identical to the emission-fused slice-3
//! result. (The sibling nested-descent still visits `{:catch}` for possible
//! siblings — those elements are tested but never leaves.)
//!
//! # Boundaries the matcher refuses
//!
//! The census holds the whole in-component tree, so ancestor/sibling resolution
//! falls out of it directly. Two things it can **not** resolve — a `{#snippet}`
//! crossed via a `{@render}` site, and a `{@render}`/component sibling's slotted
//! content — need the `metadata.sites` / `metadata.snippets` products tsv does not
//! build. The walks surface a snippet crossing as an `Err(())` (the matcher turns
//! it into a refusal); render/component/special siblings are treated as opaque
//! (contribute no possible sibling), a safe under-approximation.

use std::collections::HashMap;

use tsv_lang::Span;
use tsv_svelte::ast::internal::{
    AttributeNode, EachBlock, Element, ElementKind, Fragment, FragmentNode, Root, SpecialElement,
    SpecialElementKind,
};

/// One regular HTML element plus the ancestor chain that gives it upward
/// navigability (the oracle's `element.metadata.path`).
pub(crate) struct CensusElement<'a> {
    pub(crate) node: &'a Element<'a>,
    /// Ancestor frames, outermost (root) first, innermost (direct parent) last.
    pub(crate) path: Vec<PathFrame<'a>>,
}

/// One nesting level of an element's path: the sibling fragment it lives in, its
/// index within that fragment, and the node that owns the fragment. Mirrors the
/// oracle's alternating `[owner, fragment]` path, but bundled so a sibling scan
/// reads the index directly instead of an `indexOf`.
#[derive(Clone, Copy)]
pub(crate) struct PathFrame<'a> {
    /// The sibling node list at this level (the arena slice, not a borrow of the
    /// owning `Fragment` — the root fragment is stored by value on a local `Root`,
    /// so only its `'a` `nodes` slice can flow into the census).
    nodes: &'a [FragmentNode<'a>],
    index: usize,
    owner: Owner<'a>,
}

/// The node that owns a [`PathFrame`]'s fragment — what an upward walk inspects to
/// decide whether to keep climbing (a transparent block/component) or stop (an
/// element boundary), and whether it must refuse (a snippet crossing).
///
/// Only the discriminant matters for most kinds (a transparent block keeps the walk
/// climbing; an element stops it; a snippet refuses); the two that carry data are
/// [`Owner::Element`] (an ancestor element) and [`Owner::Each`] (the wrap-around
/// needs the block).
#[derive(Clone, Copy)]
enum Owner<'a> {
    /// The component root fragment.
    Root,
    /// A regular HTML element (an ancestor element, and a sibling-walk boundary).
    Element(&'a Element<'a>),
    /// A component — transparent for ancestor purposes (not an ancestor element),
    /// and its slot content flows into the parent for siblings.
    Component,
    /// A `{#snippet}` body — the oracle would cross to the snippet's render sites,
    /// which tsv does not resolve. The matcher refuses.
    Snippet,
    /// `<svelte:head>` content — an element-like boundary (not a block).
    Head,
    If,
    /// `is_body` distinguishes the each **body** (the each-self-adjacency wrap-around
    /// applies) from the fallback.
    Each {
        block: &'a EachBlock<'a>,
        is_body: bool,
    },
    Await,
    Key,
}

/// The census: every regular element with its path, plus a span→index map so the
/// sibling matcher can recover a deeply-nested possible sibling's own path.
/// (`Span` is not `Hash`, so the map is keyed by its `(start, end)` pair.)
pub(crate) struct ElementCensus<'a> {
    pub(crate) elements: Vec<CensusElement<'a>>,
    by_span: HashMap<(u32, u32), usize>,
}

fn span_key(span: Span) -> (u32, u32) {
    (span.start, span.end)
}

impl<'a> ElementCensus<'a> {
    /// The path of the census element with span `span`, or `None` when `span` is
    /// not a census leaf (a component, a `{:catch}` element, …).
    fn path_of<'s>(&'s self, span: Span) -> Option<&'s [PathFrame<'a>]> {
        self.by_span
            .get(&span_key(span))
            .map(|&i| &self.elements[i].path[..])
    }
}

/// Build the census by one top-down walk over the component fragment.
pub(crate) fn build_census<'a>(root: &Root<'a>) -> ElementCensus<'a> {
    let mut elements = Vec::new();
    let mut path = Vec::new();
    walk_fragment(root.fragment.nodes, Owner::Root, &mut path, &mut elements);
    let by_span = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (span_key(e.node.span), i))
        .collect();
    ElementCensus { elements, by_span }
}

/// Walk one fragment, recording each regular element and recursing into every
/// SSR-reachable sub-fragment. `owner` owns `fragment`; `path` is the frame stack
/// for `fragment`'s owner (its ancestors), extended in place per child.
fn walk_fragment<'a>(
    nodes: &'a [FragmentNode<'a>],
    owner: Owner<'a>,
    path: &mut Vec<PathFrame<'a>>,
    elements: &mut Vec<CensusElement<'a>>,
) {
    for (index, node) in nodes.iter().enumerate() {
        let frame = PathFrame {
            nodes,
            index,
            owner,
        };
        match node {
            FragmentNode::Element(element) => {
                path.push(frame);
                if element.kind == ElementKind::Component {
                    walk_fragment(element.fragment.nodes, Owner::Component, path, elements);
                } else {
                    elements.push(CensusElement {
                        node: element,
                        path: path.clone(),
                    });
                    walk_fragment(
                        element.fragment.nodes,
                        Owner::Element(element),
                        path,
                        elements,
                    );
                }
                path.pop();
            }
            FragmentNode::IfBlock(block) => {
                path.push(frame);
                walk_fragment(block.consequent.nodes, Owner::If, path, elements);
                if let Some(alternate) = &block.alternate {
                    walk_fragment(alternate.nodes, Owner::If, path, elements);
                }
                path.pop();
            }
            FragmentNode::EachBlock(block) => {
                path.push(frame);
                walk_fragment(
                    block.body.nodes,
                    Owner::Each {
                        block,
                        is_body: true,
                    },
                    path,
                    elements,
                );
                if let Some(fallback) = &block.fallback {
                    walk_fragment(
                        fallback.nodes,
                        Owner::Each {
                            block,
                            is_body: false,
                        },
                        path,
                        elements,
                    );
                }
                path.pop();
            }
            FragmentNode::AwaitBlock(block) => {
                path.push(frame);
                // Pending + then reach SSR output; `{:catch}` is dropped (see the
                // module docs), so it is not a census leaf.
                if let Some(pending) = &block.pending {
                    walk_fragment(pending.nodes, Owner::Await, path, elements);
                }
                if let Some(then) = &block.then {
                    walk_fragment(then.nodes, Owner::Await, path, elements);
                }
                path.pop();
            }
            FragmentNode::KeyBlock(block) => {
                path.push(frame);
                walk_fragment(block.fragment.nodes, Owner::Key, path, elements);
                path.pop();
            }
            FragmentNode::SnippetBlock(block) => {
                path.push(frame);
                walk_fragment(block.body.nodes, Owner::Snippet, path, elements);
                path.pop();
            }
            FragmentNode::SpecialElement(special) if is_svelte_head(special) => {
                path.push(frame);
                walk_fragment(special.fragment.nodes, Owner::Head, path, elements);
                path.pop();
            }
            // Other special elements refuse the compile elsewhere; not descended.
            // Text / expression / comment / tag nodes hold no elements.
            _ => {}
        }
    }
}

fn is_svelte_head(special: &SpecialElement<'_>) -> bool {
    matches!(special.kind, SpecialElementKind::SvelteHead)
}

/// An ancestor element and the length of its own path (a prefix of the descendant's).
pub(crate) struct AncestorRef<'a> {
    pub(crate) node: &'a Element<'a>,
    /// The ancestor's path is `descendant_path[..path_len]`.
    pub(crate) path_len: usize,
}

/// The ancestor elements of the element whose path is `path` (the oracle's
/// `get_ancestor_elements`). Innermost first. With `adjacent_only`, only the direct
/// parent element (for `>`). Returns `Err(())` when a `{#snippet}` boundary is
/// crossed — the oracle would resolve the snippet's render sites, which tsv does
/// not build, so the matcher refuses.
pub(crate) fn get_ancestor_elements<'a>(
    path: &[PathFrame<'a>],
    adjacent_only: bool,
) -> Result<Vec<AncestorRef<'a>>, ()> {
    let mut ancestors = Vec::new();
    for idx in (0..path.len()).rev() {
        match path[idx].owner {
            Owner::Snippet => return Err(()),
            Owner::Element(node) => {
                ancestors.push(AncestorRef {
                    node,
                    path_len: idx,
                });
                if adjacent_only {
                    break;
                }
            }
            // Component / Head / Root / If / Each / Await / Key are not ancestor
            // elements; keep climbing.
            _ => {}
        }
    }
    Ok(ancestors)
}

/// Whether the element with this path has any element parent (the oracle's
/// `get_element_parent(node) === null` test, used by the sibling `every_is_global`
/// fallback).
pub(crate) fn has_element_parent(path: &[PathFrame<'_>]) -> bool {
    path.iter()
        .rev()
        .any(|f| matches!(f.owner, Owner::Element(_)))
}

/// Existence certainty of a possible sibling (the oracle's
/// `NODE_PROBABLY_EXISTS` / `NODE_DEFINITELY_EXISTS`). Only the definiteness drives
/// the `adjacent_only` early stop; for matching, every possible sibling is tested.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Existence {
    Probably,
    Definitely,
}

/// One possible sibling element and its own path (borrowed from the census `'c`,
/// for further combinator resolution) — `None` for a `{:catch}` / slotted element,
/// which is still testable as a leaf but not navigable further.
#[derive(Clone, Copy)]
pub(crate) struct SiblingRef<'a, 'c> {
    pub(crate) node: &'a Element<'a>,
    pub(crate) path: Option<&'c [PathFrame<'a>]>,
}

/// The possible **preceding** element siblings of the element whose path is `path`
/// (the oracle's `get_possible_element_siblings`, BACKWARD direction — the only one
/// in scope). With `adjacent_only` (`+`), stops at the first definite element.
/// Descends preceding blocks for their possible last children and applies the
/// `{#each}` self-adjacency wrap-around. Returns `Err(())` on a `{#snippet}`
/// crossing.
pub(crate) fn get_possible_element_siblings<'a, 'c>(
    census: &'c ElementCensus<'a>,
    path: &[PathFrame<'a>],
    adjacent_only: bool,
    source: &str,
) -> Result<Vec<SiblingRef<'a, 'c>>, ()> {
    let mut result: Vec<(&'a Element<'a>, Existence)> = Vec::new();
    for level in (0..path.len()).rev() {
        let frame = &path[level];
        let nodes = frame.nodes;
        // Scan preceding siblings (BACKWARD): indices `frame.index - 1` down to 0.
        let mut j = frame.index;
        while j > 0 {
            j -= 1;
            if scan_sibling(&nodes[j], adjacent_only, source, &mut result)? {
                return Ok(finish(census, result));
            }
        }
        match frame.owner {
            // Transparent: keep climbing to the component/block's own siblings.
            Owner::Component | Owner::If | Owner::Await | Owner::Key => {}
            Owner::Each { block, is_body } => {
                if is_body {
                    // `{#each}<a /><b />{/each}` — `<b>` can be a runtime preceding
                    // sibling of `<a />` (the wrap-around). Add the each body's own
                    // possible last children.
                    let mut wrap = Vec::new();
                    possible_nested_siblings_each(block, adjacent_only, source, &mut wrap)?;
                    add_all(&mut result, &wrap);
                }
            }
            Owner::Snippet => return Err(()),
            // An element / head parent, or the component root, is a sibling-walk
            // boundary.
            Owner::Element(_) | Owner::Head | Owner::Root => break,
        }
    }
    Ok(finish(census, result))
}

fn finish<'a, 'c>(
    census: &'c ElementCensus<'a>,
    result: Vec<(&'a Element<'a>, Existence)>,
) -> Vec<SiblingRef<'a, 'c>> {
    result
        .into_iter()
        .map(|(node, _)| SiblingRef {
            node,
            path: census.path_of(node.span),
        })
        .collect()
}

/// Classify one scanned sibling node, adding possible elements to `result`.
/// Returns `Ok(true)` to stop the whole walk (an `adjacent_only` definite hit).
fn scan_sibling<'a>(
    sib: &'a FragmentNode<'a>,
    adjacent_only: bool,
    source: &str,
    result: &mut Vec<(&'a Element<'a>, Existence)>,
) -> Result<bool, ()> {
    match sib {
        FragmentNode::Element(element) if element.kind != ElementKind::Component => {
            // A slotted element (`slot="…"`) is placed elsewhere, not a flow sibling.
            if !has_slot_attribute(element, source) {
                add_one(result, element, Existence::Definitely);
                if adjacent_only {
                    return Ok(true);
                }
            }
        }
        FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_) => {
            let mut nested = Vec::new();
            possible_nested_siblings(sib, adjacent_only, source, &mut nested)?;
            let has_definite = nested.iter().any(|&(_, e)| e == Existence::Definitely);
            add_all(result, &nested);
            if adjacent_only && has_definite {
                return Ok(true);
            }
        }
        // A component / render tag / special element / snippet block is opaque: its
        // resolved content needs products tsv does not build, so it contributes no
        // possible sibling (a safe under-approximation — never a false match). A
        // text / expression node is not an element either.
        _ => {}
    }
    Ok(false)
}

/// The possible last children of a block reached as a sibling (the oracle's
/// `get_possible_nested_siblings` for `IfBlock`/`EachBlock`/`AwaitBlock`/`KeyBlock`).
fn possible_nested_siblings<'a>(
    node: &'a FragmentNode<'a>,
    adjacent_only: bool,
    source: &str,
    out: &mut Vec<(&'a Element<'a>, Existence)>,
) -> Result<(), ()> {
    match node {
        FragmentNode::IfBlock(block) => nested_over_fragments(
            &[Some(&block.consequent), block.alternate.as_ref()],
            adjacent_only,
            source,
            out,
        ),
        FragmentNode::EachBlock(block) => {
            possible_nested_siblings_each(block, adjacent_only, source, out)
        }
        FragmentNode::AwaitBlock(block) => nested_over_fragments(
            &[
                block.pending.as_ref(),
                block.then.as_ref(),
                block.catch.as_ref(),
            ],
            adjacent_only,
            source,
            out,
        ),
        FragmentNode::KeyBlock(block) => {
            nested_over_fragments(&[Some(&block.fragment)], adjacent_only, source, out)
        }
        _ => Ok(()),
    }
}

fn possible_nested_siblings_each<'a>(
    block: &'a EachBlock<'a>,
    adjacent_only: bool,
    source: &str,
    out: &mut Vec<(&'a Element<'a>, Existence)>,
) -> Result<(), ()> {
    nested_over_fragments(
        &[Some(&block.body), block.fallback.as_ref()],
        adjacent_only,
        source,
        out,
    )
}

/// The shared core of `get_possible_nested_siblings`: `loop_child` each branch,
/// then demote every result to `Probably` unless every present branch yielded a
/// definite element (the oracle's `exhaustive` flag).
fn nested_over_fragments<'a>(
    fragments: &[Option<&'a Fragment<'a>>],
    adjacent_only: bool,
    source: &str,
    out: &mut Vec<(&'a Element<'a>, Existence)>,
) -> Result<(), ()> {
    let mut result: Vec<(&'a Element<'a>, Existence)> = Vec::new();
    let mut exhaustive = true;
    for fragment in fragments {
        match fragment {
            None => exhaustive = false,
            Some(fragment) => {
                let mut map = Vec::new();
                loop_child(fragment.nodes, adjacent_only, source, &mut map)?;
                exhaustive &= map.iter().any(|&(_, e)| e == Existence::Definitely);
                add_all(&mut result, &map);
            }
        }
    }
    if !exhaustive {
        for entry in &mut result {
            entry.1 = Existence::Probably;
        }
    }
    add_all(out, &result);
    Ok(())
}

/// The oracle's `loop_child`: walk a fragment's children from the end (BACKWARD),
/// collecting possible last elements, descending nested blocks.
fn loop_child<'a>(
    children: &'a [FragmentNode<'a>],
    adjacent_only: bool,
    source: &str,
    result: &mut Vec<(&'a Element<'a>, Existence)>,
) -> Result<(), ()> {
    let _ = source;
    for child in children.iter().rev() {
        match child {
            FragmentNode::Element(element) if element.kind != ElementKind::Component => {
                add_one(result, element, Existence::Definitely);
                if adjacent_only {
                    break;
                }
            }
            FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_) => {
                let mut nested = Vec::new();
                possible_nested_siblings(child, adjacent_only, source, &mut nested)?;
                let has_definite = nested.iter().any(|&(_, e)| e == Existence::Definitely);
                add_all(result, &nested);
                if adjacent_only && has_definite {
                    break;
                }
            }
            // Component / render / special / snippet children are opaque here too.
            _ => {}
        }
    }
    Ok(())
}

/// Add one possible sibling, keeping the higher existence on a repeat span.
fn add_one<'a>(
    result: &mut Vec<(&'a Element<'a>, Existence)>,
    node: &'a Element<'a>,
    exist: Existence,
) {
    if let Some(entry) = result.iter_mut().find(|(e, _)| e.span == node.span) {
        if exist == Existence::Definitely {
            entry.1 = Existence::Definitely;
        }
    } else {
        result.push((node, exist));
    }
}

fn add_all<'a>(
    result: &mut Vec<(&'a Element<'a>, Existence)>,
    from: &[(&'a Element<'a>, Existence)],
) {
    for &(node, exist) in from {
        add_one(result, node, exist);
    }
}

/// Whether `element` carries a `slot="…"` attribute (the oracle skips slotted
/// elements when scanning for flow siblings).
fn has_slot_attribute(element: &Element<'_>, source: &str) -> bool {
    element.attributes.iter().any(|attr| {
        matches!(attr, AttributeNode::Attribute(a) if a.name_span.extract(source).eq_ignore_ascii_case("slot"))
    })
}
