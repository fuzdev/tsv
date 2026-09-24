//! Shared wire-JSON emission substrate.
//!
//! `JsonWriter` is the byte-buffer + scalar-emitter primitive the three
//! language crates' wire-JSON writers (`ast/convert/write/`) build on. It lives
//! here — not in any one language crate — so `tsv_svelte`'s writer can compose
//! `tsv_ts` (embedded `{expr}` / `<script>`) and `tsv_css` (embedded `<style>`)
//! emission into one shared buffer by passing `&mut JsonWriter` across crate
//! boundaries. Each language crate keeps its own node emitters (`node_header`,
//! field helpers, the per-language `Ctx`); only this JSON-scalar substrate is
//! shared.
//!
//! Behind the `json` feature (enabled transitively by each language crate's
//! `convert` feature) so the format-only `@fuzdev/tsv-format-wasm` build — which
//! turns `convert` off — never links `serde_json`.
//!
//! **Escape / format parity contract**: static structure and tokens are written
//! verbatim (debug-asserted escape-free); dynamic strings are escaped by hand,
//! **byte-identical** to `serde_json`'s string serialization (graded against
//! `serde_json` itself by `string_matches_serde_json`); non-integral `f64`
//! delegates to `serde_json::to_writer`, so ryu formatting is exactly its — the
//! canonical parsers' `JSON.stringify` parity the fixtures pin; integers have a
//! unique decimal form and are hand-formatted (two-digit-pair, the hot path
//! emitting several ints per node).

use crate::swar::{lanes_less_than, splat, zero_lanes};

/// `00`,`01`,…,`99` — the two-digit-pair table behind every integer emitter,
/// halving their divisions. Read it through [`dec_pair`], never directly.
///
/// Each entry holds its pair **already packed**, tens digit in the low byte, so
/// a lookup is one 16-bit load rather than two byte loads and a shift. The
/// packing is arithmetic, not a reinterpretation, so it is the same table on
/// either endianness; the byte order a caller wants back is spelled by
/// `to_le_bytes`.
///
/// Sized to the next power of two above the hundred live entries so
/// [`dec_pair`] can bound its index by masking rather than by a compare — see
/// there for why a compare is not affordable here.
const DEC_PAIRS: [u16; 128] = {
    let mut t = [0u16; 128];
    let mut i = 0;
    while i < 100 {
        t[i] = (b'0' + (i / 10) as u8) as u16 | ((b'0' + (i % 10) as u8) as u16) << 8;
        i += 1;
    }
    t
};

/// [`DEC_PAIRS`] at an index the caller has already bounded to `0..100`.
///
/// ⚠️ The mask is a **codegen** requirement, not defensive coding. Every caller
/// divides a value it knows the range of, but nothing in a signature says so,
/// so a bare index compiles to a compare and a panic edge — and those edges are
/// what push [`JsonWriter::stage_u32`] past the inliner's threshold, which
/// costs the staged header its register residency and ~5% of the parse→JSON
/// path. Masking to the table's own width makes the bound a property of the
/// type. `debug_assert` is where the precondition is actually held.
#[inline]
const fn dec_pair(k: u32) -> u16 {
    debug_assert!(k < 100);
    DEC_PAIRS[(k as usize) & (DEC_PAIRS.len() - 1)]
}

/// Decimal digits in `u64::MAX` — the width of the wide arm's scratch, and the
/// constant length of the copy that appends it.
const MAX_U64_DIGITS: usize = 20;

/// Digits that fit one `u64` of packed ASCII — the width [`JsonWriter::u32`]
/// generates entirely in a register. Every integer the writers emit (offsets,
/// lines, columns) is far below `99_999_999`; wider values take the cold arm.
const WORD_DIGITS: usize = 8;

/// Decimal digits in `u32::MAX` — the ceiling of [`decimal_width_u32`].
const MAX_U32_DIGITS: usize = 10;

/// The key between the two values [`JsonWriter::start_end`] writes.
const END_KEY: &str = ",\"end\":";

/// [`END_KEY`] padded to one word, so it is stored as a single fixed-width
/// move; the pad byte lands where the `end` digits begin and is overwritten.
/// Built from [`END_KEY`] so the two cannot drift (a key a word or longer fails the
/// build here).
const END_KEY_WORD: [u8; 8] = {
    let key = END_KEY.as_bytes();
    let mut word = [0; 8];
    let mut i = 0;
    while i < key.len() {
        word[i] = key[i];
        i += 1;
    }
    assert!(
        key.len() < word.len(),
        "the `end` digits overwrite the pad byte"
    );
    word
};

/// The fixed window [`JsonWriter::start_end`] appends before trimming: both
/// values at their full [`WORD_DIGITS`] width and the key between them.
const START_END_WINDOW: usize = 2 * WORD_DIGITS + END_KEY.len();

/// The `"start":` key [`JsonWriter::start_end_field`] and
/// [`JsonWriter::start_end_object`] lead with, after their one-byte `,` / `{`.
const START_KEY: &str = "\"start\":";

/// [`JsonWriter::start_end_led`]'s window: the lead byte and [`START_KEY`],
/// then [`START_END_WINDOW`].
const LED_WINDOW: usize = 1 + START_KEY.len() + START_END_WINDOW;

/// What [`JsonWriter::start_end_led`] appends before filling its window: the
/// key already in place behind a placeholder lead byte, zeros after it — so
/// the key rides the window's own fixed-width copy instead of a store of its
/// own.
const LED_TEMPLATE: [u8; LED_WINDOW] = {
    let mut t = [0; LED_WINDOW];
    let key = START_KEY.as_bytes();
    let mut i = 0;
    while i < key.len() {
        t[1 + i] = key[i];
        i += 1;
    }
    t
};

/// Decimal digit count of a `u32` (`0` is one digit) — the [`decimal_width`]
/// sibling for the `u32` path.
///
/// Same ascending-compare rationale, on the narrower type. The whole point of
/// the `u32` path is that every division in it is 32-bit: a `u64 / 100` lowers
/// to a full 64×64→128 multiply (`mul %rcx` + two shifts), a `u32 / 100` to a
/// single widening `imul`. Taking a `u64` here would sink the argument back
/// into 64-bit arithmetic and undo it.
///
/// Reached only from [`digit_word`]'s six-digits-and-up arm, which has
/// already established `n >= 100_000` — so the first five compares fold away
/// there, and the ladder answers only the widths that arm actually sees. The
/// widths below it are named by the arm that generated them.
#[inline]
const fn decimal_width_u32(n: u32) -> usize {
    if n < 10 {
        return 1;
    }
    if n < 100 {
        return 2;
    }
    if n < 1_000 {
        return 3;
    }
    if n < 10_000 {
        return 4;
    }
    if n < 100_000 {
        return 5;
    }
    if n < 1_000_000 {
        return 6;
    }
    /// `POW10[i] == 10^i`, up to the largest power of ten a `u32` holds.
    const POW10: [u32; MAX_U32_DIGITS] = {
        let mut t = [1u32; MAX_U32_DIGITS];
        let mut i = 1;
        while i < MAX_U32_DIGITS {
            t[i] = t[i - 1] * 10;
            i += 1;
        }
        t
    };
    let mut w = 7;
    while w < MAX_U32_DIGITS && n >= POW10[w] {
        w += 1;
    }
    w
}

/// Decimal digit count of `n` (`0` is one digit) — the exact width the wide arm
/// front-aligns its digits to.
///
/// Only the wide arm reaches this — [`JsonWriter::u32`] carries the hot path on
/// [`decimal_width_u32`] — so the ascending chain here is inherited shape rather
/// than a tuned one. It stays ascending for consistency with its sibling, whose
/// distribution (offsets, lines and columns) really is overwhelmingly small.
///
/// ⚠️ The tail is a **bounded loop**, not a call to `ilog10` — and that is a
/// codegen constraint, not a style choice. `ilog10` is out-of-line, so its
/// result is opaque to the caller: LLVM then cannot prove the returned width
/// fits the wide arm's scratch and re-inserts a `panic_bounds_check` on every
/// one of its four scratch writes. A `while w < MAX_U64_DIGITS` loop makes the
/// range provable, and the bounds checks disappear. Keep any future rewrite
/// provably in `1..=MAX_U64_DIGITS` **at the type/CFG level**.
#[inline]
const fn decimal_width(n: u64) -> usize {
    if n < 10 {
        return 1;
    }
    if n < 100 {
        return 2;
    }
    if n < 1_000 {
        return 3;
    }
    if n < 10_000 {
        return 4;
    }
    if n < 100_000 {
        return 5;
    }
    if n < 1_000_000 {
        return 6;
    }
    /// `POW10[i] == 10^i`, up to the largest power of ten a `u64` holds.
    const POW10: [u64; MAX_U64_DIGITS] = {
        let mut t = [1u64; MAX_U64_DIGITS];
        let mut i = 1;
        while i < MAX_U64_DIGITS {
            t[i] = t[i - 1] * 10;
            i += 1;
        }
        t
    };
    let mut w = 7;
    while w < MAX_U64_DIGITS && n >= POW10[w] {
        w += 1;
    }
    w
}

/// `n`'s decimal digits packed into one `u64` of ASCII — most significant
/// digit at byte 0 — **with the width that says how many of those bytes are
/// live**. The arithmetic core both integer emitters share
/// ([`JsonWriter::u32`] and [`JsonWriter::stage_u32`]), so there is one
/// implementation and one oracle.
///
/// A width past [`WORD_DIGITS`] comes back with a meaningless word: the value
/// is one no offset, line or column in a real document reaches, and each
/// caller hands it to its own wide arm.
///
/// Two-digit-pair formatting (itoa's approach) halves the divisions, and the
/// writers emit several integers per node, so this is hot.
///
/// ⚠️ The digits are generated **into a register**, never into a stack scratch,
/// and that is a codegen constraint. A constant-length copy out of a scratch
/// array a pair loop just filled with 2-byte stores cannot be store-forwarded
/// past those narrow stores, and the store writes the scratch's full width
/// (20 bytes to keep ~3). Measured, that single store was **71% of the
/// emitter's self time and ~22% of the whole parse→JSON run**. Packing into a
/// `u64` removes both: nothing round-trips through memory, and the store is one
/// word instead of two vectors.
///
/// ⚠️ **The width is derived by the same test that picks the arm, not by a
/// separate ladder ahead of it.** Asking for the width first and then
/// generating that many digits states the value's magnitude twice, and LLVM
/// resolves the second statement by specializing a pair loop per digit count —
/// which is a *cross product* of the ladder and the loop, big enough that the
/// staged emitter stops being inlined. Since a magnitude test is what an arm is
/// selected on anyway, the arm returns the width it already knows: each one
/// generates its whole span unconditionally, zero-padded, and shifts the
/// padding off by a constant.
///
/// Padding shifts **off the low end**, because the accumulation puts the most
/// significant digit at byte 0 (little-endian byte order) so the caller's write
/// is a plain prefix; the bytes past the width are therefore zero.
///
/// Five digits has an arm of its own — one leading digit ahead of the
/// four-digit word — because it is not rare: a stylesheet or component a few
/// tens of KB long spends most of its offsets there (48% of the `start`/`end`
/// values over a corpus of real stylesheets), and the general arm pays a width
/// ladder, a second four-digit word and two variable shifts for them.
///
/// ⚠️ `inline(always)`, and that is a **performance** constraint that costs
/// binary size. Out-of-lining it is +5.5% instructions on the parse→JSON path
/// (measured): the staged emitter's whole advantage is that `stage_len` and
/// the scratch base stay in registers across the header, and an opaque call
/// per integer forces them back to the stack. Plain `inline` held that decision
/// only while the body was small — adding the five-digit arm under it outlined
/// the function at the staged sites, +2.3% on the TypeScript and Svelte wire
/// paths — so the attribute is a pin, as on [`JsonWriter::stage_u32`]. The size
/// it buys — one inlined copy per staged integer site, one in
/// [`JsonWriter::u32`], and two each in [`JsonWriter::start_end`] and its led
/// sibling — is the deliberate trade.
#[expect(clippy::inline_always)]
#[inline(always)]
fn digit_word(n: u32) -> (u64, usize) {
    if n < 100 {
        let pair = dec_pair(n) as u64;
        return if n < 10 { (pair >> 8, 1) } else { (pair, 2) };
    }
    if n < 10_000 {
        let word = four_digit_word(n);
        return if n < 1_000 { (word >> 8, 3) } else { (word, 4) };
    }
    if n < 100_000 {
        // One leading digit ahead of the four-digit word.
        let lead = u64::from(b'0' + (n / 10_000) as u8);
        return (lead | four_digit_word(n % 10_000) << 8, 5);
    }
    // Six digits and up: rare enough that one variable shift is cheaper than
    // three more arms, and the wide values fall out of the same width.
    let digits = decimal_width_u32(n);
    if digits > WORD_DIGITS {
        return (0, digits);
    }
    // `hi` carries `digits - 4` digits, `lo` exactly four (zero-padded).
    let hi = four_digit_word(n / 10_000);
    let lo = four_digit_word(n % 10_000);
    (
        (hi >> ((8 - digits) * 8)) | (lo << ((digits - 4) * 8)),
        digits,
    )
}

/// `n < 10_000` as four ASCII digits packed into one `u64`, most significant
/// digit at byte 0, zero-padded — the flat core [`digit_word`] cuts its arms
/// around.
///
/// Two independent lookups off one division: nothing here is carried from one
/// pair to the next, which is the whole difference from a pair loop.
#[inline]
const fn four_digit_word(n: u32) -> u64 {
    debug_assert!(n < 10_000);
    dec_pair(n / 100) as u64 | (dec_pair(n % 100) as u64) << 16
}

/// Write a `start` / `end` pair's digit words and the `,"end":` key between
/// them into the front of `window`, returning how many of its bytes the pair
/// occupies — the fill [`JsonWriter::start_end`] and its led siblings share.
/// Each word is a [`digit_word`] result no wider than [`WORD_DIGITS`], so every
/// store is a fixed-width move at an offset bounded by the window's length.
#[expect(clippy::inline_always)]
#[inline(always)]
fn fill_start_end(window: &mut [u8], start: (u64, usize), end: (u64, usize)) -> usize {
    let (start_word, start_digits) = start;
    let (end_word, end_digits) = end;
    let window = &mut window[..START_END_WINDOW];
    window[..WORD_DIGITS].copy_from_slice(&start_word.to_le_bytes());
    window[start_digits..start_digits + END_KEY_WORD.len()].copy_from_slice(&END_KEY_WORD);
    let end_at = start_digits + END_KEY.len();
    window[end_at..end_at + WORD_DIGITS].copy_from_slice(&end_word.to_le_bytes());
    end_at + end_digits
}

/// Does `bytes` contain a byte JSON must escape?
///
/// The predicate is exactly `serde_json`'s: `0x00..=0x1F`, `"`, and `\`. It
/// answers eight bytes at a time because most strings are clean — the identifier
/// names, string-literal bodies and comment text the wire writers push through
/// [`JsonWriter::string`] nearly all spend the whole scan confirming misses, the
/// same shape (and the same reason) as the line scans in [`crate::location`].
/// Template text is the exception, and [`JsonWriter::string_escaped`] says by how
/// much.
///
/// The three lane masks are OR-ed and read as a **boolean**, never as a
/// position, so the lowest-lane guarantee documented on [`zero_lanes`] /
/// [`lanes_less_than`] is exactly what is needed: a set mask always implies a
/// genuine hit somewhere. `lanes_less_than(w, 0x20)` already covers `NUL`, so
/// the two `zero_lanes` tests are only the `"` and `\` needles.
///
/// One word is loaded once and tested three ways rather than scanned three
/// times — the same trade `next_ecmascript_terminator` makes for its two
/// needles.
///
/// The remainder is finished with an **overlapping final word** rather than a
/// byte loop whenever the slice is at least 8 bytes long. Re-testing bytes the
/// word loop already cleared is free here in a way it would not be for a
/// position-returning scan: the answer is a boolean over the *union* of the
/// bytes tested, so an overlap cannot change it. That matters because the
/// strings arriving here are short — an identifier name straddles one or two
/// words — so the tail is a large fraction of the whole scan, and a byte loop
/// there gives back much of what the word loop won.
///
/// The same union argument covers a slice **shorter** than a word, which is
/// over half of all calls (property names, short literals, the whitespace
/// between two tags): it is gathered into one word — two overlapping
/// four-byte loads, or the first, middle and last byte — and tested once,
/// where a byte loop cost about seven instructions a byte.
#[inline]
fn needs_escape(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        if word_needs_escape(*chunk) {
            return true;
        }
        i += 8;
    }
    if i == bytes.len() {
        return false;
    }
    if let Some(chunk) = bytes.last_chunk::<8>() {
        // ≥ 8 bytes: the final word covers the whole remainder (and some
        // already-cleared bytes, harmlessly — see above).
        return word_needs_escape(*chunk);
    }
    // Shorter than one word, so the loop above never ran — but the same union
    // argument still gathers the slice into one word. Four to seven bytes are
    // two overlapping four-byte loads; one to three are the first, middle and
    // last byte (which between them name every byte of a slice that short),
    // with the lanes past them filled by a byte no kernel flags.
    let word = if let (Some(lo), Some(hi)) = (bytes.first_chunk::<4>(), bytes.last_chunk::<4>()) {
        u64::from(u32::from_le_bytes(*lo)) | u64::from(u32::from_le_bytes(*hi)) << 32
    } else if let (Some(&first), Some(&last)) = (bytes.first(), bytes.last()) {
        let middle = bytes[bytes.len() / 2];
        u64::from(first) | u64::from(middle) << 8 | u64::from(last) << 16 | splat(b' ') << 24
    } else {
        return false;
    };
    escape_lanes(word) != 0
}

/// [`needs_escape`]'s per-word kernel: does any of these eight bytes need a
/// JSON escape?
#[inline]
const fn word_needs_escape(chunk: [u8; 8]) -> bool {
    escape_lanes(u64::from_le_bytes(chunk)) != 0
}

/// Lane mask of the bytes in `w` JSON must escape — `0x00..=0x1F`, `"` and `\`.
///
/// ⚠️ Only the **lowest** set lane is guaranteed genuine (the OR keeps
/// [`zero_lanes`] / [`lanes_less_than`]'s guarantee: the OR's lowest set lane
/// is its own mask's lowest). Read it with `trailing_zeros` or as a boolean.
#[inline]
const fn escape_lanes(w: u64) -> u64 {
    lanes_less_than(w, 0x20) | zero_lanes(w ^ splat(b'"')) | zero_lanes(w ^ splat(b'\\'))
}

/// What each byte becomes inside a JSON string, as `serde_json` writes it: `0` for a
/// byte written as itself, the letter after the backslash for the seven two-byte
/// short forms (`\"`, `\\`, `\b`, `\t`, `\n`, `\f`, `\r`), and `u` for every other
/// control byte, written `\u00XX` with [`HEX_DIGITS`]'s lowercase digits.
const ESCAPE: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 0x20 {
        t[i] = b'u';
        i += 1;
    }
    t[0x08] = b'b';
    t[0x09] = b't';
    t[0x0A] = b'n';
    t[0x0C] = b'f';
    t[0x0D] = b'r';
    t[b'"' as usize] = b'"';
    t[b'\\' as usize] = b'\\';
    t
};

/// The hex digits of a `\u00XX` escape — lowercase, as `serde_json` writes them.
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

/// The widest escape of one byte: a control byte's `\u00XX`.
const MAX_BYTE_ESCAPE: usize = 6;

/// The widest run [`escape_into`] writes in one step without re-checking
/// [`ESCAPE_LIMIT`]: the sub-word tail, up to seven bytes (fewer than a word) of
/// [`MAX_BYTE_ESCAPE`] each.
const MAX_TAIL_ESCAPE: usize = 7 * MAX_BYTE_ESCAPE;

/// How far into an escape window [`escape_into`] may start a step; past it,
/// [`JsonWriter::string_escaped`] cuts the window and opens a fresh one. What is left
/// of the window past it holds the widest step and the closing quote.
const ESCAPE_LIMIT: usize = ESCAPE_WINDOW - MAX_TAIL_ESCAPE - 1;

/// An escape window, 128 bytes: the widest a step can reach from [`ESCAPE_LIMIT`] is
/// the sub-word tail ([`MAX_TAIL_ESCAPE`]), plus the closing quote.
///
/// ⚠️ The bound is **exact and reachable**, not slack: a step that starts at
/// exactly [`ESCAPE_LIMIT`] with seven `\u00XX` bytes left writes the last index,
/// the closing quote. `escape_windows_hold_their_widest_step` constructs that
/// string (and its neighbours), so a window one byte short fails a test rather
/// than panicking on the first real input to reach it. Don't trim it.
///
/// A window this wide restarts every ~90 output bytes on a long string; a
/// 256-byte one restarted less often and measured no better (−0.006% to
/// −0.03% instructions), since what it saves is a handful of stores per
/// restart.
const ESCAPE_WINDOW: usize = 128;

/// The window a string shorter than a word is escaped into: the widest sub-word
/// run ([`MAX_TAIL_ESCAPE`]) and both quotes. ⚠️ Exact and reachable, like
/// [`ESCAPE_WINDOW`] — seven `\u00XX` bytes fill it to the last index.
const SHORT_ESCAPE_WINDOW: usize = MAX_TAIL_ESCAPE + 2;

/// Write `byte`'s escape at `window[at]` and return the position after it.
/// `form` is `ESCAPE[byte]`, which the caller has already found non-zero.
#[inline]
fn write_escape<const N: usize>(window: &mut [u8; N], at: usize, byte: u8, form: u8) -> usize {
    if form == b'u' {
        return write_control_escape(window, at, byte);
    }
    window[at] = b'\\';
    window[at + 1] = form;
    at + 2
}

/// [`write_escape`]'s `\u00XX` arm — a control byte with no short form, which
/// real source almost never carries. A slice rather than a window of either
/// width, so the cold path is one copy, not one per window.
#[cold]
#[inline(never)]
fn write_control_escape(window: &mut [u8], at: usize, byte: u8) -> usize {
    window[at..at + 6].copy_from_slice(&[
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX_DIGITS[usize::from(byte >> 4)],
        HEX_DIGITS[usize::from(byte & 0xF)],
    ]);
    at + 6
}

/// Escape `bytes[from..]` into `window` from `at`, until the input ends or `at`
/// passes [`ESCAPE_LIMIT`]; returns the new `(at, from)`.
///
/// Clean bytes move a **word** at a time: each eight-byte step is copied to the
/// window whole and tested with [`escape_lanes`], and a clean word is done. A
/// word that flags stops at its lowest flagged lane — the one lane the kernel
/// guarantees genuine, and every byte before it is clean and already copied —
/// and the escapes from there go a **byte** at a time until a clean byte hands
/// back to the word step, because escapes cluster: a newline and the
/// indentation after it is the commonest string that reaches this arm. Fewer
/// than eight bytes from the end, the rest is a byte at a time.
#[inline]
fn escape_into(
    window: &mut [u8; ESCAPE_WINDOW],
    bytes: &[u8],
    mut at: usize,
    mut from: usize,
) -> (usize, usize) {
    while at <= ESCAPE_LIMIT {
        let Some(word) = bytes.get(from..).and_then(<[u8]>::first_chunk::<8>) else {
            // `from` never passes the end; the clamp states it, so the slice has no
            // out-of-bounds arm.
            for &byte in &bytes[from.min(bytes.len())..] {
                let form = ESCAPE[usize::from(byte)];
                if form == 0 {
                    window[at] = byte;
                    at += 1;
                } else {
                    at = write_escape(window, at, byte, form);
                }
            }
            return (at, bytes.len());
        };
        window[at..at + 8].copy_from_slice(word);
        let lanes = escape_lanes(u64::from_le_bytes(*word));
        if lanes == 0 {
            from += 8;
            at += 8;
            continue;
        }
        let clean = (lanes.trailing_zeros() / 8) as usize;
        from += clean;
        at += clean;
        let byte = bytes[from];
        at = write_escape(window, at, byte, ESCAPE[usize::from(byte)]);
        from += 1;
        while at <= ESCAPE_LIMIT
            && let Some(&byte) = bytes.get(from)
        {
            let form = ESCAPE[usize::from(byte)];
            if form == 0 {
                break;
            }
            at = write_escape(window, at, byte, form);
            from += 1;
        }
    }
    (at, from)
}

/// Compact-JSON output buffer.
///
/// All writes are infallible (`Vec<u8>` backing). The escape-sensitive entry
/// points are [`JsonWriter::string`] (full JSON escaping, `serde_json`'s byte
/// for byte) and
/// [`JsonWriter::token`] (quoted verbatim — static ASCII tokens only,
/// debug-asserted).
pub struct JsonWriter {
    buf: Vec<u8>,
    /// Scratch for a staged run (see [`JsonWriter::stage_begin`]). A field
    /// rather than a local so it is initialized **once per writer**, not once
    /// per node — a per-call `[0; STAGE_CAP]` is a memset LLVM cannot prove
    /// dead, which is most of the cost the staging is here to remove.
    stage: [u8; STAGE_CAP],
    stage_len: usize,
}

/// Widest a staged run can be.
///
/// Sized so the widest node header cannot overrun it **on the types alone**,
/// not on an argument about reachable values: the longest node type
/// (`TSConstructSignatureDeclaration`, 31 bytes) plus every static fragment
/// plus all six integer fields at the full 20-digit `usize::MAX` width is 305
/// bytes. Real offsets are far smaller — they come from `u32` spans, so 10
/// digits — but pinning the bound to the type removes the need to re-derive
/// that reasoning whenever a field is added, and the buffer is initialized
/// once per writer, so the slack costs nothing per node.
///
/// Overrunning it panics on the slice bound rather than truncating: a staged
/// run is a fixed, auditable shape, so a field that doesn't fit is a bug to
/// size, not a case to handle. `widest_node_header_fits_the_staging_buffer`
/// is the test that holds this.
const STAGE_CAP: usize = 384;

/// Longest fragment [`JsonWriter::stage_short`] copies inline — two overlapping
/// 16-byte moves reach 32 bytes. Every node type fits (the longest is 31).
const SHORT_FRAGMENT_MAX: usize = 32;

impl JsonWriter {
    /// A fresh writer over a buffer pre-sized to `cap` bytes.
    #[inline]
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            stage: [0; STAGE_CAP],
            stage_len: 0,
        }
    }

    /// Begin a **staged run** — a fixed-shape burst of fragments assembled in
    /// the writer's scratch and appended to the output buffer as one write by
    /// [`JsonWriter::stage_flush`].
    ///
    /// This exists because of what a node header costs when written directly.
    /// The header is 16 appends per AST node (10 static fragments and 6
    /// integers), and each one pays `Vec`'s append protocol: reload `len`,
    /// compute `cap - len`, compare against the fragment width, branch to the
    /// grow path, store, update `len`. The integer emitters make it worse than
    /// it looks — [`JsonWriter::u32`] is deliberately `inline(never)` (a size
    /// constraint; see its comment), so every one of the six calls is opaque
    /// to LLVM and forces the surrounding appends to *re-load* the buffer's
    /// pointer, length and capacity afterwards. None of that bookkeeping
    /// survives staging: the scratch's base never moves, its bound is a
    /// compile-time constant, `stage_len` stays in a register across the whole
    /// header, and the integer emission inlines into the one staging site
    /// instead of reaching an out-of-line emitter per integer.
    ///
    /// The cost it trades for is a single runtime-length `extend_from_slice`
    /// per run — a `memmove` call over ~90 bytes, whose loads read bytes the
    /// staging just stored narrowly (the store-forwarding hazard that made a
    /// *20-byte* fixed blit inside `u32` a ~22%-of-run stall). At header scale
    /// that trade pays and was measured to: the copy is a loop over enough
    /// bytes to amortize both the dispatch and the drain, while the
    /// bookkeeping it removes is 16 checks and 6 reloads. **Grade any change
    /// to this shape on `cycles:u`, not instructions** — the hazard is
    /// invisible to an instruction count.
    ///
    /// Runs do not nest and are not reentrant: `stage_begin` resets the
    /// scratch, so every one must reach its `stage_flush` before the next
    /// begins.
    ///
    /// ⚠️ **This is substrate, not `tsv_ts` machinery** — it was written for
    /// that crate's node header and read as its private business for long
    /// enough that `tsv_svelte`'s writer emitted every integer through the
    /// out-of-line [`JsonWriter::u32`] instead, which cost it ~1.6% of the
    /// wire path. Today `tsv_ts`'s `node_header_impl`, `tsv_svelte`'s
    /// `name_loc` field + `Attribute`/element/`Text` headers, and `tsv_css`'s
    /// three trailing `start`/`end` bursts all stage. A `start`/`end` pair
    /// outside a run is [`JsonWriter::start_end`]'s one call, not two `u32`s.
    ///
    /// ⚠️ **The bar is not frequency alone — a run's STATIC fragments are
    /// copied twice**, once into the scratch and once through the flush, so the
    /// trade is (appends removed) against (static bytes in the run), and a run
    /// that is mostly long static fragments can be a net loss. `tsv_css`'s
    /// *head* bursts (`{"type":"Block","start":` … `,"children":`) are the
    /// measured counter-example: same five appends and same two integers as the
    /// tails, ~50 bytes of static fragment against ~17, and staging them
    /// removes 2.6× more instructions while running ~1.05 points slower. Read
    /// the paragraph above about amortizing over "enough bytes" as being about
    /// the appends a run *removes*, not its width. Each staged emitter also
    /// inlines at its site and so costs bundle bytes. Grade on `cycles`.
    #[inline]
    pub fn stage_begin(&mut self) {
        self.stage_len = 0;
    }

    /// Append a verbatim fragment to the staged run. No escaping — the same
    /// contract as [`JsonWriter::raw`].
    ///
    /// For a **constant** fragment: the copy is `copy_from_slice` at the
    /// fragment's length, which is a fixed-width move only when that length is
    /// a compile-time constant. A fragment chosen at run time (a node type
    /// handed to a header emitter) takes [`JsonWriter::stage_short`] instead.
    #[inline]
    pub fn stage_raw(&mut self, s: &str) {
        let at = self.stage_len;
        let end = at + s.len();
        self.stage[at..end].copy_from_slice(s.as_bytes());
        self.stage_len = end;
    }

    /// Append a short static fragment chosen at run time — a node's `type`
    /// name — to the staged run, byte-identical to [`JsonWriter::stage_raw`].
    ///
    /// `stage_raw`'s runtime-length copy lowers to a libc `memcpy` **call**,
    /// and for a 5–31-byte node type the call is the cost: libc's size
    /// dispatch, plus the scratch pointer and the run's other live values
    /// spilled around it. This moves the same bytes as two overlapping
    /// fixed-width copies chosen by length class — at least 16 bytes: the
    /// first 16 and the last 16; at least 8: 8 + 8; at least 4: 4 + 4; at
    /// least 2: 2 + 2; else the one byte — each a plain register load and
    /// store into a fixed-size window of the scratch. A fragment longer than
    /// [`SHORT_FRAGMENT_MAX`] takes `stage_raw`'s copy.
    ///
    /// ⚠️ The widths are the point. glibc's AVX `memmove` moves a 16–31-byte
    /// copy as the same two overlapping 16-byte halves, so at those lengths
    /// inlining removes the call and the dispatch and changes nothing else. At
    /// exactly 32 bytes the two part ways — glibc switches to 32-byte moves,
    /// this keeps the two 16-byte halves (which then meet) — at a length no
    /// node type reaches. Inlining a *wide* copy is a different trade — the
    /// build's baseline SSE2 moves against libc's 32-byte AVX ones — and an
    /// inline 128-byte blit of a whole staged header measured slower for
    /// exactly that reason. Grade a change here on `cycles`, like every other
    /// staged-run change.
    #[inline]
    pub fn stage_short(&mut self, s: &'static str) {
        let src = s.as_bytes();
        let n = src.len();
        if n > SHORT_FRAGMENT_MAX {
            self.stage_raw_cold(s);
            return;
        }
        let at = self.stage_len;
        let Some(dst) = self.stage[at..].first_chunk_mut::<SHORT_FRAGMENT_MAX>() else {
            // Within a window of the scratch's end: `stage_raw` copies exactly
            // (and panics exactly where it would have).
            self.stage_raw_cold(s);
            return;
        };
        // Each arm writes the fragment's first and last `K` bytes, which
        // overlap (or meet) because `K <= n <= 2K` — so together they are the
        // whole fragment, and `n <= SHORT_FRAGMENT_MAX` keeps both in `dst`.
        if let (Some(head), Some(tail)) = (src.first_chunk::<16>(), src.last_chunk::<16>()) {
            dst[..16].copy_from_slice(head);
            dst[n - 16..n].copy_from_slice(tail);
        } else if let (Some(head), Some(tail)) = (src.first_chunk::<8>(), src.last_chunk::<8>()) {
            dst[..8].copy_from_slice(head);
            dst[n - 8..n].copy_from_slice(tail);
        } else if let (Some(head), Some(tail)) = (src.first_chunk::<4>(), src.last_chunk::<4>()) {
            dst[..4].copy_from_slice(head);
            dst[n - 4..n].copy_from_slice(tail);
        } else if let (Some(head), Some(tail)) = (src.first_chunk::<2>(), src.last_chunk::<2>()) {
            dst[..2].copy_from_slice(head);
            dst[n - 2..n].copy_from_slice(tail);
        } else if let Some(&byte) = src.first() {
            dst[0] = byte;
        }
        self.stage_len = at + n;
    }

    /// [`JsonWriter::stage_short`]'s fallback, out of line so its libc call
    /// stays out of the header emitters: a fragment past the inline limit, or
    /// a window past the scratch's end — neither of which a node header reaches.
    #[cold]
    #[inline(never)]
    fn stage_raw_cold(&mut self, s: &str) {
        self.stage_raw(s);
    }

    /// Append a `u32`'s decimal digits to the staged run.
    ///
    /// Shares [`digit_word`] with [`JsonWriter::u32`], so both emitters have
    /// one arithmetic core and one oracle. Staging is strictly simpler than
    /// the direct path: the scratch always has `WORD_DIGITS` bytes of room, so
    /// the full word is stored and `stage_len` advances by the real digit
    /// count — no `truncate`.
    ///
    /// ⚠️ `inline(always)`, and it is the **same performance constraint
    /// [`digit_word`] carries** — stated here because this is where the
    /// inliner's cost model reads it. The whole point of a staged run is that
    /// `stage_len` and the scratch base stay in registers across the header;
    /// an opaque call per integer spills them, and that is worth ~5% of the
    /// parse→JSON path. The sites are the three writers' staged runs (with
    /// [`JsonWriter::stage_usize`], which narrows onto this), so the size this
    /// costs is bounded by how many runs stage — and it is a *pin*, not a
    /// change of policy: plain `inline` bought the same decision until the
    /// body shrank enough for the cost model to start declining it.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn stage_u32(&mut self, n: u32) {
        let (word, digits) = digit_word(n);
        if digits > WORD_DIGITS {
            self.stage_u32_wide(n, digits);
            return;
        }
        let at = self.stage_len;
        self.stage[at..at + WORD_DIGITS].copy_from_slice(&word.to_le_bytes());
        self.stage_len = at + digits;
    }

    /// The staged wide arm — a value past `99_999_999`, which no offset, line
    /// or column in a real document reaches. `cold` and out-of-line for the
    /// same reason as [`JsonWriter::u64_wide`].
    #[cold]
    #[inline(never)]
    fn stage_u32_wide(&mut self, n: u32, digits: usize) {
        let mut tmp = [0u8; MAX_U32_DIGITS];
        let mut i = digits;
        let mut n = n;
        while i >= 2 {
            let pair = n % 100;
            n /= 100;
            i -= 2;
            tmp[i..i + 2].copy_from_slice(&dec_pair(pair).to_le_bytes());
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        let at = self.stage_len;
        self.stage[at..at + digits].copy_from_slice(&tmp[..digits]);
        self.stage_len = at + digits;
    }

    /// Append a `usize` to the staged run — the line/column channel, which
    /// narrows to the `u32` worker exactly as [`JsonWriter::usize`] does.
    ///
    /// `inline(always)` for [`JsonWriter::stage_u32`]'s reason: it is a
    /// two-line narrowing in front of that worker, so out-of-lining it would
    /// re-introduce exactly the call the worker's own attribute removes.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn stage_usize(&mut self, n: usize) {
        match u32::try_from(n) {
            Ok(n) => self.stage_u32(n),
            Err(_) => self.stage_usize_wide(n),
        }
    }

    /// A line or column past `u32::MAX` — unreachable for any source a `u32`
    /// span can address, but emitted faithfully rather than silently wrong.
    #[cold]
    #[inline(never)]
    fn stage_usize_wide(&mut self, n: usize) {
        let digits = decimal_width(n as u64);
        let mut tmp = [0u8; MAX_U64_DIGITS];
        let mut i = digits;
        let mut n = n as u64;
        while i >= 2 {
            let pair = n % 100;
            n /= 100;
            i -= 2;
            tmp[i..i + 2].copy_from_slice(&dec_pair(pair as u32).to_le_bytes());
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        let at = self.stage_len;
        self.stage[at..at + digits].copy_from_slice(&tmp[..digits]);
        self.stage_len = at + digits;
    }

    /// Append the staged run to the output buffer — the single write the whole
    /// shape exists to reach. Leaves the scratch's contents behind; the next
    /// [`JsonWriter::stage_begin`] is what resets it.
    #[inline]
    pub fn stage_flush(&mut self) {
        self.buf.extend_from_slice(&self.stage[..self.stage_len]);
    }

    /// Consume the writer, yielding the emitted bytes.
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// The bytes written so far (for composing writers / diagnostics).
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Verbatim JSON structure fragment (`{"key":`, `,`, `]`…). No escaping.
    #[inline]
    pub fn raw(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// A quoted static token (node type, operator, kind, keyword). These are
    /// compile-time ASCII strings that never contain `"`, `\`, or control
    /// characters, so they skip the escape scan.
    #[inline]
    pub fn token(&mut self, s: &str) {
        debug_assert!(
            s.bytes().all(|b| b != b'"' && b != b'\\' && b >= 0x20),
            "token must be escape-free: {s:?}"
        );
        self.buf.push(b'"');
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'"');
    }

    /// A dynamic string value, JSON-escaped and quoted.
    ///
    /// Two arms. The prescan [`needs_escape`] asks whether anything here needs
    /// escaping a word at a time, and a clean answer turns the emission into
    /// [`JsonWriter::token`]'s quote-blit-quote — the common case, since
    /// identifier names, string-literal bodies and comment text are
    /// overwhelmingly escape-free. Anything else goes to
    /// [`JsonWriter::string_escaped`], a hand escaper byte-identical to
    /// `serde_json`'s string serialization.
    ///
    /// ⚠️ The prescan's predicate must stay a *superset* of the escaper's set,
    /// or a byte that needs escaping would be blitted raw. That set is
    /// `serde_json`'s: `0x00..=0x1F` plus `"` and `\` — nothing else, in
    /// particular not `DEL` and no non-ASCII byte. Both arms are graded against
    /// `serde_json` itself by `string_matches_serde_json`, exhaustively over a
    /// boundary alphabet and across the 8-byte stride; a corpus cannot see
    /// this (a mis-scan would only surface on the rare input that actually
    /// carries the byte).
    ///
    /// The escaping arm is `inline(never)`. Left to LLVM under plain `inline`
    /// it was inlined here and measured +0.11% / +0.22% / +0.25% instructions
    /// on the Svelte / CSS / TypeScript wire paths.
    #[inline]
    pub fn string(&mut self, s: &str) {
        if needs_escape(s.as_bytes()) {
            self.string_escaped(s.as_bytes());
            return;
        }
        self.buf.push(b'"');
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'"');
    }

    /// [`JsonWriter::string`]'s escaping arm: `bytes` quoted, with every byte
    /// `serde_json` escapes written the way `serde_json` writes it — the seven
    /// two-byte short forms, `\u00XX` with lowercase hex for every other
    /// control byte, and everything else as itself.
    ///
    /// ⚠️ It is not a rare arm. Over real Svelte components 16% of the strings
    /// reaching `string` need escaping, holding 30% of the bytes — template text
    /// is mostly the whitespace between tags (`"\n\t\t"`) — and over real
    /// stylesheets it is 6% of the strings and 24% of the bytes. A per-byte loop
    /// (`serde_json`'s) pays a table lookup and slice bookkeeping per byte and a
    /// libc `memcpy` call per clean run between escapes.
    ///
    /// The output goes into a **window** of the buffer — extended by a fixed
    /// width, written through a slice held in registers, then truncated to
    /// what was written — the shape [`JsonWriter::start_end`] uses, so no
    /// store pays `Vec`'s append protocol. [`escape_into`] fills it, clean
    /// bytes a word at a time. A string shorter than a word, most escaping
    /// template text, skips the word step for a byte loop into a narrower
    /// window.
    #[inline(never)]
    #[expect(clippy::expect_used)]
    fn string_escaped(&mut self, bytes: &[u8]) {
        if bytes.len() < 8 {
            let base = self.buf.len();
            self.buf.extend_from_slice(&[0; SHORT_ESCAPE_WINDOW]);
            let window = self
                .buf
                .get_mut(base..)
                .and_then(<[u8]>::first_chunk_mut::<SHORT_ESCAPE_WINDOW>)
                .expect("the window was just appended");
            window[0] = b'"';
            let mut at = 1;
            for &byte in bytes {
                let form = ESCAPE[usize::from(byte)];
                if form == 0 {
                    window[at] = byte;
                    at += 1;
                } else {
                    at = write_escape(window, at, byte, form);
                }
            }
            window[at] = b'"';
            self.buf.truncate(base + at + 1);
            return;
        }
        let (mut at, mut from) = (1, 0);
        loop {
            let base = self.buf.len();
            self.buf.extend_from_slice(&[0; ESCAPE_WINDOW]);
            let window = self
                .buf
                .get_mut(base..)
                .and_then(<[u8]>::first_chunk_mut::<ESCAPE_WINDOW>)
                .expect("the window was just appended");
            // The opening quote; past the first window the escape overwrites it.
            window[0] = b'"';
            (at, from) = escape_into(window, bytes, at, from);
            if from == bytes.len() {
                window[at] = b'"';
                self.buf.truncate(base + at + 1);
                return;
            }
            self.buf.truncate(base + at);
            at = 0;
        }
    }

    /// The same dynamic string as two consecutive field values —
    /// `"<s>"`, then the verbatim `between` fragment, then `"<s>"` again —
    /// byte-identical to `string(s); raw(between); string(s)`, with `s`
    /// escaped once.
    ///
    /// For the node that carries one string twice: a Svelte `Text` emits its
    /// `raw` and its `data`, which are the same bytes whenever the text decodes
    /// to itself — and most template text is the whitespace between tags
    /// (`"\n\t\t"`), which needs escaping, so two [`JsonWriter::string`] calls
    /// would run the escaper over it twice. The second field is instead a copy
    /// of the first one's **emitted** bytes (`extend_from_within`), which is
    /// correct by construction: the escaped form is a pure function of `s`.
    ///
    /// ⚠️ `inline(always)`, because `between` is only cheap as a constant. Under
    /// plain `inline` the release build outlined this body and tail-called it,
    /// so `between` reached it as a runtime slice and its copy became a libc
    /// `memcpy` call behind a reserve check of its own — a second call beside
    /// the copy of the escaped value, which is runtime-length either way.
    /// Inlined, the fragment is a fixed-width move at each of the two call
    /// sites, and that is −0.39% of the Svelte wire path's `instructions:u`
    /// over 3,048 real components (the TypeScript and CSS paths never reach
    /// it) for +224 bytes of native `.text`.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    pub fn string_pair(&mut self, s: &str, between: &str) {
        let start = self.buf.len();
        self.string(s);
        let end = self.buf.len();
        self.buf.extend_from_slice(between.as_bytes());
        self.buf.extend_from_within(start..end);
    }

    /// A non-integral `f64` (the rare literal tail) — `serde_json`'s ryu
    /// formatting, matching `serde_json::Number` serialization.
    #[inline]
    #[expect(clippy::expect_used)]
    pub fn f64(&mut self, n: f64) {
        serde_json::to_writer(&mut self.buf, &n).expect("Vec<u8> write is infallible");
    }

    // ⚠️ `inline(never)`, and that is a **size** constraint. The body below is
    // small enough that LLVM will happily inline it at every writer call site,
    // and each copy carries the fixed-width blit — which, with every
    // `start`/`end` pair routed through here, grew the `@fuzdev/tsv-parse-wasm`
    // bundle 6% and blew its publish size bound. Out-of-line the win is
    // unaffected: it comes from removing the libc `memmove` **call** inside the
    // body, not from removing the call *to* the body, which any out-of-line
    // integer emitter pays.
    #[inline(never)]
    pub fn u32(&mut self, n: u32) {
        // The append is a *constant*-length copy of [`digit_word`]'s register,
        // then a `truncate` — a runtime-length copy here would lower to a libc
        // `memmove` **call** whose size-ladder dispatch costs several times the
        // 1–3 bytes it moves. (The staged path pays no such copy at all: its
        // destination is the writer's scratch, which always has room for the
        // full word, so it just advances by `digits`.)
        let (word, digits) = digit_word(n);
        if digits > WORD_DIGITS {
            self.u64_wide(u64::from(n), digits);
            return;
        }
        let len = self.buf.len();
        self.buf.extend_from_slice(&word.to_le_bytes());
        self.buf.truncate(len + digits);
    }

    /// A node's `start` value, the `,"end":` key, and its `end` value —
    /// byte-identical to `u32(start); raw(",\"end\":"); u32(end)`, the pair
    /// nearly every node carries, as one call.
    ///
    /// For a caller whose own literal ends in `"start":` (a node head
    /// `{"type":"Block","start":`); a pair that opens with the key alone takes
    /// [`JsonWriter::start_end_field`] or [`JsonWriter::start_end_object`].
    /// Three appends become one: the buffer takes a fixed-width window once,
    /// both [`digit_word`] registers and the key land in it at offsets known
    /// once each width is, and a `truncate` trims the zero padding the widths
    /// leave. The key is stored as a whole word — its eighth byte is
    /// overwritten by the `end` digits, which follow it at `+7`.
    ///
    /// `inline(never)` for [`JsonWriter::u32`]'s reason, and more so: the
    /// body holds two inlined `digit_word` copies, where two `u32` calls would
    /// share one out-of-line copy with every other integer.
    #[inline(never)]
    pub fn start_end(&mut self, start: u32, end: u32) {
        let (start_word, start_digits) = digit_word(start);
        let (end_word, end_digits) = digit_word(end);
        if start_digits > WORD_DIGITS || end_digits > WORD_DIGITS {
            self.start_end_wide(start, end);
            return;
        }
        let len = self.buf.len();
        self.buf.extend_from_slice(&[0; START_END_WINDOW]);
        let window = &mut self.buf[len..len + START_END_WINDOW];
        let used = fill_start_end(window, (start_word, start_digits), (end_word, end_digits));
        self.buf.truncate(len + used);
    }

    /// `,"start":` N `,"end":` M — [`JsonWriter::start_end`] with its key,
    /// for a pair that trails other fields.
    #[inline]
    pub fn start_end_field(&mut self, start: u32, end: u32) {
        self.start_end_led(b',', start, end);
    }

    /// `{"start":` N `,"end":` M — [`JsonWriter::start_end`] with its key,
    /// for a node whose fields open with its positions.
    #[inline]
    pub fn start_end_object(&mut self, start: u32, end: u32) {
        self.start_end_led(b'{', start, end);
    }

    /// The one body behind [`JsonWriter::start_end_field`] and
    /// [`JsonWriter::start_end_object`]: [`JsonWriter::start_end`]'s window
    /// with the lead byte and `"start":` ahead of it. The key is part of the
    /// constant the window is filled from, so writing it costs the lead byte's
    /// store alone. `inline(never)` for `start_end`'s reason.
    #[inline(never)]
    fn start_end_led(&mut self, lead: u8, start: u32, end: u32) {
        let (start_word, start_digits) = digit_word(start);
        let (end_word, end_digits) = digit_word(end);
        if start_digits > WORD_DIGITS || end_digits > WORD_DIGITS {
            self.start_end_led_wide(lead, start, end);
            return;
        }
        let len = self.buf.len();
        self.buf.extend_from_slice(&LED_TEMPLATE);
        let window = &mut self.buf[len..len + LED_WINDOW];
        window[0] = lead;
        let at = 1 + START_KEY.len();
        let used = fill_start_end(
            &mut window[at..],
            (start_word, start_digits),
            (end_word, end_digits),
        );
        self.buf.truncate(len + at + used);
    }

    /// [`JsonWriter::start_end`] with either value past `99_999_999`, which no
    /// offset in a real document reaches — the three separate appends, each
    /// integer through [`JsonWriter::u32`].
    #[cold]
    #[inline(never)]
    fn start_end_wide(&mut self, start: u32, end: u32) {
        self.u32(start);
        self.raw(END_KEY);
        self.u32(end);
    }

    /// [`JsonWriter::start_end_led`]'s wide arm, for `start_end_wide`'s reason.
    #[cold]
    #[inline(never)]
    fn start_end_led_wide(&mut self, lead: u8, start: u32, end: u32) {
        self.buf.push(lead);
        self.raw(START_KEY);
        self.start_end_wide(start, end);
    }

    /// A `u64` value. **Every integer the writers actually emit — offsets,
    /// lines, columns — fits `u32`**, so this is a dispatcher, not a second
    /// implementation: the `u32` arm carries the real work and keeps its
    /// arithmetic 32-bit. The compare is one perfectly-predicted branch.
    #[inline]
    pub fn u64(&mut self, n: u64) {
        match u32::try_from(n) {
            Ok(n) => self.u32(n),
            Err(_) => self.u64_wide(n, decimal_width(n)),
        }
    }

    /// The wide arm — a value past `99_999_999`, which no offset, line or
    /// column in a real document reaches. Out-of-line and `cold` so the hot arm
    /// keeps its straight-line shape and the wide scratch never enters its
    /// stack frame.
    #[cold]
    #[inline(never)]
    fn u64_wide(&mut self, n: u64, digits: usize) {
        let mut tmp = [0u8; MAX_U64_DIGITS];
        let mut i = digits;
        let mut n = n;
        while i >= 2 {
            let pair = n % 100;
            n /= 100;
            i -= 2;
            tmp[i..i + 2].copy_from_slice(&dec_pair(pair as u32).to_le_bytes());
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        self.buf.extend_from_slice(&tmp[..digits]);
    }

    #[inline]
    pub fn i64(&mut self, n: i64) {
        if n < 0 {
            self.buf.push(b'-');
        }
        self.u64(n.unsigned_abs());
    }

    /// A `usize` value — the writers' line and column channel. Narrows to the
    /// `u32` worker rather than widening to `u64`, so lines and columns keep
    /// 32-bit arithmetic too; only a value no source could produce takes the
    /// wide arm.
    #[inline]
    pub fn usize(&mut self, n: usize) {
        self.u64(n as u64);
    }

    #[inline]
    pub fn bool(&mut self, b: bool) {
        self.raw(if b { "true" } else { "false" });
    }

    #[inline]
    pub fn null(&mut self) {
        self.raw("null");
    }
}

/// Emit a JSON array: `[` + comma-separated items + `]`.
#[inline]
pub fn write_array<T>(
    w: &mut JsonWriter,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut JsonWriter, T),
) {
    w.raw("[");
    let mut first = true;
    for item in items {
        if !first {
            w.raw(",");
        }
        first = false;
        f(w, item);
    }
    w.raw("]");
}

/// Emit a nullable node value: the item through `f`, or `null` — the writer's
/// shape for every `Option` field *without* `skip_serializing_if`.
#[inline]
pub fn write_or_null<T>(w: &mut JsonWriter, item: Option<&T>, f: impl FnOnce(&mut JsonWriter, &T)) {
    match item {
        Some(v) => f(w, v),
        None => w.null(),
    }
}

/// Integer emission is **arithmetic**, and no corpus can grade arithmetic: a
/// wrong digit width writes a wrong offset that still parses as JSON, so a
/// fixture suite, a byte-diff over thousands of files, and every audit gate can
/// all stay green through the bug. The oracle has to live at the declaration —
/// these tests grade `u64` (and the `decimal_width` reservation it trusts)
/// against `std`'s own formatting, exhaustively where exhaustion is possible
/// and over every boundary where it isn't.
#[cfg(test)]
mod tests {
    use super::*;

    fn emit(n: u64) -> String {
        let mut w = JsonWriter::with_capacity(0);
        w.u64(n);
        String::from_utf8(w.into_bytes()).expect("digits are ASCII")
    }

    fn emit_string(s: &str) -> Vec<u8> {
        let mut w = JsonWriter::with_capacity(0);
        w.string(s);
        w.into_bytes()
    }

    /// [`JsonWriter::string`] must be byte-identical to `serde_json`'s own
    /// string serialization — the parity contract this module's whole doc
    /// comment rests on — on both of its arms: the escape-free fast path and
    /// the hand escaper.
    ///
    /// Graded exhaustively over [`escape_cases`], whose alphabet carries every
    /// byte `serde_json` escapes and the bytes adjacent to each boundary: the
    /// named escapes, every unnamed control (`\u00XX`, whose hex case is part
    /// of the contract), both literal escapes, `0x20` and `DEL` (which
    /// `serde_json` does **not** escape), and multibyte characters whose UTF-8
    /// bytes are all `>= 0x80`. A corpus cannot grade this: a mis-scan only
    /// surfaces on an input that actually carries the byte, and several of
    /// these never appear in real source at all.
    #[test]
    fn string_matches_serde_json() {
        for case in &escape_cases() {
            let ours = emit_string(case);
            let theirs = serde_json::to_vec(case).expect("serde_json serializes a str");
            assert_eq!(
                ours,
                theirs,
                "escape parity broke on {case:?}: ours {:?}, serde_json {:?}",
                String::from_utf8_lossy(&ours),
                String::from_utf8_lossy(&theirs)
            );
        }
    }

    /// The escape windows' widths are exact, so the strings that reach their
    /// last byte are graded here, against `serde_json`: a lead of escapes (none,
    /// one to three short forms, one or two `\u00XX`) shifting the output by
    /// every amount, a clean run of every length to twice the window — so a
    /// step starts at every odd offset of the first window up to
    /// [`ESCAPE_LIMIT`], the largest start there is — and a tail of up to
    /// fifteen `\u00XX` bytes, the widest thing a step can write. A window one
    /// byte short panics on these (checked by shrinking each window by one);
    /// the general case set reaches the short window's bound but not
    /// [`ESCAPE_WINDOW`]'s, so this is the one test that guards it.
    #[test]
    fn escape_windows_hold_their_widest_step() {
        for lead in ["", "\"", "\"\"", "\"\"\"", "\u{1}", "\u{1}\u{1}"] {
            for clean in 0..=2 * ESCAPE_WINDOW {
                for tail in 0..=15 {
                    let case = format!("{lead}{}{}", "a".repeat(clean), "\u{1}".repeat(tail));
                    assert_eq!(
                        emit_string(&case),
                        serde_json::to_vec(&case).expect("serde_json serializes a str"),
                        "escape parity broke on {case:?}"
                    );
                }
            }
        }
    }

    /// [`JsonWriter::string_pair`] against its definition — `string`, the
    /// fragment, `string` again — over the escape-parity cases, behind a
    /// non-empty prefix so the copied range starts past the buffer's front.
    #[test]
    fn string_pair_matches_two_string_calls() {
        for case in &escape_cases() {
            let mut ours = JsonWriter::with_capacity(0);
            ours.raw("{\"raw\":");
            ours.string_pair(case, ",\"data\":");
            let mut theirs = JsonWriter::with_capacity(0);
            theirs.raw("{\"raw\":");
            theirs.string(case);
            theirs.raw(",\"data\":");
            theirs.string(case);
            let (ours, theirs) = (ours.into_bytes(), theirs.into_bytes());
            assert_eq!(
                ours,
                theirs,
                "string_pair broke on {case:?}: ours {:?}, two strings {:?}",
                String::from_utf8_lossy(&ours),
                String::from_utf8_lossy(&theirs)
            );
        }
    }

    /// The escape-parity case set `string_matches_serde_json` grades.
    ///
    /// Its alphabet is every byte `serde_json` escapes (all of `0x00..=0x1F`,
    /// `"`, `\`), the bytes either side of each cutoff that it does **not**
    /// (`0x20`, `DEL`), a plain letter, and a character of each multi-byte
    /// UTF-8 width. Over it: every string of length 0–2, and 0–3 over a
    /// boundary subset; each member at every offset of every length to 40; a
    /// run of escapes of every length at every offset; each member repeated
    /// to 40 (the all-`\u00XX` case is the widest expansion); every pair of
    /// members at every pair of offsets to 20; a needle at every offset of a
    /// 300-byte string; and a seeded mix of long strings. The long cases are
    /// there for the hand escaper's window, which restarts every ~90 output
    /// bytes, so an escape has to land on each side of every restart.
    fn escape_cases() -> Vec<String> {
        let mut alphabet: Vec<String> = (0u8..0x20).map(|b| char::from(b).to_string()).collect();
        alphabet.extend(
            [" ", "\"", "\\", "\u{7f}", "a", "é", "€", "😀"]
                .into_iter()
                .map(String::from),
        );
        const BOUNDARY: [&str; 12] = [
            "\u{0}", "\u{8}", "\t", "\n", "\u{c}", "\r", "\u{1f}", " ", "\"", "\\", "\u{7f}", "é",
        ];
        let mut cases: Vec<String> = vec![String::new()];
        for x in &alphabet {
            cases.push(x.clone());
            for y in &alphabet {
                cases.push(format!("{x}{y}"));
            }
        }
        for x in BOUNDARY {
            for y in BOUNDARY {
                for z in BOUNDARY {
                    cases.push(format!("{x}{y}{z}"));
                }
            }
        }
        // ⚠️ The axis a word-at-a-time scan fails on: the same needle at every
        // offset across the 8-byte stride. A corpus samples alignment
        // arbitrarily; this pins all of it. The **total length** is swept as
        // well as the offset, and both matter — a fixed length that is a
        // multiple of 8 never exercises the remainder, and an offset that never
        // reaches the last word never puts the needle *in* the remainder. Each
        // of those holes has hidden a corruption probe that the other caught.
        for piece in &alphabet {
            for len in 0..=40usize {
                for offset in 0..=len {
                    let mut s = "a".repeat(offset);
                    s.push_str(piece);
                    s.push_str(&"b".repeat(len - offset));
                    cases.push(s);
                }
                cases.push(piece.repeat(len));
            }
        }
        // A run of escapes of every length, starting at every offset: the
        // escaper leaves its word hop for a byte loop at the first escape and
        // returns at the first clean byte, and both hand-offs move with these.
        const RUN: [&str; 6] = ["\n", "\t", "\"", "\\", "\u{1}", "\r"];
        for run in 1..=16usize {
            for offset in 0..=24usize {
                let mut s = "a".repeat(offset);
                for j in 0..run {
                    s.push_str(RUN[(j + offset) % RUN.len()]);
                }
                s.push_str("bc");
                cases.push(s.clone());
                s.push_str(&"d".repeat(17));
                cases.push(s);
            }
        }
        const PAIR: [&str; 7] = ["\n", "\u{1}", "\"", "\\", "é", "😀", "\u{7f}"];
        for x in PAIR {
            for y in PAIR {
                for i in 0..20usize {
                    for j in i..20usize {
                        let mut s = "a".repeat(i);
                        s.push_str(x);
                        s.push_str(&"b".repeat(j - i));
                        s.push_str(y);
                        s.push_str(&"c".repeat(20 - j));
                        cases.push(s);
                    }
                }
            }
        }
        for piece in &alphabet {
            for offset in 0..300usize {
                let mut s = "a".repeat(offset);
                s.push_str(piece);
                s.push_str(&"b".repeat(300 - offset));
                cases.push(s);
            }
        }
        // Seeded long mixes, weighted toward escapes so windows fill with
        // every kind of step (a clean word, a hop, a run, the byte tail).
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..3_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let len = (seed % 257) as usize;
            let mut s = String::new();
            for _ in 0..len {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let pick = (seed % 64) as usize;
                s.push_str(if pick < alphabet.len() {
                    &alphabet[pick]
                } else {
                    "x"
                });
            }
            cases.push(s);
        }
        cases
    }

    /// The prescan's own answer, against a per-byte oracle — so a
    /// `needs_escape` regression is reported here rather than only as a
    /// slower-but-correct fallback.
    ///
    /// ⚠️ The **length** sweep is as load-bearing as the offset sweep: the
    /// prescan has three arms — whole words, then an overlapping final word for
    /// a remainder (never reached at a multiple of 8), and for a slice shorter
    /// than a word a gathered one (two overlapping four-byte loads, or the
    /// first, middle and last byte under four) — and a corruption in an arm the
    /// sweep skips reads green. Every length 1–24 × every offset within it puts
    /// each needle in the word body, the overlap and the gather.
    #[test]
    fn needs_escape_matches_the_per_byte_predicate() {
        for needle in 0..=u8::MAX {
            for len in 1..=24usize {
                for offset in 0..len {
                    let mut bytes = vec![b'a'; len];
                    bytes[offset] = needle;
                    let expected = bytes.iter().any(|&b| b < 0x20 || b == b'"' || b == b'\\');
                    assert_eq!(
                        needs_escape(&bytes),
                        expected,
                        "byte {needle:#04x} at offset {offset} of {len}"
                    );
                }
            }
        }
        assert!(!needs_escape(b""), "the empty slice needs no escaping");
    }

    #[test]
    fn decimal_width_matches_std_formatting() {
        // Exhaustive over every value std can disagree on by one digit: each
        // power of ten and its two neighbours, plus the u64 ceiling.
        let mut pow = 1u64;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    decimal_width(n),
                    n.to_string().len(),
                    "decimal_width({n}) disagrees with std"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, u64::MAX, u64::MAX - 1, u32::MAX as u64, i64::MAX as u64] {
            assert_eq!(decimal_width(n), n.to_string().len(), "decimal_width({n})");
        }
    }

    #[test]
    fn decimal_width_u32_matches_std_formatting() {
        // Exhaustive over every value std can disagree on by one digit, plus
        // the u32 ceiling — the width the hot arm front-aligns to.
        let mut pow = 1u32;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    decimal_width_u32(n),
                    n.to_string().len(),
                    "decimal_width_u32({n}) disagrees with std"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, u32::MAX, u32::MAX - 1, i32::MAX as u32] {
            assert_eq!(decimal_width_u32(n), n.to_string().len(), "u32({n})");
        }
    }

    /// Grade an integer emitter against `u32::to_string` over its whole range: every
    /// power-of-ten edge (including 8→9 digits, where the register arm hands off to the
    /// wide one), the range ends, and an LCG sweep for digit-pair indexing errors. `name`
    /// labels a failure.
    fn assert_u32_emitter(name: &str, emit: impl Fn(u32) -> String) {
        let mut pow = 1u32;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(emit(n), n.to_string(), "{name}({n}) at a power-of-ten edge");
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, 1, u32::MAX, u32::MAX - 1, i32::MAX as u32] {
            assert_eq!(emit(n), n.to_string(), "{name}({n})");
        }
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            for n in [state as u32, (state >> 17) as u32, (state >> 33) as u32] {
                assert_eq!(emit(n), n.to_string(), "{name}({n})");
            }
        }
    }

    #[test]
    fn u32_matches_std_across_its_whole_range() {
        // `u32` is the worker, so grade it directly rather than only through
        // the `u64` dispatcher.
        assert_u32_emitter("u32", |n| {
            let mut w = JsonWriter::with_capacity(0);
            w.u32(n);
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        });
    }

    #[test]
    fn start_end_family_matches_format() {
        // Every width pair — both values at each power-of-ten edge, so each
        // value's width moves the key and the second value independently,
        // including the 8→9-digit hand-off to the wide arm on either side.
        fn emit(start: u32, end: u32) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.raw("{");
            w.start_end(start, end);
            w.raw("}");
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        fn emit_led(start: u32, end: u32) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.start_end_object(start, end);
            w.start_end_field(end, start);
            w.raw("}");
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        let mut edges = vec![0, 1, u32::MAX, u32::MAX - 1];
        let mut pow = 10u32;
        loop {
            edges.extend([pow - 1, pow, pow + 1]);
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for &start in &edges {
            for &end in &edges {
                assert_eq!(
                    emit(start, end),
                    format!("{{{start},\"end\":{end}}}"),
                    "start_end({start}, {end})"
                );
                assert_eq!(
                    emit_led(start, end),
                    format!("{{\"start\":{start},\"end\":{end},\"start\":{end},\"end\":{start}}}"),
                    "start_end_object({start}, {end}) + start_end_field({end}, {start})"
                );
            }
        }
    }

    #[test]
    fn u64_matches_std_exhaustively_over_small_values() {
        // Every value through five digits — the bulk of what the writers emit
        // (offsets, lines, columns), across `digit_word`'s one- through
        // five-digit arms and the six-digit boundary, and both arms of the
        // final odd/even-digit branch at every length.
        for n in 0..=100_000u64 {
            assert_eq!(emit(n), n.to_string(), "u64({n})");
        }
    }

    #[test]
    fn u64_matches_std_at_every_boundary_and_across_the_range() {
        let mut pow = 1u64;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(emit(n), n.to_string(), "u64({n}) at a power-of-ten edge");
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [u64::MAX, u64::MAX - 1, u32::MAX as u64, i64::MAX as u64] {
            assert_eq!(emit(n), n.to_string(), "u64({n})");
        }
        // A deterministic LCG sweep, to catch a digit-pair indexing error that
        // the structured cases above could miss.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            for n in [state, state >> 17, state >> 33, state >> 49] {
                assert_eq!(emit(n), n.to_string(), "u64({n})");
            }
        }
    }

    /// The staged emitter is a **second** arithmetic path to the same digits,
    /// and it is the one every node header now goes through — so it needs its
    /// own oracle, not the direct path's — the same sweep
    /// ([`assert_u32_emitter`]).
    #[test]
    fn stage_u32_matches_std_across_its_whole_range() {
        assert_u32_emitter("stage_u32", |n| {
            let mut w = JsonWriter::with_capacity(0);
            w.stage_begin();
            w.stage_u32(n);
            w.stage_flush();
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        });
    }

    /// The staged `usize` channel (lines and columns), including the cold arm
    /// past `u32::MAX` that no real source reaches but which must still emit
    /// the true value rather than a truncated one.
    #[test]
    fn stage_usize_matches_std_including_past_u32() {
        fn emit_staged(n: usize) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.stage_begin();
            w.stage_usize(n);
            w.stage_flush();
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        for n in 0..=2_000usize {
            assert_eq!(emit_staged(n), n.to_string(), "stage_usize({n})");
        }
        for n in [
            u32::MAX as usize - 1,
            u32::MAX as usize,
            u32::MAX as usize + 1,
            u64::MAX as usize,
            usize::MAX,
        ] {
            assert_eq!(emit_staged(n), n.to_string(), "stage_usize({n})");
        }
    }

    /// A staged run must reproduce exactly what the direct emitters would have
    /// written — the property that makes the staging a pure performance
    /// change. Grades a realistic node-header shape (the widest one: a long
    /// node type plus both `character` fields) against the direct path.
    #[test]
    fn staged_run_matches_the_direct_emitters_byte_for_byte() {
        let ints = [0u32, 7, 42, 999, 1_000, 65_535, 9_999_999, 100_000_000];
        for (i, &start) in ints.iter().enumerate() {
            for &end in &ints[i..] {
                let mut staged = JsonWriter::with_capacity(0);
                staged.stage_begin();
                staged.stage_raw("{\"type\":\"TSConstructSignatureDeclaration\"");
                staged.stage_raw(",\"start\":");
                staged.stage_u32(start);
                staged.stage_raw(",\"end\":");
                staged.stage_u32(end);
                staged.stage_raw(",\"loc\":{\"start\":{\"line\":");
                staged.stage_usize(start as usize);
                staged.stage_raw(",\"character\":");
                staged.stage_u32(end);
                staged.stage_raw("}}");
                staged.stage_flush();

                let mut direct = JsonWriter::with_capacity(0);
                direct.raw("{\"type\":\"TSConstructSignatureDeclaration\"");
                direct.raw(",\"start\":");
                direct.u32(start);
                direct.raw(",\"end\":");
                direct.u32(end);
                direct.raw(",\"loc\":{\"start\":{\"line\":");
                direct.usize(start as usize);
                direct.raw(",\"character\":");
                direct.u32(end);
                direct.raw("}}");

                assert_eq!(
                    staged.into_bytes(),
                    direct.into_bytes(),
                    "staged run diverged from the direct emitters at ({start}, {end})"
                );
            }
        }
    }

    /// [`JsonWriter::stage_short`] against [`JsonWriter::stage_raw`], at every
    /// fragment length through the inline limit and past it (the fallback),
    /// behind prefixes that move the window across the scratch — and right up
    /// against the scratch's end, where the fixed window no longer fits and the
    /// fallback copies exactly. The overlap arithmetic is what can go wrong
    /// here, and a node-type corpus reaches only 5–31 bytes of it.
    #[test]
    fn stage_short_matches_stage_raw() {
        const TEXT: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for len in 0..=SHORT_FRAGMENT_MAX + 8 {
            let fragment: &'static str = &TEXT[..len];
            for prefix in [
                0,
                1,
                9,
                17,
                STAGE_CAP - SHORT_FRAGMENT_MAX - 3,
                STAGE_CAP - len - 1,
            ] {
                // The trailing `!` needs one byte past the fragment.
                if prefix + len + 1 > STAGE_CAP {
                    continue;
                }
                let lead = "x".repeat(prefix);
                let mut ours = JsonWriter::with_capacity(0);
                ours.stage_begin();
                ours.stage_raw(&lead);
                ours.stage_short(fragment);
                ours.stage_raw("!");
                ours.stage_flush();
                let mut theirs = JsonWriter::with_capacity(0);
                theirs.stage_begin();
                theirs.stage_raw(&lead);
                theirs.stage_raw(fragment);
                theirs.stage_raw("!");
                theirs.stage_flush();
                assert_eq!(
                    ours.into_bytes(),
                    theirs.into_bytes(),
                    "stage_short diverged at len {len}, prefix {prefix}"
                );
            }
        }
    }

    /// Consecutive runs must not leak: `stage_begin` is the only reset, so a
    /// shorter run following a longer one must not carry the tail of the
    /// previous run's scratch into the output.
    #[test]
    fn staged_runs_do_not_leak_between_each_other() {
        let mut w = JsonWriter::with_capacity(0);
        w.stage_begin();
        w.stage_raw(",\"aVeryLongFragmentIndeed\":");
        w.stage_u32(4_294_967_295);
        w.stage_flush();
        w.stage_begin();
        w.stage_raw(",\"x\":");
        w.stage_u32(1);
        w.stage_flush();
        assert_eq!(
            String::from_utf8(w.into_bytes()).expect("ASCII"),
            ",\"aVeryLongFragmentIndeed\":4294967295,\"x\":1"
        );
    }

    /// The staged run's widest realistic shape must fit `STAGE_CAP` — the
    /// bound is a panic, not a truncation, so it has to be proven rather than
    /// assumed. Longest node type + every position field at `u32::MAX` width +
    /// both `character` fields.
    #[test]
    fn widest_node_header_fits_the_staging_buffer() {
        let mut w = JsonWriter::with_capacity(0);
        w.stage_begin();
        w.stage_raw("{\"type\":\"");
        w.stage_raw("TSConstructSignatureDeclaration");
        w.stage_raw("\"");
        w.stage_raw(",\"start\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"end\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"loc\":{\"start\":{\"line\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"column\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"character\":");
        w.stage_usize(usize::MAX);
        w.stage_raw("},\"end\":{\"line\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"column\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"character\":");
        w.stage_usize(usize::MAX);
        w.stage_raw("}}");
        w.stage_flush();
        // Headroom check: the widest run must leave room, not just barely fit.
        assert!(
            w.as_bytes().len() < STAGE_CAP,
            "widest staged header is {} bytes, STAGE_CAP is {STAGE_CAP}",
            w.as_bytes().len()
        );
    }

    #[test]
    fn u64_appends_without_disturbing_prior_bytes() {
        // The append writes a fixed-width word and truncates back; this pins
        // that it neither clobbers what precedes it nor leaves any of the
        // over-written tail behind, across a realloc and at every digit width.
        let mut w = JsonWriter::with_capacity(0);
        let mut expected = String::new();
        for n in (0..5_000u64).map(|i| i.wrapping_mul(2_654_435_761)) {
            w.raw(",");
            expected.push(',');
            w.u64(n);
            expected.push_str(&n.to_string());
        }
        assert_eq!(String::from_utf8(w.into_bytes()).expect("ASCII"), expected);
    }

    #[test]
    fn i64_matches_std() {
        for n in [
            0,
            1,
            -1,
            i64::MAX,
            i64::MIN,
            i64::MIN + 1,
            -99,
            -100,
            i32::MIN as i64,
        ] {
            let mut w = JsonWriter::with_capacity(0);
            w.i64(n);
            assert_eq!(
                String::from_utf8(w.into_bytes()).expect("ASCII"),
                n.to_string(),
                "i64({n})"
            );
        }
    }
}
