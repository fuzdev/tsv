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
use crate::html_tree::{is_tag_valid_with_ancestor, is_tag_valid_with_parent};
use crate::refusal::Refusal;
use crate::transform_server::unsupported;
use tsv_svelte::ast::internal::{
    AttributeNode, ElementKind, Fragment, FragmentNode, Root, SpecialElementKind,
};

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
    let mut validator = Validator {
        source,
        seen_meta: Vec::new(),
        path: Vec::new(),
    };
    validator.walk_fragment(&root.fragment, true)
}

/// One entry of the oracle's `context.path`, reduced to the three things the
/// HTML-placement rules read from it. Nodes that are transparent to every rule
/// are simply not pushed — behaviorally identical to the oracle skipping them,
/// since its loop only ever reacts to these types.
enum PathEntry<'s> {
    /// The oracle's `RegularElement`, by tag name.
    Element(&'s str),
    /// Resets `parent_element` **and** stops the ancestor walk: a component
    /// (`<Foo>`, `<svelte:component>`, `<svelte:self>`), `<svelte:element>`, or a
    /// `{#snippet}`.
    Barrier,
    /// ⚠️ The oracle's one asymmetric node: `<svelte:fragment>` resets
    /// `parent_element` (`SvelteFragment.js:26`) but is **absent** from the walk's
    /// break set (`RegularElement.js:201-207`), so an ancestor check can still see
    /// past it. Unreachable today — `<svelte:fragment>` is a deliberate runes-only
    /// fence, so such a component refuses before validation — but modelled rather
    /// than collapsed into `Barrier` so the quirk survives a future un-fencing.
    ParentReset,
    /// An `{#if}` / `{#each}` / `{#await}` / `{#key}` block. Svelte compiles each
    /// into its own template string, so client-side the markup would work — which
    /// is why a violation below one is a WARNING (`node_invalid_placement_ssr`),
    /// not an error. tsv must not refuse there.
    Block,
}

struct Validator<'s> {
    source: &'s str,
    seen_meta: Vec<&'static str>,
    path: Vec<PathEntry<'s>>,
}

impl<'s> Validator<'s> {
    fn walk_fragment(
        &mut self,
        fragment: &Fragment<'_>,
        at_root: bool,
    ) -> Result<(), CompileError> {
        for node in fragment.nodes {
            self.walk_node(node, at_root)?;
        }
        Ok(())
    }

    fn walk_node(&mut self, node: &FragmentNode<'_>, at_root: bool) -> Result<(), CompileError> {
        match node {
            FragmentNode::Element(element) => {
                refuse_duplicate_attributes(element.attributes, self.source)?;
                if element.kind == ElementKind::Html {
                    self.refuse_invalid_element_placement(element.name_span.extract(self.source))?;
                }
            }
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
                    if self.seen_meta.contains(&tag) {
                        return Err(unsupported(Refusal::DuplicateSpecialElement {
                            name: tag.to_string(),
                        }));
                    }
                    self.seen_meta.push(tag);
                }
                refuse_duplicate_attributes(special.attributes, self.source)?;
            }
            // The oracle's `Text` visitor checks only whitespace-BEARING text, and
            // only when its parent is a `Fragment`. That second guard is automatic
            // here: this walk descends through fragments alone, so an attribute
            // value's text is never reached.
            FragmentNode::Text(text) => {
                // The oracle tests `node.data`, the DECODED text, so an entity must
                // be decoded first: `&#32;` is non-whitespace raw and a plain space
                // decoded, and testing the raw slice would refuse it.
                if text.data(self.source).contains(is_not_html_whitespace) {
                    self.refuse_invalid_text_placement()?;
                }
            }
            // An `{expression}` is checked with no whitespace test — the oracle
            // cannot know what it renders to, so it assumes text.
            FragmentNode::ExpressionTag(_) => self.refuse_invalid_text_placement()?,
            _ => {}
        }

        let entry = match node {
            FragmentNode::Element(element) => match element.kind {
                ElementKind::Html => {
                    Some(PathEntry::Element(element.name_span.extract(self.source)))
                }
                ElementKind::Component => Some(PathEntry::Barrier),
            },
            FragmentNode::SpecialElement(special) => match special.kind {
                SpecialElementKind::SvelteElement { .. }
                | SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteSelf => Some(PathEntry::Barrier),
                SpecialElementKind::SvelteFragment => Some(PathEntry::ParentReset),
                // `<svelte:boundary>` is deliberately absent from both the reset and
                // the break set (`SvelteBoundary.js` calls a bare `context.next()`),
                // as are `<slot>`, `<title>` and the root-only meta tags — all
                // transparent to these rules.
                _ => None,
            },
            FragmentNode::SnippetBlock(_) => Some(PathEntry::Barrier),
            FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_) => Some(PathEntry::Block),
            _ => None,
        };

        let pushed = entry.is_some();
        if let Some(entry) = entry {
            self.path.push(entry);
        }

        // Every child fragment is below the root, so `at_root` is false from here down
        // — matching the oracle's `parent.type !== 'Root'`, which is a *direct*-child
        // test: a block or an element between the root and the tag makes it invalid.
        let mut result = Ok(());
        each_child_fragment(node, &mut |child| {
            if result.is_ok() {
                result = self.walk_fragment(child, false);
            }
        });

        if pushed {
            self.path.pop();
        }
        result
    }

    /// The oracle's `parent_element` — the nearest enclosing `RegularElement`, or
    /// `None` if a barrier or a `<svelte:fragment>` intervenes.
    fn parent_element(&self) -> Option<&'s str> {
        for entry in self.path.iter().rev() {
            match entry {
                PathEntry::Element(name) => return Some(name),
                PathEntry::Barrier | PathEntry::ParentReset => return None,
                PathEntry::Block => {}
            }
        }
        None
    }

    /// `node_invalid_placement` for a text node or an `{expression}`.
    ///
    /// ⚠️ A DIFFERENT rule from the element check below, not a special case of it:
    /// `Text.js:21` / `ExpressionTag.js:15` call `is_tag_valid_with_parent` and
    /// STOP. No ancestor walk, and — because neither visitor carries `only_warn` —
    /// **no block downgrade**, so a violation under an `{#if}` is still an error
    /// here while the same violation by an element is only a warning.
    fn refuse_invalid_text_placement(&self) -> Result<(), CompileError> {
        let Some(parent_element) = self.parent_element() else {
            return Ok(());
        };
        match is_tag_valid_with_parent(TEXT_PSEUDO_TAG, parent_element) {
            Some(message) => Err(unsupported(Refusal::NodeInvalidPlacement { message })),
            None => Ok(()),
        }
    }

    /// `node_invalid_placement` for an element — a faithful transcription of the
    /// ancestor loop in `2-analyze/visitors/RegularElement.js:160-211`.
    fn refuse_invalid_element_placement(&self, child_tag: &str) -> Result<(), CompileError> {
        let Some(parent_element) = self.parent_element() else {
            return Ok(());
        };

        let mut ancestors: Vec<&str> = vec![parent_element];
        let mut past_parent = false;
        let mut only_warn = false;

        for entry in self.path.iter().rev() {
            if matches!(entry, PathEntry::Block) {
                // Set before this iteration's check, as the oracle does: a block
                // anywhere between the child and the ancestor under test downgrades
                // that check to a warning.
                only_warn = true;
                continue;
            }

            if !past_parent {
                // ⚠️ The break arm is the `else` of this branch in the oracle, so a
                // barrier encountered BEFORE the parent is found does not break.
                // Unreachable — a barrier between the node and its nearest element
                // would have made `parent_element` `None` — but kept faithful.
                if let PathEntry::Element(name) = entry
                    && *name == parent_element
                {
                    self.report(
                        is_tag_valid_with_parent(child_tag, parent_element),
                        only_warn,
                    )?;
                    past_parent = true;
                }
            } else if let PathEntry::Element(name) = entry {
                ancestors.push(name);
                self.report(is_tag_valid_with_ancestor(child_tag, &ancestors), only_warn)?;
            } else if matches!(entry, PathEntry::Barrier) {
                break;
            }
        }

        Ok(())
    }

    /// A violation the oracle reports as an ERROR becomes a refusal; one it reports
    /// as `node_invalid_placement_ssr` is a WARNING, and a warning must never
    /// refuse — the component compiles on both sides.
    fn report(&self, message: Option<String>, only_warn: bool) -> Result<(), CompileError> {
        match message {
            Some(message) if !only_warn => {
                Err(unsupported(Refusal::NodeInvalidPlacement { message }))
            }
            _ => Ok(()),
        }
    }
}

/// The name the oracle passes for a text node or an `{expression}`.
const TEXT_PSEUDO_TAG: &str = "#text";

/// The oracle's `regex_not_whitespace` (`phases/patterns.js:9`) is `/[^ \t\r\n]/`
/// — a narrow four-character class, deliberately NOT JS `\s` and so also not
/// Rust's `char::is_whitespace`.
fn is_not_html_whitespace(c: char) -> bool {
    !matches!(c, ' ' | '\t' | '\r' | '\n')
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
