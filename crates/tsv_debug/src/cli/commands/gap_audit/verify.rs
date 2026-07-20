//! The audit's self-verify pass: re-derives each kept finding's observable claim by
//! re-splicing the payload at its recorded offset and checking whether the formatted
//! output's comment content actually changed. Split out of `gap_audit.rs` for navigability.

use std::collections::BTreeMap;

use crate::audit::properties::{Formatted, Verdict, ledger_format_with_comments};
use tsv_cli::cli::input::ParserType;

use super::{Example, Kind, Payload};

/// Re-derive a finding's **observable** claim, independently of the ledger that made it.
///
/// The ledger is an instrument, and an instrument that only ever agrees with itself is not
/// evidence — every mistake found while building this audit was of exactly that shape (a
/// stale needle, a char-vs-byte offset, checking the injected comment when the finding was
/// about a bystander). So each kept example is re-run and the ledger is made to *predict*
/// something falsifiable: if it says this format drops `d` comments and double-prints `p`,
/// then the output must reparse to exactly `parsed - d + p` comments. Anything else means
/// the ledger's account and the actual output disagree.
///
/// The caller runs this over up to `VERIFY_EXAMPLES` examples per shape and reduces the
/// per-example verdicts into a `VerifyOutcome` ratio: all-confirmed is clean, all-unconfirmed
/// is a uniform instrument gap, and a split is a mixed real drop.
///
/// Deciding via the multiset of comment **contents** rather than a count is what makes this
/// both sound and decisive. A printer may legitimately re-indent a multi-line comment, which
/// a raw text match would false-alarm on — so each content is whitespace-normalized
/// ([`normalize_comment_text`]) before it becomes a multiset element: a re-indent
/// (`/* a⏎   b */` → `/* a⏎b */`) keeps the newline and normalizes equal, while a **mangle**
/// (`/* a⏎b */` → `/* ab */`) drops the newline and normalizes different. And unlike the
/// earlier `parsed - dropped + double` count, the multiset closes that count's two blind
/// spots: a balancing drop+duplicate nets zero (equal count, unequal multiset), and a
/// mangle is count-invariant (equal count, unequal content).
///
/// So: the injected source's comment contents vs the output's. Equal ⇒ every comment is
/// content-conserved, so a ledger finding here is contradicted by the output — a genuine
/// **instrument gap** ([`Verdict::Unconfirmed`], now provably so). Unequal ⇒ a content is
/// missing, mangled, or duplicated — real loss/corruption ([`Verdict::Confirmed`]).
///
/// The residual blind spot, named rather than hidden and far narrower than the count's: a
/// multiset can still balance if the SAME content is dropped in one place and duplicated in
/// another. No example in the corpus does this, and the kept examples are a sample of the
/// shape's hits, never a proof about all of them.
pub(super) fn verify_example(example: &Example, kind: Kind, parser: ParserType) -> Verdict {
    // A panic is self-evident: it either happens or it doesn't, and it was caught to get here.
    if kind == Kind::Panic {
        return Verdict::Confirmed;
    }
    let Ok(source) = std::fs::read_to_string(&example.path) else {
        return Verdict::Unconfirmed;
    };
    let Some(payload) = Payload::from_label(example.payload) else {
        return Verdict::Unconfirmed;
    };
    // Re-create the finding by re-splicing at the INJECTION offset (never the attribution one)
    // — a bystander drop only reproduces from the perturbation that caused it.
    let offset = example.injection_offset;
    if offset > source.len() || !source.is_char_boundary(offset) {
        return Verdict::Unconfirmed;
    }
    let mut injected = String::with_capacity(source.len() + 24);
    injected.push_str(&source[..offset]);
    injected.push_str(payload.text());
    injected.push_str(&source[offset..]);

    let Formatted::Ok {
        findings,
        comments: input_comments,
        output,
        ..
    } = ledger_format_with_comments(&injected, parser)
    else {
        return Verdict::Unconfirmed;
    };
    if findings.is_empty() {
        // The example no longer fires at all — the ledger and the re-run disagree outright.
        return Verdict::Unconfirmed;
    }
    let Formatted::Ok {
        comments: output_comments,
        ..
    } = ledger_format_with_comments(&output, parser)
    else {
        // The formatter's own output doesn't parse. A real bug, but `roundtrip_audit`'s.
        return Verdict::Unconfirmed;
    };

    if comment_content_multiset(&input_comments) == comment_content_multiset(&output_comments) {
        // Content conserved: the ledger's drop/double-print claim is not observable in the
        // output — an instrument gap, not the content loss it is filed as.
        Verdict::Unconfirmed
    } else {
        // A content is missing, mangled, or duplicated — the ledger's claim is real.
        Verdict::Confirmed
    }
}

/// The multiset of comment **contents**, each whitespace-normalized so a legitimate re-indent
/// reads as conserved while a mangle reads as changed (see [`verify_example`]).
fn comment_content_multiset(texts: &[String]) -> BTreeMap<String, usize> {
    let mut ms: BTreeMap<String, usize> = BTreeMap::new();
    for text in texts {
        *ms.entry(normalize_comment_text(text)).or_insert(0) += 1;
    }
    ms
}

/// Split a comment's text on newlines, trim each line, and rejoin with `\n`. A re-indent of a
/// multi-line block comment changes per-line leading/trailing whitespace but keeps the
/// newline count, so it normalizes equal; a mangle that collapses the newlines yields fewer
/// lines and normalizes different. `trim` also drops a `\r`, so `\r\n` vs `\n` line endings
/// normalize alike.
fn normalize_comment_text(text: &str) -> String {
    text.split('\n')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Map a bystander victim's span-start from the injected source's coordinates back to the
/// seed's, across the single-payload splice.
///
/// The inject loop builds `injected = seed[..injection_offset] + payload + seed[injection_offset..]`,
/// so `payload_len` bytes were inserted at `injection_offset`. A **bystander** finding's
/// comment — never the injected one — therefore sits either wholly *before* the splice (its
/// start unchanged) or wholly *at or after* it (its start shifted right by `payload_len`). Its
/// start never lands in `[injection_offset, injection_offset + payload_len)`: that range is the
/// injected comment, which the caller classifies `injected` and never routes here.
///
/// Returns the seed-space offset, or `None` — **checked, never a panic** — when the mapped
/// offset is out of the seed's range or lands mid-`char`-boundary (a reflow the linear
/// span-shift can't place, e.g. a multi-line comment re-indented across the splice). The caller
/// then falls back to injection-offset keying and counts it, so a stray victim is
/// mis-attributed rather than crashing the audit. This arithmetic is the "corpus can't grade
/// it" class — an off-by-one leaves every ASCII file byte-identical — so it is unit-tested
/// directly.
pub(super) fn victim_seed_offset(
    seed: &str,
    injection_offset: usize,
    payload_len: usize,
    victim_start: usize,
) -> Option<usize> {
    let seed_offset = if victim_start < injection_offset {
        victim_start
    } else if victim_start >= injection_offset + payload_len {
        victim_start - payload_len
    } else {
        // Inside the injected payload — impossible for a bystander (that range IS the injected
        // comment). Refuse rather than fabricate an offset.
        return None;
    };
    (seed_offset <= seed.len() && seed.is_char_boundary(seed_offset)).then_some(seed_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verify decision: a re-indented multi-line comment normalizes EQUAL (not a
    /// finding), while a mangle that eats the newline normalizes DIFFERENT (a finding). This
    /// is the property the count-based verify was blind to, so no corpus run grades it.
    #[test]
    fn comment_content_multiset_normalizes_reindent_but_not_mangle() {
        // Re-indent: leading whitespace before `b` changes, newline kept ⇒ conserved.
        let injected = vec!["/* a\n   b */".to_string()];
        let reindented = vec!["/* a\nb */".to_string()];
        assert_eq!(
            comment_content_multiset(&injected),
            comment_content_multiset(&reindented),
            "a re-indent must not read as a change"
        );
        // Mangle: the newline is gone, so the line count drops ⇒ NOT conserved.
        let mangled = vec!["/* ab */".to_string()];
        assert_ne!(
            comment_content_multiset(&injected),
            comment_content_multiset(&mangled),
            "a mangle that collapses the newline must read as a change"
        );
        // A plain drop: the content is simply absent from the output multiset.
        assert_ne!(
            comment_content_multiset(&injected),
            comment_content_multiset(&[]),
            "a dropped comment must read as a change"
        );
        // A duplicate: the same content twice is a distinct multiset from once.
        let once = vec!["/* c */".to_string()];
        let twice = vec!["/* c */".to_string(), "/* c */".to_string()];
        assert_ne!(
            comment_content_multiset(&once),
            comment_content_multiset(&twice),
            "a double-print must read as a change"
        );
    }

    /// The splice-mapping arithmetic — the "corpus can't grade it" class (an offset error
    /// leaves every ASCII file byte-identical, so no corpus run grades it; only this does).
    /// A victim BEFORE the injection maps unchanged; one AT-OR-AFTER `injection + payload_len`
    /// maps back by `payload_len`; an offset inside the payload range, out of range, or
    /// mid-`char` falls back to `None` (the caller's injection-offset keying).
    #[test]
    fn victim_seed_offset_maps_across_the_splice() {
        // 8 ASCII bytes, every offset a char boundary. Injecting a 4-byte payload at offset 3
        // yields `injected = "abc" + PPPP + "defgh"` (length 12).
        let seed = "abcdefgh";
        let inj = 3;
        let plen = 4;

        // Before the splice: unchanged (injected 0..3 == seed 0..3).
        assert_eq!(victim_seed_offset(seed, inj, plen, 0), Some(0));
        assert_eq!(victim_seed_offset(seed, inj, plen, 2), Some(2));

        // At or after the splice: shift back by payload_len. Seed `d` sits at injected 7
        // (3 + 4) and maps back to 3; seed `h` at injected 11 → 7; the seed's end (injected
        // 12) → seed.len() 8.
        assert_eq!(victim_seed_offset(seed, inj, plen, 7), Some(3));
        assert_eq!(victim_seed_offset(seed, inj, plen, 11), Some(7));
        assert_eq!(victim_seed_offset(seed, inj, plen, 12), Some(8));

        // Inside the payload range [3, 7): impossible for a bystander ⇒ None (fallback). The
        // low end (== injection) is the injected comment, already classified `injected`.
        assert_eq!(victim_seed_offset(seed, inj, plen, 3), None);
        assert_eq!(victim_seed_offset(seed, inj, plen, 6), None);

        // Out of range past the seed's end ⇒ None (13 - 4 = 9 > 8).
        assert_eq!(victim_seed_offset(seed, inj, plen, 13), None);

        // Multibyte: a mapped offset that lands mid-`char` falls back to None. `é` is two
        // bytes at seed [1, 3). Injecting a 2-byte payload at 0 → `injected = "PP" + "aébc"`.
        let seed2 = "aébc";
        // Injected 3 → seed 1 (the start of `é`, a boundary) ⇒ mapped.
        assert_eq!(victim_seed_offset(seed2, 0, 2, 3), Some(1));
        // Injected 4 → seed 2, which is mid-`é` ⇒ None.
        assert_eq!(victim_seed_offset(seed2, 0, 2, 4), None);
    }
}
