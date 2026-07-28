//! Dep-free hashing for the side tables the printers, the wire writer, and the
//! checker keep.
//!
//! `std`'s default `HashMap` hasher is SipHash-1-3 — a *keyed* hash chosen to
//! make collision attacks on untrusted input infeasible. None of tsv's tables
//! take untrusted keys in a way that matters: they are keyed by an AST-node
//! pointer, a source-span offset, a `(start, end)` pair, or an interned name
//! from the source text — all derived from a file the caller already chose to
//! parse, and each table lives for one document. Paying SipHash's mixing for
//! that is pure overhead — it showed up on the format board as ~0.5% of
//! `fuz_app` cycles.
//!
//! **Substituting the hasher is byte-identical by construction, but only while
//! every consumer stays order-free.** A `HashMap`'s *iteration order* depends
//! on the hasher; its `get`/`insert`/`contains`/`clear` semantics do not. Nearly
//! every table in the tree is used only through the latter set; the exceptions
//! each neutralize the order at the iteration site —
//! `WriterComments::debug_assert_consumed` is an order-independent `all`, and
//! `tsv_check`'s two iterating tables sort before consuming, each on a *total*
//! order so no tie can fall back to the map's (its symbol tables by
//! first-declaration span then name, its flow-label scratch by `FlowNodeId`). ⚠️ A
//! future consumer that iterates one of these maps and lets the order reach the
//! output would make the hasher observable — that is the line to watch, not the
//! hash quality.

use std::hash::{BuildHasherDefault, Hasher};

/// The multiply-xor constant (rustc's `FxHasher`): an odd 64-bit value whose
/// bit pattern mixes low bits upward under multiplication.
const SEED: u64 = 0x517c_c1b7_2722_0a95;

/// Multiply-xor hasher — one rotate, one xor, one multiply per word.
///
/// Deliberately **not** collision-resistant. See the module docs for why that
/// is the right trade here, and for the constraint on consumers.
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    /// Live through `tsv_check`'s name-keyed tables — its atom interner and its
    /// merge globals hash `str` keys, which route here. The printer and wire
    /// writer tables are integer-keyed and skip this entirely for the methods
    /// below.
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.add(u64::from_ne_bytes(word));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_ne_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `BuildHasher` for [`FxHasher`] — the `S` parameter of the aliases below.
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// `HashMap` over [`FxHasher`]. Construct with `default()`, not `new()`.
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;

/// `HashSet` over [`FxHasher`]. Construct with `default()`, not `new()`.
pub type FxHashSet<T> = std::collections::HashSet<T, FxBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::Entry;

    /// The property the substitution actually rests on: map *semantics* are
    /// hash-independent. Graded against `std`'s own `HashMap` over the key
    /// shapes tsv uses — node pointers (`usize`), span starts (`u32`), and
    /// `(start, end)` pairs.
    #[test]
    fn map_semantics_match_the_default_hasher() {
        let mut fx: FxHashMap<usize, u32> = FxHashMap::default();
        let mut std_map: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for step in 0..50_000u32 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            // A pointer-shaped key space: 8-byte-aligned, clustered.
            let key = ((state >> 20) as usize & 0xffff) * 8;
            match step % 4 {
                0 => {
                    assert_eq!(fx.insert(key, step), std_map.insert(key, step));
                }
                1 => assert_eq!(fx.get(&key), std_map.get(&key)),
                2 => assert_eq!(fx.remove(&key), std_map.remove(&key)),
                _ => {
                    let a = matches!(fx.entry(key), Entry::Occupied(_));
                    let b = matches!(std_map.entry(key), Entry::Occupied(_));
                    assert_eq!(a, b);
                }
            }
            assert_eq!(fx.len(), std_map.len(), "len diverged at step {step}");
        }
        let mut fx_items: Vec<_> = fx.into_iter().collect();
        let mut std_items: Vec<_> = std_map.into_iter().collect();
        fx_items.sort_unstable();
        std_items.sort_unstable();
        assert_eq!(fx_items, std_items, "contents diverged");
    }

    /// The same differential over `String` keys — the name-keyed shape
    /// `tsv_check`'s atom interner and merge globals introduced. These route
    /// through `Hasher::write` (the byte path) rather than the integer methods,
    /// so it is the one grading that path against `std`'s own map.
    #[test]
    fn string_keyed_map_semantics_match_the_default_hasher() {
        let mut fx: FxHashMap<String, u32> = FxHashMap::default();
        let mut std_map: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut state = 0x853c_49e6_748f_ea9bu64;
        for step in 0..20_000u32 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            // Identifier-shaped keys of varied length, so the byte path sees
            // both whole 8-byte words and every tail length.
            let key = "sym".repeat(1 + (step % 5) as usize) + &((state >> 24) % 4096).to_string();
            match step % 4 {
                0 => {
                    assert_eq!(fx.insert(key.clone(), step), std_map.insert(key, step));
                }
                1 => assert_eq!(fx.get(&key), std_map.get(&key)),
                2 => assert_eq!(fx.remove(&key), std_map.remove(&key)),
                _ => {
                    let a = matches!(fx.entry(key.clone()), Entry::Occupied(_));
                    let b = matches!(std_map.entry(key), Entry::Occupied(_));
                    assert_eq!(a, b);
                }
            }
            assert_eq!(fx.len(), std_map.len(), "len diverged at step {step}");
        }
        let mut fx_items: Vec<_> = fx.into_iter().collect();
        let mut std_items: Vec<_> = std_map.into_iter().collect();
        fx_items.sort_unstable();
        std_items.sort_unstable();
        assert_eq!(fx_items, std_items, "contents diverged");
    }

    #[test]
    fn set_semantics_match_the_default_hasher() {
        let mut fx: FxHashSet<u32> = FxHashSet::default();
        let mut std_set: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for i in 0..20_000u32 {
            let key = i.wrapping_mul(2_654_435_761) % 4096;
            assert_eq!(fx.insert(key), std_set.insert(key));
            assert_eq!(fx.contains(&key), std_set.contains(&key));
        }
        assert_eq!(fx.len(), std_set.len());
        fx.clear();
        std_set.clear();
        assert_eq!(fx.len(), std_set.len());
    }

    #[test]
    fn tuple_keys_are_distinguished() {
        // `(u32, u32)` span keys: the writer's comment map. A hasher that
        // folded the pair into one word would collide `(a, b)` with `(b, a)`.
        let mut fx: FxHashMap<(u32, u32), u32> = FxHashMap::default();
        for a in 0..64u32 {
            for b in 0..64u32 {
                fx.insert((a, b), a * 64 + b);
            }
        }
        assert_eq!(fx.len(), 64 * 64);
        for a in 0..64u32 {
            for b in 0..64u32 {
                assert_eq!(fx.get(&(a, b)), Some(&(a * 64 + b)));
            }
        }
    }

    /// Re-derive one word's fold by hand — pins the algorithm (rotate-xor-mul by
    /// `SEED`), not just self-consistency.
    #[test]
    fn write_u32_matches_manual_fold() {
        let mut h = FxHasher::default();
        h.write_u32(0xdead_beef);
        let expected = (0u64.rotate_left(5) ^ 0xdead_beef_u64).wrapping_mul(SEED);
        assert_eq!(h.finish(), expected);
    }

    /// Two folds compose in order (the running hash is carried, not reset).
    #[test]
    fn two_writes_fold_in_order() {
        let mut h = FxHasher::default();
        h.write_u32(1);
        h.write_u32(2);
        let s1 = (0u64.rotate_left(5) ^ 1).wrapping_mul(SEED);
        let s2 = (s1.rotate_left(5) ^ 2).wrapping_mul(SEED);
        assert_eq!(h.finish(), s2);
    }

    /// `write` folds full 8-byte native-endian words then a zero-padded tail.
    #[test]
    fn write_bytes_chunks_then_tail() {
        let mut h = FxHasher::default();
        h.write(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let w1 = u64::from_ne_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
        let w2 = u64::from_ne_bytes([9, 10, 11, 12, 0, 0, 0, 0]);
        let s1 = (0u64.rotate_left(5) ^ w1).wrapping_mul(SEED);
        let s2 = (s1.rotate_left(5) ^ w2).wrapping_mul(SEED);
        assert_eq!(h.finish(), s2);
    }

    /// A scalar routed through the derived `Hash` impl lands on the same fold —
    /// the path every integer-keyed table below actually takes.
    #[test]
    fn hash_trait_routes_through_fold() {
        use std::hash::Hash;
        let mut h = FxHasher::default();
        0x0063_u32.hash(&mut h);
        let expected = (0u64.rotate_left(5) ^ 0x0063_u64).wrapping_mul(SEED);
        assert_eq!(h.finish(), expected);
    }

    /// `usize` folds as a 64-bit word, so a 32-bit target (the shipped wasm
    /// bundles) and a 64-bit one agree on any key both can express. This leg
    /// holds on either width; the discriminating one is below.
    #[test]
    fn write_usize_folds_as_a_64_bit_word() {
        let mut h = FxHasher::default();
        h.write_usize(0xabcd);
        let mut w = FxHasher::default();
        w.write_u64(0xabcd);
        assert_eq!(h.finish(), w.finish());
    }

    /// The half that actually discriminates: a key above 2^32 must fold as the
    /// whole 64-bit word, never its truncated low half. A small-value test
    /// passes under a truncating impl too, so it proves nothing on its own.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn write_usize_does_not_truncate_above_32_bits() {
        let mut h = FxHasher::default();
        h.write_usize(0x1_0000_0001);
        let mut full = FxHasher::default();
        full.write_u64(0x1_0000_0001);
        assert_eq!(
            h.finish(),
            full.finish(),
            "usize did not fold the full word"
        );

        let mut truncated = FxHasher::default();
        truncated.write_u64(0x0000_0001);
        assert_ne!(
            h.finish(),
            truncated.finish(),
            "usize key truncated to its low 32 bits"
        );
    }

    /// Not a quality bar — just a tripwire against a hasher that degenerates to
    /// a constant, which would silently turn every table into a linked list.
    #[test]
    fn distinct_keys_produce_distinct_hashes() {
        use std::hash::BuildHasher;
        let build = FxBuildHasher::default();
        let mut seen = std::collections::HashSet::new();
        for i in 0..10_000usize {
            seen.insert(build.hash_one(i * 8));
        }
        assert!(
            seen.len() > 9_900,
            "pointer-shaped keys collided {} times",
            10_000 - seen.len()
        );
    }
}
