//! The address keys of the printer's fragment memos, typed end to end.
//!
//! The fragment edge index (`Printer::index_fragment_edges`), the glue cache
//! (`Printer::fragment_glue`) and the root's `format-ignore` pairing (`IgnoreRanges`) key a
//! run of fragment nodes by its address in the node store: a run is a slice of the store, so
//! its address names it exactly, and a sub-slice's offset into the fragment it lies in is
//! address arithmetic. But a `FragmentNode` is `Copy`, so a slice over a COPY — a node read by
//! value and handed on through `std::slice::from_ref`, or a run gathered into a `Vec` — has an
//! address of its own on the stack or heap, which the next copy can reuse: a later lookup then
//! hits another run's entry, silently, with no type error to catch it.
//!
//! So a key is a [`NodeSliceAddr`] (or a [`NodeSliceKey`], the address with the run's
//! length), whose field is private to this module: the only way to make one is from a
//! `&[FragmentNode]` ([`NodeSliceAddr::of`], [`NodeSliceKey::of`]), and in debug builds the
//! mint asserts the slice lies in the node store (`SvelteView::holds_node_address`) — the
//! store-range guard the experimental typechecker's address map asserts on its keys.

use crate::ast::{FragmentNode, SvelteView};

/// The address of a run of fragment nodes in the node store: the start of a key the edge
/// index orders by. Stable while the document's stores live, which every memo keyed by it
/// lives within.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(super) struct NodeSliceAddr(usize);

impl NodeSliceAddr {
    /// The address of `nodes`, a slice of the node store — one an accessor returned, or a
    /// sub-slice of it, never one over a copied node (asserted in debug builds; an empty slice
    /// has no node to lie anywhere, so it passes). A plain `debug_assert!` here leaves release
    /// `.text` byte-identical, unlike tsv_ts's `NodeAddr::of`, whose guard is `cfg`-gated.
    #[inline]
    #[track_caller]
    pub(super) fn of(view: SvelteView<'_>, nodes: &[FragmentNode]) -> Self {
        debug_assert!(
            nodes.first().zip(nodes.last()).is_none_or(|(first, last)| {
                view.holds_node_address(std::ptr::from_ref(first).addr())
                    && view.holds_node_address(std::ptr::from_ref(last).addr())
            }),
            "a fragment-node slice outside the node store was keyed by address"
        );
        Self(nodes.as_ptr().addr())
    }

    /// How many nodes `self` lies past `start`, the address of a run it lies in (no earlier).
    #[inline]
    pub(super) const fn nodes_past(self, start: Self) -> usize {
        (self.0 - start.0) / size_of::<FragmentNode>()
    }
}

/// A run of fragment nodes by its address in the node store and its length: two runs that
/// start at one node and stop at different ones are different keys.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct NodeSliceKey {
    addr: NodeSliceAddr,
    len: usize,
}

impl NodeSliceKey {
    /// The key of `nodes`, a slice of the node store ([`NodeSliceAddr::of`]'s guard).
    #[inline]
    #[track_caller]
    pub(super) fn of(view: SvelteView<'_>, nodes: &[FragmentNode]) -> Self {
        Self {
            addr: NodeSliceAddr::of(view, nodes),
            len: nodes.len(),
        }
    }

    /// The run's start address.
    #[inline]
    pub(super) const fn addr(self) -> NodeSliceAddr {
        self.addr
    }
}
