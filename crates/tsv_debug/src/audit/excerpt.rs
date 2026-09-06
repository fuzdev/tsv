//! The **report-side excerpting** an audit's human output does: how a finding points back at
//! the source it was found in.
//!
//! Two questions live here, both because several audits ask them and each had drifted into a
//! per-audit spelling of the same walk:
//!
//! - [`line_context`] — "which source line is this byte offset on?", for a finding keyed by an
//!   offset (`authoring_audit`, `paren_audit`).
//! - [`first_line_diff`] — "where do these two outputs first disagree?", for a finding that IS
//!   a difference between two formats (`paren_audit`, `razor_audit`, `lex_diff`).
//!
//! ⚠️ Neither is a snapshot key. [`shape`](super::shape) owns that alphabet, and the two must
//! not be confused: a **key** has to survive an ordinary fixture edit without churning a
//! committed ratchet, so it is deliberately coarse; an **excerpt** has to be legible to a human
//! reading a failure, so it is deliberately verbatim. A key built out of an excerpt would churn
//! on every rename.
//!
//! Two near neighbours are deliberately NOT folded in here, because each answers a different
//! question rather than this one with options:
//!
//! - `tsc_conformance`'s baseline diff splits on `\r\n` (tsc's baselines are CRLF) and
//!   truncates each side with an ellipsis — a different walk over a different line class.
//! - `width_audit`'s `excerpt` keeps a line's HEAD **and TAIL** with the middle elided, because
//!   what makes an over-width line legible is where it ends. [`line_context`] caps from the
//!   head, which is right for a finding anchored at an offset near the line's start and wrong
//!   for one anchored at column 100.

/// The source line holding byte `offset`, trimmed and capped for a one-line report row.
///
/// The cap is a display concern, not a claim about the line: a finding that needs the whole
/// line has the file and the offset to go look.
pub(crate) fn line_context(source: &str, offset: usize) -> String {
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = source[offset..]
        .find('\n')
        .map_or(source.len(), |i| offset + i);
    source[start..end]
        .trim()
        .chars()
        .take(CONTEXT_CHARS)
        .collect()
}

/// How much of a context line a report row shows. One value, so two audits' findings line up
/// when read side by side.
const CONTEXT_CHARS: usize = 90;

/// The first line at which `a` and `b` differ: its **1-based** number, and each side's line —
/// `None` for a side that has no such line, i.e. the one that ran out first.
///
/// `None` overall when the two are line-for-line identical. Lines are `str::lines`, so this is
/// an LF question; callers spell their own placeholder for a missing side (`<eof>`), because
/// that string belongs to the report, not to the walk.
pub(crate) fn first_line_diff<'a>(
    a: &'a str,
    b: &'a str,
) -> Option<(usize, Option<&'a str>, Option<&'a str>)> {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return Some((i + 1, Some(la), Some(lb)));
        }
    }
    // A shared prefix but different lengths: the first difference is the line one side does not
    // have. Reported at the shorter side's end, with the longer side's extra line — which is
    // the informative half and the reason this returns `Option` per side rather than one
    // `<eof>` for both.
    let (na, nb) = (a.lines().count(), b.lines().count());
    if na == nb {
        return None;
    }
    let shared = na.min(nb);
    Some((shared + 1, a.lines().nth(shared), b.lines().nth(shared)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_context_trims_and_locates() {
        let src = "first\n\tsecond line\nthird\n";
        assert_eq!(line_context(src, 0), "first");
        assert_eq!(line_context(src, 8), "second line");
        assert_eq!(line_context(src, src.len() - 1), "third");
    }

    /// The cap is a char count, not a byte count — a multi-byte line must not slice mid-char.
    #[test]
    fn line_context_caps_by_chars() {
        let line = "é".repeat(200);
        assert_eq!(line_context(&line, 0).chars().count(), CONTEXT_CHARS);
    }

    #[test]
    fn first_line_diff_finds_the_line() {
        assert_eq!(
            first_line_diff("a\nb\nc", "a\nX\nc"),
            Some((2, Some("b"), Some("X")))
        );
        assert_eq!(first_line_diff("a\nb", "a\nb"), None);
    }

    /// A shared prefix with different lengths reports the extra line, not two placeholders.
    #[test]
    fn first_line_diff_reports_the_extra_line() {
        assert_eq!(first_line_diff("a\nb", "a"), Some((2, Some("b"), None)));
        assert_eq!(first_line_diff("a", "a\nb"), Some((2, None, Some("b"))));
    }
}
