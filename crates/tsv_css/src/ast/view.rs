//! How a stylesheet's readers reach what its stores hold.
//!
//! One [`CssView`] serves the whole tree: every list the tree holds (the stylesheet's
//! top-level comments aside, a `Vec` read directly) and every string it keeps as text of its
//! own is read through the view. The view is one word, so a walk holds one register for it.
//!
//! A list is a `Run` over the store of its element type, read as a slice through the accessor
//! the stores' macro declares beside them (`nodes`, `block_children`, `complex_selectors`,
//! `relative_selectors`, `simple_selectors`, `part_spans`, `values`, `condition_parts`,
//! `condition_segments`); a string is a `PoolText<CssText>` in the stores' text pool, read
//! through [`CssView::text`].

use tsv_lang::store::PoolText;

use super::stores::RunStores;
use super::{CssStores, CssStyleSheet, CssText, ParsedCss};

/// The read access every consumer of the tree holds: resolves the tree's handles.
///
/// `Copy`, and bound to the stores of the stylesheet whose tree it reads.
#[derive(Clone, Copy)]
pub struct CssView<'a> {
    stores: &'a CssStores,
}

// One word, in the check build too: a walk's frame and registers are what it pays for.
#[cfg(test)]
const _: () = assert!(size_of::<CssView<'static>>() == size_of::<usize>());

impl<'a> CssView<'a> {
    /// The text `text` names: a decoded or normalized string of the tree. A miss ends in the
    /// pool's shared cold path.
    ///
    /// Inline: a declaration's property and an at-rule's name are text, so this read is on the
    /// path of every declaration and at-rule, where an out-of-line call would add a call and
    /// its register saves to each.
    #[inline]
    pub fn text(self, text: PoolText<CssText>) -> &'a str {
        self.stores.text.get(text)
    }

    /// The list stores, for the accessors their macro declares beside them.
    #[inline]
    pub(super) fn runs(self) -> &'a RunStores {
        &self.stores.runs
    }
}

impl CssStores {
    /// The view reading these stores.
    #[inline]
    pub fn view(&self) -> CssView<'_> {
        CssView { stores: self }
    }
}

impl ParsedCss {
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
    pub stylesheet: &'a CssStyleSheet,
    /// The view over the stores the tree's handles index.
    pub view: CssView<'a>,
}

impl<'a> From<&'a ParsedCss> for CssParsedRef<'a> {
    #[inline]
    fn from(parsed: &'a ParsedCss) -> Self {
        CssParsedRef {
            stylesheet: &parsed.stylesheet,
            view: parsed.view(),
        }
    }
}
