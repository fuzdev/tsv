use super::identifiers::IDENT_CONTINUE_LUT;
use super::lex_err;
use super::token::{Token, TokenKind};
use crate::number::{continues_unit, exponent_len};
use tsv_lang::ParseError;

/// Read a CSS number, percentage, or dimension
/// Numbers: 42, 1.5, .5, -42, +1.5
/// Percentages: 50%, -100%
/// Dimensions: 16px, 1.5em, -2.5rem
pub(crate) fn read_number(source: &str, pos: &mut usize) -> Result<Token, Box<ParseError>> {
    let start = *pos;
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut p = start;

    // The mantissa and exponent grammar is entirely ASCII — digits, `.`, `e`/`E`, sign,
    // `%` — so it reads straight off the byte slice. The former per-char `chars().next()`
    // decoded a `char` for every digit of every number in the stylesheet.

    // Read optional sign
    if matches!(bytes.get(p), Some(b'-' | b'+')) {
        p += 1;
    }

    // Read integer part
    let int_start = p;
    while p < len && bytes[p].is_ascii_digit() {
        p += 1;
    }
    let has_integer_digits = p > int_start;

    // Read decimal part
    if bytes.get(p) == Some(&b'.') {
        let peek_char = source[p + 1..].chars().next();
        if peek_char.is_some_and(|ch| ch.is_ascii_digit()) {
            p += 1; // consume '.'
            while p < len && bytes[p].is_ascii_digit() {
                p += 1;
            }
        } else if has_integer_digits
            && (exponent_len(&source[p + 1..]) > 0
                || peek_char.is_none_or(|ch| !continues_unit(ch)))
        {
            // Trailing dot that belongs to the number: before an exponent
            // (`1.e1` → `1e1`) or a terminator (`;`, `)`, `,`, `%`, whitespace,
            // EOF — `1.` → `1`). A following identifier char is left alone, so
            // `1.png` (a url path, or the invalid number-dot-ident sequence) is
            // preserved verbatim rather than merged into a dimension.
            p += 1; // consume '.'
        }
    }

    // Read scientific-notation exponent: [eE][+-]?\d+
    // Consumed as part of the number so it normalizes (and isn't mistaken for a
    // dimension unit). `1em` keeps `em` as a unit because `m` is not a digit.
    if exponent_len(&source[p..]) > 0 {
        p += 1; // 'e' / 'E'
        if matches!(bytes.get(p), Some(b'+' | b'-')) {
            p += 1;
        }
        while p < len && bytes[p].is_ascii_digit() {
            p += 1;
        }
    }

    let num_end = p;
    *pos = p;

    // Validate number (parseable as f64)
    let num_str = &source[start..num_end];
    num_str
        .parse::<f64>()
        .map_err(|_| lex_err(format!("Invalid number: {num_str}"), start))?;

    // Check for percentage
    if bytes.get(p) == Some(&b'%') {
        p += 1;
        *pos = p;
        return Ok(Token {
            kind: TokenKind::Percentage,
            start: start as u32,
            end: p as u32,
        });
    }

    // Check for dimension (unit). A unit is an identifier, so its body continues on the
    // same predicate `read_identifier` uses — hence the shared LUT. The *opening* test is
    // Unicode `is_alphabetic()`, so one char is decoded for it; the body then runs the
    // ASCII table and yields to the char arm at the first non-ASCII byte, leaving a
    // non-ASCII unit lexing exactly as before.
    if let Some(ch) = source[p..].chars().next()
        && (ch.is_alphabetic() || ch == '-')
    {
        let unit_start = p;

        loop {
            while p < len && IDENT_CONTINUE_LUT[bytes[p] as usize] {
                p += 1;
            }
            match source[p..].chars().next() {
                Some(ch) if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                    p += ch.len_utf8();
                }
                _ => break,
            }
        }

        let unit_len = p - unit_start;
        if unit_len > 0 {
            *pos = p;
            return Ok(Token {
                kind: TokenKind::Dimension {
                    unit_len: unit_len as u8,
                },
                start: start as u32,
                end: p as u32,
            });
        }

        // Reset position if we didn't find a valid unit
        p = unit_start;
    }

    // Just a number
    *pos = p;
    Ok(Token {
        kind: TokenKind::Number,
        start: start as u32,
        end: p as u32,
    })
}
