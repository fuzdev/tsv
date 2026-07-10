//! A hand-rolled Fx-style multiply-xor hasher for the checker's integer-keyed
//! maps.
//!
//! std's SipHash is DoS-resistant but slow for the small integer keys (node
//! ids, symbol ids, arena addresses) the checker hashes on its hot paths. This
//! is the well-known Fx fold — rotate, xor the next word, multiply by a fixed
//! odd constant — re-implemented in ~20 lines with no external dependency (the
//! crate-private infrastructure the design calls for). It is deliberately
//! **not** DoS-resistant: these maps are fed program-internal ids and addresses,
//! never adversarial network input. The fold consumes fixed 8-byte little-endian
//! words, so a 32-bit wasm target and a 64-bit native target produce identical
//! hashes.
//
// tsgo uses xxh3-128 for variable-arity list hashing; the Fx fold is tsv's
// hand-rolled substitute here (no new dependency).

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// The Fx seed: a fixed odd 64-bit constant (the fractional bits of the golden
/// ratio — the constant rustc-hash uses).
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// The per-step left rotation.
const ROTATE: u32 = 5;

/// A fast, non-cryptographic hasher over the Fx multiply-xor fold.
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    /// Fold one 64-bit word into the running hash.
    #[inline]
    fn fold(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(ROTATE) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            // `chunks_exact(8)` yields slices of exactly 8 bytes, so the array
            // conversion always succeeds; the fallback keeps this total.
            let word = u64::from_le_bytes(<[u8; 8]>::try_from(chunk).unwrap_or([0; 8]));
            self.fold(word);
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.fold(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.fold(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.fold(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.fold(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.fold(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.fold(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// The `BuildHasher` for [`FxHasher`] (zero-state, so `Default`-constructible).
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// A `HashMap` keyed through the Fx fold.
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// A `HashSet` keyed through the Fx fold. Part of the map/set alias pair; the
/// binder uses `FxHashMap` today, and scope-membership sets will use this.
#[allow(dead_code)]
pub type FxHashSet<K> = HashSet<K, FxBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    /// Re-derive one word's fold by hand — pins the algorithm (rotate-xor-mul by
    /// `SEED`), not just self-consistency.
    #[test]
    fn write_u32_matches_manual_fold() {
        let mut h = FxHasher::default();
        h.write_u32(0xdead_beef);
        let expected = (0u64.rotate_left(ROTATE) ^ 0xdead_beef_u64).wrapping_mul(SEED);
        assert_eq!(h.finish(), expected);
    }

    /// Two folds compose in order (the running hash is carried, not reset).
    #[test]
    fn two_writes_fold_in_order() {
        let mut h = FxHasher::default();
        h.write_u32(1);
        h.write_u32(2);
        let s1 = (0u64.rotate_left(ROTATE) ^ 1).wrapping_mul(SEED);
        let s2 = (s1.rotate_left(ROTATE) ^ 2).wrapping_mul(SEED);
        assert_eq!(h.finish(), s2);
    }

    /// `write` folds full 8-byte words then a zero-padded tail.
    #[test]
    fn write_bytes_chunks_then_tail() {
        let mut h = FxHasher::default();
        h.write(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let w1 = u64::from_le_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
        let w2 = u64::from_le_bytes([9, 10, 11, 12, 0, 0, 0, 0]);
        let s1 = (0u64.rotate_left(ROTATE) ^ w1).wrapping_mul(SEED);
        let s2 = (s1.rotate_left(ROTATE) ^ w2).wrapping_mul(SEED);
        assert_eq!(h.finish(), s2);
    }

    /// A single scalar hashes the same through the derived `Hash` impl.
    #[test]
    fn hash_trait_routes_through_fold() {
        let mut h = FxHasher::default();
        0x0063_u32.hash(&mut h);
        let expected = (0u64.rotate_left(ROTATE) ^ 0x0063_u64).wrapping_mul(SEED);
        assert_eq!(h.finish(), expected);
    }

    #[test]
    fn map_and_set_round_trip() {
        let mut m: FxHashMap<u32, u32> = FxHashMap::default();
        m.insert(7, 70);
        m.insert(9, 90);
        assert_eq!(m.get(&7), Some(&70));
        assert_eq!(m.get(&9), Some(&90));
        assert_eq!(m.get(&8), None);

        let mut s: FxHashSet<usize> = FxHashSet::default();
        s.insert(0xabcd);
        assert!(s.contains(&0xabcd));
        assert!(!s.contains(&0x1234));
    }
}
