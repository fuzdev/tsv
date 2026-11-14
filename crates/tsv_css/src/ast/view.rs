//! How a stylesheet's readers reach what its stores hold.
//!
//! One [`CssView`] serves the whole tree: a string the stores hold is a `PoolText<CssText>`,
//! and every read of one goes through the view. The view is one word, so a walk holds one
//! register for it.

use tsv_lang::store::PoolText;

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
    #[inline]
    pub fn text(self, text: PoolText<CssText>) -> &'a str {
        self.stores.text.get(text)
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
