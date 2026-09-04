//! SWAR byte-search kernels — the word-at-a-time primitives the line scans
//! ([`crate::location`]), the width and quote scans ([`crate::printing`]), the
//! wire-JSON escape prescan ([`crate::json_writer`]) and the language lexers'
//! token-body scans share.
//!
//! Every kernel here answers a question about the eight bytes packed in one
//! `u64`. They exist as one module rather than one copy per caller because
//! their correctness argument is subtle and identical: a SWAR subtract borrows
//! **across** lanes, so a lane's flag is not independently trustworthy. What
//! *is* guaranteed — and what every caller relies on — is stated per kernel
//! below. Read the guarantee before adding a caller.
//!
//! [`next_byte_of`] and [`has_ascii_uppercase`] are the **public** entry points, and
//! the only items here that are scans rather than lane kernels: `tsv_ts` and `tsv_css`
//! reach them from their lexers and printers, which is what a language crate wants and
//! what keeps a fourth hand-rolled word loop from being written. The density caveat is
//! stated on [`next_byte_of`].

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
#[inline]
pub(crate) const fn lanes_less_than(v: u64, n: u8) -> u64 {
    v.wrapping_sub(splat(n)) & !v & HIGH_BITS
}

/// Index of the first byte at or after `from` that equals any of `needles`, or
/// `bytes.len()` when none does.
///
/// The word-at-a-time face of the byte scans a lexer is made of. A run of
/// "everything that is not one of these `N` bytes" spelled as a compare chain
/// costs about `2 + 2N` instructions and `N + 2` branches per byte and does not
/// vectorize — the resume the caller does at a hit makes the stride
/// data-dependent, so LLVM keeps the scalar loop. This asks the same question of
/// eight bytes at once: one load, `5N` lane operations, and one branch per word.
///
/// ⚠️ **Read `hits` with `trailing_zeros` only** — the OR of several
/// [`zero_lanes`] masks keeps that kernel's lowest-lane guarantee (a spurious
/// lane in either mask is preceded by a genuine one in the same mask, and the
/// OR's lowest set lane is therefore the lowest set lane of whichever mask holds
/// it) but nothing stronger.
///
/// The needles are a `[u8; N]` rather than a slice so `N` is a constant at each
/// call site: the lane loop unrolls and the splats hoist out of the word loop.
/// A caller whose class is not a plain byte set passes the **leads** of the loose
/// superset and re-tests the exact class at each hit — the shape
/// [`zero_or_high_lanes`] documents, and the one `tsv_ts`'s line-comment scan
/// uses for `<LS>` / `<PS>`.
///
/// ⚠️ **It has a density axis, because the splats are paid per CALL and the word
/// is paid per eight bytes.** A run shorter than one word costs the setup and
/// finds its hit in the first word anyway, so a caller entered many times for a
/// near-empty run loses. Measured on synthetic documents that are nothing but one
/// construct, `instructions:u` against the compare chain: a string literal breaks
/// even at **3–4 content bytes** (`''` **+1.11%**, 4 bytes −0.11%, 16 bytes
/// −2.25%, 64 bytes −9.88%) and a block comment at **~3** (`/**/` **+0.57%**, 16
/// bytes −2.09%, 120 bytes −12.45%). Real source sits far past both — a mean
/// string body of 17.3 bytes and a mean block comment of 259 across 1,666 `.ts`
/// files — so the tax is bounded by a shape no corpus contains. Census the run
/// length before adding a caller whose construct is routinely empty.
///
/// ⭐ **A caller whose runs are routinely EMPTY should test the first byte
/// itself rather than skip this.** The entry costs about fifteen instructions,
/// and two ASCII compares retire an empty run for two — `tsv_css`'s
/// `string_end` is the shape (half its runs are the `\` of an icon-font escape
/// sitting against the opening quote), and spelling that pre-test as a
/// 256-entry skip table instead measured **0.44 to 0.70 points of cycles
/// slower** on two corpora and two entry points, because a table puts a
/// dependent L1 load on the branch's critical path where a compare does not.
/// ⚠️ Escalating after a longer *bounded* prefix does NOT work: the bound's own
/// bookkeeping costs about what the entry does, so the two cancel.
///
/// ⚠️ **That pre-test is conditional on the EMPTY-RUN share, and is not free to
/// add.** It pays where empty runs dominate; at `tsv_css`'s
/// `extract_function_parts` hop, where the adjacent-paren case is 8.6% of the
/// runs against `string_end`'s ~50%, the same two compares cost instructions
/// and bought no measurable cycles. Read the census's zero bucket before
/// reaching for it, the same way the density note above asks for the mean.
///
/// ⭐ **A caller need not be spelled as a scan.** What decides whether this
/// primitive fits is the fraction of bytes that can move the caller's state —
/// `extract_function_parts` is a paren-depth counter with a wide `_ => {}` arm
/// that acts on 5% of the bytes it reads (mean 18.7 between hits), and it sits
/// on this rung for exactly the reason a lexer's string run does.
///
/// ⚠️ **The per-byte cost above is an `N` = 1–2 figure — but the needle count is a
/// far weaker axis than it looks, and a wide class does NOT hand the site back to
/// the skip table.** Disassembled at every width (`objdump` over a ten-way probe,
/// plus the seven live call sites), the word loop costs:
///
/// | `N` | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
/// | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
/// | insns/word | 12 | 16 | 20 | 20 | 24 | 28 | 32 | 28 | 32 | 36 |
/// | insns/byte | 1.5 | 2.0 | 2.5 | 2.5 | 3.0 | 3.5 | 4.0 | 3.5 | 4.0 | 4.5 |
///
/// Two effects flatten it. Below `N` = 4 the marginal is **four** instructions a
/// needle, not six, because `!(v ^ splat) & HIGH_BITS` folds to a single shared
/// `!v & HIGH_BITS` for every needle below `0x80` — so all-ASCII needles share one
/// `not`. (A needle that is non-ASCII or not a compile-time constant breaks that
/// sharing and costs about four more: the live `[quote, b'\\', b'\n', b'\r']` site
/// reads 29 where four ASCII constants read 20.) At `N` >= 4 LLVM **vectorizes the
/// lane loop** — `pshufd`/`pxor`/`paddq`/`por`, two needles per XMM register — and
/// the marginal halves again; the curve is not even monotone, `N` = 8 landing
/// cheaper than `N` = 7.
///
/// So a ten-needle hop is **4.5** instructions a byte against a 256-entry skip
/// table's flat **6** (measured on the same binary: `movzbl`, table `cmpb`, `inc`,
/// bound `cmp`, two branches), and the instruction crossover is somewhere past
/// `N` = 16, not at eight. On the other channels the two never converge at all —
/// the word loop retires **two branches and one load per eight bytes** where the
/// table pays two branches and two dependent loads per byte, which is the cost 0ap
/// found dominating. **Choose the rung by run length; alphabet width, up to about
/// ten, is not a reason to prefer the table.**
///
/// The tail loop is a compare chain, but it runs only within eight bytes of the
/// **slice's** end, not the run's: callers pass the whole source, so even a
/// two-byte run is answered by the word loop everywhere but the last word of the
/// file.
#[inline]
pub fn next_byte_of<const N: usize>(bytes: &[u8], from: usize, needles: [u8; N]) -> usize {
    let splats = needles.map(splat);
    let mut i = from;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        let mut hits = 0;
        let mut k = 0;
        while k < N {
            hits |= zero_lanes(w ^ splats[k]);
            k += 1;
        }
        if hits != 0 {
            return i + (hits.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < bytes.len() && !needles.contains(&bytes[i]) {
        i += 1;
    }
    i
}

/// The first byte in `bytes[from..end)` that is below `floor`, at or above `0x80`, or one of
/// `needles` — `end` when there is none. The **host-range** form of [`next_byte_of`]: `bytes`
/// is the whole document and `[from, end)` the region asked about, so a region shorter than a
/// word is still answered by one word read from the host (a lane past `end` is ignored), and
/// only a region within seven bytes of the document's end falls to the scalar tail. What it
/// exists for is a "does this short slice hold any byte of a class" question — `tsv_css`'s
/// value normalizer asks it of every identifier value, 10.6 bytes on average and 42% under
/// eight on a real corpus — where a scan over the bare slice can only test a byte at a time.
///
/// The three classes share one word read: the high bits answer "at or above `0x80`" exactly,
/// the borrow of `floor` answers "below `floor`" ([`lanes_less_than`], exact in its lowest set
/// lane), and each needle is one [`zero_lanes`]. Only the lowest set lane of the union is read,
/// and every mask's spurious lanes lie above a genuine lane of its own, so that lane is a
/// genuine hit. `floor` must be at most `0x80`.
#[inline]
pub fn next_byte_below_or_of<const N: usize>(
    bytes: &[u8],
    from: usize,
    end: usize,
    floor: u8,
    needles: [u8; N],
) -> usize {
    debug_assert!(from <= end && end <= bytes.len() && floor <= 0x80);
    let splats = needles.map(splat);
    let mut i = from;
    while i < end {
        let Some(chunk) = bytes[i..].first_chunk::<8>() else {
            break;
        };
        let w = u64::from_le_bytes(*chunk);
        let mut hits = high_bit_lanes(w) | lanes_less_than(w, floor);
        let mut k = 0;
        while k < N {
            hits |= zero_lanes(w ^ splats[k]);
            k += 1;
        }
        if hits != 0 {
            let at = i + (hits.trailing_zeros() / 8) as usize;
            return if at < end { at } else { end };
        }
        i += 8;
    }
    if i >= end {
        return end;
    }
    // Within eight bytes of the HOST's end — the one place no word is readable.
    while i < end && bytes[i] >= floor && bytes[i] < 0x80 && !needles.contains(&bytes[i]) {
        i += 1;
    }
    i
}

/// Lane mask of the ASCII uppercase bytes (`A`..=`Z`) in `v`.
///
/// Computed on the word with its high bits cleared, so every lane is at most `0x7f` and
/// neither add can carry into a neighbour: a lane is at or above `A` iff adding
/// `0x80 - b'A'` sets its high bit, and past `Z` iff adding `0x80 - (b'Z' + 1)` does. The
/// original high bits mask the result, so a non-ASCII byte whose low seven bits land in the
/// range is not a hit. **Exact per lane** — no borrow, no carry — so the mask may be read
/// any way; the boolean caller reads `!= 0`.
#[inline]
pub(crate) const fn uppercase_lanes(v: u64) -> u64 {
    let low7 = v & !HIGH_BITS;
    let at_least_a = low7.wrapping_add(splat(0x80 - b'A'));
    let past_z = low7.wrapping_add(splat(0x80 - (b'Z' + 1)));
    at_least_a & !past_z & !v & HIGH_BITS
}

/// Does `bytes` hold an ASCII uppercase letter (`A`..=`Z`)? [`uppercase_lanes`] a word at
/// a time.
///
/// The last word read is the slice's LAST eight bytes, overlapping the word before it, so
/// a slice of eight bytes or more never falls to the scalar tail; the tail runs only for a
/// slice shorter than one word. That overlap is what the caller this was written for
/// needs: a CSS property name is nine bytes on average, one word and a byte, and the byte
/// would otherwise cost what the word did.
#[inline]
pub fn has_ascii_uppercase(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        if uppercase_lanes(u64::from_le_bytes(*chunk)) != 0 {
            return true;
        }
        i += 8;
    }
    if i == bytes.len() {
        return false;
    }
    if let Some(last) = bytes.last_chunk::<8>() {
        return uppercase_lanes(u64::from_le_bytes(*last)) != 0;
    }
    bytes[i..].iter().any(u8::is_ascii_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`has_ascii_uppercase`] against the scalar predicate: every byte value, at every
    /// position of slices from empty to two words and a tail, over a background that
    /// includes the non-ASCII bytes whose low seven bits spell a letter (`0xC1` = `A` with
    /// the high bit) — the false positive the `& !v` term exists to refuse.
    #[test]
    fn has_ascii_uppercase_matches_the_scalar_predicate() {
        let background = [b'a', b'-', b'z', 0xC1, 0xDA, 0x80, b'0', b' ', 0xFF, b'{'];
        for len in 0..=19 {
            for pos in 0..len.max(1) {
                for b in 0..=255u8 {
                    let mut v: Vec<u8> =
                        (0..len).map(|k| background[k % background.len()]).collect();
                    if pos < len {
                        v[pos] = b;
                    }
                    let expected = v.iter().any(u8::is_ascii_uppercase);
                    assert_eq!(
                        has_ascii_uppercase(&v),
                        expected,
                        "len {len} pos {pos} byte {b:#x}"
                    );
                }
            }
        }
    }

    /// [`next_byte_of`] against a plain scalar scan, over every needle count the
    /// crate uses and every alignment of a hit within and across words — the
    /// property the word loop's borrow behaviour could break silently.
    ///
    /// ⚠️ The **non-ASCII needle** is not decoration. Where every needle is below
    /// `0x80`, `w ^ splat(needle)` has each lane's high bit unchanged, so LLVM is
    /// free to fold [`zero_lanes`]'s `& !x` term against the *unxored* word — a
    /// strength reduction it does take, and one that is wrong for a needle at or
    /// above `0x80`. The `0xE2` case is `tsv_ts`'s line-comment scan, whose lead
    /// class carries `<LS>` / `<PS>`'s first byte; nothing else here would fail if
    /// that fold ever leaked.
    #[test]
    fn next_byte_of_matches_a_scalar_scan() {
        fn scalar(bytes: &[u8], from: usize, needles: &[u8]) -> usize {
            let mut i = from;
            while i < bytes.len() && !needles.contains(&bytes[i]) {
                i += 1;
            }
            i
        }
        // Filler bytes that exercise the borrow chain: zero, the sub-needle
        // 0x01, ASCII, and the 0x80 axis a borrow must not cross.
        let filler = [0x00u8, 0x01, b'a', 0x7f, 0x80, 0xff];
        for &f in &filler {
            for len in 0..40usize {
                for hit in 0..=len {
                    let mut v = vec![f; len];
                    if hit < len {
                        v[hit] = b'*';
                    }
                    for from in 0..=len {
                        assert_eq!(
                            next_byte_of(&v, from, [b'*']),
                            scalar(&v, from, b"*"),
                            "1 needle, filler {f:#x}, len {len}, hit {hit}, from {from}"
                        );
                        assert_eq!(
                            next_byte_of(&v, from, [b'*', b'\\', b'\n', b'\r']),
                            scalar(&v, from, b"*\\\n\r"),
                            "4 needles, filler {f:#x}, len {len}, hit {hit}, from {from}"
                        );
                    }
                }
                // The same sweep with a NON-ASCII needle present and the hit
                // landing on it, which is the case the `& !x` fold would break.
                for hit in 0..=len {
                    let mut v = vec![f; len];
                    if hit < len {
                        v[hit] = 0xE2;
                    }
                    for from in 0..=len {
                        assert_eq!(
                            next_byte_of(&v, from, [b'\n', b'\r', 0xE2]),
                            scalar(&v, from, b"\n\r\xE2"),
                            "0xE2 lead, filler {f:#x}, len {len}, hit {hit}, from {from}"
                        );
                    }
                }
            }
        }
    }

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

    /// The host-range scan against a scalar reference over every (from, end) window of a
    /// buffer, for fillers on both sides of every class boundary and a single hit byte from
    /// each class — the floor, a non-ASCII byte, each needle — at every position.
    #[test]
    fn next_byte_below_or_of_matches_a_scalar_scan() {
        fn scalar(bytes: &[u8], from: usize, end: usize, floor: u8, needles: &[u8]) -> usize {
            let mut i = from;
            while i < end && bytes[i] >= floor && bytes[i] < 0x80 && !needles.contains(&bytes[i]) {
                i += 1;
            }
            i
        }
        const NEEDLES: [u8; 6] = [b'(', b')', b',', b'/', b'\'', b'"'];
        let filler = [0x21u8, b'a', 0x7f];
        let hits = [0x00u8, 0x0b, 0x20, b'(', b'"', b'/', 0x80, 0xff];
        for &f in &filler {
            for len in 0..20usize {
                for &h in &hits {
                    for hit in 0..=len {
                        let mut v = vec![f; len];
                        if hit < len {
                            v[hit] = h;
                        }
                        for from in 0..=len {
                            for end in from..=len {
                                assert_eq!(
                                    next_byte_below_or_of(&v, from, end, 0x21, NEEDLES),
                                    scalar(&v, from, end, 0x21, &NEEDLES),
                                    "filler {f:#x} len {len} hit {h:#x}@{hit} [{from}, {end})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
