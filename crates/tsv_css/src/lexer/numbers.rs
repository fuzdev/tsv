use super::token::{Token, TokenKind};
use crate::number::{continues_unit, exponent_len};
use tsv_lang::ParseError;

/// Read a CSS number, percentage, or dimension
/// Numbers: 42, 1.5, .5, -42, +1.5
/// Percentages: 50%, -100%
/// Dimensions: 16px, 1.5em, -2.5rem
pub(crate) fn read_number(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;

    // Read optional sign
    if let Some(ch) = source[*pos..].chars().next()
        && (ch == '-' || ch == '+')
    {
        *pos += 1;
    }

    // Read integer part
    let int_start = *pos;
    loop {
        match source[*pos..].chars().next() {
            Some(ch) if ch.is_ascii_digit() => {
                *pos += 1;
            }
            _ => break,
        }
    }
    let has_integer_digits = *pos > int_start;

    // Read decimal part
    if source[*pos..].starts_with('.') {
        let peek_char = source[*pos + 1..].chars().next();
        if peek_char.is_some_and(|ch| ch.is_ascii_digit()) {
            *pos += 1; // consume '.'

            loop {
                match source[*pos..].chars().next() {
                    Some(ch) if ch.is_ascii_digit() => {
                        *pos += 1;
                    }
                    _ => break,
                }
            }
        } else if has_integer_digits
            && (exponent_len(&source[*pos + 1..]) > 0
                || peek_char.is_none_or(|ch| !continues_unit(ch)))
        {
            // Trailing dot that belongs to the number: before an exponent
            // (`1.e1` → `1e1`) or a terminator (`;`, `)`, `,`, `%`, whitespace,
            // EOF — `1.` → `1`). A following identifier char is left alone, so
            // `1.png` (a url path, or the invalid number-dot-ident sequence) is
            // preserved verbatim rather than merged into a dimension.
            *pos += 1; // consume '.'
        }
    }

    // Read scientific-notation exponent: [eE][+-]?\d+
    // Consumed as part of the number so it normalizes (and isn't mistaken for a
    // dimension unit). `1em` keeps `em` as a unit because `m` is not a digit.
    if exponent_len(&source[*pos..]) > 0 {
        *pos += 1; // 'e' / 'E'
        if let Some(ch) = source[*pos..].chars().next()
            && (ch == '+' || ch == '-')
        {
            *pos += 1;
        }
        loop {
            match source[*pos..].chars().next() {
                Some(ch) if ch.is_ascii_digit() => {
                    *pos += 1;
                }
                _ => break,
            }
        }
    }

    let num_end = *pos;

    // Validate number (parseable as f64)
    let num_str = &source[start..num_end];
    num_str
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidSyntax {
            message: format!("Invalid number: {num_str}"),
            position: start,
            context: None,
        })?;

    // Check for percentage
    if source[*pos..].starts_with('%') {
        *pos += 1;
        return Ok(Token {
            kind: TokenKind::Percentage,
            start,
            end: *pos,
            decoded: None,
        });
    }

    // Check for dimension (unit)
    if let Some(ch) = source[*pos..].chars().next()
        && (ch.is_alphabetic() || ch == '-')
    {
        let unit_start = *pos;

        loop {
            match source[*pos..].chars().next() {
                Some(ch) if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                    *pos += ch.len_utf8();
                }
                _ => break,
            }
        }

        let unit_len = *pos - unit_start;
        if unit_len > 0 {
            return Ok(Token {
                kind: TokenKind::Dimension {
                    unit_len: unit_len as u8,
                },
                start,
                end: *pos,
                decoded: None,
            });
        }

        // Reset position if we didn't find a valid unit
        *pos = unit_start;
    }

    // Just a number
    Ok(Token {
        kind: TokenKind::Number,
        start,
        end: *pos,
        decoded: None,
    })
}
