//! The shared template traversals.
//!
//! Several of them, all here for the same reason: an analysis that hand-writes
//! its own walk drifts from the others, and the drift is silent.
//! [`each_template_item`] is the whole-fragment walk yielding every
//! reference-bearing expression; the `each_*_attribute_expression` pair below is
//! the per-element one it and the other analyses share; and [`each_child_fragment`]
//! is the pure structural seam — "which sub-fragments does a node contain" — that
//! the `fragment_has_*` predicates (via [`fragment_any`]) and the snippet-name
//! collector ride, so the fragment-recursion shape lives in exactly one
//! exhaustively-matched place.
//!
//! Both the snippet hoist analysis (`snippet.rs`) and the `needs_context` walk
//! (`needs_context.rs`) must see every attribute expression the compiled output
//! can reference. They previously hand-wrote the same iteration and drifted:
//! the component-spread arm existed in one but not the other, so a top-level
//! snippet whose only instance-binding reference sat in a `<Foo {...binding} />`
//! spread was wrongly module-hoisted — a runtime `ReferenceError` the reparse
//! self-validation cannot catch (a free reference parses fine). This module is
//! the single definition of "reference-bearing attribute expression"; an
//! attribute shape that newly reaches emission must be added HERE so every
//! analysis sees it at once.
//!
//! Two traversals share that definition. `each_attribute_expression` is the
//! **emitted-path** view: it visits everything not refused at emission — plain
//! attribute values, component spreads, and the no-op drop family
//! (`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`) on a regular
//! element, which is dropped-but-analyzed exactly like an event handler. It skips
//! the positions that *do* refuse (an element spread, `class:`/`style:`/`bind:`/
//! legacy `on:`/`let:`), because the emission refusal is what keeps their
//! references out of output — and its `each_emitted_directive_name` companion
//! surfaces the drop-family directive *names* (which an expression traversal can't
//! reach). `each_reference_bearing_attribute_expression` is the **dropped-fragment**
//! view (a `{:catch}` branch the emitter discards without walking): there no
//! emission refusal fires, so every attribute reference must be counted to match
//! the oracle. The dropped-fragment view has two more entry points for the same
//! reason — `each_reference_bearing_directive_name` (a directive whose *name* is a
//! value binding, the wider set including a `style:` shorthand) and
//! `special_element_reference_expression` (a `<svelte:element>`/`<svelte:component>`
//! `this={…}`); every special element is refused at emission elsewhere, so its
//! references, too, are only reachable through the dropped-fragment path.

use tsv_svelte::ast::internal::{
    AttributeNode, AttributeValue, Element, ElementKind, Fragment, FragmentNode, SpecialElement,
    SpecialElementKind, StyleDirectiveValue,
};
use tsv_ts::ast::internal::Expression;

use crate::CompileError;

/// One TypeScript- or rune-bearing item of a template fragment.
pub(crate) enum TemplateItem<'a, 'arena> {
    /// A borrowed expression — the overwhelming majority.
    Expression(&'a Expression<'arena>),
    /// A `{#snippet}`'s `<T>` clause. TypeScript with no `Expression` to yield,
    /// so it gets its own item rather than being missed.
    SnippetTypeParameters,
}

/// Visit every TypeScript- or rune-bearing item a template fragment holds,
/// recursing into every child fragment.
///
/// This is the **dropped-fragment** view (it uses
/// [`each_reference_bearing_attribute_expression`], the widest attribute set, and
/// visits the SSR-dropped `{#each}` key, `{#key}` expression, `{:catch}` binding
/// and branch): its two consumers ask what a region *contains*, not what it
/// *emits*, and a dropped region's contents never reach an emission refusal at
/// all.
///
/// - The **document-wide TypeScript gate** (`refuse_template_typescript`): without
///   `lang="ts"` the oracle's parser rejects TypeScript anywhere in the document,
///   dropped positions included.
/// - The **rune guard over a dropped `{:catch}` branch** (`guard_dropped_fragment`):
///   the oracle rejects a misplaced rune in its *analysis* phase, before it decides
///   what to emit, so dropping the branch cannot make the component valid.
///
/// The `FragmentNode` match is exhaustive on purpose — a new template shape fails
/// compilation here instead of slipping silently past both gates.
pub(crate) fn each_template_item<'a, 'arena>(
    fragment: &'a Fragment<'arena>,
    f: &mut impl FnMut(TemplateItem<'a, 'arena>) -> Result<(), CompileError>,
) -> Result<(), CompileError> {
    for node in fragment.nodes {
        each_node_item(node, f)?;
    }
    Ok(())
}

fn each_node_item<'a, 'arena>(
    node: &'a FragmentNode<'arena>,
    f: &mut impl FnMut(TemplateItem<'a, 'arena>) -> Result<(), CompileError>,
) -> Result<(), CompileError> {
    let mut expr = |e: &'a Expression<'arena>| f(TemplateItem::Expression(e));
    match node {
        FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
        FragmentNode::Element(element) => {
            each_attribute_item(element.attributes, f)?;
            each_template_item(&element.fragment, f)?;
        }
        FragmentNode::SpecialElement(special) => {
            if let Some(this) = special_element_reference_expression(special) {
                expr(this)?;
            }
            each_attribute_item(special.attributes, f)?;
            each_template_item(&special.fragment, f)?;
        }
        FragmentNode::ExpressionTag(tag) => expr(&tag.expression)?,
        FragmentNode::HtmlTag(tag) => expr(&tag.expression)?,
        FragmentNode::RenderTag(tag) => expr(&tag.expression)?,
        FragmentNode::DebugTag(tag) => {
            for identifier in tag.identifiers {
                expr(identifier)?;
            }
        }
        FragmentNode::ConstTag(tag) => {
            expr(&tag.id)?;
            expr(&tag.init)?;
        }
        FragmentNode::DeclarationTag(tag) => {
            for declarator in tag.declaration.declarations {
                expr(&declarator.id)?;
                if let Some(init) = &declarator.init {
                    expr(init)?;
                }
            }
        }
        FragmentNode::IfBlock(block) => {
            expr(&block.test)?;
            each_template_item(&block.consequent, f)?;
            if let Some(alternate) = &block.alternate {
                each_template_item(alternate, f)?;
            }
        }
        FragmentNode::EachBlock(block) => {
            expr(&block.expression)?;
            for e in block.context.iter().chain(block.key.iter()) {
                expr(e)?;
            }
            each_template_item(&block.body, f)?;
            if let Some(fallback) = &block.fallback {
                each_template_item(fallback, f)?;
            }
        }
        FragmentNode::AwaitBlock(block) => {
            expr(&block.expression)?;
            for e in block.value.iter().chain(block.error.iter()) {
                expr(e)?;
            }
            for child in [&block.pending, &block.then, &block.catch]
                .into_iter()
                .flatten()
            {
                each_template_item(child, f)?;
            }
        }
        FragmentNode::KeyBlock(block) => {
            expr(&block.expression)?;
            each_template_item(&block.fragment, f)?;
        }
        FragmentNode::SnippetBlock(snippet) => {
            // `type_params_raw` is the raw-text fallback for a `<T>` clause whose
            // inner parse failed — still TypeScript, still surfaced.
            if snippet.type_parameters.is_some() || snippet.type_params_raw.is_some() {
                f(TemplateItem::SnippetTypeParameters)?;
            }
            for param in snippet.parameters {
                f(TemplateItem::Expression(param))?;
            }
            each_template_item(&snippet.body, f)?;
        }
    }
    Ok(())
}

fn each_attribute_item<'a, 'arena>(
    attributes: &'a [AttributeNode<'arena>],
    f: &mut impl FnMut(TemplateItem<'a, 'arena>) -> Result<(), CompileError>,
) -> Result<(), CompileError> {
    let mut found: Vec<&'a Expression<'arena>> = Vec::new();
    each_reference_bearing_attribute_expression(attributes, &mut |e| found.push(e));
    for e in found {
        f(TemplateItem::Expression(e))?;
    }
    Ok(())
}

/// The single, exhaustively-matched enumeration of a `FragmentNode`'s child
/// fragments — "which sub-fragments does this node contain".
///
/// Every purely-structural recursion that only needs to descend the fragment
/// tree rides this one match: the `fragment_has_*` predicates (via
/// [`fragment_any`]) and the snippet-name collector (`snippet.rs`). So a new
/// `FragmentNode` variant — or a new child fragment on an existing variant — fails
/// compilation HERE instead of silently drifting across the hand-written copies
/// (which is how `fragment_contains_block` came to skip `SpecialElement` while its
/// siblings recursed).
///
/// This is fragment recursion *only*. A block's own condition/key expressions are
/// not fragments and are out of scope; an expression-bearing walk uses
/// [`each_template_item`]. The scope-tracking / dropped-`{:catch}` walks
/// (`needs_context.rs`, `snippet.rs`'s free-variable collector) keep their own
/// exhaustive matches, because their descent is entangled with per-node scope
/// mutation and the emission-vs-dropped distinction this uniform enumeration
/// cannot express without changing behavior.
pub(crate) fn each_child_fragment<'a, 'arena>(
    node: &'a FragmentNode<'arena>,
    f: &mut impl FnMut(&'a Fragment<'arena>),
) {
    match node {
        FragmentNode::Text(_)
        | FragmentNode::Comment(_)
        | FragmentNode::ExpressionTag(_)
        | FragmentNode::HtmlTag(_)
        | FragmentNode::RenderTag(_)
        | FragmentNode::DebugTag(_)
        | FragmentNode::ConstTag(_)
        | FragmentNode::DeclarationTag(_) => {}
        FragmentNode::Element(element) => f(&element.fragment),
        FragmentNode::SpecialElement(special) => f(&special.fragment),
        FragmentNode::IfBlock(block) => {
            f(&block.consequent);
            if let Some(alternate) = &block.alternate {
                f(alternate);
            }
        }
        FragmentNode::EachBlock(block) => {
            f(&block.body);
            if let Some(fallback) = &block.fallback {
                f(fallback);
            }
        }
        FragmentNode::AwaitBlock(block) => {
            for fragment in [&block.pending, &block.then, &block.catch]
                .into_iter()
                .flatten()
            {
                f(fragment);
            }
        }
        FragmentNode::KeyBlock(block) => f(&block.fragment),
        FragmentNode::SnippetBlock(snippet) => f(&snippet.body),
    }
}

/// Whether any node in `fragment`, or recursively in any of its child fragments,
/// satisfies `test`. The descent rides [`each_child_fragment`], so every
/// `fragment_has_*` predicate shares one exhaustively-matched recursion and none
/// can drift; each predicate supplies only its own narrow per-node `test`.
pub(crate) fn fragment_any<'arena>(
    fragment: &Fragment<'arena>,
    test: &impl Fn(&FragmentNode<'arena>) -> bool,
) -> bool {
    fragment.nodes.iter().any(|node| {
        test(node) || {
            let mut found = false;
            each_child_fragment(node, &mut |child| {
                found = found || fragment_any(child, test);
            });
            found
        }
    })
}

/// Visit every attribute expression of `element` the oracle walks for scope /
/// `needs_context` on the **emitted** path — everything not refused at emission:
///
/// - plain attribute expression values (single-expression and mixed-value
///   chunks) on any element;
/// - `{...spread}` expressions on **components** (emitted as `$.spread_props`
///   array elements);
/// - the **no-op drop family** on a regular HTML element (`use:`/`transition:`/
///   `in:`/`out:`/`animate:` expressions and `{@attach}`). These contribute
///   nothing to the tag but are dropped-but-analyzed, exactly like an event
///   handler: the oracle discards their output yet still walks the expression
///   (its `context.visit`), so a `new`/prop-rooted access inside one must still
///   fire the `$$renderer.component` wrapper. On a **component** these refuse at
///   emission (a `ComponentDirective`), so they are skipped there — visiting them
///   would let an analysis refusal fire first and shift corpus buckets.
///
/// An *element* spread is refused at emission, so its expression never reaches
/// output — and visiting it here would let an analysis refusal fire before the
/// emission refusal, shifting corpus buckets. `class:`/`style:`/`bind:`/legacy
/// `on:`/`let:` are likewise refused at emission and not visited.
///
/// A drop-family directive's **name** (`use:action`, `transition:fade`) is also a
/// binding reference the oracle counts, but tsv stores it as a verbatim name span
/// rather than an `Expression`, so an expression traversal cannot reach it — it is
/// surfaced separately by [`each_emitted_directive_name`] (needed by the
/// snippet-hoist analysis; a bare name never fires `needs_context`).
pub(crate) fn each_attribute_expression<'a, 'arena>(
    element: &'a Element<'arena>,
    f: &mut impl FnMut(&'a Expression<'arena>),
) {
    let is_html = element.kind == ElementKind::Html;
    for attr_node in element.attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                if let Some(values) = attr.value {
                    for value in values {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
            }
            AttributeNode::SpreadAttribute(spread) if element.kind == ElementKind::Component => {
                f(&spread.expression);
            }
            // The no-op drop family: dropped-but-analyzed on a regular element.
            AttributeNode::AttachTag(attach) if is_html => f(&attach.expression),
            AttributeNode::UseDirective(d) if is_html => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::TransitionDirective(d) if is_html => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::AnimateDirective(d) if is_html => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            _ => {}
        }
    }
}

/// Visit the NAME of every no-op-drop-family directive that names a value binding
/// on an **emitted** regular element: `use:` (action), `transition:`/`in:`/`out:`
/// (transition fn), `animate:` (animation fn). Dropped from the tag output, but
/// the oracle still counts the referenced binding — a top-level `{#snippet}` whose
/// only instance-binding reference is such a directive name must **not**
/// module-hoist. The name may be a member path (`use:a.b`); the raw slice is
/// surfaced for the consumer to reduce to its root identifier.
///
/// Components refuse these at emission, so their names are skipped (the refusal
/// fires). The dropped-`{:catch}` path uses
/// [`each_reference_bearing_directive_name`], whose wider set also includes a
/// `style:` shorthand (which refuses on the emitted path). A bare name never fires
/// `needs_context`, so only the snippet-hoist analysis consumes this.
pub(crate) fn each_emitted_directive_name<'s>(
    element: &Element<'_>,
    source: &'s str,
    f: &mut impl FnMut(&'s str),
) {
    if element.kind != ElementKind::Html {
        return;
    }
    for attr_node in element.attributes {
        let name_span = match attr_node {
            AttributeNode::UseDirective(d) => d.name_span,
            AttributeNode::TransitionDirective(d) => d.name_span,
            AttributeNode::AnimateDirective(d) => d.name_span,
            _ => continue,
        };
        f(name_span.extract(source));
    }
}

/// Visit every reference-bearing attribute expression of `element`, **including
/// the positions `each_attribute_expression` deliberately skips** — element
/// `{...spread}`, every directive expression, and `{@attach}`.
///
/// This is the traversal for a **dropped** fragment (a `{:catch}` branch), whose
/// contents the emitter discards without walking. There the emission refusal that
/// `each_attribute_expression`'s exclusions rely on never fires, so the analyses
/// must count these references themselves to match the oracle, which counts every
/// reference inside a dropped `{:catch}` (a snippet whose only instance-binding
/// reference sits in such a position must **not** hoist; a `new`/prop-rooted
/// access there must still trigger the `$$renderer.component` wrapper). On the
/// non-dropped (emitted) path these positions still refuse at emission, so the
/// default `each_attribute_expression` remains correct there.
///
/// A directive's **name** (`use:`/`transition:`/`in:`/`out:`/`animate:`) is also a
/// binding reference the oracle counts, but tsv stores it as a verbatim name span
/// rather than an `Expression`, so an expression traversal cannot reach it — it is
/// surfaced separately by `each_reference_bearing_directive_name`.
///
/// `LetDirective` binds a name (a slot prop) and contributes no free reference, so
/// it is excluded.
///
/// Takes `&[AttributeNode]` (not `&Element`) so it also serves a special element's
/// attributes on the dropped path.
pub(crate) fn each_reference_bearing_attribute_expression<'a, 'arena>(
    attributes: &'a [AttributeNode<'arena>],
    f: &mut impl FnMut(&'a Expression<'arena>),
) {
    for attr_node in attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                if let Some(values) = attr.value {
                    for value in values {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
            }
            AttributeNode::SpreadAttribute(spread) => f(&spread.expression),
            AttributeNode::AttachTag(attach) => f(&attach.expression),
            AttributeNode::OnDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::BindDirective(d) => f(&d.expression),
            AttributeNode::ClassDirective(d) => f(&d.expression),
            AttributeNode::StyleDirective(d) => match &d.value {
                StyleDirectiveValue::ExpressionTag(tag) => f(&tag.expression),
                StyleDirectiveValue::Parts(parts) => {
                    for value in *parts {
                        if let AttributeValue::ExpressionTag(tag) = value {
                            f(&tag.expression);
                        }
                    }
                }
                StyleDirectiveValue::True => {}
            },
            AttributeNode::UseDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::TransitionDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::AnimateDirective(d) => {
                if let Some(expr) = &d.expression {
                    f(expr);
                }
            }
            AttributeNode::LetDirective(_) => {}
        }
    }
}

/// Visit the NAME of every directive whose name is a value-binding reference:
/// `use:` (action), `transition:`/`in:`/`out:` (transition fn), `animate:`
/// (animation fn), and a `style:` **shorthand** (`style:color` ≡
/// `style:color={color}` — the name references the same-named binding). These name
/// a binding the oracle counts; the other directives' names are event /
/// DOM-property / class / CSS names (not references) and `let:` binds a name. tsv
/// stores the name as a verbatim `name_span` rather than an `Expression` — and it
/// may be a member path (`use:a.b`) — so the raw slice is surfaced for the
/// consumer to reduce to its root identifier.
///
/// `style:` is the one directive whose shorthand does NOT auto-generate an
/// `expression` node (`bind:`/`class:` shorthands do, so their reference already
/// flows through `each_reference_bearing_attribute_expression`); a non-shorthand
/// `style:color={…}` / `style:color="a{…}"` names a CSS property (not a reference)
/// and its expression is handled there too — so only the `True` (shorthand) arm
/// contributes a name here.
///
/// Consumed only by the snippet-hoist analysis on the dropped-`{:catch}` path: a
/// bare name reference never triggers `needs_context` (which fires on `new` /
/// member-call roots only), and on the emitted path these directives refuse.
pub(crate) fn each_reference_bearing_directive_name<'s>(
    attributes: &[AttributeNode<'_>],
    source: &'s str,
    f: &mut impl FnMut(&'s str),
) {
    for attr_node in attributes {
        let name_span = match attr_node {
            AttributeNode::UseDirective(d) => d.name_span,
            AttributeNode::TransitionDirective(d) => d.name_span,
            AttributeNode::AnimateDirective(d) => d.name_span,
            AttributeNode::StyleDirective(d) if matches!(d.value, StyleDirectiveValue::True) => {
                d.name_span
            }
            _ => continue,
        };
        f(name_span.extract(source));
    }
}

/// The `this={…}` expression a special element carries as a binding reference:
/// `<svelte:element this={tag}>` and `<svelte:component this={Component}>`. The
/// other special-element kinds carry no such expression (their references live in
/// attributes / children). Used only on the dropped-`{:catch}` path — every
/// special element is refused at emission elsewhere.
pub(crate) fn special_element_reference_expression<'a, 'arena>(
    se: &'a SpecialElement<'arena>,
) -> Option<&'a Expression<'arena>> {
    match &se.kind {
        SpecialElementKind::SvelteElement { tag } => Some(tag),
        SpecialElementKind::SvelteComponent { expression } => Some(expression),
        _ => None,
    }
}
