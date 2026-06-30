// Low-level byte scanning utilities for parser lookahead
//
// These are generic helpers for scanning raw bytes, used by both expression
// parsing (arrow function detection) and type parsing (index signature detection).

use std::borrow::Cow;

/// Skip ASCII whitespace characters in a byte slice, returning new position
#[inline]
pub(super) fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

/// Skip a line comment (// ...), returning position after the newline
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_line_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip //
    pos += 2;
    // Read until line terminator or EOF — U+2028/U+2029 (UTF-8 e2 80 a8/a9)
    // terminate line comments like LF/CR per the spec
    while pos < bytes.len() && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
        if bytes[pos] == 0xe2
            && pos + 2 < bytes.len()
            && bytes[pos + 1] == 0x80
            && (bytes[pos + 2] == 0xa8 || bytes[pos + 2] == 0xa9)
        {
            break;
        }
        pos += 1;
    }
    pos
}

/// Skip a block comment (/* ... */), returning position after the closing */
/// Assumes `pos` is at the first `/`
#[inline]
pub(super) fn skip_block_comment(bytes: &[u8], mut pos: usize) -> usize {
    // Skip /*
    pos += 2;
    while pos + 1 < bytes.len() {
        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
            return pos + 2;
        }
        pos += 1;
    }
    pos
}

/// Skip whitespace and comments, returning new position
#[inline]
pub(super) fn skip_whitespace_and_comments(bytes: &[u8], mut pos: usize) -> usize {
    loop {
        let start = pos;
        pos = skip_whitespace(bytes, pos);
        // Check for comments
        if pos + 1 < bytes.len() && bytes[pos] == b'/' {
            if bytes[pos + 1] == b'/' {
                pos = skip_line_comment(bytes, pos);
            } else if bytes[pos + 1] == b'*' {
                pos = skip_block_comment(bytes, pos);
            } else {
                break;
            }
        } else {
            break;
        }
        // Continue loop to handle whitespace after comment
        if pos == start {
            break;
        }
    }
    pos
}

/// Check if a byte can start an identifier (letter, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `ID_Start` validation (`lexer::ident::is_id_start`) on the decoded char.
#[inline]
pub(super) fn is_identifier_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b > 127
}

/// Check if a byte can continue an identifier (alphanumeric, underscore, dollar sign, or non-ASCII)
///
/// Non-ASCII bytes (> 127) are included for lookahead purposes - they're part of multi-byte
/// UTF-8 sequences that are likely unicode identifier chars. The actual lexer uses proper
/// `ID_Continue` validation (`lexer::ident::is_id_continue`) on the decoded char.
#[inline]
pub(super) fn is_identifier_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b > 127
}

/// Skip an identifier, returning position after the identifier
/// Assumes `pos` is at the start of an identifier
#[inline]
pub(super) fn skip_identifier(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && is_identifier_continue(bytes[pos]) {
        pos += 1;
    }
    pos
}

/// Parse a JS number literal (hex, binary, octal, scientific, BigInt)
/// Returns f64 (BigInt suffix 'n' is ignored for value, preserved in raw)
///
/// Note: Precision loss for large integers (>2^52) matches JS behavior.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn parse_number_literal(raw: &str) -> Result<f64, std::num::ParseFloatError> {
    // Numeric separators (`_`) are uncommon; only allocate to strip them when
    // they're actually present. The common literal (`42`, `0xff`, `3.14`) carries
    // no separator and borrows the source slice directly — no per-literal alloc.
    let clean: Cow<'_, str> = if raw.as_bytes().contains(&b'_') {
        Cow::Owned(raw.chars().filter(|&c| c != '_').collect())
    } else {
        Cow::Borrowed(raw)
    };

    // Strip BigInt suffix
    let clean = clean.strip_suffix('n').unwrap_or(&clean);

    if clean.len() >= 2 {
        let prefix = &clean[..2];
        let digits = &clean[2..];
        match prefix {
            // Hex: 0xff
            "0x" | "0X" => return Ok(parse_radix_f64(digits, 16)),
            // Binary: 0b1010
            "0b" | "0B" => return Ok(parse_radix_f64(digits, 2)),
            // Octal: 0o77
            "0o" | "0O" => return Ok(parse_radix_f64(digits, 8)),
            _ => {}
        }
    }

    // Regular decimal (including scientific notation)
    clean.parse::<f64>()
}

/// Fold radix digits into an `f64`, rounding at each digit — exactly acorn's
/// `readInt` accumulation, which past 2^53 can land one ulp below the
/// correctly rounded value (e.g. `0x47874750d3a412a2`); matching acorn is the
/// conformance target, so don't "fix" this with a u128 cast. An integer-typed
/// accumulator would also overflow to 0 on long literals like
/// `0x123abcdef456ABCDEF`.
fn parse_radix_f64(digits: &str, radix: u32) -> f64 {
    digits.chars().fold(0f64, |acc, c| {
        // These radixes are all powers of two, so `acc * radix` only rescales the
        // exponent and is exact — making this byte-identical to `mul_add`. Keep the
        // explicit multiply-and-add as a faithful transcription of acorn's
        // `total = total * radix + val` rather than fusing it.
        #[allow(clippy::suboptimal_flops)]
        {
            acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
        }
    })
}
