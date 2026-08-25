//! Text analysis helpers for Svelte template whitespace.
//!
//! Newline/blank-line predicates over `str`, used by the printer's blank-line decisions.
//! (Whitespace *collapsing* itself lives in the doc-based content path —
//! `build_text_fill_doc_trimmed` and friends.)
//!
//! ⚠️ **Three "is there a blank line" predicates exist, and they are NOT interchangeable.**
//! Each answers a different question; picking the wrong one is how a formatter fabricates
//! whitespace the author never wrote. Name the question at the call site:
//!
//! | predicate | question | shape |
//! | --- | --- | --- |
//! | [`has_authored_blank_line`] | did the author write a blank line *somewhere* in this text? | run, anywhere |
//! | [`has_leading_blank_line`] | did the author write a blank line *before this text's first content*? | run, leading only |
//! | [`has_trailing_blank_line`] | did the author write a blank line *after this text's last content*? | run, trailing only |
//!
//! The last two are the two halves of one question — *is there a blank line at this SEAM* — and
//! a caller that has a side in hand (a boundary's `leading` flag) picks between them rather than
//! reaching for the `anywhere` scan: a blank *inside* the text belongs to the text, and relaying
//! it to the seam relocates the author's blank rather than preserving it.
//!
//! All three scan for a **run** — two newlines separated by horizontal whitespace only. A newline
//! *total* (`count >= 2`) is not a fourth option: two SEPARATE single breaks (`\ntext1\ntext2`)
//! reach it with no blank line present, so a total coincides with the authoring signal only on a
//! string that is nothing but whitespace, and every call site that would reach such a
//! predicate is a string with content in it — trailing text after a `format-ignore`
//! range end, a range marker between a section comment and its section. Don't introduce it: a
//! caller that "knows" its string is pure whitespace is one refactor away from being wrong. The
//! near miss to watch for is a caller that has already sliced an edge run
//! ([`crate::ast::internal::text_edge_ws`]) and counts newlines in *that* — correct only because
//! the slice is pure whitespace by construction, which is the same one refactor away. Ask the
//! seam predicate instead; it takes the whole `raw` and does its own scoping.
//!
//! A module doc rather than a plain comment on purpose: the table's intra-doc links are then
//! checked by `docs:audit`, so a rename can't rot it silently.

/// Whether `raw` holds an authored blank line **anywhere** — two newlines separated by
/// horizontal whitespace only.
///
/// The Tier-2 authoring signal for "did the author leave a blank line inside this text".
/// Contrast the two seam predicates, [`has_leading_blank_line`] and
/// [`has_trailing_blank_line`], which ask the same of one edge whitespace run alone.
pub(crate) fn has_authored_blank_line(raw: &str) -> bool {
    let mut newlines = 0u32;
    for b in raw.bytes() {
        match b {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b if is_horizontal_ws(b) => {}
            _ => newlines = 0,
        }
    }
    false
}

/// Whether `raw` **opens** with an authored blank line — two newlines, separated by horizontal
/// whitespace only, before any content byte.
///
/// The question a *seam* asks: "did the author leave a blank line immediately after the thing
/// that precedes this text?" Scoping to the leading run is what separates that from
/// [`has_authored_blank_line`] — a blank *inside* the text (`\ntext1\n\ntext2`) belongs to the
/// text, not to the seam, so relaying it to the seam would relocate the author's blank rather
/// than preserve it.
///
/// Identical to [`has_authored_blank_line`] except that a content byte ENDS the answer instead
/// of resetting the run. On a whitespace-only string the two therefore agree — the leading run
/// is the whole string.
///
/// Kept as its own scan rather than folded into its siblings. Both folds are correct and both
/// were measured to GROW `.text` (`objcopy -O binary --only-section=.text target/corpus/tsv`,
/// against this pair): delegating to
/// `has_authored_blank_line(internal::text_edge_ws(raw, true))` costs +128 bytes, and one
/// `const LEADING_ONLY: bool` scan behind two wrappers costs +112 — `#[inline]` recovers
/// neither. The same behaviour-neutral-but-not-code-neutral trade the `is_horizontal_ws` note
/// below records, and declined for the same reason, [`has_trailing_blank_line`] included. What
/// the duplication actually buys back is small: the bodies differ in exactly one `match` arm
/// (and, for the trailing one, the scan direction), each doc comment names the others, and
/// `leading_blank_line_stops_at_the_first_content_byte` /
/// `trailing_blank_line_stops_at_the_last_content_byte` pin both the case where they differ and
/// the whitespace-only case where they must agree.
pub(crate) fn has_leading_blank_line(raw: &str) -> bool {
    let mut newlines = 0u32;
    for b in raw.bytes() {
        match b {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b if is_horizontal_ws(b) => {}
            // Content ends the leading run — a later blank is the text's, not the seam's.
            _ => return false,
        }
    }
    false
}

/// Whether `raw` **ends** with an authored blank line — two newlines, separated by horizontal
/// whitespace only, after the last content byte.
///
/// The mirror of [`has_leading_blank_line`], asked at the seam that FOLLOWS a text rather than
/// the one that precedes it: "did the author leave a blank line immediately before the thing
/// that follows this text?" Scoped to the trailing run for the same reason — a blank *inside*
/// the text (`text1\n\ntext2\n`) belongs to the text, not to the seam.
///
/// Identical to [`has_leading_blank_line`] with the scan reversed; see that one for why the
/// three bodies are hand-rolled rather than folded. On a whitespace-only string all three agree.
pub(crate) fn has_trailing_blank_line(raw: &str) -> bool {
    let mut newlines = 0u32;
    for b in raw.bytes().rev() {
        match b {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b if is_horizontal_ws(b) => {}
            // Content ends the trailing run — an earlier blank is the text's, not the seam's.
            _ => return false,
        }
    }
    false
}

/// Whether `b` is **horizontal** collapsible whitespace — whitespace that does not end a line.
///
/// [`crate::ast::internal::is_collapsible_ws`] minus the line feed (`[ \t\r]`). The callers are
/// the three run scans above: does this byte let the newline run continue rather than end it. A
/// form feed is content, so it breaks the run — an FF between two newlines is not a blank line,
/// matching the compiler's own class (`regex_not_whitespace` = `/[^ \t\r\n]/`).
///
/// Spelled as the explicit set rather than `internal::is_collapsible_ws(b) && b != b'\n'` — the same set,
/// in the form the scans' `match` arms want.
///
/// A second spelling of this set lives on `text_starts_with_linebreak` in `fragment_doc.rs`. It is
/// deliberately left alone: it feeds a `str` pattern, and swapping the `[char; 3]` array for a
/// predicate fn changes the `Pattern` monomorphization (a measured `.text` growth), so folding it
/// in is a behaviour-neutral-but-not-code-neutral change rather than part of this cleanup.
#[inline]
fn is_horizontal_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r')
}

#[cfg(test)]
mod tests {
    use super::{has_authored_blank_line, has_leading_blank_line, has_trailing_blank_line};

    #[test]
    fn authored_blank_line_wants_a_run_not_a_total() {
        assert!(has_authored_blank_line("a\n\nb"));
        assert!(has_authored_blank_line("\n \n"));
        // Two SEPARATE breaks reach the total but are not a blank line.
        assert!(!has_authored_blank_line("\na\nb"));
        assert!(!has_authored_blank_line("a\nb\nc"));
        // A form feed is content, so it breaks the run.
        assert!(!has_authored_blank_line("\n\u{c}\n"));
    }

    #[test]
    fn leading_blank_line_stops_at_the_first_content_byte() {
        assert!(has_leading_blank_line("\n\ntext"));
        assert!(has_leading_blank_line("\n\t\n\ttext"));
        assert!(!has_leading_blank_line("\ntext1\ntext2"));
        // The discriminator: a blank INSIDE the text belongs to the text, not the seam.
        assert!(!has_leading_blank_line("\ntext1\n\ntext2"));
        assert!(has_authored_blank_line("\ntext1\n\ntext2"));
        // On a whitespace-only string the two agree — the leading run is the whole string.
        for ws in ["\n\n", "\n \n", "\n", "", "\t"] {
            assert_eq!(has_leading_blank_line(ws), has_authored_blank_line(ws));
        }
    }

    #[test]
    fn trailing_blank_line_stops_at_the_last_content_byte() {
        assert!(has_trailing_blank_line("text\n\n"));
        assert!(has_trailing_blank_line("text\t\n\t\n"));
        assert!(!has_trailing_blank_line("text1\ntext2\n"));
        // The discriminator: a blank INSIDE the text belongs to the text, not the seam.
        assert!(!has_trailing_blank_line("text1\n\ntext2\n"));
        assert!(has_authored_blank_line("text1\n\ntext2\n"));
        // A form feed is content, so it breaks the run — same class as the siblings.
        assert!(!has_trailing_blank_line("text\n\u{c}\n"));
        // The exact mirror of the leading scan.
        for s in ["\n\ntext", "text\n\n", "\ntext1\n\ntext2\n", "a", ""] {
            let reversed: String = s.chars().rev().collect();
            assert_eq!(
                has_trailing_blank_line(s),
                has_leading_blank_line(&reversed)
            );
        }
        // On a whitespace-only string all three agree — the trailing run is the whole string.
        for ws in ["\n\n", "\n \n", "\n", "", "\t"] {
            assert_eq!(has_trailing_blank_line(ws), has_authored_blank_line(ws));
            assert_eq!(has_trailing_blank_line(ws), has_leading_blank_line(ws));
        }
    }
}
