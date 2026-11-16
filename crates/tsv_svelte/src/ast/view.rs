//! How a document's readers reach what its stores hold.
//!
//! One [`SvelteView`] serves the whole document: the template's lists and the strings it keeps
//! as text of its own are read through its accessors, the embedded TypeScript through the
//! TypeScript view it hands out ([`SvelteView::ts`]) and the top-level `<style>`'s stylesheet
//! through the CSS view ([`SvelteView::css`]). The view is one word, so a walk holds one
//! register for it.
//!
//! A list is a `Run` over the store of its element type in the own half, read as a slice
//! through the accessor the stores' macro declares beside them (`nodes`, `attributes`,
//! `attribute_values`, `modifiers`, `wire_params`, and [`SvelteView::opt_attribute_values`]
//! for an attribute's optional value); a string is a `PoolText<SvelteText>` in the own half's
//! text pool, read through [`SvelteView::text`].

use tsv_css::ast::CssView;
use tsv_lang::store::{PoolText, Run};
use tsv_ts::ast::AstView;

use super::stores::SvelteRuns;
use super::{AttributeValue, ParsedRoot, SvelteStores, SvelteText};

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

    /// The template's list stores, for the accessors their macro declares beside them.
    #[inline]
    pub(super) fn runs(self) -> &'a SvelteRuns {
        &self.stores.own.runs
    }

    /// The elements of `list`, when it is present: an attribute's value parts.
    #[inline]
    pub fn opt_attribute_values(
        self,
        list: Option<Run<AttributeValue>>,
    ) -> Option<&'a [AttributeValue]> {
        list.map(|list| self.attribute_values(list))
    }

    /// The text `text` names: a string the template keeps as its own. A miss ends in the
    /// pool's shared cold path.
    #[inline]
    pub fn text(self, text: PoolText<SvelteText>) -> &'a str {
        self.stores.own.text.get(text)
    }
}

impl SvelteStores {
    /// The view reading these stores.
    #[inline]
    pub fn view(&self) -> SvelteView<'_> {
        SvelteView { stores: self }
    }
}

impl ParsedRoot {
    /// The view reading this tree's stores.
    #[inline]
    pub fn view(&self) -> SvelteView<'_> {
        self.stores.view()
    }
}
