//! How a stylesheet's readers reach what its stores hold.
//!
//! One [`CssView`] serves the whole tree: every list the tree holds and every string it keeps
//! as text of its own is read through the view. The view is one word, so a walk holds one
//! register for it.
//!
//! The tree still holds its lists as slices and its strings as `str`s in the caller's arena,
//! spelled with the aliases here, each one's accessor returning its argument; the aliases
//! name what the stores will hold in their place — a `Run` over a store of the element type,
//! and a `PoolText<CssText>` in the stores' text pool.

use super::{
    ComplexSelector, ConditionPart, ConditionSegment, CssBlockChild, CssNode, CssStores,
    CssStyleSheet, CssValue, ParsedCss, RelativeSelector, SimpleSelector, Span,
};

/// A stylesheet's top-level rules and at-rules, read through [`CssView::nodes`].
// TODO: `Run<CssNode>`.
pub type CssNodeRun<'a> = &'a [CssNode<'a>];

/// The children of a block: a rule's declarations, an at-rule block's children, read through
/// [`CssView::block_children`].
// TODO: `Run<CssBlockChild>`.
pub type CssBlockChildRun<'a> = &'a [CssBlockChild<'a>];

/// A selector list's complex selectors, read through [`CssView::complex_selectors`].
// TODO: `Run<ComplexSelector>`.
pub type ComplexSelectorRun<'a> = &'a [ComplexSelector<'a>];

/// A complex selector's relative selectors, read through [`CssView::relative_selectors`].
// TODO: `Run<RelativeSelector>`.
pub type RelativeSelectorRun<'a> = &'a [RelativeSelector<'a>];

/// A relative selector's compound, its simple selectors, read through
/// [`CssView::simple_selectors`]. Empty for a combinator with no compound after it.
// TODO: `Run<SimpleSelector>`.
pub type SimpleSelectorRun<'a> = &'a [SimpleSelector<'a>];

/// The spans of a `::part()` argument's names, read through [`CssView::part_spans`].
// TODO: `Run<Span>`.
pub type SpanRun<'a> = &'a [Span];

/// The members of a value list — a function's arguments, a space- or comma-separated
/// list's members, an `@import` prelude's values — read through [`CssView::values`].
// TODO: `Run<CssValue>`.
pub type CssValueRun<'a> = &'a [CssValue<'a>];

/// A condition query's parts, read through [`CssView::condition_parts`].
// TODO: `Run<ConditionPart>`.
pub type ConditionPartRun<'a> = &'a [ConditionPart<'a>];

/// A condition part's segments, read through [`CssView::condition_segments`].
// TODO: `Run<ConditionSegment>`.
pub type ConditionSegmentRun<'a> = &'a [ConditionSegment<'a>];

/// One string the tree keeps as text of its own rather than as a span — a decoded name or
/// string, a normalized prelude or condition text, a copied An+B or attribute flag — read
/// through [`CssView::text`].
// TODO: `PoolText<CssText>`.
pub type PoolStr<'a> = &'a str;

/// The read access every consumer of the tree holds: resolves the tree's handles.
///
/// `Copy`, and bound to the stores of the stylesheet whose tree it reads.
#[derive(Clone, Copy)]
pub struct CssView<'a> {
    #[expect(
        dead_code,
        reason = "the tree's lists and strings live in the arena, so every accessor returns its \
                  argument"
    )]
    stores: &'a CssStores,
}

// One word, in the check build too: a walk's frame and registers are what it pays for.
#[cfg(test)]
const _: () = assert!(size_of::<CssView<'static>>() == size_of::<usize>());

impl<'a> CssView<'a> {
    /// The elements of `list`: a stylesheet's top-level rules and at-rules.
    #[inline]
    pub fn nodes(self, list: CssNodeRun<'a>) -> &'a [CssNode<'a>] {
        list
    }

    /// The elements of `list`: a rule's declarations or an at-rule block's children.
    #[inline]
    pub fn block_children(self, list: CssBlockChildRun<'a>) -> &'a [CssBlockChild<'a>] {
        list
    }

    /// The elements of `list`: a selector list's complex selectors.
    #[inline]
    pub fn complex_selectors(self, list: ComplexSelectorRun<'a>) -> &'a [ComplexSelector<'a>] {
        list
    }

    /// The elements of `list`: a complex selector's relative selectors.
    #[inline]
    pub fn relative_selectors(self, list: RelativeSelectorRun<'a>) -> &'a [RelativeSelector<'a>] {
        list
    }

    /// The elements of `list`: a relative selector's simple selectors.
    #[inline]
    pub fn simple_selectors(self, list: SimpleSelectorRun<'a>) -> &'a [SimpleSelector<'a>] {
        list
    }

    /// The elements of `list`: a `::part()` argument's name spans.
    #[inline]
    pub fn part_spans(self, list: SpanRun<'a>) -> &'a [Span] {
        list
    }

    /// The elements of `list`: a value list's members.
    #[inline]
    pub fn values(self, list: CssValueRun<'a>) -> &'a [CssValue<'a>] {
        list
    }

    /// The elements of `list`: a condition query's parts.
    #[inline]
    pub fn condition_parts(self, list: ConditionPartRun<'a>) -> &'a [ConditionPart<'a>] {
        list
    }

    /// The elements of `list`: a condition part's segments.
    #[inline]
    pub fn condition_segments(self, list: ConditionSegmentRun<'a>) -> &'a [ConditionSegment<'a>] {
        list
    }

    /// The text `text` names: a decoded or normalized string of the tree.
    ///
    /// Inline: a declaration's property and an at-rule's name are text, so this read is on the
    /// path of every declaration and at-rule, where an out-of-line call would add a call and
    /// its register saves to each.
    #[inline]
    pub fn text(self, text: PoolStr<'a>) -> &'a str {
        text
    }
}

impl CssStores {
    /// The view reading these stores.
    #[inline]
    pub fn view(&self) -> CssView<'_> {
        CssView { stores: self }
    }
}

impl ParsedCss<'_> {
    /// The view reading this tree's stores.
    #[inline]
    pub fn view(&self) -> CssView<'_> {
        self.stores.view()
    }
}

/// A stylesheet as a reading entry point takes it: the tree and the view its handles resolve
/// through.
#[derive(Clone, Copy)]
pub struct CssParsedRef<'a> {
    /// The tree.
    pub stylesheet: &'a CssStyleSheet<'a>,
    /// The view over the stores the tree's handles index.
    pub view: CssView<'a>,
}

impl<'a, 'arena: 'a> From<&'a ParsedCss<'arena>> for CssParsedRef<'a> {
    #[inline]
    fn from(parsed: &'a ParsedCss<'arena>) -> Self {
        CssParsedRef {
            stylesheet: &parsed.stylesheet,
            view: parsed.view(),
        }
    }
}
