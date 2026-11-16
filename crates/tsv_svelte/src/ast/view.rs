//! How a document's readers reach what its stores hold.
//!
//! One [`SvelteView`] serves the whole document: the template's lists and the strings it keeps
//! as text of its own are read through its accessors, the embedded TypeScript through the
//! TypeScript view it hands out ([`SvelteView::ts`]) and the top-level `<style>`'s stylesheet
//! through the CSS view ([`SvelteView::css`]). The view is one word, so a walk holds one
//! register for it.
//!
//! The template still holds its lists as slices and its strings as `str`s in the caller's
//! arena, spelled with the aliases here, each one's accessor returning its argument; the
//! aliases name what the stores will hold in their place — a `Run` over a store of the
//! element type, and a pool text.

use tsv_css::ast::CssView;
use tsv_ts::ast::AstView;

use super::{
    AttributeNode, AttributeValue, FragmentNode, ParsedRoot, SnippetWireParameters, Span,
    SvelteStores,
};

/// A fragment's nodes, read through [`SvelteView::nodes`].
// TODO: `Run<FragmentNode>`.
pub type FragmentNodeRun<'a> = &'a [FragmentNode<'a>];

/// An element's, a special element's, `<svelte:options>`'s, a `<script>`'s or a `<style>`'s
/// attributes and directives, read through [`SvelteView::attributes`].
// TODO: `Run<AttributeNode>`.
pub type AttributeNodeRun<'a> = &'a [AttributeNode<'a>];

/// An attribute's or a style directive's value parts, read through
/// [`SvelteView::attribute_values`] (or [`SvelteView::opt_attribute_values`] for an
/// attribute's optional value).
// TODO: `Run<AttributeValue>`.
pub type AttributeValueRun<'a> = &'a [AttributeValue];

/// The spans of a directive's modifiers, read through [`SvelteView::modifiers`].
// TODO: `Run<Span>`.
pub type SpanRun<'a> = &'a [Span];

/// The wire's parameter lists of the `{#snippet}` heads that hold a grouping pair, read
/// through [`SvelteView::wire_params`].
// TODO: `Run<SnippetWireParameters>`.
pub type SnippetWireParametersRun<'a> = &'a [SnippetWireParameters];

/// One string the template keeps as text of its own rather than as a span — a `{#snippet}`
/// head's raw generics — read through [`SvelteView::text`].
// TODO: a pool text.
pub type PoolStr<'a> = &'a str;

/// The read access every consumer of the document holds: resolves the template's lists and
/// strings, and hands out the views of the embedded TypeScript and CSS.
///
/// `Copy`, and bound to the stores of the document whose tree it reads.
#[derive(Clone, Copy)]
pub struct SvelteView<'a> {
    stores: &'a SvelteStores,
}

// One word, in the check build too: a walk's frame and registers are what it pays for.
#[cfg(test)]
const _: () = assert!(size_of::<SvelteView<'static>>() == size_of::<usize>());

impl<'a> SvelteView<'a> {
    /// The view reading the TypeScript half: the document's expressions and programs.
    #[inline]
    pub fn ts(self) -> AstView<'a> {
        self.stores.ts.view()
    }

    /// The view reading the CSS half: the top-level `<style>`'s stylesheet.
    #[inline]
    pub fn css(self) -> CssView<'a> {
        self.stores.css.view()
    }

    /// The elements of `list`: a fragment's nodes.
    #[inline]
    pub fn nodes(self, list: FragmentNodeRun<'a>) -> &'a [FragmentNode<'a>] {
        list
    }

    /// The elements of `list`: an element's or a section's attributes and directives.
    #[inline]
    pub fn attributes(self, list: AttributeNodeRun<'a>) -> &'a [AttributeNode<'a>] {
        list
    }

    /// The elements of `list`: an attribute's or a style directive's value parts.
    #[inline]
    pub fn attribute_values(self, list: AttributeValueRun<'a>) -> &'a [AttributeValue] {
        list
    }

    /// The elements of `list`, when it is present: an attribute's value parts.
    #[inline]
    pub fn opt_attribute_values(
        self,
        list: Option<AttributeValueRun<'a>>,
    ) -> Option<&'a [AttributeValue]> {
        list.map(|list| self.attribute_values(list))
    }

    /// The elements of `list`: a directive's modifier spans.
    #[inline]
    pub fn modifiers(self, list: SpanRun<'a>) -> &'a [Span] {
        list
    }

    /// The elements of `list`: the wire's `{#snippet}` parameter lists.
    #[inline]
    pub fn wire_params(self, list: SnippetWireParametersRun<'a>) -> &'a [SnippetWireParameters] {
        list
    }

    /// The text `text` names: a string the template keeps as its own.
    #[inline]
    pub fn text(self, text: PoolStr<'a>) -> &'a str {
        text
    }
}

impl SvelteStores {
    /// The view reading these stores.
    #[inline]
    pub fn view(&self) -> SvelteView<'_> {
        SvelteView { stores: self }
    }
}

impl ParsedRoot<'_> {
    /// The view reading this tree's stores.
    #[inline]
    pub fn view(&self) -> SvelteView<'_> {
        self.stores.view()
    }
}
