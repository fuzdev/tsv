//! Dense integer identities for the checker's side tables.
//!
//! `NodeId` is a program-dense pre-order index over the AST nodes the binder
//! addresses; `FileId` indexes the per-program file set. Both are `u32`-width
//! newtypes — the checker's struct-of-arrays columns are `Vec`s indexed by
//! these, so an id is a plain array offset. tsgo keys the same facts through
//! global integer ids into flat `nodeLinks`/`symbolLinks` arrays; the deviation
//! is that we assign the ids **eagerly** in the bind walk rather than lazily on
//! first touch (unobservable, and it makes every column dense from the start).
//!
//! Distinct newtypes make cross-index bugs uncompilable (tsgo uses raw
//! `uint32`s/pointers and relies on review) — a `NodeId` can never be used where
//! a `FileId` is expected.
//
// tsgo: internal/ast/ids.go (NodeId/SymbolId are global atomic counters)

use std::num::NonZeroU32;

/// A dense, pre-order node identity assigned by the binder walk.
///
/// Ids start at 1 so `Option<NodeId>` niche-packs into 4 bytes — a `None` parent
/// for the root costs no discriminant, the sentinel idiom without a magic
/// `u32::MAX`. Convert to a 0-based column index with [`NodeId::index`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    /// The first node id assigned in a pre-order walk (the program root).
    pub const FIRST: NodeId = NodeId(NonZeroU32::MIN);

    /// Build a `NodeId` from a 0-based dense index (`index + 1`).
    ///
    /// Total by construction: real ASTs never approach `u32::MAX` nodes, but a
    /// wrap is clamped to [`NodeId::FIRST`] rather than panicking — the crate's
    /// `unwrap_used` / `panic` clippy lints are warn-level, so this stays
    /// panic-free by construction rather than by an `#[allow]` at a fallible call.
    #[inline]
    #[must_use]
    pub fn from_index(index: usize) -> NodeId {
        let raw = (index as u32).wrapping_add(1);
        match NonZeroU32::new(raw) {
            Some(n) => NodeId(n),
            None => NodeId::FIRST,
        }
    }

    /// The 0-based column index this id addresses (`id - 1`).
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    /// The raw 1-based id value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// A `NodeId` from a raw 1-based value, or `None` for 0 (the "no node"
    /// sentinel the flow graph's `subject` column uses).
    #[inline]
    #[must_use]
    pub fn from_raw_opt(raw: u32) -> Option<NodeId> {
        NonZeroU32::new(raw).map(NodeId)
    }
}

/// A dense, per-file flow-node identity assigned by the flow-graph walk
/// ([`crate::binder`]'s third walk).
///
/// Ids start at 1 (`Option<FlowNodeId>` niche-packs into 4 bytes — the same
/// idiom as [`NodeId`], the sentinel without a magic `u32::MAX`). The per-file
/// `unreachableFlow` singleton is minted first, so it is **id 1 by
/// construction** — tsgo's pointer-identity unreachable test becomes id
/// equality against [`FlowNodeId::UNREACHABLE`].
//
// tsgo: internal/ast/ids.go / internal/binder/binder.go (flowNodeArena — a
//       per-file bump arena; ours is a per-file dense id space)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FlowNodeId(NonZeroU32);

impl FlowNodeId {
    /// The per-file `unreachableFlow` singleton — minted first, so id 1
    /// (binder.go:126). Pointer identity in tsgo becomes id equality here.
    pub const UNREACHABLE: FlowNodeId = FlowNodeId(NonZeroU32::MIN);

    /// Build a `FlowNodeId` from a 0-based dense index (`index + 1`).
    ///
    /// Total by construction (a wrap clamps to [`FlowNodeId::UNREACHABLE`]
    /// rather than panicking — the crate's `panic`/`unwrap_used` lints are
    /// warn-level, so this stays panic-free without an `#[allow]`).
    #[inline]
    #[must_use]
    pub fn from_index(index: usize) -> FlowNodeId {
        let raw = (index as u32).wrapping_add(1);
        match NonZeroU32::new(raw) {
            Some(n) => FlowNodeId(n),
            None => FlowNodeId::UNREACHABLE,
        }
    }

    /// Build a `FlowNodeId` from a raw 1-based value, or `None` for 0 (the
    /// "no antecedent" / "no subject" sentinel in the SoA columns).
    #[inline]
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<FlowNodeId> {
        NonZeroU32::new(raw).map(FlowNodeId)
    }

    /// The 0-based column index this id addresses (`id - 1`).
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    /// The raw 1-based id value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// A dense per-program file identity (0-based). Single-file callers use
/// [`FileId::ROOT`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct FileId(pub u32);

impl FileId {
    /// The first file in a program (the single unit of a single-file test).
    pub const ROOT: FileId = FileId(0);

    /// The 0-based column index this id addresses.
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_index_round_trips() {
        for i in [0usize, 1, 2, 41, 1000, 100_000] {
            let id = NodeId::from_index(i);
            assert_eq!(id.index(), i);
            assert_eq!(id.get(), i as u32 + 1);
        }
    }

    #[test]
    fn first_id_is_one() {
        assert_eq!(NodeId::FIRST.get(), 1);
        assert_eq!(NodeId::from_index(0), NodeId::FIRST);
    }

    #[test]
    fn option_node_id_is_four_bytes() {
        // The niche is the whole point of starting ids at 1.
        assert_eq!(size_of::<Option<NodeId>>(), 4);
    }

    #[test]
    fn file_id_root_and_index() {
        assert_eq!(FileId::ROOT, FileId(0));
        assert_eq!(FileId(3).index(), 3);
    }
}
