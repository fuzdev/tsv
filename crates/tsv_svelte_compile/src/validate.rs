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
//!
//! **Oracle phase**: phase 1's parse-time element rules
//! (`phases/1-parse/state/element.js`) plus the phase-2 validations that fire on a
//! node's presence anywhere in the component. Running first — before erasure and
//! before the binding table — is what lets this walk take only `(root, source)`,
//! and is why it is the designated home for the whole-component validations still
//! open (see `../../docs/checklist_svelte_compiler.md`). See
//! [the walk inventory](crate#the-walks-and-their-oracle-phases).
//!
//! Every rule here is a port of an oracle **error**, never a warning
//! ([`Validator::report`] returns `Ok` on the `only_warn` path). So a component
//! this walk refuses is one the oracle rejects too — it lands in a corpus run's
//! `oracle_rejected` bucket, never in `oracle_accepted`, and therefore never in the
//! `achievable`-parity denominator that the `fenced` subtraction operates on.

use crate::CompileError;
use crate::analyze::{NameSet, pattern_binding_names};
use crate::attr_refs::each_child_fragment;
use crate::html_tree::{is_tag_valid_with_ancestor, is_tag_valid_with_parent};
use crate::refusal::Refusal;
use crate::script_decls::{
    ScriptDeclaration, VarScope, each_script_declaration, plain_identifier_name,
};
use crate::snippet::SnippetAnalysis;
use crate::text_class::js_trim;
use crate::transform_server::unsupported;
use tsv_lang::SharedInterner;
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, ElementKind, Fragment, FragmentNode, Root,
    SpecialElementKind,
};
use tsv_ts::ast::internal::{
    ExportNamedDeclaration, Expression, LiteralValue, ModuleExportName, Statement,
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
        slot_path: Vec::new(),
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

/// One entry of the **slot** rule's view of `context.path` — a separate stack from
/// [`PathEntry`], and deliberately so.
///
/// `PathEntry` answers the HTML-placement rules' questions and collapses every
/// walk-stopping node into one `Barrier`; the slot rule must tell those apart (a
/// component owner errors where a `<svelte:element>` owner does not). More
/// importantly the two stacks have different MEMBERSHIP: `PathEntry` omits nodes
/// transparent to placement — `<svelte:boundary>`, `<slot>`, `<title>`, the meta
/// tags — while the slot rule reads `path.at(-2)`, the element's immediate parent,
/// so a node it omitted would silently promote a grandparent into the parent slot.
/// A `<svelte:boundary>` between a component and a slotted child is exactly that
/// case, and the oracle rejects it (live-verified), so **every** fragment node is
/// pushed here.
///
/// The oracle's own path additionally carries a `Fragment` between every parent and
/// child, which is why it reads `at(-2)` rather than `at(-1)`; this stack simply
/// omits the fragments, so `last()` IS the parent.
enum SlotAncestor {
    /// A component invocation, `<svelte:component>`, or `<svelte:self>` — the owner
    /// class the placement rule fires on when it is not the direct parent.
    ComponentLike,
    /// `<svelte:element>` or a custom-element regular element — a slot owner that
    /// ENDS the search but never raises the error (the oracle's `if (owner)` block
    /// tests for the component types alone, so a non-component owner falls through
    /// with no report).
    NonComponentOwner,
    /// A `{#snippet}` — the oracle's early return: a slot attribute directly inside
    /// one is governed by `slot_attribute_invalid`, not by placement.
    Snippet,
    /// Any other node. Not an owner, but still occupies the parent position.
    Transparent,
}

struct Validator<'s> {
    source: &'s str,
    seen_meta: Vec<&'static str>,
    path: Vec<PathEntry<'s>>,
    slot_path: Vec<SlotAncestor>,
}

impl<'s> Validator<'s> {
    fn walk_fragment(
        &mut self,
        fragment: &Fragment<'_>,
        at_root: bool,
    ) -> Result<(), CompileError> {
        refuse_duplicate_snippet_names(fragment, self.source)?;
        for node in fragment.nodes {
            self.walk_node(node, at_root)?;
        }
        Ok(())
    }

    fn walk_node(&mut self, node: &FragmentNode<'_>, at_root: bool) -> Result<(), CompileError> {
        match node {
            FragmentNode::Element(element) => {
                let name = element.name_span.extract(self.source);
                // The oracle's parse-time `svelte_meta_invalid_tag`
                // (`phases/1-parse/state/element.js:142`, before `attribute_duplicate`
                // at `:250` and `tag_invalid_name` at `:151`). Every KNOWN `svelte:`
                // name parses to a `SpecialElementKind` — and `svelte:options` to
                // `Root.options` — so a `svelte:`-prefixed *regular* element is by
                // construction an unknown meta tag. A component name cannot carry the
                // prefix (it is lowercase), so `ElementKind::Html` is the only host.
                if element.kind == ElementKind::Html && name.starts_with("svelte:") {
                    return Err(unsupported(Refusal::SvelteMetaInvalidTag {
                        name: name.to_string(),
                    }));
                }
                refuse_duplicate_attributes(element.attributes, self.source)?;
                match element.kind {
                    ElementKind::Html => {
                        self.validate_element(element.attributes)?;
                        self.refuse_invalid_element_placement(name)?;
                    }
                    ElementKind::Component => self.validate_component(element.attributes)?,
                }
            }
            FragmentNode::SpecialElement(special) => {
                match special.kind {
                    SpecialElementKind::SvelteElement { .. } => {
                        self.validate_element(special.attributes)?;
                    }
                    // `visit_component`'s other two callers. `<svelte:component>`
                    // and `<svelte:self>` are deliberate fences, so this arm is
                    // unreachable today — written out rather than folded into the
                    // catch-all so it stays correct if either is ever un-fenced.
                    SpecialElementKind::SvelteComponent { .. } | SpecialElementKind::SvelteSelf => {
                        self.validate_component(special.attributes)?;
                    }
                    _ => {}
                }
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

        // The two `SnippetBlock` rules that read the snippet's PARENT. Checked
        // from the parent side because the oracle's `path.at(-2)` — the parent of
        // the snippet's containing `Fragment` — is exactly "the node whose own
        // fragment holds this snippet". See [`refuse_component_snippet_rules`].
        refuse_component_snippet_rules(node, self.source)?;

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

        // ⚠️ Two invariants the slot rule depends on, both encoded here rather than
        // where it reads them:
        //   - UNCONDITIONAL, unlike `path` above. The rule reads the immediate
        //     parent, so skipping a "transparent" node would promote its grandparent
        //     — which is how a `<svelte:boundary>` between a component and a slotted
        //     child would wrongly compile.
        //   - AFTER this node's own checks ran, so the stack holds ancestors ONLY.
        //     That is the oracle's shape: a custom element carrying `slot` is not
        //     its own owner. Moving this above the match silently accepts that case.
        self.slot_path.push(slot_ancestor(node, self.source));

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
        self.slot_path.pop();
        result
    }

    /// The attribute rules of the oracle's `validate_element`
    /// (`2-analyze/visitors/shared/element.js:29`).
    ///
    /// ⚠️ The grouping is the point: these fire together because they are checks
    /// inside ONE oracle function, whose only callers are `RegularElement.js`
    /// and `SvelteElement.js`. So a **component** is exempt from them — its visitor
    /// never reaches here, which is why `<F 3aa="abc" />` is legal while
    /// `<p 3aa="abc">` is not, and `<Button onbar="bar" />` is legal while
    /// `<button onbar="bar" />` is not. Adding a further check from that function
    /// belongs here, not at either call site.
    ///
    /// ⚠️ TWO rules are exceptions shared with the component path, and both are
    /// deliberately not folded in: the sequence-expression rule (the component
    /// visitor applies its own copy — [`refuse_unparenthesized_sequence`], called
    /// from both) and the unquoted-sequence rule (`validate_attribute` is called
    /// from both callers — [`refuse_unquoted_attribute_sequence`]). See
    /// [`Self::validate_component`] for the half that does not correspond.
    fn validate_element(&self, attributes: &[AttributeNode<'_>]) -> Result<(), CompileError> {
        for attribute in attributes {
            let AttributeNode::Attribute(a) = attribute else {
                continue;
            };
            // ONE loop in the oracle's own per-attribute order — unquoted-sequence
            // (`validate_attribute`) → sequence-expression → name → event handler →
            // slot placement. The order is preserved because the bucket a corpus
            // file lands in is observable: `<p foo={x, y} 3aa="1">` reports the
            // sequence, never the name — the oracle aborts on the first attribute's
            // first error, so a whole-list pre-pass for any single rule would
            // reorder multi-error elements.
            refuse_unquoted_attribute_sequence(a, self.source)?;
            if let Some(expression) = expression_attribute_value(a) {
                refuse_unparenthesized_sequence(expression, self.source)?;
            }
            refuse_invalid_attribute_name(a, self.source)?;
            refuse_invalid_event_handler(a, self.source)?;
            if a.name_span.extract(self.source) == "slot" {
                self.refuse_invalid_slot_placement()?;
            }
        }
        Ok(())
    }

    /// The attribute rules of the oracle's `visit_component`
    /// (`2-analyze/visitors/shared/component.js:78-117`), for the subset tsv
    /// reaches — a `<Foo>`, `<svelte:component>` or `<svelte:self>`.
    ///
    /// ⚠️ This is NOT `validate_element`'s rule set with the element bits removed;
    /// the two oracle functions overlap only on `validate_attribute` (the
    /// unquoted-sequence rule) and the sequence-expression check — and on the
    /// latter they differ in REACH. The component half additionally applies it
    /// to an `{@attach}` expression (`:115`), which the element half does not — so
    /// `<span {@attach a, b} />` compiles and `<Foo {@attach a, b} />` does not.
    /// Both directions live-probed; collapsing the two sites breaks one of them.
    fn validate_component(&self, attributes: &[AttributeNode<'_>]) -> Result<(), CompileError> {
        for attribute in attributes {
            match attribute {
                AttributeNode::Attribute(a) => {
                    // The oracle's order: `validate_attribute` (the
                    // unquoted-sequence rule — the ONE rule this path shares
                    // with `validate_element` besides the sequence scan) runs
                    // before the sequence check (`shared/component.js:93-100`).
                    refuse_unquoted_attribute_sequence(a, self.source)?;
                    if let Some(expression) = expression_attribute_value(a) {
                        refuse_unparenthesized_sequence(expression, self.source)?;
                    }
                }
                AttributeNode::AttachTag(attach) => {
                    refuse_unparenthesized_sequence(&attach.expression, self.source)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The oracle's `validate_slot_attribute`
    /// (`2-analyze/visitors/shared/attribute.js:56-125`), for the `is_component =
    /// false` callers — a regular element or `<svelte:element>`.
    ///
    /// The component caller (`is_component = true`) suppresses both error branches,
    /// and `<svelte:fragment>`, the third caller, is a deliberate fence — so the
    /// false case is the whole reachable rule.
    ///
    /// `slot_path` excludes the element itself (it is pushed after this runs), which
    /// is the oracle's own shape: its walk is over ancestors, so a custom element
    /// carrying `slot` is not its own owner.
    fn refuse_invalid_slot_placement(&self) -> Result<(), CompileError> {
        // The oracle's early return — placement does not apply directly inside a
        // `{#snippet}`. (Its sibling rule there, `slot_attribute_invalid` for a
        // non-text value, is not ported; a text value is legal and must compile.)
        if matches!(self.slot_path.last(), Some(SlotAncestor::Snippet)) {
            return Ok(());
        }

        let owner = self.slot_path.iter().rposition(|ancestor| {
            matches!(
                ancestor,
                SlotAncestor::ComponentLike | SlotAncestor::NonComponentOwner
            )
        });

        match owner {
            // No owner at all — the oracle's trailing `else if (!is_component)`.
            None => Err(unsupported(Refusal::SlotAttributeInvalidPlacement)),
            Some(index) => {
                // `owner !== parent` — the parent is the innermost entry, so the
                // owner being the parent means it sits at the top of the stack.
                // A non-component owner reports nothing either way.
                let is_parent = index == self.slot_path.len() - 1;
                if matches!(self.slot_path[index], SlotAncestor::ComponentLike) && !is_parent {
                    return Err(unsupported(Refusal::SlotAttributeInvalidPlacement));
                }
                // An owner that IS the parent is the oracle-ACCEPTED named-slot
                // shape, which tsv declines separately as the deliberate
                // `ComponentNamedSlot` fence. Refusing it here would move the file
                // out of the fenced count and flatter the parity denominator.
                Ok(())
            }
        }
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

/// The oracle's `Scope.declare` same-scope collision for snippet names
/// (`phases/scope.js:684-691`, reached from the `SnippetBlock` scope visitor at
/// `:1331-1335`, which declares into `state.scope` — the scope of the fragment
/// the snippet sits in).
///
/// ⚠️ The scope is the **fragment**, not the component. Every `Fragment` visitor
/// opens a child scope (`scope.js:1349`), and `declare` forwards to the parent
/// only for `var` and `import` — a snippet declares `'function'` — so two
/// `{#snippet a}` siblings collide while `<div>{#snippet a}{/snippet}</div>` +
/// a root-level `{#snippet a}` do not. Both live-probed.
///
/// A snippet whose name this port cannot read (an escaped identifier) is skipped;
/// `SnippetEscapedName` refuses such a component downstream anyway.
fn refuse_duplicate_snippet_names(
    fragment: &Fragment<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let mut seen: Vec<&str> = Vec::new();
    for node in fragment.nodes {
        let FragmentNode::SnippetBlock(snippet) = node else {
            continue;
        };
        let Some(name) = crate::snippet::snippet_name(snippet, source) else {
            continue;
        };
        if seen.contains(&name) {
            return Err(unsupported(Refusal::DuplicateSnippetName {
                name: name.to_string(),
            }));
        }
        seen.push(name);
    }
    Ok(())
}

/// The oracle's `snippet_shadowing_prop` and `snippet_conflict`
/// (`2-analyze/visitors/SnippetBlock.js:51-79`), applied to the direct
/// `{#snippet}` children of `node`'s own fragment.
///
/// Both read `path.at(-2)` — the parent of the snippet's containing `Fragment`,
/// i.e. the node this function is called with — so checking from the parent side
/// is the same question asked once per parent instead of once per snippet.
///
/// ⚠️ The two rules take **different** parent sets and harmonizing them is a bug:
/// shadowing is `Component` **only**, while conflict also accepts
/// `<svelte:component>` / `<svelte:self>`. Both special elements are deliberate
/// runes-only fences in tsv, so they refuse at emission — but this walk runs
/// *before* emission, so the conflict arm genuinely reaches them and is not dead
/// code.
///
/// The rules are mutually exclusive with `declaration_duplicate`
/// ([`validate_top_level_snippets`]): that one requires `is_top_level`, i.e. no
/// `path.at(-2)` at all.
fn refuse_component_snippet_rules(
    node: &FragmentNode<'_>,
    source: &str,
) -> Result<(), CompileError> {
    // `Component` proper, and the two special elements the conflict rule adds.
    let (attributes, fragment, is_component) = match node {
        FragmentNode::Element(element) if matches!(element.kind, ElementKind::Component) => {
            (element.attributes, &element.fragment, true)
        }
        FragmentNode::SpecialElement(special)
            if matches!(
                special.kind,
                SpecialElementKind::SvelteComponent { .. } | SpecialElementKind::SvelteSelf
            ) =>
        {
            (special.attributes, &special.fragment, false)
        }
        _ => return Ok(()),
    };

    for child in fragment.nodes {
        let FragmentNode::SnippetBlock(snippet) = child else {
            continue;
        };
        let Some(name) = crate::snippet::snippet_name(snippet, source) else {
            continue;
        };

        // `snippet_shadowing_prop` — a plain `Attribute` OR a `BindDirective` of
        // the same name. A directive of any other kind (`class:`, `on:`, …) is
        // deliberately absent from the oracle's `some(…)` predicate.
        if is_component
            && attributes.iter().any(|attribute| match attribute {
                AttributeNode::Attribute(a) => a.name_span.extract(source) == name,
                AttributeNode::BindDirective(d) => d.name_span.extract(source) == name,
                _ => false,
            })
        {
            return Err(unsupported(Refusal::SnippetShadowingProp {
                name: name.to_string(),
            }));
        }

        // `snippet_conflict` — reached only when shadowing did not fire, and only
        // for the exact name `children`.
        if name != "children" {
            continue;
        }
        // ⚠️ The oracle scans ALL of the parent's fragment nodes, not just the
        // ones preceding the snippet.
        if fragment.nodes.iter().any(|sibling| match sibling {
            FragmentNode::SnippetBlock(_) | FragmentNode::Comment(_) => false,
            FragmentNode::Text(text) => !js_trim(&text.data(source)).is_empty(),
            _ => true,
        }) {
            return Err(unsupported(Refusal::SnippetChildrenConflict));
        }
    }
    Ok(())
}

/// The oracle's `declaration_duplicate` at `2-analyze/visitors/SnippetBlock.js:34`
/// — a **top-level** `{#snippet}` (a direct child of the root fragment) whose name
/// the instance script also declares.
///
/// ⚠️ A different rule from [`refuse_duplicate_snippet_names`], which ports the
/// *other* call site of the same oracle error code (`Scope.declare`). This one is
/// snippet-vs-script and top-level-only: `<script>let foo = 1</script><div>{#snippet
/// foo()}…{/snippet}</div>` is legal (live-probed), because the nested snippet
/// declares into the `<div>`'s fragment scope rather than the instance scope.
///
/// ⚠️ An **import** local is not an instance-scope declaration and must NOT fire
/// this rule: `Scope.declare` forwards an `import` to the PARENT scope
/// (`phases/scope.js:679-681`), and the instance scope's parent is the module
/// scope — so an instance-script `import C from './C.svelte'` lands in
/// `module.scope.declarations` and is never in the `instance.scope.declarations`
/// this rule tests. `import_names` is therefore subtracted, exactly as
/// [`crate::snippet::analyze_snippets`] subtracts it from its own blocker set.
/// A module-script declaration never reaches here either (the same parent-scope
/// argument), and `instance_binding_names` holds only instance bindings.
///
/// Runs after the binding table rather than inside [`validate_document`]'s walk
/// because `analysis.instance.scope.declarations` is its input.
pub(crate) fn validate_top_level_snippets(
    root: &Root<'_>,
    source: &str,
    instance_binding_names: &NameSet,
    import_names: &NameSet,
) -> Result<(), CompileError> {
    for node in root.fragment.nodes {
        let FragmentNode::SnippetBlock(snippet) = node else {
            continue;
        };
        let Some(name) = crate::snippet::snippet_name(snippet, source) else {
            continue;
        };
        if instance_binding_names.contains(name) && !import_names.contains(name) {
            return Err(unsupported(Refusal::SnippetDeclarationDuplicate {
                name: name.to_string(),
            }));
        }
    }
    Ok(())
}

/// The oracle's module-export check (`2-analyze/index.js:823-836`): every
/// `export { … }` specifier in the **module** script must name something the
/// module scope holds.
///
/// ⚠️ **Module scope FIRST, snippet names second.** A hoistable top-level snippet
/// has its binding written INTO the module scope
/// (`SnippetBlock.js:40-44`), so `<script module>export {foo}</script>{#snippet
/// foo()}static{/snippet}` is valid and must NOT report `snippet_invalid_export`.
/// Only a snippet the oracle could not hoist — one referencing the instance script,
/// or one nested below the root fragment — reaches the error. Both directions
/// live-probed.
///
/// `export { x } from 'y'` is exempt (`node.source == null` in the oracle), and a
/// type-only export never reaches here: it is dropped by [`crate::erase`], exactly
/// as the oracle's own phase-1 `remove_typescript_nodes` drops it before analysis.
///
/// Returns `Ok` without checking anything when a module declaration carries a name
/// this port cannot read (an escaped identifier) — the module scope would be
/// under-populated and the check would refuse a valid export.
pub(crate) fn validate_module_exports(
    module_body: &[Statement<'_>],
    source: &str,
    snippets: &SnippetAnalysis,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    // `module_illegal_default_export` (`ExportNamedDeclaration.js:14-23`): an
    // `export { x as default }` specifier. Runs FIRST — before the opaque
    // early-return below and regardless of `export.source` — because the oracle
    // gates neither on the module scope being readable nor on a re-export's
    // `node.source`, unlike the snippet-export check that follows.
    for stmt in module_body {
        if let Statement::ExportNamedDeclaration(export) = stmt
            && export_named_has_default_specifier(export, source, interner)
        {
            return Err(unsupported(Refusal::ModuleDefaultExport));
        }
    }

    let Some(module_declared) = collect_module_declared(module_body, source) else {
        return Ok(());
    };

    for stmt in module_body {
        let Statement::ExportNamedDeclaration(export) = stmt else {
            continue;
        };
        if export.source.is_some() {
            continue;
        }
        for specifier in export.specifiers {
            // The oracle's `specifier.local.type !== 'Identifier'` skip — a string
            // local is only reachable in a re-export, which already `continue`d.
            let ModuleExportName::Identifier(local) = &specifier.local else {
                continue;
            };
            let Some(name) = plain_identifier_name(local, source) else {
                continue;
            };
            // The oracle's `analysis.module.scope.get(name)` — the module script's
            // own declarations PLUS every hoisted snippet's binding.
            if module_declared.contains(&name) || snippets.has_hoisted_snippet_named(&name) {
                continue;
            }
            if snippets.names.contains(&name) {
                return Err(unsupported(Refusal::SnippetInvalidExport { name }));
            }
            return Err(unsupported(Refusal::ExportUndefined { name }));
        }
    }
    Ok(())
}

/// Whether an `export { … }` declaration carries a specifier exported *as*
/// `default` — the trigger for the oracle's `module_illegal_default_export` from
/// its `ExportNamedDeclaration` visitor. Shared by [`validate_module_exports`] (the
/// compile path) and the sole-blocker census, so the rule lives once.
pub(crate) fn export_named_has_default_specifier(
    export: &ExportNamedDeclaration<'_>,
    source: &str,
    interner: &SharedInterner,
) -> bool {
    export
        .specifiers
        .iter()
        .any(|specifier| module_export_name_is_default(&specifier.exported, source, interner))
}

/// The oracle's `specifier.exported.type === 'Identifier' ? .name === 'default'
/// : .value === 'default'` (`ExportNamedDeclaration.js:17-19`): an identifier
/// `default` or a string-literal `"default"` export name. Both sides decode
/// exactly — the identifier via [`tsv_ts::ast::internal::Identifier::name`] (so an
/// escaped alias `default` is caught) and the string via
/// [`StringCooked::resolve`](tsv_ts::ast::internal::StringCooked::resolve) — so
/// the comparison matches the oracle's post-decode `.name`/`.value` on every form.
fn module_export_name_is_default(
    name: &ModuleExportName<'_>,
    source: &str,
    interner: &SharedInterner,
) -> bool {
    match name {
        ModuleExportName::Identifier(id) => id.name(source, &interner.borrow()) == "default",
        ModuleExportName::Literal(lit) => match &lit.value {
            LiteralValue::String(cooked) => cooked.resolve(lit.span, source) == "default",
            _ => false,
        },
    }
}

/// Every name the module script declares at module scope, or `None` when one of
/// them is unreadable to this port (see [`validate_module_exports`]).
///
/// Rides [`each_script_declaration`], the crate's single answer to "what does this
/// script declare", with `WithHoistedVars` because a `var` in a nested block is
/// function-scoped and so reaches module scope exactly like a top-level one.
fn collect_module_declared(module_body: &[Statement<'_>], source: &str) -> Option<NameSet> {
    let mut declared = NameSet::default();
    let mut opaque = false;
    let result: Result<(), CompileError> =
        each_script_declaration(module_body, VarScope::WithHoistedVars, &mut |decl| {
            match decl {
                ScriptDeclaration::Declarator { declarator, .. } => {
                    let mut names = Vec::new();
                    if pattern_binding_names(&declarator.id, source, &mut names).is_err() {
                        opaque = true;
                    } else {
                        declared.extend(names);
                    }
                }
                ScriptDeclaration::Function(id)
                | ScriptDeclaration::Class(id)
                | ScriptDeclaration::Import { local: id, .. } => {
                    match plain_identifier_name(id, source) {
                        Some(name) => {
                            declared.insert(name);
                        }
                        None => opaque = true,
                    }
                }
            }
            Ok(())
        });
    if result.is_err() || opaque {
        return None;
    }
    Some(declared)
}

/// The name the oracle passes for a text node or an `{expression}`.
const TEXT_PSEUDO_TAG: &str = "#text";

/// The oracle's `regex_not_whitespace` (`phases/patterns.js:9`) is `/[^ \t\r\n]/`
/// — a narrow four-character class, deliberately NOT JS `\s` and so also not
/// Rust's `char::is_whitespace`.
fn is_not_html_whitespace(c: char) -> bool {
    !matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// Classify one fragment node for the slot rule's ancestor stack.
///
/// The owner set is transcribed from the oracle
/// (`2-analyze/visitors/shared/attribute.js:69-77`): a component,
/// `<svelte:component>`, `<svelte:self>`, `<svelte:element>`, or a
/// **custom-element** regular element.
///
/// # The catch-alls, and which way they fail
///
/// Unlike this module's other walks, the two `_ =>` arms below are deliberate
/// rather than exhaustive — so per the crate's rule they must name their failure
/// direction. A new variant defaulting to [`SlotAncestor::Transparent`] can only
/// **over-refuse**, never over-accept:
///
/// - the parent position is never wrong, because *every* node is pushed — only
///   owner *candidacy* is lost;
/// - losing a candidate either finds no owner (refuse) or finds an outer one. An
///   outer `ComponentLike` that is not the parent refuses — stricter. An outer
///   `NonComponentOwner` accepts, but so does the oracle in that shape (its true
///   owner would have been the parent, taking the non-erroring `else` branch);
/// - a variant that should have been [`SlotAncestor::Snippet`] skips the early
///   return and runs the owner search, which refuses where the oracle accepts.
///
/// So an unhandled variant lands in the class the refusal contract *permits* — a
/// "not yet" — never in the class that fails the validation gate. An exhaustive
/// match is still preferable and cheap here; the catch-alls exist only because
/// `SpecialElementKind` and `FragmentNode` carry many variants that are all
/// genuinely transparent, and listing them would drift on every new one for no
/// behavioral gain.
fn slot_ancestor(node: &FragmentNode<'_>, source: &str) -> SlotAncestor {
    match node {
        FragmentNode::Element(element) => match element.kind {
            ElementKind::Component => SlotAncestor::ComponentLike,
            ElementKind::Html => {
                let name = element.name_span.extract(source);
                if is_custom_element(name, element.attributes, source) {
                    SlotAncestor::NonComponentOwner
                } else {
                    SlotAncestor::Transparent
                }
            }
        },
        FragmentNode::SpecialElement(special) => match special.kind {
            SpecialElementKind::SvelteComponent { .. } | SpecialElementKind::SvelteSelf => {
                SlotAncestor::ComponentLike
            }
            SpecialElementKind::SvelteElement { .. } => SlotAncestor::NonComponentOwner,
            _ => SlotAncestor::Transparent,
        },
        FragmentNode::SnippetBlock(_) => SlotAncestor::Snippet,
        _ => SlotAncestor::Transparent,
    }
}

/// The oracle's `is_custom_element_node` (`phases/nodes.js:40-46`): a regular
/// element whose tag contains a `-`, **or** which carries an `is` attribute.
fn is_custom_element(name: &str, attributes: &[AttributeNode<'_>], source: &str) -> bool {
    name.contains('-') || has_plain_attribute(attributes, source, "is")
}

/// Does this attribute list carry a plain `Attribute` of this name?
///
/// Both callers key on the oracle's own `attribute.type === 'Attribute' &&
/// attribute.name === …` test, so a directive (`bind:slot`), a spread, and an
/// `{@attach}` all correctly miss.
fn has_plain_attribute(attributes: &[AttributeNode<'_>], source: &str, name: &str) -> bool {
    attributes.iter().any(
        |attribute| matches!(attribute, AttributeNode::Attribute(a) if a.name_span.extract(source) == name),
    )
}

/// The oracle's `attribute_invalid_name`
/// (`2-analyze/visitors/shared/element.js:56-60`) — a transcription of
/// `regex_illegal_attribute_character` (`phases/patterns.js:23`):
///
/// ```text
/// /(^[0-9-.])|[\^$@%&#?!|()[\]{}^*+~;]/
/// ```
///
/// Two independent alternatives, and reading them as one class is the trap: the
/// first is anchored (an illegal **leading** character — a digit, `-`, or `.`),
/// the second is unanchored (an illegal character **anywhere**). `.` and `-`
/// appear ONLY in the anchored half, so `a.b` and `data-x` are legal while `.a`
/// and `-a` are not — and `data-` names are ubiquitous, so collapsing the two
/// alternatives would refuse most real components.
///
/// ⚠️ Only a plain `Attribute` participates. A directive, a spread and an
/// `{@attach}` carry their own grammar (`bind:`, `class:`, `on:` — every one of
/// which contains a `:` that this class does not even list) and the oracle's loop
/// guards on `attribute.type === 'Attribute'` before testing.
fn refuse_invalid_attribute_name(
    attribute: &Attribute<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let name = attribute.name_span.extract(source);
    if has_illegal_attribute_character(name) {
        return Err(unsupported(Refusal::AttributeInvalidName {
            name: name.to_string(),
        }));
    }
    Ok(())
}

/// The oracle's `attribute_unquoted_sequence` — the error half of
/// `validate_attribute` (`2-analyze/visitors/shared/attribute.js:41-48`): a plain
/// attribute whose value is TWO OR MORE chunks (`href=/{path}`, `data-x={a}{b}`)
/// must be quote-delimited.
///
/// The quote test is the oracle's span comparison
/// (`attribute.value.at(-1)?.end !== attribute.end`): a quoted value's closing
/// quote sits between the last chunk's end and the attribute's end, so EQUALITY
/// means the value runs flush to the attribute — unquoted. The early returns are
/// the oracle's own (`value === true || !Array.isArray(value) || length === 1`):
/// a bare attribute and a single-chunk value are exempt whatever their quoting —
/// the single-expression carve-out is what keeps the ubiquitous `href={path}`
/// legal.
///
/// ⚠️ Reached from BOTH paths — `validate_attribute` is called from
/// `shared/element.js:43` AND `shared/component.js:93` — unlike the
/// name/event-handler rules, which are element-only. Its warning half
/// (`w.attribute_quoted`) gates nothing and is not ported.
fn refuse_unquoted_attribute_sequence(
    attribute: &Attribute<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let Some(chunks) = attribute.value else {
        return Ok(());
    };
    let Some(last) = chunks.last() else {
        return Ok(());
    };
    if chunks.len() == 1 {
        return Ok(());
    }
    let last_end = match last {
        AttributeValue::Text(text) => text.span.end,
        AttributeValue::ExpressionTag(tag) => tag.span.end,
    };
    if last_end == attribute.span.end {
        return Err(unsupported(Refusal::AttributeUnquotedSequence {
            name: attribute.name_span.extract(source).to_string(),
        }));
    }
    Ok(())
}

/// The oracle's `is_expression_attribute` + `get_attribute_expression`
/// (`utils/ast.js:43,59`) as one operation: the attribute's value when it is a
/// SINGLE expression, `None` otherwise.
///
/// The three `None` shapes are all real inputs and all distinct: a BARE attribute
/// (`onclick`, the oracle's `value === true`), a text value (`onclick="foo"`), and
/// a multi-chunk value (`onclick="{a}{b}"`). Every one of them is a rejected event
/// handler, which is why the event-handler rule keys on this returning `None`
/// rather than testing for a text value.
fn expression_attribute_value<'a>(attribute: &'a Attribute<'a>) -> Option<&'a Expression<'a>> {
    match attribute.value {
        Some([AttributeValue::ExpressionTag(tag)]) => Some(&tag.expression),
        _ => None,
    }
}

/// The oracle's `attribute_invalid_event_handler`
/// (`2-analyze/visitors/shared/element.js:62-66`).
///
/// ⚠️ `length > 2` is load-bearing and is NOT a guard against a short slice: the
/// bare attribute `on` is LEGAL and `onx` is not (both live-probed). Writing it as
/// `starts_with("on")` alone silently refuses a valid `<button on>`.
fn refuse_invalid_event_handler(
    attribute: &Attribute<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let name = attribute.name_span.extract(source);
    if name.starts_with("on") && name.len() > 2 && expression_attribute_value(attribute).is_none() {
        return Err(unsupported(Refusal::AttributeInvalidEventHandler {
            name: name.to_string(),
        }));
    }
    Ok(())
}

/// The oracle's `disallow_unparenthesized_sequences`
/// (`2-analyze/visitors/shared/component.js:168-178`, duplicated inline at
/// `shared/element.js:48-56`).
///
/// ⚠️ The backward SOURCE scan is reproduced rather than replaced by a span test,
/// because the two are not equivalent. A parenthesized sequence is not a distinct
/// AST node — ESTree drops the parens — so the only record that the author wrote
/// `{(x, y)}` rather than `{x, y}` is the byte before the sequence's start. The
/// scan walks back to the first `(` (legal — parenthesized) or `{` (illegal — bare
/// against the attribute delimiter), skipping whitespace and anything else.
///
/// The `(`-first case is what makes a NESTED sequence legal: in `{[x, (y, z)]}` the
/// inner sequence starts at `y` and the scan immediately finds its `(`. Verified
/// against tsv's own spans at three shapes, including `{(x), y}` — where the
/// sequence starts at the `(` of its first element, so the scan correctly steps
/// PAST that paren to the `{` and rejects.
fn refuse_unparenthesized_sequence(
    expression: &Expression<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let Expression::SequenceExpression(sequence) = expression else {
        return Ok(());
    };
    // The oracle's `while (--i > 0)`: start one byte before the sequence and stop
    // before index 0, so byte 0 is never examined. A `{` can never sit at byte 0 of
    // a component that reached here anyway.
    let bytes = source.as_bytes();
    let mut i = sequence.span.start as usize;
    while i > 1 {
        i -= 1;
        match bytes[i] {
            b'(' => return Ok(()),
            b'{' => return Err(unsupported(Refusal::AttributeInvalidSequenceExpression)),
            _ => {}
        }
    }
    Ok(())
}

/// `regex_illegal_attribute_character` as a predicate — see
/// [`refuse_invalid_attribute_name`] for why the two alternatives stay separate.
fn has_illegal_attribute_character(name: &str) -> bool {
    // The ANCHORED alternative, `(^[0-9-.])`. Byte-wise is sound: every member is
    // ASCII, so a multi-byte leading char simply does not match.
    if matches!(name.as_bytes().first(), Some(b'0'..=b'9' | b'-' | b'.')) {
        return true;
    }
    // The UNANCHORED alternative. `^` and `[` appear twice in the source class
    // (`\^` … `^`, and `[` … `[\]`); the duplicates are inert.
    name.contains([
        '^', '$', '@', '%', '&', '#', '?', '!', '|', '(', ')', '[', ']', '{', '}', '*', '+', '~',
        ';',
    ])
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
