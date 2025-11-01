use super::token::{Token, TokenKind};
use tsv_lang::ParseError;

/// Read a CSS number, percentage, or dimension
/// Numbers: 42, 1.5, .5
/// Percentages: 50%
/// Dimensions: 16px, 1.5em
pub(crate) fn read_number(source: &str, pos: &mut usize) -> Result<Token, ParseError> {
    let start = *pos;
    let mut num_str = String::new();

    // Read integer part
    loop {
        match source[*pos..].chars().next() {
            Some(ch) if ch.is_ascii_digit() => {
                num_str.push(ch);
                *pos += 1;
            }
            _ => break,
        }
    }

    // Read decimal part
    if source[*pos..].starts_with('.') {
        let peek_char = source[*pos + 1..].chars().next();
        if peek_char.is_some_and(|ch| ch.is_ascii_digit()) {
            num_str.push('.');
            *pos += 1;

            loop {
                match source[*pos..].chars().next() {
                    Some(ch) if ch.is_ascii_digit() => {
                        num_str.push(ch);
                        *pos += 1;
                    }
                    _ => break,
                }
            }
        }
    }

    // Validate number (parseable as f64), but preserve source string
    num_str
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidSyntax {
            message: format!("Invalid number: {}", num_str),
            position: start,
            context: None,
        })?;

    // Check for percentage
    if source[*pos..].starts_with('%') {
        *pos += 1;
        return Ok(Token {
            kind: TokenKind::Percentage(num_str),
            start,
            end: *pos,
        });
    }

    // Check for dimension (unit)
    if let Some(ch) = source[*pos..].chars().next()
        && (ch.is_alphabetic() || ch == '-')
    {
        let unit_start = *pos;
        let mut unit = String::new();

        loop {
            match source[*pos..].chars().next() {
                Some(ch) if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                    unit.push(ch);
                    *pos += ch.len_utf8();
                }
                _ => break,
            }
        }

        if !unit.is_empty() {
            return Ok(Token {
                kind: TokenKind::Dimension(num_str, unit),
                start,
                end: *pos,
            });
        }

        // Reset position if we didn't find a valid unit
        *pos = unit_start;
    }

    // Just a number
    Ok(Token {
        kind: TokenKind::Number(num_str),
        start,
        end: *pos,
    })
}
