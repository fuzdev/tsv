//! The strip's one source-copy pass: the collected type-only ranges become whitespace.
//!
//! Every character of a blanked range becomes one space per UTF-16 unit (two for an astral
//! character), except an ECMAScript line terminator — LF, CR, U+2028, U+2029 — which is
//! copied verbatim, so every surviving character keeps its line and UTF-16 column. A range
//! may carry one single-character substitution ([`Mark`]), the only non-blank text the
//! strip writes.

/// The one character a blanked range writes in place of a blank, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    /// Blanks only.
    Plain,
    /// A `;` in place of the range's first character: the range would otherwise let the
    /// next line continue the statement or member before it.
    Semicolon,
    /// A `(` in place of the range's first character: a type-parameter list whose line
    /// break would otherwise end a `return` (or strand an `async`) before the parameters.
    OpenParen,
    /// A `)` in place of the range's last UTF-16 unit: an arrow's parameter list closed
    /// after its blanked return type, so no line terminator stands before the `=>`.
    CloseParen,
}

/// One range to blank, in source byte offsets.
#[derive(Debug, Clone, Copy)]
pub(super) struct Edit {
    pub start: u32,
    pub end: u32,
    pub mark: Mark,
}

/// Whether `c` is an ECMAScript line terminator, the characters a blank keeps.
const fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Copy `source` with every range in `edits` blanked. `edits` is sorted here; a range
/// overlapping an earlier one is clipped to its uncovered tail (the walk never produces one,
/// but the copy stays total on it), and a clipped range drops a leading [`Mark`].
pub(super) fn apply(source: &str, edits: &mut [Edit]) -> String {
    edits.sort_by_key(|edit| edit.start);
    let mut out = String::with_capacity(source.len());
    let mut copied = 0usize;
    for edit in edits.iter() {
        let end = edit.end as usize;
        if end <= copied {
            continue;
        }
        let start = (edit.start as usize).max(copied);
        let mark = match edit.mark {
            Mark::Semicolon | Mark::OpenParen if start != edit.start as usize => Mark::Plain,
            mark => mark,
        };
        out.push_str(&source[copied..start]);
        blank_into(&mut out, &source[start..end], mark);
        copied = end;
    }
    out.push_str(&source[copied..]);
    out
}

/// Append `text` blanked, with `mark`'s substitution. A substitution never replaces a line
/// terminator: the walk anchors each one on a token, never on a line break.
fn blank_into(out: &mut String, text: &str, mark: Mark) {
    let mut chars = text.chars();
    let lead = match mark {
        Mark::Semicolon => Some(';'),
        Mark::OpenParen => Some('('),
        Mark::Plain | Mark::CloseParen => None,
    };
    if let Some(lead) = lead
        && let Some(first) = chars.next()
    {
        if is_line_terminator(first) {
            debug_assert!(false, "a substitution anchored on a line terminator");
            out.push(first);
        } else {
            out.push(lead);
            push_spaces(out, first.len_utf16() - 1);
        }
    }
    let last = if mark == Mark::CloseParen {
        chars.next_back()
    } else {
        None
    };
    for c in chars {
        if is_line_terminator(c) {
            out.push(c);
        } else {
            push_spaces(out, c.len_utf16());
        }
    }
    if let Some(last) = last {
        if is_line_terminator(last) {
            debug_assert!(false, "a substitution anchored on a line terminator");
            out.push(last);
        } else {
            push_spaces(out, last.len_utf16() - 1);
            out.push(')');
        }
    }
}

fn push_spaces(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::{Edit, Mark, apply};

    fn run(source: &str, edits: &[(u32, u32, Mark)]) -> String {
        let mut edits: Vec<Edit> = edits
            .iter()
            .map(|&(start, end, mark)| Edit { start, end, mark })
            .collect();
        apply(source, &mut edits)
    }

    #[test]
    fn blanks_per_utf16_unit_and_keeps_terminators() {
        // `é` is one unit, `😀` two; CR, LF, U+2028 and U+2029 survive
        let source = "a:é😀\r\n\u{2028}\u{2029}b";
        let out = run(source, &[(1, source.len() as u32 - 1, Mark::Plain)]);
        assert_eq!(out, "a    \r\n\u{2028}\u{2029}b");
    }

    #[test]
    fn writes_each_substitution() {
        assert_eq!(run("x as T", &[(1, 6, Mark::Semicolon)]), "x;    ");
        assert_eq!(
            run("<T>(a)", &[(0, 3, Mark::OpenParen), (3, 4, Mark::Plain)]),
            "(   a)"
        );
        assert_eq!(run("(a): U", &[(2, 6, Mark::CloseParen)]), "(a   )");
    }

    #[test]
    fn close_paren_takes_the_last_unit_of_an_astral_character() {
        assert_eq!(run("a: 😀", &[(1, 7, Mark::CloseParen)]), "a   )");
    }

    #[test]
    fn sorts_and_clips_overlaps() {
        assert_eq!(
            run(
                "abcdef",
                &[
                    (3, 5, Mark::Plain),
                    (0, 2, Mark::Plain),
                    (1, 4, Mark::Semicolon)
                ]
            ),
            "     f"
        );
    }
}
