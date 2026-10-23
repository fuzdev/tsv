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
    blank_text_into(out, chars.as_str());
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

/// Append `text` blanked with no substitution, reading bytes rather than characters: a
/// UTF-8 lead byte opens one UTF-16 unit and a four-byte lead (`0xF0` and up) a second, while a
/// continuation byte opens none, so the units of a valid `&str` are counted exactly; a line
/// terminator — `\n`, `\r`, or the three bytes of U+2028 / U+2029 — is copied, and the spaces
/// between two terminators go out as one run.
fn blank_text_into(out: &mut String, text: &str) {
    let bytes = text.as_bytes();
    let mut units = 0;
    let mut at = 0;
    loop {
        // eight bytes at a time while none is `\n`, `\r` or outside ASCII: one space each
        while let Some(word) = bytes[at..].first_chunk::<8>() {
            if word_needs_a_closer_look(u64::from_ne_bytes(*word)) {
                break;
            }
            units += 8;
            at += 8;
        }
        let Some(&byte) = bytes.get(at) else {
            break;
        };
        let terminator_len = match byte {
            b'\n' | b'\r' => 1,
            // U+2028 / U+2029: `E2 80 A8` / `E2 80 A9`
            0xE2 if bytes.get(at + 1) == Some(&0x80)
                && matches!(bytes.get(at + 2), Some(0xA8 | 0xA9)) =>
            {
                3
            }
            _ => 0,
        };
        if terminator_len == 0 {
            units += usize::from(byte & 0xC0 != 0x80) + usize::from(byte >= 0xF0);
            at += 1;
        } else {
            push_spaces(out, units);
            units = 0;
            out.push_str(&text[at..at + terminator_len]);
            at += terminator_len;
        }
    }
    push_spaces(out, units);
}

/// Whether any byte of `word` is `\n`, `\r` or non-ASCII — a byte [`blank_text_into`] cannot
/// count as one space without looking at it. Exact in the direction the caller relies on: a
/// `false` means every byte is ASCII and neither terminator (the zero-byte test on `word` XOR a
/// terminator is nonzero exactly when some byte equals it). A `true` only sends the word to the
/// byte-at-a-time step, which decides each byte itself.
const fn word_needs_a_closer_look(word: u64) -> bool {
    const ONES: u64 = u64::from_ne_bytes([0x01; 8]);
    const HIGHS: u64 = u64::from_ne_bytes([0x80; 8]);
    let lf = word ^ (ONES * b'\n' as u64);
    let cr = word ^ (ONES * b'\r' as u64);
    ((lf.wrapping_sub(ONES) & !lf) | (cr.wrapping_sub(ONES) & !cr) | word) & HIGHS != 0
}

/// The run [`push_spaces`] copies from.
const SPACES: &str = "                                                                ";

fn push_spaces(out: &mut String, mut count: usize) {
    while count > SPACES.len() {
        out.push_str(SPACES);
        count -= SPACES.len();
    }
    out.push_str(&SPACES[..count]);
}

#[cfg(test)]
mod tests {
    use super::{Edit, Mark, apply, blank_text_into, is_line_terminator};

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

    /// The blank by definition, a character at a time: a terminator copied, anything else one
    /// space per UTF-16 unit.
    fn blank_by_chars(text: &str) -> String {
        let mut out = String::new();
        for c in text.chars() {
            if is_line_terminator(c) {
                out.push(c);
            } else {
                out.extend(std::iter::repeat_n(' ', c.len_utf16()));
            }
        }
        out
    }

    #[test]
    fn blanks_by_bytes_as_by_characters() {
        // every character class the byte step tells apart — ASCII, two-, three- and four-byte
        // UTF-8, a three-byte character sharing U+2028's first two bytes, and one sharing its
        // first and last, each terminator —
        // at every offset across the eight-byte words, in runs longer than one `SPACES` copy
        let specials = [
            "\n", "\r", "\r\n", "\u{2028}", "\u{2029}", "\u{2027}", "\u{202A}", "\u{20a9}", "é",
            "\u{a0}", "中", "\u{feff}", "😀", "\t", "x",
        ];
        for special in specials {
            for second in specials {
                for pad in 0..20 {
                    for tail in [0, 7, 70, 140] {
                        let text = format!(
                            "{}{special}{}{second}{}",
                            "a".repeat(pad),
                            "b".repeat(pad % 9),
                            "c".repeat(tail)
                        );
                        let mut out = String::new();
                        blank_text_into(&mut out, &text);
                        assert_eq!(out, blank_by_chars(&text), "{text:?}");
                    }
                }
            }
        }
    }
}
