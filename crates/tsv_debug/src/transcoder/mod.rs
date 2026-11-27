//! The tape transcoder: the JSON wire → tape oracle every gate grades a direct tape against.
//!
//! The tape is tsv's binary parse wire, specified in `docs/tape_format.md`; its types, constants
//! and generated schema tables are `tsv_lang::tape` (behind `tsv_lang`'s `_tape` feature). The
//! language crates' wire writers write it directly (their `_tape` tape copies), and the bindings
//! ship those; this module is the second implementation they are graded against.
//!
//! [`Transcoder`] reads the compact JSON one of the writers emitted, with the source it was
//! parsed from, and writes the tape the offline JS encoder (`scripts/tape_encode.ts`) writes from
//! the same wire, byte for byte (`npm run check:tape`) — the canonical refs a direct writer's
//! true-offset refs are compared through (`transcode(tape_to_json(direct)) == transcode(json)`,
//! [`crate::tape::grade_direct_tape`]). It recurses nowhere — an explicit frame stack holds every
//! open object — so any depth the parsers accept transcodes on any stack.

mod transcode;

pub use transcode::{TapeError, Transcoder};

use tsv_lang::tape::shapes;

/// [`TYPE_SLOTS`]' size, a power of two well over the number of distinct `type` values, so a
/// probe sequence stays short.
const TYPE_SLOT_BITS: u32 = 10;
/// An empty [`TYPE_SLOTS`] slot, and [`NEXT_OF_TYPE`]'s end of a run.
const NO_TYPE_ENTRY: u16 = u16::MAX;

/// The first candidate entry of each `type` value, open-addressed by [`type_hash`]: the
/// transcoder's per-object `type` lookup, built from [`shapes::ENTRIES`] at compile time.
static TYPE_SLOTS: [u16; 1 << TYPE_SLOT_BITS] = type_slots();
/// Each entry's next candidate of the same `type`, in table order, or [`NO_TYPE_ENTRY`].
static NEXT_OF_TYPE: [u16; shapes::ENTRIES.len()] = NEXT_OF_TYPE_CONST;
/// How many candidates an entry's `type` has (read at a type's first entry).
static CANDIDATES_OF_TYPE: [u16; shapes::ENTRIES.len()] = build_candidates_of_type();

/// The bytes at `b[at..at + 8]` (zero past the end) as a little-endian word.
const fn word(b: &[u8], at: usize) -> u64 {
    let mut w = 0u64;
    let mut i = 0;
    while i < 8 && at + i < b.len() {
        w |= (b[at + i] as u64) << (8 * i);
        i += 1;
    }
    w
}

/// A `type` value's home slot: its length and first and last eight bytes, mixed.
const fn type_hash(b: &[u8]) -> usize {
    let n = b.len();
    let last = if n > 8 { word(b, n - 8) } else { 0 };
    type_mix(word(b, 0), last, n)
}

/// [`type_hash`]'s mix of a value's first and last words and its length, shared with the
/// runtime lookup, which loads the words directly.
const fn type_mix(first: u64, last: u64, n: usize) -> usize {
    let h = (first ^ last.rotate_left(29) ^ n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (h >> (64 - TYPE_SLOT_BITS)) as usize
}

const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The first entry whose `type` is `ty`, scanning the table from the start.
const fn first_entry_of(ty: &[u8]) -> usize {
    let mut e = 0;
    while e < shapes::ENTRIES.len() {
        if let Some(t) = shapes::ENTRIES[e].ty
            && bytes_eq(t.as_bytes(), ty)
        {
            return e;
        }
        e += 1;
    }
    usize::MAX
}

const fn type_slots() -> [u16; 1 << TYPE_SLOT_BITS] {
    let mut slots = [NO_TYPE_ENTRY; 1 << TYPE_SLOT_BITS];
    let mut e = 0;
    while e < shapes::ENTRIES.len() {
        if let Some(t) = shapes::ENTRIES[e].ty
            && first_entry_of(t.as_bytes()) == e
        {
            let mut slot = type_hash(t.as_bytes());
            while slots[slot] != NO_TYPE_ENTRY {
                slot = (slot + 1) & ((1 << TYPE_SLOT_BITS) - 1);
            }
            slots[slot] = e as u16;
        }
        e += 1;
    }
    slots
}

const fn build_next_of_type() -> [u16; shapes::ENTRIES.len()] {
    let mut next = [NO_TYPE_ENTRY; shapes::ENTRIES.len()];
    let mut e = 0;
    while e < shapes::ENTRIES.len() {
        if let Some(t) = shapes::ENTRIES[e].ty {
            let mut later = e + 1;
            while later < shapes::ENTRIES.len() {
                if let Some(u) = shapes::ENTRIES[later].ty
                    && bytes_eq(t.as_bytes(), u.as_bytes())
                {
                    next[e] = later as u16;
                    break;
                }
                later += 1;
            }
        }
        e += 1;
    }
    next
}

const fn build_candidates_of_type() -> [u16; shapes::ENTRIES.len()] {
    let mut counts = [0u16; shapes::ENTRIES.len()];
    let mut e = 0;
    while e < shapes::ENTRIES.len() {
        let mut n = 0;
        let mut at = e;
        while at != NO_TYPE_ENTRY as usize {
            n += 1;
            at = NEXT_OF_TYPE_CONST[at] as usize;
        }
        counts[e] = n;
        e += 1;
    }
    counts
}

/// [`NEXT_OF_TYPE`] for the const evaluator, which reads a `const` where it may not read a
/// `static`.
const NEXT_OF_TYPE_CONST: [u16; shapes::ENTRIES.len()] = build_next_of_type();

/// `a == b`, compared inline for the short texts it is asked about — keys, `type` values,
/// keywords — where a call to the library's `memcmp` costs more than the compare.
#[inline]
pub(crate) fn eq_short(a: &[u8], b: &[u8]) -> bool {
    let n = a.len();
    if n != b.len() {
        return false;
    }
    if n >= 8 {
        return load8(a, 0) == load8(b, 0)
            && load8(a, n - 8) == load8(b, n - 8)
            && (n <= 16 || a[8..n - 8] == b[8..n - 8]);
    }
    if n >= 4 {
        return load4(a, 0) == load4(b, 0) && load4(a, n - 4) == load4(b, n - 4);
    }
    n == 0 || (a[0] == b[0] && a[n - 1] == b[n - 1] && a[n / 2] == b[n / 2])
}

/// The eight bytes at `x[at..]` as a word.
#[expect(
    clippy::inline_always,
    reason = "measured: as closures, `eq_short`'s word loads stayed out-of-line calls"
)]
#[inline(always)]
fn load8(x: &[u8], at: usize) -> u64 {
    let x = &x[at..at + 8];
    u64::from_le_bytes([x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]])
}

/// The four bytes at `x[at..]` as a word.
#[expect(clippy::inline_always, reason = "as `load8`")]
#[inline(always)]
fn load4(x: &[u8], at: usize) -> u32 {
    let x = &x[at..at + 4];
    u32::from_le_bytes([x[0], x[1], x[2], x[3]])
}

/// The first candidate entry of a `type` value (`docs/tape_format.md` §Producing a tape: an
/// object tries the entries of its `type` in table order), or `None` for a `type` no entry
/// holds.
#[must_use]
pub(crate) fn first_of_type(ty: &[u8]) -> Option<u16> {
    let n = ty.len();
    let first = if n >= 8 {
        u64::from_le_bytes([ty[0], ty[1], ty[2], ty[3], ty[4], ty[5], ty[6], ty[7]])
    } else {
        word(ty, 0)
    };
    let last = if n > 8 {
        let t = &ty[n - 8..];
        u64::from_le_bytes([t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7]])
    } else {
        0
    };
    let mut slot = type_mix(first, last, n);
    loop {
        let e = TYPE_SLOTS[slot];
        if e == NO_TYPE_ENTRY {
            return None;
        }
        if shapes::ENTRIES[usize::from(e)]
            .ty
            .is_some_and(|t| eq_short(t.as_bytes(), ty))
        {
            return Some(e);
        }
        slot = (slot + 1) & ((1 << TYPE_SLOT_BITS) - 1);
    }
}

/// How many candidate entries the `type` of `first` (a [`first_of_type`] answer) has.
#[must_use]
pub(crate) fn candidates_of_type(first: u16) -> u16 {
    CANDIDATES_OF_TYPE[usize::from(first)]
}

/// The candidate after `entry` among its `type`'s, in table order.
#[must_use]
pub(crate) fn next_of_type(entry: u16) -> Option<u16> {
    let next = NEXT_OF_TYPE[usize::from(entry)];
    (next != NO_TYPE_ENTRY).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hashed `type` lookup gives every `type` its candidates in table order — the entries
    /// a scan of the table finds — and none to a `type` no entry holds.
    #[test]
    fn the_type_lookup_is_a_table_scan() {
        let scan = |ty: &[u8]| -> Vec<u16> {
            (0..shapes::ENTRIES.len() as u16)
                .filter(|&e| shapes::ENTRIES[usize::from(e)].ty.map(str::as_bytes) == Some(ty))
                .collect()
        };
        let types: Vec<&str> = shapes::ENTRIES.iter().filter_map(|e| e.ty).collect();
        for ty in types {
            let mut walked = Vec::new();
            let mut at = first_of_type(ty.as_bytes());
            while let Some(entry) = at {
                walked.push(entry);
                at = next_of_type(entry);
            }
            assert_eq!(walked, scan(ty.as_bytes()), "{ty}");
            assert_eq!(
                usize::from(candidates_of_type(walked[0])),
                walked.len(),
                "{ty}"
            );
            for near in [format!("{ty}x"), ty[1..].to_owned(), ty.to_lowercase()] {
                assert_eq!(
                    first_of_type(near.as_bytes()),
                    scan(near.as_bytes()).first().copied(),
                    "{near}"
                );
            }
        }
        assert_eq!(first_of_type(b""), None);
    }
}
