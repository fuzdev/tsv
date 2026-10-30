//! The member-chain argument share: a per-scope `(AST node address, builder tag) →
//! recorded build ([`SharedBuild`])` table the TS printer consults so a chain's `conditional_group`
//! candidates build each argument subtree once (the O(4^depth) rebuild fix), parked in
//! the [`DocArena`](super::arena::DocArena) so its capacity outlives the scope.
//!
//! **Two tiers, because the table is written far more than it is read.** One format of
//! the real-code TS corpus opens ~65 K share scopes and records ~96 K entries in them;
//! **4 %** of its ~100 K lookups hit, so 96 % of the recorded docs are never read again.
//! Two thirds of the scopes hold one to four entries, a third hold none, and ~120 hold
//! more than sixteen. A `HashMap` priced every one of those inserts at a hash, a
//! control-group probe and a bucket write — latency-bound, at 1.7× the pass's mean cost
//! per instruction — and, its capacity having grown past 200 in the deep scopes and been
//! retained by design, every exit `clear()` at a memset of the whole control array. So
//! the common scope lives in a **linear tier**: a `Vec` of entries scanned by key, whose
//! insert is a push and whose clear is a length store; the **hashed tier** takes over on
//! the seventeenth insert of one scope, where a key scan would cost what the probe did.
//! Both tiers keep their capacity across scopes and across
//! [`DocArena::reset`](super::arena::DocArena::reset); a scope's entries never outlive it
//! (the consumer clears at entry and exit).
//!
//! Only ever affects allocation and time, never output: a hit is byte-identical to a
//! rebuild by construction of the consumer's key, and the store is never iterated. A hit
//! skips the build, so an entry also carries what the build did to the consumer's state
//! beyond its doc ([`SharedBuild`]), and the consumer replays that on a hit.

use crate::hash::FxHashMap;

use super::arena::DocId;

/// A share key: the AST node's address (stable — the AST arena is immutable while the
/// printer runs) and the consumer's builder tag, which names the builder asking and the
/// printer state it asks under, so a hit is byte-identical to a rebuild.
pub type ShareKey = (usize, u8);

/// Entries one scope may hold in the linear tier before the hashed tier takes over —
/// past this a key scan costs what the hash probe it replaces did. Sixteen covers all
/// but ~0.2 % of scopes on real TS code (see the module docs).
const LINEAR_CAPACITY: usize = 16;

/// One recorded build: its doc, and the side effect a hit replays because skipping the
/// build would otherwise drop it.
///
/// `deferred_runs` is how far the build raised the TS printer's deferred-run count — the
/// trailing runs it deferred out of stripped grouping pairs and left to an enclosing list
/// element to flush. Saturated at `u16::MAX`, which keeps an entry at sixteen bytes: the
/// count's readers ask only whether it moved, never by how much.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedBuild {
    /// The built doc a hit returns.
    pub doc: DocId,
    /// The deferred runs the build added, which a hit adds again.
    pub deferred_runs: u16,
}

/// A linear-tier entry: the key and the [`SharedBuild`], flattened so the replayed count
/// rides in the padding the key and doc leave.
#[derive(Clone, Copy)]
struct ShareEntry {
    node: usize,
    doc: DocId,
    deferred_runs: u16,
    tag: u8,
}

// rustc's layout, at zero slack, so a test-build bound: a toolchain that lays it out wider
// fails `cargo test`, never a consumer's build
#[cfg(test)]
const _: () = assert!(size_of::<ShareEntry>() <= 16);

impl ShareEntry {
    #[inline]
    fn build(self) -> SharedBuild {
        SharedBuild {
            doc: self.doc,
            deferred_runs: self.deferred_runs,
        }
    }
}

/// The two-tier share table (see the module docs).
#[derive(Default)]
pub struct ChainShareStore {
    /// The linear tier: a scope's first [`LINEAR_CAPACITY`] entries, scanned by key.
    linear: Vec<ShareEntry>,
    /// The hashed tier, populated only once a scope's entry past [`LINEAR_CAPACITY`]
    /// arrives ([`Self::promote`]); while it is empty the linear tier is the table.
    hashed: FxHashMap<ShareKey, SharedBuild>,
}

impl ChainShareStore {
    /// The build recorded for `key` in the current scope, if any.
    #[inline]
    pub fn get(&self, key: ShareKey) -> Option<SharedBuild> {
        if self.hashed.is_empty() {
            self.linear
                .iter()
                .find(|e| e.node == key.0 && e.tag == key.1)
                .map(|e| e.build())
        } else {
            self.hashed.get(&key).copied()
        }
    }

    /// Record `build` for `key`. The consumer records only after a miss, so a key is never
    /// recorded twice within a scope.
    #[inline]
    pub fn insert(&mut self, key: ShareKey, build: SharedBuild) {
        if self.hashed.is_empty() {
            if self.linear.len() < LINEAR_CAPACITY {
                self.linear.push(ShareEntry {
                    node: key.0,
                    doc: build.doc,
                    deferred_runs: build.deferred_runs,
                    tag: key.1,
                });
                return;
            }
            self.promote();
        }
        self.hashed.insert(key, build);
    }

    /// Hand the scope over to the hashed tier: move the linear entries across. Out of
    /// line — it runs at most once per scope, and only in the deep ones.
    #[cold]
    #[inline(never)]
    fn promote(&mut self) {
        self.hashed
            .extend(self.linear.drain(..).map(|e| ((e.node, e.tag), e.build())));
    }

    /// Forget every entry, keeping both tiers' capacity. A length store, plus the
    /// hashed tier's own empty-table short-circuit, unless the scope reached that tier.
    #[inline]
    pub fn clear(&mut self) {
        self.linear.clear();
        self.hashed.clear();
    }

    /// Whether the current scope has reached the hashed tier.
    #[cfg(test)]
    fn is_hashed(&self) -> bool {
        !self.hashed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::super::arena::DocArena;
    use super::*;

    /// `n` distinct builds — each `indent` mints a fresh node, and each build records a
    /// distinct deferred-run count, so a lookup returning the wrong entry's count fails too.
    fn docs(arena: &DocArena, n: usize) -> Vec<SharedBuild> {
        let mut out = Vec::with_capacity(n);
        let mut cur = arena.text("x");
        for i in 0..n {
            cur = arena.indent(cur);
            out.push(SharedBuild {
                doc: cur,
                deferred_runs: u16::try_from(i).unwrap(),
            });
        }
        out
    }

    #[test]
    fn linear_tier_records_and_finds_by_node_and_tag() {
        let arena = DocArena::new();
        let ids = docs(&arena, 4);
        let mut store = ChainShareStore::default();
        assert_eq!(store.get((10, 1)), None);
        store.insert((10, 1), ids[0]);
        store.insert((10, 2), ids[1]);
        store.insert((20, 1), ids[2]);
        assert_eq!(store.get((10, 1)), Some(ids[0]));
        assert_eq!(store.get((10, 2)), Some(ids[1]));
        assert_eq!(store.get((20, 1)), Some(ids[2]));
        assert_eq!(store.get((20, 2)), None);
        assert_eq!(store.get((30, 1)), None);
        assert!(!store.is_hashed());
    }

    #[test]
    fn seventeenth_entry_promotes_and_keeps_every_earlier_one() {
        let arena = DocArena::new();
        let ids = docs(&arena, LINEAR_CAPACITY + 4);
        let mut store = ChainShareStore::default();
        for (i, &id) in ids.iter().enumerate() {
            store.insert((i * 8, 1), id);
            assert_eq!(store.is_hashed(), i >= LINEAR_CAPACITY, "entry {i}");
        }
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(store.get((i * 8, 1)), Some(id), "entry {i}");
        }
        assert_eq!(store.get((0, 2)), None);
    }

    #[test]
    fn clear_forgets_both_tiers_and_the_next_scope_starts_linear() {
        let arena = DocArena::new();
        let ids = docs(&arena, LINEAR_CAPACITY + 1);
        let mut store = ChainShareStore::default();
        for (i, &id) in ids.iter().enumerate() {
            store.insert((i, 1), id);
        }
        assert!(store.is_hashed());
        store.clear();
        assert!(!store.is_hashed());
        assert_eq!(store.get((0, 1)), None);
        store.insert((0, 1), ids[1]);
        assert!(!store.is_hashed());
        assert_eq!(store.get((0, 1)), Some(ids[1]));
        store.clear();
        assert_eq!(store.get((0, 1)), None);
    }
}
