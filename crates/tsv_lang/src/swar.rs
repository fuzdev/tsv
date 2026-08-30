//! SWAR byte-search kernels — the word-at-a-time primitives the line scans
//! ([`crate::location`]) and the wire-JSON escape prescan
//! ([`crate::json_writer`]) share.
//!
//! Every kernel here answers a question about the eight bytes packed in one
//! `u64`. They exist as one module rather than one copy per caller because
//! their correctness argument is subtle and identical: a SWAR subtract borrows
//! **across** lanes, so a lane's flag is not independently trustworthy. What
//! *is* guaranteed — and what every caller relies on — is stated per kernel
//! below. Read the guarantee before adding a caller.

/// `0x01` in every lane — the borrow unit the has-zero / has-less kernels
/// subtract.
const LOW_BITS: u64 = 0x0101_0101_0101_0101;

/// `0x80` in every lane — the bit each kernel reports its answer in.
const HIGH_BITS: u64 = 0x8080_8080_8080_8080;

/// Broadcast `b` to every lane of a `u64` word — the SWAR needle. Written as a
/// multiply rather than a byte array so it carries no endianness.
#[inline]
pub(crate) const fn splat(b: u8) -> u64 {
    b as u64 * LOW_BITS
}

/// Lane mask of the bytes in `v` with their high bit set — i.e. the non-ASCII
/// ones.
///
/// The odd one out, and worth saying so: this kernel is a plain `AND`, with no
/// subtract and therefore **no borrow**. Every lane's flag is independently
/// genuine, so unlike [`zero_lanes`] / [`lanes_less_than`] its mask may be read
/// any way at all — popcount, highest bit, whatever the caller wants.
#[inline]
pub(crate) const fn high_bit_lanes(v: u64) -> u64 {
    v & HIGH_BITS
}

/// Lane mask of the zero bytes in `v`: the high bit of lane `k` is set if
/// `v`'s byte `k` is zero. The classic `has_zero` kernel.
///
/// ⚠️ **Only the LOWEST set lane is guaranteed genuine.** A zero byte borrows
/// into the next lane, which can flag lane `k+1` spuriously — but a spurious
/// flag at `k+1` requires lane `k` to have been zero, and a zero lane always
/// flags itself, so the lowest flagged lane is always a real match. Read this
/// mask with `trailing_zeros`, never with a popcount or a highest-bit scan.
///
/// The corollary the boolean callers use: **`mask != 0` ⟺ some lane genuinely
/// matched**, since a spurious lane cannot exist without a genuine one below it
/// and a genuine lane always flags itself.
#[inline]
pub(crate) const fn zero_lanes(v: u64) -> u64 {
    v.wrapping_sub(LOW_BITS) & !v & HIGH_BITS
}

/// Lane mask of the bytes in `v` that are zero **or** have their high bit set —
/// [`zero_lanes`] with its `& !v` term dropped.
///
/// That term is the whole cost of [`zero_lanes`]'s precision: without it a lane
/// still flags whenever the borrow leaves its high bit set, which is every lane
/// at or above `0x80`. So this kernel answers "zero, or non-ASCII" for the price
/// of "zero" minus an operation, and a caller that was going to look for a
/// non-ASCII byte anyway gets that needle for free.
///
/// ⚠️ **Same lowest-lane guarantee as [`zero_lanes`], and the same reading
/// rule.** A zero lane flags itself (`0 - 1` is `0xFF`, and a borrow-in only
/// deepens it), a lane at or above `0x80` flags itself through the `| v`, and a
/// lane in `0x01..=0x7F` can only flag on a borrow-in — which requires a genuine
/// zero below it. Read with `trailing_zeros`, never a popcount.
///
/// ⚠️ **The non-ASCII lanes are FALSE POSITIVES to the caller's own needle**, so
/// only a caller that can tell them apart afterwards may use this.
/// `crate::printing`'s line terminator scan is the shape: it takes two of these
/// where three [`zero_lanes`] used to stand — seven operations against fourteen,
/// with `<LS>` / `<PS>`'s own `0xE2` lead inside the loose class for free — and
/// hands the word that fired to the exact kernel when it holds a non-ASCII byte
/// at all. **That fallback is not optional**: handing a loose hit straight to the
/// caller measures `-0.354%` on real source and `+20%` on a document that is 98%
/// non-ASCII, where every byte becomes a hit to step over.
#[inline]
pub(crate) const fn zero_or_high_lanes(v: u64) -> u64 {
    (v.wrapping_sub(LOW_BITS) | v) & HIGH_BITS
}

/// Lane mask of the bytes in `v` that are less than `n`, for `n <= 0x80`.
///
/// ⚠️ **Same lowest-lane guarantee as [`zero_lanes`], and the same reason.** A
/// byte below `n` borrows, and the borrow can flag the next lane spuriously;
/// but every borrow chain *originates* at a lane that genuinely underflows
/// without a borrow-in, and such a lane always flags itself. So `mask != 0` ⟺
/// some lane is genuinely `< n`, which is what the boolean callers ask.
///
/// A lane at or above `0x80` — a UTF-8 continuation byte, say — is never
/// flagged: `!v`'s high bit is clear there, and it can never underflow against
/// an `n <= 0x80` either, so it neither reports nor propagates.
#[cfg(feature = "json")]
#[inline]
pub(crate) const fn lanes_less_than(v: u64, n: u8) -> u64 {
    v.wrapping_sub(splat(n)) & !v & HIGH_BITS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grade `kernel`'s mask against a per-byte `oracle` over every word whose
    /// lanes are drawn from `alphabet` — exhaustively in the low four lanes,
    /// with the high four mirroring them, which covers a hit at every position
    /// and every borrow-chain length.
    ///
    /// Asserts the lowest-lane guarantee in **both** directions: no genuine hit
    /// is missed, *and* no mask is set when the oracle says no lane matches.
    /// The second direction is the one a caller can never check for itself — a
    /// spurious hit is merely correct-but-slower downstream, so it is invisible
    /// everywhere except here.
    fn assert_lowest_lane(
        alphabet: &[u8],
        label: &str,
        kernel: impl Fn(u64) -> u64,
        oracle: impl Fn(u8) -> bool,
    ) {
        let mut lanes = [0u8; 8];
        for &a in alphabet {
            for &b in alphabet {
                for &c in alphabet {
                    for &d in alphabet {
                        for (i, byte) in [a, b, c, d, d, c, b, a].into_iter().enumerate() {
                            lanes[i] = byte;
                        }
                        let mask = kernel(u64::from_le_bytes(lanes));
                        match lanes.iter().position(|&x| oracle(x)) {
                            Some(k) => {
                                assert_ne!(mask, 0, "{label}: missed lane {k} in {lanes:?}");
                                assert_eq!(
                                    (mask.trailing_zeros() / 8) as usize,
                                    k,
                                    "{label}: wrong lowest lane for {lanes:?}"
                                );
                            }
                            None => assert_eq!(mask, 0, "{label}: spurious hit in {lanes:?}"),
                        }
                    }
                }
            }
        }
    }

    /// The lowest-lane guarantee for the has-zero kernel — the property every
    /// caller's correctness rests on, and one no corpus can grade.
    #[test]
    fn zero_lanes_lowest_set_lane_is_the_first_zero_byte() {
        assert_lowest_lane(
            &[0x00, 0x01, 0x7f, 0x80, 0xff],
            "zero_lanes",
            zero_lanes,
            |b| b == 0,
        );
    }

    /// Same, for the has-less kernel. The alphabet carries the escape prescan's
    /// `n = 0x20` boundary and the `0x80` axis a borrow must not cross, and `n`
    /// itself is swept because the guarantee is claimed for every `n <= 0x80`.
    #[cfg(feature = "json")]
    #[test]
    fn lanes_less_than_lowest_set_lane_is_the_first_byte_below_n() {
        for n in [0x01u8, 0x20, 0x80] {
            assert_lowest_lane(
                &[0x00, 0x1f, 0x20, 0x21, 0x80, 0xff],
                "lanes_less_than",
                |v| lanes_less_than(v, n),
                |b| b < n,
            );
        }
    }

    /// [`high_bit_lanes`] has no borrow, so its mask is exact per lane — the
    /// lowest-lane reading the shared helper checks is simply the strongest of
    /// the several valid ones here.
    #[test]
    fn high_bit_lanes_flags_exactly_the_non_ascii_bytes() {
        assert_lowest_lane(
            &[0x00, 0x7f, 0x80, 0xff],
            "high_bit_lanes",
            high_bit_lanes,
            |b| b >= 0x80,
        );
    }
}
